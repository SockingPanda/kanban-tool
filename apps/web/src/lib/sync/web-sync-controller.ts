import { classifyEvent } from "./invalidation"
import type {
  BusinessValidationResult,
  ControlValidationResult,
  EnvelopeParseResult,
  InvalidationPlan,
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
  private pollingAbort: AbortController | null = null
  private pollingBoundaryCommit: Promise<boolean> | null = null
  private recovery: Promise<void> | null = null
  private recoveryBuffer: ValidatedBusinessEvent[] = []
  private recoverySawLiveFrame = false
  private recoveryStage: RecoveryStage | null = null
  private readonly recoveryPreApplied = new Map<string, string>()
  private recoveryRevision = 0
  private ingestTail: Promise<void> = Promise.resolve()
  private readonly seenIds = new Map<number, string>()
  private readonly seenEventIds = new Map<string, string>()
  private readonly anomalyAttempts = new Map<string, number>()

  constructor(options: WebSyncControllerOptions) {
    this.boardId = options.boardId
    this.streamUrl = options.streamUrl
    this.transport = options.transport
    this.adapter = options.adapter
    this.sink = options.sink
    this.clock = options.clock ?? systemClock()
    this.telemetry = options.telemetry ?? NOOP_TELEMETRY
    this.livenessTimeoutMs = options.livenessTimeoutMs ?? 35_000
    this.pollingIntervalMs = options.pollingIntervalMs ?? 5_000
    this.reconnectDelayMs = options.reconnectDelayMs ?? 250
    this.maxSeenEntries = options.maxSeenEntries ?? 2_048
    this.maxAnomalyAttempts = options.maxAnomalyAttempts ?? 3
    if (!Number.isSafeInteger(this.livenessTimeoutMs) || this.livenessTimeoutMs <= 0) throw new RangeError("livenessTimeoutMs must be positive")
    if (!Number.isSafeInteger(this.pollingIntervalMs) || this.pollingIntervalMs <= 0) throw new RangeError("pollingIntervalMs must be positive")
    if (!Number.isSafeInteger(this.maxSeenEntries) || this.maxSeenEntries <= 0) throw new RangeError("maxSeenEntries must be positive")
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
    this.closeConnection()
    this.stopPolling()
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
    this.lastConfirmedCursor = 0
    this.seenIds.clear()
    this.seenEventIds.clear()
    this.anomalyAttempts.clear()
    this.recovery = null
    this.dropRecoveryStage()
    this.closeConnection()
    this.stopPolling()
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
    void this.beginRecovery("R", "manual-retry", false)
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
          void this.processFrame(frame, token)
        },
        onError: (error) => {
          void this.enqueueIngest(() => this.handleTransportFailure(error, token))
        },
        onEof: () => {
          void this.enqueueIngest(() => this.handleTransportFailure(new Error("SSE EOF"), token))
        },
      })
    } catch (error) {
      void this.handleTransportFailure(error, token)
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
      if (this.recoveryBuffer.length >= this.maxSeenEntries * 2) {
        await this.protocolFailure("recovery-buffer-overflow", token, String(this.maxSeenEntries * 2))
        return
      }
      this.recoveryBuffer.push(event)
      // Staging this event makes duplicate SSE deliveries idempotent without
      // publishing its cursor or fingerprints before the recovery barrier.
      this.commitEvent(event)
      return
    }
    await this.applyEvent(event, token)
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

  private async applyEvent(event: ValidatedBusinessEvent, token: SyncToken): Promise<void> {
    if (!this.isCurrent(token)) return
    const plan: InvalidationPlan = classifyEvent(event)
    await this.sink.onEvent(event, plan, token)
    if (!this.isCurrent(token)) return

    if (!event.known) {
      // E is lossless and exactly once; the event remains generation-local
      // while the non-budgeted F barrier establishes the authoritative cursor.
      void this.beginRecovery("F", "unknown-event", false, [event])
      return
    }
    this.commitEvent(event)
  }

  private isValidRecoveryResult(result: RecoveryResult, token: SyncToken, expectedRevision: number): boolean {
    const boundary = result.boundary
    if (!result.noGap || !Number.isSafeInteger(result.confirmedCursor) || result.confirmedCursor < 0 || !boundary.published || boundary.revision !== expectedRevision || !tokenEquals(boundary.token, token) || !Number.isSafeInteger(boundary.highWatermark) || boundary.highWatermark < result.confirmedCursor || !Array.isArray(boundary.events)) return false
    for (const [id, fingerprint] of boundary.byId) {
      if (!Number.isSafeInteger(id) || id < 0 || id > boundary.highWatermark || typeof fingerprint !== "string") return false
    }
    for (const [eventId, fingerprint] of boundary.byEventId) {
      if (eventId.length === 0 || typeof fingerprint !== "string") return false
    }
    return true
  }

  private commitEvent(event: ValidatedBusinessEvent): void {
    const stage = this.recoveryStage
    const seenIds = stage?.seenIds ?? this.seenIds
    const seenEventIds = stage?.seenEventIds ?? this.seenEventIds
    seenIds.set(event.id, event.canonicalFingerprint)
    seenEventIds.set(event.eventId, event.canonicalFingerprint)
    this.trimSeen(seenIds)
    this.trimSeen(seenEventIds)
    if (stage !== null) stage.cursor = Math.max(stage.cursor, event.id)
    else this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, event.id)
  }

  private trimSeen<K>(map: Map<K, string>): void {
    while (map.size > this.maxSeenEntries) {
      const oldest = map.keys().next().value
      if (oldest === undefined) break
      map.delete(oldest)
    }
  }

  private markLive(token: SyncToken): void {
    if (!this.isCurrent(token) || this.recovery !== null || this.state === "circuit-open") return
    this.state = "live"
    this.stopPolling()
    this.armLiveness(token)
  }

  private armLiveness(token: SyncToken): void {
    this.clearLiveness()
    this.livenessTimer = this.clock.setTimeout(() => {
      if (!this.running || !this.isCurrent(token) || this.state === "circuit-open") return
      this.emit("stalled")
      void this.beginRecovery("R", "stalled", false)
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
    if (this.recovery !== null) this.fenceRecovery()
    // Start the barrier after all prior frames in this queue have committed,
    // then release the queue so the new epoch can stage concurrent SSE frames.
    void this.beginRecovery("R", "transport-failure", false)
  }

  private anomalyKey(): string {
    return `${this.boardId}:${this.lastConfirmedCursor}`
  }

  private async protocolFailure(reason: string, token: SyncToken, code: string): Promise<void> {
    if (!this.running || !this.isCurrent(token) || this.state === "circuit-open") return
    this.emit("protocol-anomaly", { reason, code })
    if (this.recoveryStartTimer !== null) {
      this.emit("protocol-anomaly-suppressed", { reason, code })
      return
    }
    const key = this.anomalyKey()
    const attempts = (this.anomalyAttempts.get(key) ?? 0) + 1
    this.anomalyAttempts.set(key, attempts)
    if (attempts > this.maxAnomalyAttempts) {
      if (this.recovery !== null) this.fenceRecovery()
      this.openCircuit(reason)
      return
    }
    if (this.recovery !== null) this.fenceRecovery()
    else this.fenceConnection()
    if (this.recoveryStartTimer !== null) return
    this.scheduleRecoveryStart("F", reason, true, this.recoveryBackoff(attempts))
  }

  private fenceRecovery(): void {
    this.recovery = null
    this.fenceConnection()
  }

  private fenceConnection(): void {
    this.closeConnection()
    this.connectionEpoch += 1
    this.generation += 1
    this.recoverySawLiveFrame = false
    this.dropRecoveryStage()
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
      void this.beginRecovery(mode, reason, budgeted)
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
    this.clearLiveness()
    this.clearRecoveryStartTimer()
    this.recovery = null
    this.dropRecoveryStage()
    this.emit("circuit-open", { reason })
    this.schedulePolling()
  }

  private async beginRecovery(mode: "F" | "R", reason: string, budgeted: boolean, preApplied: readonly ValidatedBusinessEvent[] = []): Promise<void> {
    if (!this.running || this.state === "circuit-open") return
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
    this.recoveryPreApplied.clear()
    for (const event of preApplied) {
      this.recoveryPreApplied.set(event.eventId, event.canonicalFingerprint)
      this.commitEvent(event)
    }
    this.recoverySawLiveFrame = false
    this.schedulePolling()
    this.emit("recovery-start", { mode, reason })

    const operation = this.performRecovery(mode, token, after, budgeted, expectedRevision)
    this.recovery = operation
    // R/F barrier and the next SSE subscription overlap. Frames from this
    // generation are staged until the boundary is published and replayed.
    this.openConnection()
    try {
      await operation
    } finally {
      if (this.recovery === operation) this.recovery = null
    }
  }

  private async performRecovery(mode: "F" | "R", token: SyncToken, after: number, budgeted: boolean, expectedRevision: number): Promise<void> {
    let countsBudget = budgeted
    let boundaryValidated = false
    try {
      const result = await this.sink.refetchObserved(mode, token, after, expectedRevision)
      if (!this.isCurrent(token)) return
      if (result.boundary.token.boardId !== token.boardId || result.boundary.token.connectionEpoch !== token.connectionEpoch || result.boundary.token.generation !== token.generation || result.boundary.revision !== expectedRevision) {
        // A response from an older publication/generation is not an anomaly:
        // discard it and issue a fresh barrier under a new token.
        this.emit("recovery-stale-result", { revision: result.boundary.revision, expectedRevision })
        this.fenceRecovery()
        await this.beginRecovery(mode, "stale-result", false)
        return
      }
      if (!this.isValidRecoveryResult(result, token, expectedRevision) || result.confirmedCursor < after) {
        countsBudget = true
        throw new Error("recovery boundary is not gap-free")
      }
      boundaryValidated = true

      const boundary = result.boundary
      this.seedBoundary(boundary)
      const replayedBoundaryEvents = await this.replayBoundaryEvents(token, boundary, after)
      const finalBoundary = await this.replayRecoveryBuffer(token, boundary, replayedBoundaryEvents)
      if (!this.isCurrent(token)) return
      this.promoteRecoveryStage(Math.max(result.confirmedCursor, finalBoundary.highWatermark))
      if (result.confirmedCursor > after) this.anomalyAttempts.delete(`${this.boardId}:${after}`)
      if (!this.isCurrent(token)) return
      this.state = this.recoverySawLiveFrame ? "live" : "connecting"
      this.recovery = null
      this.recoveryBuffer = []
      this.recoveryPreApplied.clear()
      if (this.state === "live") this.stopPolling()
      this.armLiveness(this.currentToken())
      this.emit("recovery-complete", { mode, confirmedCursor: result.confirmedCursor })
    } catch (error) {
      if (!this.isCurrent(token)) return
      if (boundaryValidated) countsBudget = true
      this.emit("recovery-failure", { message: error instanceof Error ? error.message : String(error) })
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
      if (!this.isCurrent(token)) return replayed
      if (event.boardId !== this.boardId || !Number.isSafeInteger(event.id) || event.id < 0 || event.id > boundary.highWatermark) throw new Error("recovery boundary event is out of scope")
      if (event.id < previousId) throw new Error("recovery boundary events are not ordered")
      const idFingerprint = boundary.byId.get(event.id)
      const eventFingerprint = boundary.byEventId.get(event.eventId)
      if (idFingerprint !== event.canonicalFingerprint || eventFingerprint !== event.canonicalFingerprint) throw new Error("recovery boundary event fingerprint mismatch")
      previousId = event.id
      if (event.id > after) eligible.push(event)
    }
    for (const event of eligible) {
      if (!this.isCurrent(token)) return replayed
      replayed.add(event.eventId)
      if (this.recoveryPreApplied.get(event.eventId) === event.canonicalFingerprint) continue
      await this.sink.onEvent(event, classifyEvent(event), token)
      if (!this.isCurrent(token)) return replayed
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
      let previousBufferedId = -1
      // Validate the complete batch before emitting any projection effect.
      for (const event of buffered) {
        if (event.boardId !== this.boardId || !Number.isSafeInteger(event.id) || event.id < 0 || event.id < previousBufferedId) throw new Error("recovery buffer event is out of scope")
        previousBufferedId = event.id
        if (event.id <= boundary.highWatermark) {
          const idFingerprint = boundary.byId.get(event.id)
          const eventFingerprint = boundary.byEventId.get(event.eventId)
          if (idFingerprint !== event.canonicalFingerprint || eventFingerprint !== event.canonicalFingerprint) throw new Error("recovery boundary overlap conflict")
        }
      }
      for (const event of buffered) {
        if (!this.isCurrent(token)) return boundary
        if (event.id <= boundary.highWatermark) {
          if (this.recoveryPreApplied.get(event.eventId) === event.canonicalFingerprint || replayedBoundaryEvents.has(event.eventId)) continue
          await this.sink.onEvent(event, classifyEvent(event), token)
          if (!event.known) {
            await this.restartUnknownRecovery(token, event)
            return boundary
          }
          continue
        }
        if (this.recoveryPreApplied.get(event.eventId) === event.canonicalFingerprint) continue
        await this.sink.onEvent(event, classifyEvent(event), token)
        if (!this.isCurrent(token)) return boundary
        if (!event.known) {
          await this.restartUnknownRecovery(token, event)
          return boundary
        }
      }
    }
    return boundary
  }

  private async restartUnknownRecovery(token: SyncToken, event: ValidatedBusinessEvent): Promise<void> {
    if (!this.isCurrent(token)) return
    this.fenceRecovery()
    await this.beginRecovery("F", "unknown-event-during-recovery", false, [event])
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
    this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, cursor, stage.cursor)
    this.recoveryStage = null
  }

  private dropRecoveryStage(): void {
    this.recoveryStage = null
    this.recoveryBuffer = []
    this.recoveryPreApplied.clear()
  }

  private schedulePolling(): void {
    if (this.pollingTimer !== null || !this.running) return
    const schedule = (): void => {
      if (!this.running || this.state === "live") return
      this.pollingTimer = this.clock.setTimeout(() => {
        this.pollingTimer = null
        void this.pollOnce().finally(() => schedule())
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
  private applyPollingBoundary(page: PollEventsPage, token: SyncToken): Promise<boolean> {
    if (this.pollingBoundaryCommit !== null) return this.pollingBoundaryCommit
    const operation = this.commitPollingBoundary(page, token)
    const tracked = operation.finally(() => {
      if (this.pollingBoundaryCommit === tracked) this.pollingBoundaryCommit = null
    })
    this.pollingBoundaryCommit = tracked
    return tracked
  }

  private async commitPollingBoundary(page: PollEventsPage, token: SyncToken): Promise<boolean> {
    const boundary = page.boundary
    if (boundary === undefined) {
      this.pollProtocolFailure("poll-boundary-missing", "published-boundary-required")
      return false
    }
    const confirmedCursor = page.confirmedCursor ?? boundary.highWatermark
    if (!Number.isSafeInteger(confirmedCursor) || confirmedCursor < 0 || !boundary.published || boundary.revision <= 0 || boundary.highWatermark < confirmedCursor || !tokenEquals(boundary.token, token) || !Array.isArray(boundary.events)) {
      this.pollProtocolFailure("poll-boundary-invalid", "incompatible-boundary")
      return false
    }
    const stagedIds = new Map(this.seenIds)
    const stagedEventIds = new Map(this.seenEventIds)
    let previousId = -1
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
        if (event.boardId !== this.boardId || !Number.isSafeInteger(event.id) || event.id < previousId || event.id < 0 || event.id > boundary.highWatermark) throw new Error("boundary-event-out-of-scope")
        const idFingerprint = boundary.byId.get(event.id)
        const eventFingerprint = boundary.byEventId.get(event.eventId)
        if (idFingerprint !== event.canonicalFingerprint || eventFingerprint !== event.canonicalFingerprint) throw new Error("boundary-event-fingerprint-mismatch")
        previousId = event.id
        if (event.id <= this.lastConfirmedCursor) continue
        const existing = stagedIds.get(event.id)
        if (existing !== undefined && existing !== event.canonicalFingerprint) throw new Error("boundary-id-conflict")
        const existingEvent = stagedEventIds.get(event.eventId)
        if (existingEvent !== undefined && existingEvent !== event.canonicalFingerprint) throw new Error("boundary-event-conflict")
        eligible.push(event)
      }
      for (const event of eligible) {
        await this.sink.onEvent(event, classifyEvent(event), token)
      }
    } catch (error) {
      this.pollProtocolFailure("poll-boundary-invalid", error instanceof Error ? error.message : String(error))
      return false
    }
    const staleAnomalyKey = this.anomalyKey()
    this.trimSeen(stagedIds)
    this.trimSeen(stagedEventIds)
    this.seenIds.clear()
    for (const [id, fingerprint] of stagedIds) this.seenIds.set(id, fingerprint)
    this.seenEventIds.clear()
    for (const [eventId, fingerprint] of stagedEventIds) this.seenEventIds.set(eventId, fingerprint)
    this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, confirmedCursor, boundary.highWatermark)
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
        page = await this.sink.pollEvents({ board: this.boardId, after: queryAfter, limit: 100 }, abort.signal)
        if (!this.isCurrent(token)) return
        if (!Number.isSafeInteger(page.nextAfter) || page.nextAfter < 0 || page.nextAfter < queryAfter || (page.hasMore && page.nextAfter === queryAfter)) {
          this.pollProtocolFailure("poll-invalid-cursor", String(page.nextAfter))
          return
        }
        if (!page.noGap) {
          if (this.state === "circuit-open") this.pollProtocolFailure("poll-gap", String(page.nextAfter))
          else await this.protocolFailure("poll-gap", token, String(page.nextAfter))
          return
        }
        if (this.state === "circuit-open" && !(await this.applyPollingBoundary(page, token))) return
        for (const rawEvent of page.events) {
          const parsed = this.adapter.parsePollingEnvelope(rawEvent)
          if (parsed.status === "invalid") {
            if (this.state === "circuit-open") this.pollProtocolFailure("poll-invalid-envelope", parsed.code)
            else await this.protocolFailure("poll-invalid-envelope", token, parsed.code)
            return
          }
          await this.enqueueIngest(() => this.ingestEnvelope(parsed.envelope, token, "poll"))
          if (!this.isCurrent(token)) return
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
