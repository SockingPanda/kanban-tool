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
  private pollingAbort: AbortController | null = null
  private recovery: Promise<void> | null = null
  private recoveryBuffer: ValidatedBusinessEvent[] = []
  private recoverySawLiveFrame = false
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
    this.recovery = null
    this.recoveryBuffer = []
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
    this.recoveryBuffer = []
    this.closeConnection()
    this.stopPolling()
    this.clearLiveness()
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
    return this.ingestFrame(frame, token)
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
          void this.ingestFrame(frame, token)
        },
        onError: (error) => {
          void this.handleTransportFailure(error, token)
        },
        onEof: () => {
          void this.handleTransportFailure(new Error("SSE EOF"), token)
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

    if (frame.eventName === "kb-heartbeat") {
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
      if (event.id > this.lastConfirmedCursor) this.lastConfirmedCursor = event.id
      return
    }

    if (this.recovery !== null) {
      if (this.recoveryBuffer.length < this.maxSeenEntries * 2) this.recoveryBuffer.push(event)
      return
    }
    await this.applyEvent(event, token)
  }

  private dedupeDecision(event: ValidatedBusinessEvent): "new" | "duplicate" | "conflict" | "stale" {
    const idFingerprint = this.seenIds.get(event.id)
    const eventFingerprint = this.seenEventIds.get(event.eventId)
    if (idFingerprint === undefined && eventFingerprint === undefined) {
      return event.id <= this.lastConfirmedCursor ? "stale" : "new"
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
      const recovery = await this.sink.refetchObserved("F", token, this.lastConfirmedCursor)
      if (!this.isCurrent(token)) return
      if (!this.isValidRecoveryResult(recovery, token) || recovery.confirmedCursor < event.id) {
        await this.protocolFailure("unknown-refetch-gap", token, event.eventId)
        return
      }
      this.commitEvent(event)
      this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, recovery.confirmedCursor)
      return
    }
    this.commitEvent(event)
  }

  private isValidRecoveryResult(result: RecoveryResult, token: SyncToken): boolean {
    return result.noGap && result.boundary.published && result.boundary.token.boardId === token.boardId && result.boundary.token.connectionEpoch === token.connectionEpoch && result.boundary.token.generation === token.generation && result.boundary.highWatermark >= result.confirmedCursor
  }

  private commitEvent(event: ValidatedBusinessEvent): void {
    this.seenIds.set(event.id, event.canonicalFingerprint)
    this.seenEventIds.set(event.eventId, event.canonicalFingerprint)
    while (this.seenIds.size > this.maxSeenEntries) {
      const oldest = this.seenIds.keys().next().value
      if (oldest === undefined) break
      this.seenIds.delete(oldest)
    }
    while (this.seenEventIds.size > this.maxSeenEntries) {
      const oldest = this.seenEventIds.keys().next().value
      if (oldest === undefined) break
      this.seenEventIds.delete(oldest)
    }
    this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, event.id)
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
    await this.beginRecovery("R", "transport-failure", false)
  }

  private anomalyKey(): string {
    return `${this.boardId}:${this.lastConfirmedCursor}`
  }

  private async protocolFailure(reason: string, token: SyncToken, code: string): Promise<void> {
    if (!this.running || !this.isCurrent(token) || this.state === "circuit-open") return
    this.emit("protocol-anomaly", { reason, code })
    const key = this.anomalyKey()
    const attempts = (this.anomalyAttempts.get(key) ?? 0) + 1
    this.anomalyAttempts.set(key, attempts)
    if (attempts > this.maxAnomalyAttempts) {
      if (this.recovery !== null) this.fenceRecovery()
      this.openCircuit(reason)
      return
    }
    if (this.recovery !== null) this.fenceRecovery()
    await this.beginRecovery("F", reason, true)
  }

  private fenceRecovery(): void {
    this.recovery = null
    this.closeConnection()
    this.connectionEpoch += 1
    this.generation += 1
    this.recoveryBuffer = []
    this.recoverySawLiveFrame = false
  }

  private openCircuit(reason: string): void {
    this.state = "circuit-open"
    this.closeConnection()
    this.clearLiveness()
    this.emit("circuit-open", { reason })
    this.schedulePolling()
  }

  private async beginRecovery(mode: "F" | "R", reason: string, budgeted: boolean): Promise<void> {
    if (!this.running || this.state === "circuit-open") return
    if (this.recovery !== null) return this.recovery

    this.closeConnection()
    this.clearLiveness()
    this.connectionEpoch += 1
    this.generation += 1
    const token = this.currentToken()
    const after = this.lastConfirmedCursor
    this.state = "recovering"
    this.recoveryBuffer = []
    this.recoverySawLiveFrame = false
    this.schedulePolling()
    this.emit("recovery-start", { mode, reason })

    const operation = this.performRecovery(mode, token, after, budgeted)
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

  private async performRecovery(mode: "F" | "R", token: SyncToken, after: number, budgeted: boolean): Promise<void> {
    try {
      const result = await this.sink.refetchObserved(mode, token, after)
      if (!this.isCurrent(token)) return
      if (!this.isValidRecoveryResult(result, token) || result.confirmedCursor < after) throw new Error("recovery boundary is not gap-free")

      const boundary = result.boundary
      this.seedBoundary(boundary)
      this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, after)
      await this.replayRecoveryBuffer(token, boundary)
      if (!this.isCurrent(token)) return
      this.lastConfirmedCursor = Math.max(this.lastConfirmedCursor, result.confirmedCursor, boundary.highWatermark)
      if (result.confirmedCursor > after) this.anomalyAttempts.delete(`${this.boardId}:${after}`)
      if (!this.isCurrent(token)) return
      this.state = this.recoverySawLiveFrame ? "live" : "connecting"
      this.recovery = null
      if (this.state === "live") this.stopPolling()
      this.armLiveness(this.currentToken())
      this.emit("recovery-complete", { mode, confirmedCursor: result.confirmedCursor })
    } catch (error) {
      if (!this.isCurrent(token)) return
      this.emit("recovery-failure", { message: error instanceof Error ? error.message : String(error) })
      let attempts = 0
      if (budgeted) {
        const key = this.anomalyKey()
        attempts = (this.anomalyAttempts.get(key) ?? 0) + 1
        this.anomalyAttempts.set(key, attempts)
        if (attempts > this.maxAnomalyAttempts) {
          this.openCircuit("recovery-failure")
          return
        }
      }
      this.closeConnection()
      this.connectionEpoch += 1
      this.generation += 1
      this.recoveryBuffer = []
      this.recovery = null
      this.state = "polling"
      const delay = budgeted ? [250, 1_000, 5_000][Math.min(Math.max(attempts, 1) - 1, 2)] ?? 5_000 : this.reconnectDelayMs
      this.clock.setTimeout(() => {
        if (!this.running || this.state === "circuit-open") return
        this.state = "connecting"
        this.openConnection()
      }, delay)
    }
  }

  private seedBoundary(boundary: RecoveryBoundary): void {
    for (const [id, fingerprint] of boundary.byId) {
      const current = this.seenIds.get(id)
      if (current !== undefined && current !== fingerprint) throw new Error("recovery boundary id fingerprint conflict")
      this.seenIds.set(id, fingerprint)
    }
    for (const [eventId, fingerprint] of boundary.byEventId) {
      const current = this.seenEventIds.get(eventId)
      if (current !== undefined && current !== fingerprint) throw new Error("recovery boundary event fingerprint conflict")
      this.seenEventIds.set(eventId, fingerprint)
    }
    while (this.seenIds.size > this.maxSeenEntries) {
      const oldest = this.seenIds.keys().next().value
      if (oldest === undefined) break
      this.seenIds.delete(oldest)
    }
    while (this.seenEventIds.size > this.maxSeenEntries) {
      const oldest = this.seenEventIds.keys().next().value
      if (oldest === undefined) break
      this.seenEventIds.delete(oldest)
    }
  }

  private async replayRecoveryBuffer(token: SyncToken, initialBoundary: RecoveryBoundary): Promise<void> {
    let boundary = initialBoundary
    for (;;) {
      // Let the concurrently opened SSE/poll transport deliver frames that
      // arrived immediately after the boundary response settled.
      await Promise.resolve()
      if (this.recoveryBuffer.length === 0) break
      const buffered = this.recoveryBuffer.splice(0).sort((left, right) => left.id - right.id)
      for (const event of buffered) {
        if (!this.isCurrent(token)) return
        if (event.id <= boundary.highWatermark) {
          const idFingerprint = boundary.byId.get(event.id)
          const eventFingerprint = boundary.byEventId.get(event.eventId)
          if (idFingerprint !== event.canonicalFingerprint || eventFingerprint !== event.canonicalFingerprint) {
            throw new Error("recovery boundary overlap conflict")
          }
          continue
        }
        const decision = this.dedupeDecision(event)
        if (decision === "conflict" || decision === "stale") throw new Error("recovery buffer conflict")
        if (decision === "duplicate") continue
        const plan = classifyEvent(event)
        await this.sink.onEvent(event, plan, token)
        if (!this.isCurrent(token)) return
        if (!event.known) {
          const unknownRecovery = await this.sink.refetchObserved("F", token, this.lastConfirmedCursor)
          if (!this.isValidRecoveryResult(unknownRecovery, token)) throw new Error("unknown recovery boundary is not gap-free")
          boundary = unknownRecovery.boundary
          this.seedBoundary(boundary)
        }
        this.commitEvent(event)
      }
    }
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
        if (!page.noGap) {
          await this.protocolFailure("poll-gap", token, String(page.nextAfter))
          return
        }
        for (const rawEvent of page.events) {
          const parsed = this.adapter.parsePollingEnvelope(rawEvent)
          if (parsed.status === "invalid") {
            await this.protocolFailure("poll-invalid-envelope", token, parsed.code)
            return
          }
          await this.ingestEnvelope(parsed.envelope, token, "poll")
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
