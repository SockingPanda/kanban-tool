import { classifyEvent } from "./invalidation"
import type { ApiListEventsQueryContract } from "../api/generated/contracts/api-list-events-query"
import { parseApiListEventsQuery } from "../api/generated/contracts/api-list-events-query"
import type {
  BusinessValidationResult,
  ControlValidationResult,
  EnvelopeParseResult,
  PollEventsPage,
  RawSseFrame,
  RecoveryBoundary,
  RecoveryResult,
  StreamContractAdapter,
  SseTransport,
  SseTransportConnection,
  SyncClock,
  SyncQuerySink,
  SyncTelemetry,
  SyncToken,
  ValidatedBusinessEvent,
} from "./contracts"

type ControllerState = "idle" | "connecting" | "live" | "recovering" | "polling" | "circuit-open" | "stopped"

interface RecoveryStage {
  readonly baseCursor: number
  readonly expectedRevision: number
  readonly seenIds: Map<number, string>
  readonly seenEventIds: Map<string, string>
  cursor: number
}

export interface StreamUrlContext {
  readonly boardId: string
  readonly after: number
}

export interface WebSyncControllerOptions {
  readonly boardId: string
  readonly streamUrl: string | ((context: StreamUrlContext) => string)
  readonly transport: SseTransport
  readonly adapter: StreamContractAdapter
  readonly sink: SyncQuerySink
  readonly clock?: SyncClock
  readonly telemetry?: SyncTelemetry
  readonly livenessTimeoutMs?: number
  readonly pollingIntervalMs?: number
  readonly reconnectDelayMs?: number
  readonly maxSeenEntries?: number
  readonly maxAnomalyAttempts?: number
  /** Explicit test-only seam for exercising small bounded buffers/budgets. */
  readonly testOnlyLimits?: boolean
  readonly maxRecoveryBufferBytes?: number
  readonly maxBoundaryEvents?: number
  readonly maxBoundaryBytes?: number
  readonly effectTimeoutMs?: number
}

export interface WebSyncSnapshot {
  readonly state: ControllerState
  readonly boardId: string
  readonly connectionEpoch: number
  readonly generation: number
  readonly lastConfirmedCursor: number
  readonly seenIds: number
  readonly seenEventIds: number
  readonly polling: boolean
  readonly circuitOpen: boolean
}

const NOOP_TELEMETRY: SyncTelemetry = { record: () => undefined }
const MAX_SEEN_ENTRIES = 8_192
const MAX_BOUNDARY_BYTES = 8 * 1024 * 1024
const MAX_RECOVERY_BUFFER_BYTES = 8 * 1024 * 1024
const MAX_EFFECT_TIMEOUT_MS = 30_000
const DEFAULT_LIVENESS_TIMEOUT_MS = 35_000
const DEFAULT_POLLING_INTERVAL_MS = 5_000
const DEFAULT_RECONNECT_DELAY_MS = 250
const DEFAULT_MAX_SEEN_ENTRIES = 2_048

class SinkEffectError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SinkEffectError"
  }
}

class SinkEffectCancelled extends Error {
  constructor() {
    super("sink effect cancelled")
    this.name = "SinkEffectCancelled"
  }
}

function systemClock(): SyncClock {
  let nextTimer = 0
  const timers = new Map<number, ReturnType<typeof globalThis.setTimeout>>()
  return {
    now: () => Date.now(),
    setTimeout: (callback, delayMs) => {
      const id = nextTimer
      nextTimer += 1
      timers.set(id, globalThis.setTimeout(() => {
        timers.delete(id)
        callback()
      }, delayMs))
      return id
    },
    clearTimeout: (timer) => {
      const handle = timers.get(timer)
      if (handle === undefined) return
      timers.delete(timer)
      globalThis.clearTimeout(handle)
    },
  }
}

function appendStreamQuery(baseUrl: string, context: StreamUrlContext): string {
  const isAbsolute = /^[a-z][a-z\d+.-]*:\/\//i.test(baseUrl)
  const url = new URL(baseUrl, "http://127.0.0.1")
  url.searchParams.set("board", context.boardId)
  url.searchParams.set("after", String(context.after))
  if (isAbsolute) return url.toString()
  return `${url.pathname}${url.search}${url.hash}`
}

function tokenEquals(left: SyncToken, right: SyncToken): boolean {
  return left.boardId === right.boardId && left.connectionEpoch === right.connectionEpoch && left.generation === right.generation
}

function recoveryTokenEquals(left: SyncToken, right: SyncToken): boolean {
  return left.boardId === right.boardId && left.generation === right.generation
}

function valueByteLength(value: unknown): number {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength
  } catch {
    return Number.POSITIVE_INFINITY
  }
}

function validatedEventByteLength(event: ValidatedBusinessEvent): number {
  return valueByteLength({
    id: event.id,
    eventId: event.eventId,
    boardId: event.boardId,
    taskId: event.taskId,
    runId: event.runId,
    kind: event.kind,
    createdAt: event.createdAt,
    scope: event.scope,
    canonicalFingerprint: event.canonicalFingerprint,
    known: event.known,
    raw: event.raw,
  })
}

export class WebSyncController {
  private boardId: string
  private readonly transport: SseTransport
  private readonly adapter: StreamContractAdapter
  private readonly sink: SyncQuerySink
  private readonly clock: SyncClock
  private readonly telemetry: SyncTelemetry
  private readonly streamUrl: WebSyncControllerOptions["streamUrl"]
  private readonly livenessTimeoutMs: number
  private readonly pollingIntervalMs: number
  private readonly reconnectDelayMs: number
  private readonly maxSeenEntries: number
  private readonly maxAnomalyAttempts: number
  private readonly maxRecoveryBufferBytes: number
  private readonly maxBoundaryEvents: number
  private readonly maxBoundaryBytes: number
  private readonly effectTimeoutMs: number

  private state: ControllerState = "idle"
  private running = false
  private connectionEpoch = 0
  private generation = 0
  private lastConfirmedCursor = 0
  private connection: SseTransportConnection | null = null
  private connectionAbort: AbortController | null = null
  private livenessTimer: number | null = null
  private pollingTimer: number | null = null
  private recoveryStartTimer: number | null = null
  private recoveryConnectionRetryTimer: number | null = null
  private pollingAbort: AbortController | null = null
  private pollingBoundaryCommit: Promise<boolean> | null = null
  private recoveryAbort: AbortController | null = null
  private recovery: Promise<void> | null = null
  private recoveryBuffer: ValidatedBusinessEvent[] = []
  private recoveryBufferBytes = 0
  private recoverySawLiveFrame = false
  private recoveryStage: RecoveryStage | null = null
  private readonly recoveryPreApplied = new Map<string, string>()
  private readonly recoveryAppliedEvents = new Map<string, ValidatedBusinessEvent>()
  private recoveryRevision = 0
  private ingestTail: Promise<void> = Promise.resolve()
  private readonly seenIds = new Map<number, string>()
  private readonly seenEventIds = new Map<string, string>()
  private readonly seenEventsById = new Map<number, ValidatedBusinessEvent>()
  private readonly seenEventsByEventId = new Map<string, ValidatedBusinessEvent>()
  private readonly anomalyAttempts = new Map<string, number>()
  private readonly activeEffects = new Set<AbortController>()

  constructor(options: WebSyncControllerOptions) {
    this.boardId = options.boardId
    this.streamUrl = options.streamUrl
    this.transport = options.transport
    this.adapter = options.adapter
    this.sink = options.sink
    this.clock = options.clock ?? systemClock()
    this.telemetry = options.telemetry ?? NOOP_TELEMETRY
    this.livenessTimeoutMs = options.livenessTimeoutMs ?? DEFAULT_LIVENESS_TIMEOUT_MS
    this.pollingIntervalMs = options.pollingIntervalMs ?? DEFAULT_POLLING_INTERVAL_MS
    this.reconnectDelayMs = options.reconnectDelayMs ?? DEFAULT_RECONNECT_DELAY_MS
    this.maxSeenEntries = options.maxSeenEntries ?? DEFAULT_MAX_SEEN_ENTRIES
    this.maxAnomalyAttempts = options.maxAnomalyAttempts ?? 3
    if (!options.testOnlyLimits && (this.maxSeenEntries < 2_048 || this.maxSeenEntries > MAX_SEEN_ENTRIES)) throw new RangeError("maxSeenEntries must be between 2048 and 8192")
    if (!options.testOnlyLimits && this.maxAnomalyAttempts > 3) throw new RangeError("maxAnomalyAttempts must be at most 3")
    if (!Number.isSafeInteger(this.maxAnomalyAttempts) || this.maxAnomalyAttempts <= 0) throw new RangeError("maxAnomalyAttempts must be positive")
    this.maxRecoveryBufferBytes = options.maxRecoveryBufferBytes ?? MAX_RECOVERY_BUFFER_BYTES
    this.maxBoundaryEvents = options.maxBoundaryEvents ?? this.maxSeenEntries
    this.maxBoundaryBytes = options.maxBoundaryBytes ?? MAX_BOUNDARY_BYTES
    this.effectTimeoutMs = options.effectTimeoutMs ?? 30_000
    if (!Number.isSafeInteger(this.livenessTimeoutMs) || this.livenessTimeoutMs <= 0) throw new RangeError("livenessTimeoutMs must be positive")
    if (!Number.isSafeInteger(this.pollingIntervalMs) || this.pollingIntervalMs <= 0) throw new RangeError("pollingIntervalMs must be positive")
    if (!Number.isSafeInteger(this.reconnectDelayMs) || this.reconnectDelayMs <= 0) throw new RangeError("reconnectDelayMs must be positive")
    if (!options.testOnlyLimits && (this.reconnectDelayMs !== DEFAULT_RECONNECT_DELAY_MS || this.pollingIntervalMs !== DEFAULT_POLLING_INTERVAL_MS || this.livenessTimeoutMs !== DEFAULT_LIVENESS_TIMEOUT_MS)) throw new RangeError("production SLO timing overrides require testOnlyLimits")
    if (!Number.isSafeInteger(this.maxSeenEntries) || this.maxSeenEntries <= 0) throw new RangeError("maxSeenEntries must be positive")
    if (!Number.isSafeInteger(this.maxRecoveryBufferBytes) || this.maxRecoveryBufferBytes <= 0) throw new RangeError("maxRecoveryBufferBytes must be positive")
    if (!Number.isSafeInteger(this.maxBoundaryEvents) || this.maxBoundaryEvents <= 0) throw new RangeError("maxBoundaryEvents must be positive")
    if (!Number.isSafeInteger(this.maxBoundaryBytes) || this.maxBoundaryBytes <= 0) throw new RangeError("maxBoundaryBytes must be positive")
    if (!Number.isSafeInteger(this.effectTimeoutMs) || this.effectTimeoutMs <= 0) throw new RangeError("effectTimeoutMs must be positive")
    if (!options.testOnlyLimits && (this.maxSeenEntries !== DEFAULT_MAX_SEEN_ENTRIES || this.maxRecoveryBufferBytes !== MAX_RECOVERY_BUFFER_BYTES || this.maxBoundaryEvents !== DEFAULT_MAX_SEEN_ENTRIES || this.maxBoundaryBytes !== MAX_BOUNDARY_BYTES || this.effectTimeoutMs !== MAX_EFFECT_TIMEOUT_MS)) throw new RangeError("production safety limit overrides require testOnlyLimits")
  }

  start(): void {
    if (this.running) return
    this.running = true
    this.state = "connecting"
    this.connectionEpoch += 1
    this.openConnection()
    this.armLiveness(this.currentToken())
  }

  stop(): void {
    if (!this.running && this.state === "stopped") return
    this.running = false
    this.connectionEpoch += 1
    this.generation += 1
    this.recoveryAbort?.abort()
    this.recoveryAbort = null
    this.cancelEffects()
    this.closeConnection()
    this.stopPolling()
    this.pollingBoundaryCommit = null
    this.clearRecoveryConnectionRetryTimer()
    this.clearLiveness()
    this.clearRecoveryStartTimer()
    this.recovery = null
    this.dropRecoveryStage()
    this.state = "stopped"
  }

  switchBoard(boardId: string): void {
    if (boardId === this.boardId) return
    this.boardId = boardId
    this.connectionEpoch += 1
    this.generation += 1
    this.recoveryAbort?.abort()
    this.recoveryAbort = null
    this.cancelEffects()
    this.lastConfirmedCursor = 0
    this.seenIds.clear()
    this.seenEventIds.clear()
    this.seenEventsById.clear()
    this.seenEventsByEventId.clear()
    this.anomalyAttempts.clear()
    this.recovery = null
    this.dropRecoveryStage()
    this.closeConnection()
    this.stopPolling()
    this.pollingBoundaryCommit = null
    this.clearRecoveryConnectionRetryTimer()
    this.clearLiveness()
    this.clearRecoveryStartTimer()
    if (this.running) {
      this.state = "connecting"
      this.openConnection()
      this.armLiveness(this.currentToken())
    }
  }

  retry(): void {
    this.anomalyAttempts.delete(this.anomalyKey())
    if (this.state !== "circuit-open") return
    this.state = "recovering"
    void this.beginRecovery("R", "manual-retry", false).catch((error) => this.detachedFailure("manual-retry", error))
  }

  /** Public test/adapter seam; transport callbacks call this with their epoch token. */
  processFrame(frame: RawSseFrame, token = this.currentToken()): Promise<void> {
    return this.enqueueIngest(() => this.ingestFrame(frame, token))
  }

  private enqueueIngest<T>(operation: () => Promise<T>): Promise<T> {
    const run = this.ingestTail.then(operation)
    this.ingestTail = run.then(() => undefined, () => undefined)
    return run
  }

  snapshot(): WebSyncSnapshot {
    return {
      state: this.state,
      boardId: this.boardId,
      connectionEpoch: this.connectionEpoch,
      generation: this.generation,
      lastConfirmedCursor: this.lastConfirmedCursor,
      seenIds: this.seenIds.size,
      seenEventIds: this.seenEventIds.size,
      polling: this.pollingTimer !== null,
      circuitOpen: this.state === "circuit-open",
    }
  }

  private currentToken(): SyncToken {
    return { boardId: this.boardId, connectionEpoch: this.connectionEpoch, generation: this.generation }
  }

  private isCurrent(token: SyncToken): boolean {
    return tokenEquals(token, this.currentToken())
  }

  private emit(type: string, details?: Readonly<Record<string, unknown>>): void {
    this.telemetry.record({ type, boardId: this.boardId, cursor: this.lastConfirmedCursor, ...(details ? { details } : {}) })
  }

  private isRecoveryCurrent(token: SyncToken): boolean {
    return this.running && this.state !== "circuit-open" && recoveryTokenEquals(token, this.currentToken())
  }

  private detachedFailure(kind: string, error: unknown): void {
    this.emit("detached-async-failure", { kind, message: error instanceof Error ? error.message : String(error) })
  }

  private cancelEffects(): void {
    for (const effect of this.activeEffects) effect.abort()
  }

  private async invokeEffect(event: ValidatedBusinessEvent, token: SyncToken, source: "sse" | "poll" | "recovery" | "poll-boundary", allowCircuit = false): Promise<void> {
    if (!this.running || (!allowCircuit && !this.isRecoveryOrConnectionTokenCurrent(token)) || (allowCircuit && !this.running)) return
    const controller = new AbortController()
    this.activeEffects.add(controller)
    let timer: number | null = null
    let timedOut = false
    const cancellation = new Promise<never>((_resolve, reject) => {
      controller.signal.addEventListener("abort", () => reject(new SinkEffectCancelled()), { once: true })
    })
    const canInvokeSink = (): boolean => {
      if (controller.signal.aborted || !this.running) return false
      return allowCircuit ? this.isCurrent(token) : this.isRecoveryOrConnectionTokenCurrent(token)
    }
    const operation = Promise.resolve().then(() => {
      if (!canInvokeSink()) throw new SinkEffectCancelled()
      return this.sink.onEvent(event, classifyEvent(event), token, controller.signal)
    })
    operation.catch(() => undefined)
    const timeout = new Promise<never>((_resolve, reject) => {
      timer = this.clock.setTimeout(() => {
        timedOut = true
        controller.abort()
        reject(new SinkEffectError(`sink effect exceeded ${this.effectTimeoutMs}ms`))
      }, this.effectTimeoutMs)
    })
    try {
      await Promise.race([operation, cancellation, timeout])
      if (!this.running || (allowCircuit ? !this.isCurrent(token) : !this.isRecoveryOrConnectionTokenCurrent(token))) return
      const appliedAt = this.clock.now()
      const createdAtMs = event.createdAt < 1_000_000_000_000 ? event.createdAt * 1_000 : event.createdAt
      this.emit("event-applied", {
        eventId: event.eventId,
        source,
        createdAt: event.createdAt,
        appliedAt,
        latencyMs: Math.max(0, appliedAt - createdAtMs),
      })
    } catch (error) {
      if (error instanceof SinkEffectCancelled && !timedOut && !this.isRecoveryOrConnectionTokenCurrent(token)) return
      if (error instanceof SinkEffectCancelled) throw error
      if (error instanceof SinkEffectError) throw error
      throw new SinkEffectError(error instanceof Error ? error.message : String(error))
    } finally {
      if (timer !== null) this.clock.clearTimeout(timer)
      this.activeEffects.delete(controller)
    }
  }

  private isRecoveryOrConnectionTokenCurrent(token: SyncToken): boolean {
    return this.running && this.state !== "circuit-open" && (this.isCurrent(token) || this.isRecoveryCurrent(token))
  }

  private openConnection(): void {
    if (!this.running || this.state === "circuit-open") return
    this.closeConnection()
    const token = this.currentToken()
    const abort = new AbortController()
    this.connectionAbort = abort
    const url = typeof this.streamUrl === "function" ? this.streamUrl({ boardId: this.boardId, after: this.lastConfirmedCursor }) : appendStreamQuery(this.streamUrl, { boardId: this.boardId, after: this.lastConfirmedCursor })
    try {
      this.connection = this.transport({
        url,
        signal: abort.signal,
        headers: { "Last-Event-ID": String(this.lastConfirmedCursor) },
        onFrame: (frame) => {
          void this.processFrame(frame, token).catch((error) => this.detachedFailure("frame", error))
        },
        onError: (error) => {
          void this.enqueueIngest(() => this.handleTransportFailure(error, token)).catch((failure) => this.detachedFailure("transport-error", failure))
        },
        onEof: () => {
          void this.enqueueIngest(() => this.handleTransportFailure(new Error("SSE EOF"), token)).catch((failure) => this.detachedFailure("transport-eof", failure))
        },
      })
    } catch (error) {
      void this.handleTransportFailure(error, token).catch((failure) => this.detachedFailure("transport-open", failure))
    }
  }

  private closeConnection(): void {
    this.connectionAbort?.abort()
    this.connectionAbort = null
    this.connection?.close()
    this.connection = null
  }

  private async ingestFrame(frame: RawSseFrame, token: SyncToken): Promise<void> {
    if (!this.running || !this.isCurrent(token)) return

    if (this.adapter.isControlFrame(frame)) {
      const control: ControlValidationResult = this.adapter.validateControl(frame)
      if (control.status === "invalid") {
        await this.protocolFailure("invalid-control", token, control.code)
        return
      }
      if (this.recovery !== null) this.recoverySawLiveFrame = true
      this.markLive(token)
      return
    }

    const parsed: EnvelopeParseResult = this.adapter.parseEnvelope(frame)
    if (parsed.status === "invalid") {
      await this.protocolFailure("invalid-envelope", token, parsed.code)
      return
    }
    await this.ingestEnvelope(parsed.envelope, token, "sse")
  }

  private async ingestEnvelope(envelope: NonNullable<Extract<EnvelopeParseResult, { status: "valid" }>["envelope"]>, token: SyncToken, source: "sse" | "poll"): Promise<void> {
    if (!this.running || !this.isCurrent(token)) return
    if (envelope.boardId !== this.boardId) {
      this.emit("isolation-anomaly", { eventKind: envelope.kind })
      await this.protocolFailure("board-isolation", token, envelope.boardId)
      return
    }

    const validation: BusinessValidationResult = this.adapter.validateBusiness(envelope)
    if (validation.status === "invalid") {
      await this.protocolFailure("invalid-business-event", token, validation.code)
      return
    }
    const event = validation.event
    if (event.boardId !== this.boardId) {
      this.emit("isolation-anomaly", { eventKind: event.kind })
      await this.protocolFailure("board-isolation", token, event.boardId)
      return
    }

    if (source === "sse") {
      if (this.recovery !== null) this.recoverySawLiveFrame = true
      this.markLive(token)
    }
    const decision = this.dedupeDecision(event)
    if (decision === "conflict") {
      await this.protocolFailure("duplicate-conflict", token, event.eventId)
      return
    }
    if (decision === "stale") {
      await this.protocolFailure("stale-cursor", token, String(event.id))
      return
    }
    if (decision === "duplicate") {
      if (this.recoveryStage !== null) this.recoveryStage.cursor = Math.max(this.recoveryStage.cursor, event.id)
      else if (event.id > this.lastConfirmedCursor) this.lastConfirmedCursor = event.id
      return
    }

    if (this.recovery !== null) {
      const eventBytes = validatedEventByteLength(event)
      if (this.recoveryBuffer.length >= this.maxSeenEntries * 2 || this.recoveryBufferBytes + eventBytes > this.maxRecoveryBufferBytes) {
        await this.protocolFailure("recovery-buffer-overflow", token, `${this.recoveryBuffer.length}/${this.recoveryBufferBytes}`)
        return
      }
      this.recoveryBuffer.push(event)
      this.recoveryBufferBytes += eventBytes
      // Staging this event makes duplicate SSE deliveries idempotent without
      // publishing its cursor or fingerprints before the recovery barrier.
      this.commitEvent(event)
      return
    }
    await this.applyEvent(event, token, source)
  }

  private dedupeDecision(event: ValidatedBusinessEvent): "new" | "duplicate" | "conflict" | "stale" {
    const stage = this.recoveryStage
    const idFingerprint = (stage?.seenIds ?? this.seenIds).get(event.id)
    const eventFingerprint = (stage?.seenEventIds ?? this.seenEventIds).get(event.eventId)
    if (idFingerprint === undefined && eventFingerprint === undefined) {
      const cursor = stage?.cursor ?? this.lastConfirmedCursor
      return event.id <= cursor ? "stale" : "new"
    }
    if (idFingerprint === event.canonicalFingerprint && eventFingerprint === event.canonicalFingerprint) return "duplicate"
    return "conflict"
  }

  private async applyEvent(event: ValidatedBusinessEvent, token: SyncToken, source: "sse" | "poll"): Promise<void> {
    if (!this.isCurrent(token)) return
    try {
      await this.invokeEffect(event, token, source, source === "poll" && this.state === "circuit-open")
    } catch (error) {
      this.emit("sink-effect-failure", { eventId: event.eventId, message: error instanceof Error ? error.message : String(error) })
      void this.beginRecovery("F", "sink-effect-failure", false).catch((failure) => this.detachedFailure("sink-effect-failure", failure))
      return
    }
    if (!this.isCurrent(token)) return

    if (!event.known) {
      // E is lossless and exactly once; the event remains generation-local
      // while the non-budgeted F barrier establishes the authoritative cursor.
      void this.beginRecovery("F", "unknown-event", false, [event]).catch((error) => this.detachedFailure("unknown-event", error))
      return
    }
    this.commitEvent(event)
  }

  private isValidBoundaryShape(boundary: RecoveryBoundary, baseCursor: number): boolean {
    if (!Number.isSafeInteger(baseCursor) || baseCursor < 0) return false
    if (!Number.isSafeInteger(boundary.highWatermark) || boundary.highWatermark < baseCursor) return false
    if (!(boundary.byId instanceof Map) || !(boundary.byEventId instanceof Map) || !Array.isArray(boundary.events) || boundary.byId.size > this.maxBoundaryEvents || boundary.byEventId.size > this.maxBoundaryEvents) return false
    if (boundary.events.length > this.maxBoundaryEvents) return false
    let boundaryBytes = 0
    const eventIds = new Set<number>()
    const eventKeys = new Set<string>()
    let previousId = -1
    let maxEventId = baseCursor
    for (const event of boundary.events) {
      const eventBytes = validatedEventByteLength(event)
      if (!Number.isFinite(eventBytes) || (boundaryBytes += eventBytes) > this.maxBoundaryBytes) return false
      if (event.boardId !== this.boardId || !Number.isSafeInteger(event.id) || event.id < 0 || event.id > boundary.highWatermark || event.id <= previousId || typeof event.eventId !== "string" || event.eventId.length === 0 || typeof event.canonicalFingerprint !== "string") return false
      if (eventIds.has(event.id) || eventKeys.has(event.eventId)) return false
      eventIds.add(event.id)
      eventKeys.add(event.eventId)
      previousId = event.id
      maxEventId = Math.max(maxEventId, event.id)
      if (boundary.byId.get(event.id) !== event.canonicalFingerprint || boundary.byEventId.get(event.eventId) !== event.canonicalFingerprint) return false
    }
    for (const [id, fingerprint] of boundary.byId) {
      const entryBytes = valueByteLength(id) + valueByteLength(fingerprint)
      if (!Number.isFinite(entryBytes) || (boundaryBytes += entryBytes) > this.maxBoundaryBytes) return false
      if (!Number.isSafeInteger(id) || id < 0 || id > boundary.highWatermark || typeof fingerprint !== "string") return false
      if (id > baseCursor && !eventIds.has(id)) return false
      if (id <= baseCursor) {
        if (!this.oldBoundaryIdPairMatches(boundary, id, fingerprint)) return false
      }
    }
    for (const [eventId, fingerprint] of boundary.byEventId) {
      const entryBytes = valueByteLength(eventId) + valueByteLength(fingerprint)
      if (!Number.isFinite(entryBytes) || (boundaryBytes += entryBytes) > this.maxBoundaryBytes) return false
      if (typeof eventId !== "string" || eventId.length === 0 || typeof fingerprint !== "string") return false
      if (eventId.length > 0 && !eventKeys.has(eventId)) {
        if (!this.oldBoundaryEventPairMatches(boundary, eventId, fingerprint)) return false
      }
    }
    if (boundary.highWatermark > baseCursor && (boundary.events.length === 0 || maxEventId !== boundary.highWatermark)) return false
    return true
  }

  private isValidRecoveryResult(result: RecoveryResult, token: SyncToken, expectedRevision: number, after: number): boolean {
    const boundary = result.boundary
    return result.noGap && Number.isSafeInteger(result.confirmedCursor) && result.confirmedCursor >= 0 && boundary.published && boundary.revision === expectedRevision && recoveryTokenEquals(boundary.token, token) && boundary.highWatermark >= result.confirmedCursor && this.isValidBoundaryShape(boundary, after)
  }

  private commitEvent(event: ValidatedBusinessEvent): void {
    const stage = this.recoveryStage
    const seenIds = stage?.seenIds ?? this.seenIds
    const seenEventIds = stage?.seenEventIds ?? this.seenEventIds
    seenIds.set(event.id, event.canonicalFingerprint)
    seenEventIds.set(event.eventId, event.canonicalFingerprint)
    this.trimSeen(seenIds)
    this.trimSeen(seenEventIds)
    if (stage === null) this.rememberEvent(event)
    if (stage !== null) stage.cursor = Math.max(stage.cursor, event.id)
    else this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, event.id)
  }

  private rememberEvent(event: ValidatedBusinessEvent): void {
    this.seenEventsById.set(event.id, event)
    this.seenEventsByEventId.set(event.eventId, event)
    while (this.seenEventsById.size > this.maxSeenEntries) {
      const oldest = this.seenEventsById.keys().next().value
      if (oldest === undefined) break
      this.seenEventsById.delete(oldest)
    }
    while (this.seenEventsByEventId.size > this.maxSeenEntries) {
      const oldest = this.seenEventsByEventId.keys().next().value
      if (oldest === undefined) break
      this.seenEventsByEventId.delete(oldest)
    }
  }

  private knownEventForId(id: number): ValidatedBusinessEvent | undefined {
    const remembered = this.seenEventsById.get(id)
    if (remembered !== undefined) return remembered
    for (const event of this.recoveryAppliedEvents.values()) if (event.id === id) return event
    return undefined
  }

  private knownEventForEventId(eventId: string): ValidatedBusinessEvent | undefined {
    const remembered = this.seenEventsByEventId.get(eventId)
    if (remembered !== undefined) return remembered
    const applied = this.recoveryAppliedEvents.get(eventId)
    if (applied !== undefined) return applied
    return undefined
  }

  private oldBoundaryIdPairMatches(boundary: RecoveryBoundary, id: number, fingerprint: string): boolean {
    const event = this.knownEventForId(id)
    return event !== undefined && event.boardId === this.boardId && event.canonicalFingerprint === fingerprint && boundary.byEventId.get(event.eventId) === fingerprint
  }

  private oldBoundaryEventPairMatches(boundary: RecoveryBoundary, eventId: string, fingerprint: string): boolean {
    const event = this.knownEventForEventId(eventId)
    return event !== undefined && event.boardId === this.boardId && event.canonicalFingerprint === fingerprint && boundary.byId.get(event.id) === fingerprint
  }

  private trimSeen<K>(map: Map<K, string>): void {
    while (map.size > this.maxSeenEntries) {
      const oldest = map.keys().next().value
      if (oldest === undefined) break
      map.delete(oldest)
    }
  }

  private markLive(token: SyncToken): void {
    if (!this.isCurrent(token) || this.state === "circuit-open") return
    if (this.recovery !== null) {
      this.armLiveness(token)
      return
    }
    this.state = "live"
    this.stopPolling()
    this.armLiveness(token)
  }

  private armLiveness(token: SyncToken): void {
    this.clearLiveness()
    this.livenessTimer = this.clock.setTimeout(() => {
      if (!this.running || !this.isCurrent(token) || this.state === "circuit-open") return
      this.emit("stalled")
      if (this.recovery !== null) {
        this.fenceRecoveryConnection()
        this.scheduleRecoveryConnectionRetry("stalled", 1)
        return
      }
      void this.beginRecovery("R", "stalled", false).catch((error) => this.detachedFailure("stalled-recovery", error))
    }, this.livenessTimeoutMs)
  }

  private clearLiveness(): void {
    if (this.livenessTimer === null) return
    this.clock.clearTimeout(this.livenessTimer)
    this.livenessTimer = null
  }

  private async handleTransportFailure(error: unknown, token: SyncToken): Promise<void> {
    if (!this.running || !this.isCurrent(token) || this.state === "circuit-open") return
    this.emit("transport-failure", { message: error instanceof Error ? error.message : String(error) })
    if (this.recovery !== null) {
      // A failed overlapping SSE epoch must not cancel the in-flight F/R
      // barrier or discard its staged/pre-applied evidence.
      this.fenceRecoveryConnection()
      this.scheduleRecoveryConnectionRetry("transport-failure", 1)
      return
    }
    // Start the barrier after all prior frames in this queue have committed,
    // then release the queue so the new epoch can stage concurrent SSE frames.
    void this.beginRecovery("R", "transport-failure", false).catch((failure) => this.detachedFailure("transport-recovery", failure))
  }

  private anomalyKey(): string {
    return `${this.boardId}:${this.lastConfirmedCursor}`
  }

  private async protocolFailure(reason: string, token: SyncToken, code: string): Promise<void> {
    if (!this.running || !this.isCurrent(token) || this.state === "circuit-open") return
    this.emit("protocol-anomaly", { reason, code })
    if (this.recovery !== null) {
      const key = this.anomalyKey()
      const attempts = (this.anomalyAttempts.get(key) ?? 0) + 1
      this.anomalyAttempts.set(key, attempts)
      this.fenceRecoveryConnection()
      if (attempts >= this.maxAnomalyAttempts) {
        this.openCircuit(reason)
        return
      }
      this.scheduleRecoveryConnectionRetry(reason, attempts)
      return
    }
    if (this.recoveryStartTimer !== null) {
      this.emit("protocol-anomaly-suppressed", { reason, code })
      return
    }
    const key = this.anomalyKey()
    const attempts = (this.anomalyAttempts.get(key) ?? 0) + 1
    this.anomalyAttempts.set(key, attempts)
    if (attempts >= this.maxAnomalyAttempts) {
      this.openCircuit(reason)
      return
    }
    this.fenceConnection()
    if (this.recoveryStartTimer !== null) return
    this.scheduleRecoveryStart("F", reason, true, this.recoveryBackoff(attempts))
  }

  private fenceRecovery(): void {
    this.recovery = null
    this.recoveryAbort?.abort()
    this.recoveryAbort = null
    this.fenceConnection()
  }

  private fenceConnection(): void {
    this.closeConnection()
    this.clearRecoveryConnectionRetryTimer()
    this.connectionEpoch += 1
    this.generation += 1
    this.cancelEffects()
    this.recoverySawLiveFrame = false
    this.dropRecoveryStage()
  }

  private fenceRecoveryConnection(): void {
    this.closeConnection()
    this.clearRecoveryConnectionRetryTimer()
    this.clearLiveness()
    this.recoverySawLiveFrame = false
    this.connectionEpoch += 1
  }

  private scheduleRecoveryConnectionRetry(reason: string, attempt: number): void {
    if (!this.running || this.state === "circuit-open" || this.recoveryConnectionRetryTimer !== null) return
    const delayMs = this.recoveryBackoff(attempt)
    this.recoveryConnectionRetryTimer = this.clock.setTimeout(() => {
      this.recoveryConnectionRetryTimer = null
      if (!this.running || this.state === "circuit-open") return
      this.openConnection()
      this.armLiveness(this.currentToken())
      this.emit("recovery-connection-retry", { reason, attempt, delayMs })
    }, delayMs)
  }

  private clearRecoveryConnectionRetryTimer(): void {
    if (this.recoveryConnectionRetryTimer === null) return
    this.clock.clearTimeout(this.recoveryConnectionRetryTimer)
    this.recoveryConnectionRetryTimer = null
  }

  private recoveryBackoff(attempt: number): number {
    return [250, 1_000, 5_000][Math.min(Math.max(attempt, 1) - 1, 2)] ?? 5_000
  }

  private scheduleRecoveryStart(mode: "F" | "R", reason: string, budgeted: boolean, delayMs: number): void {
    if (!this.running || this.state === "circuit-open") return
    this.state = "polling"
    this.schedulePolling()
      this.recoveryStartTimer = this.clock.setTimeout(() => {
        this.recoveryStartTimer = null
        if (!this.running || this.state === "circuit-open") return
        void this.beginRecovery(mode, reason, budgeted).catch((error) => this.detachedFailure("scheduled-recovery", error))
      }, delayMs)
  }

  private clearRecoveryStartTimer(): void {
    if (this.recoveryStartTimer === null) return
    this.clock.clearTimeout(this.recoveryStartTimer)
    this.recoveryStartTimer = null
  }

  private openCircuit(reason: string): void {
    this.state = "circuit-open"
    this.closeConnection()
    this.recoveryAbort?.abort()
    this.recoveryAbort = null
    this.cancelEffects()
    this.clearLiveness()
    this.clearRecoveryStartTimer()
    this.clearRecoveryConnectionRetryTimer()
    this.recovery = null
    this.dropRecoveryStage()
    this.stopPolling()
    this.pollingBoundaryCommit = null
    this.emit("circuit-open", { reason })
    this.schedulePolling()
  }

  private async beginRecovery(mode: "F" | "R", reason: string, budgeted: boolean, preApplied: readonly ValidatedBusinessEvent[] = []): Promise<void> {
    if (!this.running || (this.state === "circuit-open" && budgeted)) return
    if (this.recovery !== null) return this.recovery

    this.closeConnection()
    this.clearLiveness()
    this.connectionEpoch += 1
    this.generation += 1
    const token = this.currentToken()
    const after = this.lastConfirmedCursor
    const expectedRevision = this.recoveryRevision + 1
    this.recoveryRevision = expectedRevision
    this.recoveryStage = {
      baseCursor: after,
      expectedRevision,
      seenIds: new Map(this.seenIds),
      seenEventIds: new Map(this.seenEventIds),
      cursor: after,
    }
    this.state = "recovering"
    this.recoveryBuffer = [...preApplied]
    this.recoveryBufferBytes = preApplied.reduce((total, event) => total + validatedEventByteLength(event), 0)
    if (this.recoveryBufferBytes > this.maxRecoveryBufferBytes || this.recoveryBuffer.length > this.maxSeenEntries * 2) {
      this.recoveryBuffer = []
      this.recoveryBufferBytes = 0
      this.emit("recovery-failure", { message: "recovery pre-applied buffer exceeds limits" })
      this.fenceRecovery()
      this.scheduleRecoveryStart("R", "recovery-buffer-overflow", false, this.reconnectDelayMs)
      return
    }
    this.recoveryPreApplied.clear()
    this.recoveryAppliedEvents.clear()
    for (const event of preApplied) {
      this.recoveryPreApplied.set(event.eventId, event.canonicalFingerprint)
      this.recoveryAppliedEvents.set(event.eventId, event)
      this.commitEvent(event)
    }
    this.recoverySawLiveFrame = false
    this.schedulePolling()
    this.emit("recovery-start", { mode, reason })

    const recoveryAbort = new AbortController()
    this.recoveryAbort = recoveryAbort
    const operation = this.performRecovery(mode, token, after, budgeted, expectedRevision, recoveryAbort.signal)
    this.recovery = operation
    // R/F barrier and the next SSE subscription overlap. Frames from this
    // generation are staged until the boundary is published and replayed.
    this.openConnection()
    this.armLiveness(this.currentToken())
    try {
      await operation
    } finally {
      if (this.recovery === operation) this.recovery = null
      if (this.recoveryAbort === recoveryAbort) this.recoveryAbort = null
    }
  }

  private async performRecovery(mode: "F" | "R", token: SyncToken, after: number, budgeted: boolean, expectedRevision: number, signal: AbortSignal): Promise<void> {
    let countsBudget = budgeted
    try {
      const result = await this.sink.refetchObserved(mode, token, after, expectedRevision, signal)
      if (signal.aborted || !this.isRecoveryCurrent(token)) return
      if (result.boundary.token.boardId !== token.boardId || result.boundary.token.generation !== token.generation || result.boundary.revision !== expectedRevision) {
        // A response from an older publication/generation is not an anomaly:
        // discard it and issue a fresh barrier under a new token.
        this.emit("recovery-stale-result", { revision: result.boundary.revision, expectedRevision })
        this.fenceRecovery()
        await this.beginRecovery(mode, "stale-result", false)
        return
      }
      const validResult = this.isValidRecoveryResult(result, token, expectedRevision, after)
      if (!validResult || result.confirmedCursor < after) {
        throw new Error("recovery boundary is not gap-free")
      }

      const boundary = result.boundary
      this.seedBoundary(boundary)
      const replayedBoundaryEvents = await this.replayBoundaryEvents(token, boundary, after)
      const finalBoundary = await this.replayRecoveryBuffer(token, boundary, replayedBoundaryEvents)
      if (signal.aborted || !this.isRecoveryCurrent(token)) return
      this.promoteRecoveryStage(Math.max(result.confirmedCursor, finalBoundary.highWatermark))
      this.clearRecoveryConnectionRetryTimer()
      if (result.confirmedCursor > after) this.anomalyAttempts.delete(`${this.boardId}:${after}`)
      if (signal.aborted || !this.isRecoveryCurrent(token)) return
      const hasCurrentConnection = this.connection !== null && !this.connection.closed && this.connectionAbort !== null && !this.connectionAbort.signal.aborted
      if (!hasCurrentConnection) this.openConnection()
      this.state = this.recoverySawLiveFrame && hasCurrentConnection ? "live" : "connecting"
      this.recovery = null
      this.recoveryBuffer = []
      this.recoveryBufferBytes = 0
      this.recoveryPreApplied.clear()
      if (this.state === "live") this.stopPolling()
      this.armLiveness(this.currentToken())
      this.emit("recovery-complete", { mode, confirmedCursor: result.confirmedCursor })
    } catch (error) {
      if (signal.aborted || !this.isRecoveryCurrent(token)) return
      this.emit("recovery-failure", { message: error instanceof Error ? error.message : String(error) })
      if (signal.aborted || !this.isRecoveryCurrent(token)) return
      if (error instanceof SinkEffectCancelled) return
      if (error instanceof SinkEffectError) countsBudget = false
      if (countsBudget) {
        const key = this.anomalyKey()
        const attempts = this.anomalyAttempts.get(key) ?? 1
        this.anomalyAttempts.set(key, attempts)
        if (attempts >= this.maxAnomalyAttempts) {
          this.openCircuit("recovery-failure")
          return
        }
        const nextAttempt = attempts + 1
        this.anomalyAttempts.set(key, nextAttempt)
        this.recovery = null
        this.fenceConnection()
        this.scheduleRecoveryStart("F", "recovery-failure", true, this.recoveryBackoff(nextAttempt))
        return
      }
      this.recovery = null
      this.fenceConnection()
      this.scheduleRecoveryStart("R", "recovery-failure", false, this.reconnectDelayMs)
    }
  }

  private seedBoundary(boundary: RecoveryBoundary): void {
    const stage = this.recoveryStage
    if (stage === null) throw new Error("recovery stage is missing")
    for (const [id, fingerprint] of boundary.byId) {
      const current = stage.seenIds.get(id)
      if (current !== undefined && current !== fingerprint) throw new Error("recovery boundary id fingerprint conflict")
      stage.seenIds.set(id, fingerprint)
    }
    for (const [eventId, fingerprint] of boundary.byEventId) {
      const current = stage.seenEventIds.get(eventId)
      if (current !== undefined && current !== fingerprint) throw new Error("recovery boundary event fingerprint conflict")
      stage.seenEventIds.set(eventId, fingerprint)
    }
    this.trimSeen(stage.seenIds)
    this.trimSeen(stage.seenEventIds)
    stage.cursor = Math.max(stage.cursor, boundary.highWatermark)
  }

  private async replayBoundaryEvents(token: SyncToken, boundary: RecoveryBoundary, after: number): Promise<Set<string>> {
    const replayed = new Set<string>()
    let previousId = -1
    const eligible: ValidatedBusinessEvent[] = []
    for (const event of boundary.events) {
      if (!this.isRecoveryCurrent(token)) return replayed
      if (event.boardId !== this.boardId || !Number.isSafeInteger(event.id) || event.id < 0 || event.id > boundary.highWatermark || typeof event.eventId !== "string" || event.eventId.length === 0) throw new Error("recovery boundary event is out of scope")
      if (event.id <= previousId) throw new Error("recovery boundary events are not ordered")
      const idFingerprint = boundary.byId.get(event.id)
      const eventFingerprint = boundary.byEventId.get(event.eventId)
      if (idFingerprint !== event.canonicalFingerprint || eventFingerprint !== event.canonicalFingerprint) throw new Error("recovery boundary event fingerprint mismatch")
      previousId = event.id
      if (event.id > after) eligible.push(event)
    }
    for (const event of eligible) {
      if (!this.isRecoveryCurrent(token)) return replayed
      replayed.add(event.eventId)
      if (this.recoveryPreApplied.get(event.eventId) === event.canonicalFingerprint) continue
      await this.invokeEffect(event, token, "recovery")
      if (!this.isRecoveryCurrent(token)) return replayed
      this.recoveryAppliedEvents.set(event.eventId, event)
      this.recoveryPreApplied.set(event.eventId, event.canonicalFingerprint)
      if (!event.known) {
        await this.restartUnknownRecovery(token, event)
        return replayed
      }
    }
    return replayed
  }

  private async replayRecoveryBuffer(token: SyncToken, initialBoundary: RecoveryBoundary, replayedBoundaryEvents: ReadonlySet<string>): Promise<RecoveryBoundary> {
    const boundary = initialBoundary
    for (;;) {
      // Let the concurrently opened SSE/poll transport deliver frames that
      // arrived immediately after the boundary response settled.
      await Promise.resolve()
      if (this.recoveryBuffer.length === 0) break
      const buffered = this.recoveryBuffer.splice(0).sort((left, right) => left.id - right.id)
      this.recoveryBufferBytes = 0
      let previousBufferedId = -1
      // Validate the complete batch before emitting any projection effect.
      for (const event of buffered) {
        if (event.boardId !== this.boardId || !Number.isSafeInteger(event.id) || event.id < 0 || event.id <= previousBufferedId || typeof event.eventId !== "string" || event.eventId.length === 0) throw new Error("recovery buffer event is out of scope")
        previousBufferedId = event.id
        if (event.id <= boundary.highWatermark) {
          const idFingerprint = boundary.byId.get(event.id)
          const eventFingerprint = boundary.byEventId.get(event.eventId)
          if (idFingerprint !== event.canonicalFingerprint || eventFingerprint !== event.canonicalFingerprint) throw new Error("recovery boundary overlap conflict")
        }
      }
      for (const event of buffered) {
        if (!this.isRecoveryCurrent(token)) return boundary
        if (event.id <= boundary.highWatermark) {
          if (this.recoveryPreApplied.get(event.eventId) === event.canonicalFingerprint || replayedBoundaryEvents.has(event.eventId)) continue
          await this.invokeEffect(event, token, "recovery")
          if (!this.isRecoveryCurrent(token)) return boundary
          this.recoveryAppliedEvents.set(event.eventId, event)
          this.recoveryPreApplied.set(event.eventId, event.canonicalFingerprint)
          if (!event.known) {
            await this.restartUnknownRecovery(token, event)
            return boundary
          }
          continue
        }
        if (this.recoveryPreApplied.get(event.eventId) === event.canonicalFingerprint) continue
        await this.invokeEffect(event, token, "recovery")
        if (!this.isRecoveryCurrent(token)) return boundary
        this.recoveryAppliedEvents.set(event.eventId, event)
        this.recoveryPreApplied.set(event.eventId, event.canonicalFingerprint)
        if (!event.known) {
          await this.restartUnknownRecovery(token, event)
          return boundary
        }
      }
    }
    return boundary
  }

  private async restartUnknownRecovery(token: SyncToken, event: ValidatedBusinessEvent): Promise<void> {
    if (!this.isRecoveryCurrent(token)) return
    const preApplied = [...this.recoveryAppliedEvents.values()]
    if (!preApplied.some((candidate) => candidate.eventId === event.eventId)) preApplied.push(event)
    this.fenceRecovery()
    await this.beginRecovery("F", "unknown-event-during-recovery", false, preApplied)
  }

  private promoteRecoveryStage(cursor: number): void {
    const stage = this.recoveryStage
    if (stage === null) throw new Error("recovery stage is missing")
    this.seenIds.clear()
    for (const [id, fingerprint] of stage.seenIds) this.seenIds.set(id, fingerprint)
    this.seenEventIds.clear()
    for (const [eventId, fingerprint] of stage.seenEventIds) this.seenEventIds.set(eventId, fingerprint)
    this.trimSeen(this.seenIds)
    this.trimSeen(this.seenEventIds)
    for (const event of this.recoveryAppliedEvents.values()) this.rememberEvent(event)
    this.recoveryAppliedEvents.clear()
    this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, cursor, stage.cursor)
    this.recoveryStage = null
  }

  private dropRecoveryStage(): void {
    this.recoveryStage = null
    this.recoveryBuffer = []
    this.recoveryBufferBytes = 0
    this.recoveryPreApplied.clear()
    this.recoveryAppliedEvents.clear()
  }

  private schedulePolling(): void {
    if (this.pollingTimer !== null || !this.running) return
    const schedule = (): void => {
      if (!this.running || this.state === "live") return
      this.pollingTimer = this.clock.setTimeout(() => {
        this.pollingTimer = null
        void this.pollOnce().then(() => schedule(), (error) => {
          this.detachedFailure("poll", error)
          schedule()
        })
      }, this.pollingIntervalMs)
    }
    schedule()
  }

  private stopPolling(): void {
    if (this.pollingTimer !== null) {
      this.clock.clearTimeout(this.pollingTimer)
      this.pollingTimer = null
    }
    this.pollingAbort?.abort()
    this.pollingAbort = null
  }

  private pollProtocolFailure(reason: string, code: string): void {
    this.emit("poll-protocol-anomaly", { reason, code })
  }

  /** Commit a published catch-up boundary while the SSE circuit is open. */
  private applyPollingBoundary(page: PollEventsPage, token: SyncToken, signal: AbortSignal): Promise<boolean> {
    if (this.pollingBoundaryCommit !== null) return this.pollingBoundaryCommit
    const operation = this.commitPollingBoundary(page, token, signal)
    const tracked = operation.finally(() => {
      if (this.pollingBoundaryCommit === tracked) this.pollingBoundaryCommit = null
    })
    this.pollingBoundaryCommit = tracked
    return tracked
  }

  private async commitPollingBoundary(page: PollEventsPage, token: SyncToken, signal: AbortSignal): Promise<boolean> {
    const boundary = page.boundary
    if (boundary === undefined) {
      this.pollProtocolFailure("poll-boundary-missing", "published-boundary-required")
      return false
    }
    const confirmedCursor = page.confirmedCursor ?? boundary.highWatermark
    if (signal.aborted || !this.running || this.state !== "circuit-open" || !this.isCurrent(token) || !Number.isSafeInteger(confirmedCursor) || confirmedCursor < 0 || !boundary.published || boundary.revision <= 0 || boundary.highWatermark < confirmedCursor || !tokenEquals(boundary.token, token) || !this.isValidBoundaryShape(boundary, this.lastConfirmedCursor)) {
      this.pollProtocolFailure("poll-boundary-invalid", "incompatible-boundary")
      return false
    }
    if (confirmedCursor <= this.lastConfirmedCursor && boundary.highWatermark <= this.lastConfirmedCursor) {
      this.pollProtocolFailure("poll-boundary-no-progress", String(confirmedCursor))
      return false
    }
    const stagedIds = new Map(this.seenIds)
    const stagedEventIds = new Map(this.seenEventIds)
    const stagedKnownEvents: ValidatedBusinessEvent[] = []
    try {
      for (const [id, fingerprint] of boundary.byId) {
        if (!Number.isSafeInteger(id) || id < 0 || id > boundary.highWatermark || typeof fingerprint !== "string") throw new Error("boundary-id-invalid")
        const existing = stagedIds.get(id)
        if (existing !== undefined && existing !== fingerprint) throw new Error("boundary-id-conflict")
        stagedIds.set(id, fingerprint)
      }
      for (const [eventId, fingerprint] of boundary.byEventId) {
        if (eventId.length === 0 || typeof fingerprint !== "string") throw new Error("boundary-event-id-invalid")
        const existing = stagedEventIds.get(eventId)
        if (existing !== undefined && existing !== fingerprint) throw new Error("boundary-event-conflict")
        stagedEventIds.set(eventId, fingerprint)
      }
      const eligible: ValidatedBusinessEvent[] = []
      for (const event of boundary.events) {
        if (event.id <= this.lastConfirmedCursor) continue
        const existing = stagedIds.get(event.id)
        if (existing !== undefined && existing !== event.canonicalFingerprint) throw new Error("boundary-id-conflict")
        const existingEvent = stagedEventIds.get(event.eventId)
        if (existingEvent !== undefined && existingEvent !== event.canonicalFingerprint) throw new Error("boundary-event-conflict")
        eligible.push(event)
      }
      for (const event of eligible) {
        if (signal.aborted || !this.running || !this.isCurrent(token) || this.state !== "circuit-open") return false
        try {
          await this.invokeEffect(event, token, "poll-boundary", true)
        } catch (error) {
          if (signal.aborted || !this.running || !this.isCurrent(token) || this.state !== "circuit-open") return false
          this.emit("sink-effect-failure", { eventId: event.eventId, message: error instanceof Error ? error.message : String(error) })
          void this.beginRecovery("F", "sink-effect-failure", false).catch((failure) => this.detachedFailure("poll-sink-effect-failure", failure))
          return false
        }
        if (signal.aborted || !this.running || !this.isCurrent(token) || this.state !== "circuit-open") return false
        if (event.known) stagedKnownEvents.push(event)
        if (!event.known) {
          this.state = "recovering"
          void this.beginRecovery("F", "unknown-event-during-poll", false, [event]).catch((failure) => this.detachedFailure("poll-unknown-event", failure))
          return false
        }
      }
    } catch (error) {
      this.pollProtocolFailure("poll-boundary-invalid", error instanceof Error ? error.message : String(error))
      return false
    }
    if (signal.aborted || !this.running || !this.isCurrent(token) || this.state !== "circuit-open") return false
    const staleAnomalyKey = this.anomalyKey()
    this.trimSeen(stagedIds)
    this.trimSeen(stagedEventIds)
    this.seenIds.clear()
    for (const [id, fingerprint] of stagedIds) this.seenIds.set(id, fingerprint)
    this.seenEventIds.clear()
    for (const [eventId, fingerprint] of stagedEventIds) this.seenEventIds.set(eventId, fingerprint)
    this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, confirmedCursor, boundary.highWatermark)
    for (const event of stagedKnownEvents) this.rememberEvent(event)
    this.anomalyAttempts.delete(staleAnomalyKey)
    this.state = "connecting"
    this.openConnection()
    this.armLiveness(this.currentToken())
    this.emit("poll-boundary-complete", { confirmedCursor: this.lastConfirmedCursor })
    return true
  }

  private async pollOnce(): Promise<void> {
    if (!this.running || this.state === "live") return
    const token = this.currentToken()
    const abort = new AbortController()
    this.pollingAbort = abort
    try {
      let queryAfter = this.lastConfirmedCursor
      let page: PollEventsPage
      do {
        const query: ApiListEventsQueryContract = parseApiListEventsQuery({ board: this.boardId, after: queryAfter, limit: 100 })
        page = await this.sink.pollEvents(query, abort.signal)
        if (abort.signal.aborted || !this.running || !this.isCurrent(token)) return
        if (!Number.isSafeInteger(page.nextAfter) || page.nextAfter < 0 || page.nextAfter < queryAfter || (page.hasMore && page.nextAfter === queryAfter)) {
          this.pollProtocolFailure("poll-invalid-cursor", String(page.nextAfter))
          return
        }
        if (!page.noGap) {
          if (this.state === "circuit-open") this.pollProtocolFailure("poll-gap", String(page.nextAfter))
          else await this.protocolFailure("poll-gap", token, String(page.nextAfter))
          return
        }
        if (this.state === "circuit-open" && !(await this.applyPollingBoundary(page, token, abort.signal))) return
        for (const rawEvent of page.events) {
          const parsed = this.adapter.parsePollingEnvelope(rawEvent)
          if (parsed.status === "invalid") {
            if (this.state === "circuit-open") this.pollProtocolFailure("poll-invalid-envelope", parsed.code)
            else await this.protocolFailure("poll-invalid-envelope", token, parsed.code)
            return
          }
          await this.enqueueIngest(() => this.ingestEnvelope(parsed.envelope, token, "poll"))
          if (abort.signal.aborted || !this.running || !this.isCurrent(token)) return
        }
        queryAfter = page.nextAfter
      } while (page.hasMore)
      if (this.isCurrent(token)) this.emit("poll-complete")
    } catch (error) {
      if (this.isCurrent(token)) this.emit("poll-failure", { message: error instanceof Error ? error.message : String(error) })
    } finally {
      if (this.pollingAbort === abort) this.pollingAbort = null
    }
  }
}

export function defaultStreamUrl(baseUrl: string, context: StreamUrlContext): string {
  return appendStreamQuery(baseUrl, context)
}
