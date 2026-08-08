import { describe, expect, test, vi, type Mock } from "vitest"

import type {
  BusinessValidationResult,
  ControlValidationResult,
  EnvelopeParseResult,
  RawSseFrame,
  StreamContractAdapter,
  SseTransport,
  InvalidationPlan,
  SyncToken,
  SyncClock,
  ValidatedBusinessEvent,
  RecoveryResult,
  PollEventsPage,
} from "./contracts"
import { WebSyncController } from "./web-sync-controller"
import type { ApiListEventsQueryContract } from "../api/generated/contracts/api-list-events-query"

function businessEvent(overrides: Partial<ValidatedBusinessEvent> = {}): ValidatedBusinessEvent {
  return {
    id: 1,
    eventId: "e-1",
    boardId: "board-a",
    taskId: "task-a",
    runId: null,
    kind: "task.updated",
    createdAt: 1_700_000_000,
    raw: { id: 1 },
    scope: { taskId: "task-a" },
    canonicalFingerprint: "fp-1",
    known: true,
    ...overrides,
  }
}

function adapterFor(event: ValidatedBusinessEvent): StreamContractAdapter {
  return {
    parseEnvelope: (frame: RawSseFrame): EnvelopeParseResult => ({
      status: "valid",
      envelope: {
        id: Number(frame.id),
        eventId: `e-${frame.id}`,
        boardId: "board-a",
        taskId: event.taskId,
        runId: event.runId,
        kind: frame.eventName,
        createdAt: event.createdAt,
        raw: event.raw,
      },
    }),
    parsePollingEnvelope: (): EnvelopeParseResult => ({
      status: "valid",
      envelope: {
        id: event.id,
        eventId: event.eventId,
        boardId: event.boardId,
        taskId: event.taskId,
        runId: event.runId,
        kind: event.kind,
        createdAt: event.createdAt,
        raw: event.raw,
      },
    }),
    validateBusiness: (envelope): BusinessValidationResult => {
      const currentEvent = envelope.id === event.id
        ? event
        : { ...event, id: envelope.id, eventId: envelope.eventId, kind: envelope.kind, raw: envelope.raw, canonicalFingerprint: `fp-${envelope.id}` }
      return currentEvent.known ? { status: "known", event: currentEvent } : { status: "unknown", event: currentEvent }
    },
    isControlFrame: (frame): boolean => frame.eventName === "kb-heartbeat",
    validateControl: (): ControlValidationResult => ({ status: "valid", control: { raw: {} } }),
  }
}

function fakeClock(): { clock: SyncClock; advance: (ms: number) => void } {
  let now = 0
  let nextId = 0
  const timers = new Map<number, { at: number; callback: () => void }>()
  const clock: SyncClock = {
    now: () => now,
    setTimeout: (callback, delay) => {
      const id = nextId
      nextId += 1
      timers.set(id, { at: now + delay, callback })
      return id
    },
    clearTimeout: (id) => {
      timers.delete(id)
    },
  }
  return {
    clock,
    advance(ms) {
      now += ms
      for (const [id, timer] of [...timers.entries()]) {
        if (timer.at > now) continue
        timers.delete(id)
        timer.callback()
      }
    },
  }
}

type EventHandler = (event: ValidatedBusinessEvent, plan: InvalidationPlan, token: SyncToken) => Promise<void>
type RecoveryHandler = (mode: "F" | "R", token: SyncToken, after: number, expectedRevision: number, signal: AbortSignal) => Promise<RecoveryResult>
type PollHandler = (query: ApiListEventsQueryContract, signal: AbortSignal) => Promise<PollEventsPage>

interface TestSink {
  readonly events: ValidatedBusinessEvent[]
  readonly recoveries: string[]
  readonly onEvent: Mock<EventHandler>
  readonly refetchObserved: Mock<RecoveryHandler>
  readonly pollEvents: Mock<PollHandler>
}

function sink(): TestSink {
  const events: ValidatedBusinessEvent[] = []
  const recoveries: string[] = []
  const state = {
    events,
    recoveries,
  }
  return {
    ...state,
    onEvent: vi.fn<EventHandler>(async (event) => {
      state.events.push(event)
    }),
    refetchObserved: vi.fn<RecoveryHandler>(async (mode, token, _after, expectedRevision) => {
      state.recoveries.push(mode)
      return {
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, "fp-1"]]),
          byEventId: new Map([["e-1", "fp-1"]]),
          events: [businessEvent()],
          token,
          revision: expectedRevision,
          published: true,
        },
      }
    }),
    pollEvents: vi.fn<PollHandler>(async () => ({ events: [], nextAfter: 0, hasMore: false, noGap: true })),
  }
}

describe("WebSyncController", () => {
  test("keeps production buffer/budget/timing invariants unless an explicit test seam is enabled", () => {
    const base = {
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport: vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() })),
      adapter: adapterFor(businessEvent()),
      sink: sink(),
    }
    expect(() => new WebSyncController({ ...base, maxSeenEntries: 1 })).toThrow(/between 2048 and 8192/)
    expect(() => new WebSyncController({ ...base, maxAnomalyAttempts: 4 })).toThrow(/at most 3/)
    expect(() => new WebSyncController({ ...base, livenessTimeoutMs: 120_001 })).toThrow(/testOnlyLimits/)
    expect(() => new WebSyncController({ ...base, livenessTimeoutMs: 1 })).toThrow(/testOnlyLimits/)
    expect(() => new WebSyncController({ ...base, pollingIntervalMs: 1 })).toThrow(/testOnlyLimits/)
    expect(() => new WebSyncController({ ...base, maxBoundaryBytes: 128 })).toThrow(/testOnlyLimits/)
    expect(() => new WebSyncController({ ...base, maxSeenEntries: 1, maxAnomalyAttempts: 1, testOnlyLimits: true })).not.toThrow()
  })

  test("runs epoch/control/envelope/board/known/dedupe before sink and cursor commit", async () => {
    const observed: RawSseFrame[] = []
    const querySink = sink()
    const transport = vi.fn<SseTransport>(({ onFrame }) => {
      observed.push({ eventName: "task.updated", id: "1", data: "{}" })
      const frame = observed[0]
      if (frame !== undefined) onFrame(frame)
      return { closed: false, close: vi.fn() }
    })
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
    })

    controller.start()
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(querySink.onEvent.mock.calls[0]?.[1].eventKind).toBe("task.updated")
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)
    expect(controller.snapshot().seenIds).toBe(1)
  })

  test("records typed event application latency only after sink success", async () => {
    const querySink = sink()
    const telemetry: { type: string; details?: Readonly<Record<string, unknown>> }[] = []
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const { clock } = fakeClock()
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent({ createdAt: 1_700_000_000 })),
      sink: querySink,
      clock,
      telemetry: { record: (entry) => telemetry.push({ type: entry.type, details: entry.details }) },
    })
    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await vi.waitFor(() => expect(telemetry.some((entry) => entry.type === "event-applied")).toBe(true))
    const applied = telemetry.find((entry) => entry.type === "event-applied")
    expect(applied?.details).toMatchObject({ eventId: "e-1", source: "sse", createdAt: 1_700_000_000 })
    expect(applied?.details?.latencyMs).toBeTypeOf("number")
  })

  test("serializes frames and EOF so transport recovery starts after the committed cursor", async () => {
    const querySink = sink()
    let releaseFirst: () => void = () => undefined
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve
    })
    querySink.onEvent.mockImplementation(async (event) => {
      if (event.id === 1) await firstGate
    })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
    })

    controller.start()
    const request = transport.mock.calls[0]?.[0]
    request?.onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    request?.onFrame({ eventName: "task.updated", id: "2", data: "{}" })
    request?.onEof()
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(querySink.refetchObserved).not.toHaveBeenCalled()
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)

    releaseFirst()
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledTimes(2))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("R", expect.anything(), 2, expect.any(Number), expect.any(AbortSignal)))
    expect(controller.snapshot().lastConfirmedCursor).toBe(2)
  })

  test("routes named heartbeat to liveness only", async () => {
    const querySink = sink()
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const { clock, advance } = fakeClock()
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
      livenessTimeoutMs: 35,
      testOnlyLimits: true,
    })

    controller.start()
    const frame = transport.mock.calls[0]?.[0]
    frame?.onFrame({ eventName: "kb-heartbeat", id: null, data: "{}" })
    await Promise.resolve()
    advance(34)
    await Promise.resolve()
    expect(querySink.refetchObserved).not.toHaveBeenCalled()
    advance(1)
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("R", expect.anything(), 0, expect.any(Number), expect.any(AbortSignal)))
    expect(querySink.onEvent).not.toHaveBeenCalled()
  })

  test("unknown event applies lossless E then waits for F barrier", async () => {
    const querySink = sink()
    const unknown = businessEvent({ kind: "task.attachment.created", known: false })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(unknown),
      sink: querySink,
    })

    controller.start()
    const frame = transport.mock.calls[0]?.[0]
    frame?.onFrame({ eventName: unknown.kind, id: String(unknown.id), data: "{}" })
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("F", expect.anything(), 0, expect.any(Number), expect.any(AbortSignal)))
    expect(querySink.onEvent).toHaveBeenCalledOnce()
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)
  })

  test("turns a sink rejection into a conservative F without consuming anomaly budget", async () => {
    const querySink = sink()
    const failures: string[] = []
    querySink.onEvent.mockRejectedValueOnce(new Error("projection failed"))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      telemetry: { record: (entry) => { if (entry.type === "sink-effect-failure") failures.push(entry.type) } },
    })
    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("F", expect.anything(), 0, expect.any(Number), expect.any(AbortSignal)))
    await vi.waitFor(() => expect(controller.snapshot().lastConfirmedCursor).toBe(1))
    expect(failures).toEqual(["sink-effect-failure"])
    expect(querySink.onEvent).toHaveBeenCalledTimes(2)
  })

  test("does not repeat unknown E while its F barrier is in flight", async () => {
    const querySink = sink()
    const unknown = businessEvent({ kind: "task.attachment.created", known: false })
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    querySink.refetchObserved.mockImplementationOnce((_mode, token) => new Promise<RecoveryResult>((resolve) => {
      resolveBarrier = () => resolve({
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, "fp-1"]]),
          byEventId: new Map([["e-1", "fp-1"]]),
          events: [unknown],
          token,
          revision: 1,
          published: true,
        },
      })
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(unknown),
      sink: querySink,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: unknown.kind, id: String(unknown.id), data: "{}" })
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("F", expect.anything(), 0, expect.any(Number), expect.any(AbortSignal)))
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
    expect(controller.snapshot().seenIds).toBe(0)
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    transport.mock.calls[1]?.[0].onFrame({ eventName: unknown.kind, id: String(unknown.id), data: "{}" })
    await Promise.resolve()
    expect(querySink.onEvent).toHaveBeenCalledOnce()
    expect(querySink.refetchObserved).toHaveBeenCalledOnce()

    resolveBarrier({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, "fp-1"]]),
        byEventId: new Map([["e-1", "fp-1"]]),
        events: [unknown],
        token: {
          boardId: "board-a",
          connectionEpoch: controller.snapshot().connectionEpoch,
          generation: controller.snapshot().generation,
        },
        revision: 1,
        published: true,
      },
    })
    await vi.waitFor(() => expect(controller.snapshot().lastConfirmedCursor).toBe(1))
    expect(querySink.onEvent).toHaveBeenCalledOnce()
  })

  test("fences old epoch frames after board switch", async () => {
    const querySink = sink()
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
    })

    controller.start()
    const oldFrame = transport.mock.calls[0]?.[0]
    controller.switchBoard("board-b")
    oldFrame?.onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await Promise.resolve()
    expect(querySink.onEvent).not.toHaveBeenCalled()
  })

  test("fences a sink invocation queued behind a stop or board switch microtask", async () => {
    const frame = { eventName: "task.updated", id: "1", data: "{}" }
    const firstSink = sink()
    const firstTransport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const stopped = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport: firstTransport,
      adapter: adapterFor(businessEvent()),
      sink: firstSink,
    })
    stopped.start()
    const stoppedProcessing = stopped.processFrame(frame)
    await Promise.resolve()
    stopped.stop()
    await stoppedProcessing
    expect(firstSink.onEvent).not.toHaveBeenCalled()

    const switchedSink = sink()
    const switchedTransport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const switched = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport: switchedTransport,
      adapter: adapterFor(businessEvent()),
      sink: switchedSink,
    })
    switched.start()
    const switchedProcessing = switched.processFrame(frame)
    await Promise.resolve()
    switched.switchBoard("board-b")
    await switchedProcessing
    expect(switchedSink.onEvent).not.toHaveBeenCalled()
    switched.stop()
  })

  test("starts the next SSE epoch before R barrier settles and replays a buffered frame", async () => {
    const querySink = sink()
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    querySink.refetchObserved.mockImplementationOnce((_mode, token) => new Promise<RecoveryResult>((resolve) => {
      resolveBarrier = () => resolve({
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, "fp-1"]]),
          byEventId: new Map([["e-1", "fp-1"]]),
          events: [businessEvent()],
          token,
          revision: 1,
          published: true,
        },
      })
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    transport.mock.calls[1]?.[0].onFrame({ eventName: "task.updated", id: "2", data: "{}" })
    await Promise.resolve()
    expect(querySink.onEvent).not.toHaveBeenCalled()
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
    expect(controller.snapshot().seenIds).toBe(0)

    resolveBarrier({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, "fp-1"]]),
        byEventId: new Map([["e-1", "fp-1"]]),
        events: [businessEvent()],
        token: {
          boardId: "board-a",
          connectionEpoch: controller.snapshot().connectionEpoch,
          generation: controller.snapshot().generation,
        },
        revision: 1,
        published: true,
      },
    })
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledTimes(2))
    expect(controller.snapshot().lastConfirmedCursor).toBe(2)
  })

  test("fences only the failed overlapping connection while preserving the in-flight barrier", async () => {
    const querySink = sink()
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    querySink.refetchObserved.mockImplementationOnce((_mode, token) => new Promise<RecoveryResult>((resolve) => {
      resolveBarrier = () => resolve({
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, "fp-1"]]),
          byEventId: new Map([["e-1", "fp-1"]]),
          events: [businessEvent()],
          token,
          revision: 1,
          published: true,
        },
      })
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
    })
    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    transport.mock.calls[1]?.[0].onFrame({ eventName: "task.updated", id: "2", data: "{}" })
    transport.mock.calls[1]?.[0].onEof()
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(3))
    transport.mock.calls[2]?.[0].onFrame({ eventName: "task.updated", id: "3", data: "{}" })
    resolveBarrier({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, "fp-1"]]),
        byEventId: new Map([["e-1", "fp-1"]]),
        events: [businessEvent()],
        token: {
          boardId: "board-a",
          connectionEpoch: controller.snapshot().connectionEpoch - 1,
          generation: controller.snapshot().generation,
        },
        revision: 1,
        published: true,
      },
    })
    await vi.waitFor(() => expect(controller.snapshot().lastConfirmedCursor).toBe(3))
    expect(querySink.refetchObserved).toHaveBeenCalledOnce()
    expect(querySink.onEvent.mock.calls.map((call) => call[0].id)).toEqual([1, 2, 3])
  })

  test("aggregates active-recovery protocol anomalies by cursor with bounded epoch backoff", async () => {
    const querySink = sink()
    querySink.refetchObserved.mockImplementation(async () => new Promise<RecoveryResult>(() => undefined))
    const { clock, advance } = fakeClock()
    const anomalies: string[] = []
    const adapter: StreamContractAdapter = {
      ...adapterFor(businessEvent()),
      validateControl: () => ({ status: "invalid", code: "malformed-control" }),
    }
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter,
      sink: querySink,
      clock,
      telemetry: { record: (entry) => { if (entry.type === "protocol-anomaly") anomalies.push(String(entry.details?.reason)) } },
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))

    const malformed = { eventName: "kb-heartbeat", id: null, data: "{}" }
    transport.mock.calls[1]?.[0].onFrame(malformed)
    await vi.waitFor(() => expect(anomalies).toHaveLength(1))
    advance(250)
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(3))
    transport.mock.calls[2]?.[0].onFrame(malformed)
    await vi.waitFor(() => expect(anomalies).toHaveLength(2))
    advance(1_000)
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(4))
    transport.mock.calls[3]?.[0].onFrame(malformed)
    await vi.waitFor(() => expect(anomalies).toHaveLength(3))
    await vi.waitFor(() => expect(controller.snapshot().circuitOpen).toBe(true))

    expect(querySink.refetchObserved).toHaveBeenCalledOnce()
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
    controller.stop()
  })

  test("resets recovery liveness on overlapping heartbeat and rotates only the connection after timeout", async () => {
    const querySink = sink()
    querySink.refetchObserved.mockImplementation(async () => new Promise<RecoveryResult>(() => undefined))
    const { clock, advance } = fakeClock()
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
      livenessTimeoutMs: 35,
      testOnlyLimits: true,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    advance(34)
    transport.mock.calls[1]?.[0].onFrame({ eventName: "kb-heartbeat", id: null, data: "{}" })
    await Promise.resolve()
    advance(1)
    await Promise.resolve()
    expect(querySink.refetchObserved).toHaveBeenCalledOnce()
    expect(transport).toHaveBeenCalledTimes(2)

    advance(34)
    await Promise.resolve()
    expect(transport).toHaveBeenCalledTimes(2)
    advance(1)
    await Promise.resolve()
    expect(querySink.refetchObserved).toHaveBeenCalledOnce()
    advance(248)
    expect(transport).toHaveBeenCalledTimes(2)
    advance(1)
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(3))
    expect(querySink.refetchObserved).toHaveBeenCalledOnce()
    controller.stop()
  })

  test("reopens a current connection when the recovery barrier wins after transport loss", async () => {
    const querySink = sink()
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => new Promise<RecoveryResult>((resolve) => {
      resolveBarrier = () => resolve({
        confirmedCursor: 0,
        noGap: true,
        boundary: { highWatermark: 0, byId: new Map(), byEventId: new Map(), events: [], token, revision: expectedRevision, published: true },
      })
    }))
    const { clock, advance } = fakeClock()
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    const closedConnection = transport.mock.results[1]?.value as { closed: boolean } | undefined
    if (closedConnection !== undefined) closedConnection.closed = true
    advance(249)
    expect(transport).toHaveBeenCalledTimes(2)
    resolveBarrier({
      confirmedCursor: 0,
      noGap: true,
      boundary: { highWatermark: 0, byId: new Map(), byEventId: new Map(), events: [], token: querySink.refetchObserved.mock.calls[0]?.[1] ?? controller.snapshot(), revision: querySink.refetchObserved.mock.calls[0]?.[3] ?? 1, published: true },
    })
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(3))
    expect(controller.snapshot().state).toBe("connecting")
    expect(controller.snapshot().circuitOpen).toBe(false)
    controller.stop()
  })

  test("bounds repeated active-recovery transport factory throws without protocol budget", async () => {
    const querySink = sink()
    querySink.refetchObserved.mockImplementation(async () => new Promise<RecoveryResult>(() => undefined))
    const { clock, advance } = fakeClock()
    const failures: string[] = []
    const transport = vi.fn<SseTransport>(() => {
      if (transport.mock.calls.length > 1) throw new Error("transport factory failed")
      return { closed: false, close: vi.fn() }
    })
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
      telemetry: { record: (entry) => { if (entry.type === "protocol-anomaly") failures.push(entry.type) } },
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    advance(249)
    expect(transport).toHaveBeenCalledTimes(2)
    advance(1)
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(3))
    await Promise.resolve()
    advance(249)
    expect(transport).toHaveBeenCalledTimes(3)
    advance(1)
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(4))
    expect(querySink.refetchObserved).toHaveBeenCalledOnce()
    expect(failures).toEqual([])
    controller.stop()
  })

  test("replays accepted boundary events before publishing its cursor", async () => {
    const querySink = sink()
    const accepted = businessEvent({ id: 1, eventId: "e-1", canonicalFingerprint: "fp-1" })
    querySink.refetchObserved.mockImplementationOnce((_mode, token) => Promise.resolve({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, accepted.canonicalFingerprint]]),
        byEventId: new Map([[accepted.eventId, accepted.canonicalFingerprint]]),
        events: [accepted],
        token,
        revision: 1,
        published: true,
      },
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(accepted),
      sink: querySink,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(querySink.onEvent.mock.calls[0]?.[0]).toEqual(accepted)
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)
  })

  test("allows only already-seen <=C0 boundary map entries to omit events", async () => {
    const querySink = sink()
    const failures: string[] = []
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      telemetry: { record: (entry) => { if (entry.type === "recovery-failure") failures.push(entry.type) } },
    })
    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await vi.waitFor(() => expect(controller.snapshot().lastConfirmedCursor).toBe(1))
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => Promise.resolve({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, "fp-1"]]),
        byEventId: new Map([["e-1", "fp-1"]]),
        events: [],
        token,
        revision: expectedRevision,
        published: true,
      },
    }))
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("connecting"))
    expect(failures).toEqual([])
  })

  test("rejects exact-pair evidence from a failed generation and clears it on board switch", async () => {
    const querySink = sink()
    const known = businessEvent()
    const unknown = businessEvent({ id: 2, eventId: "e-2", canonicalFingerprint: "fp-2", known: false, kind: "task.attachment.created" })
    const failures: string[] = []
    const baseAdapter = adapterFor(known)
    const adapter: StreamContractAdapter = {
      ...baseAdapter,
      validateBusiness: (envelope) => envelope.id === unknown.id
        ? { status: "unknown", event: { ...unknown, raw: envelope.raw, kind: envelope.kind } }
        : baseAdapter.validateBusiness(envelope),
    }
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter,
      sink: querySink,
      telemetry: { record: (entry) => { if (entry.type === "recovery-failure") failures.push(entry.type) } },
    })

    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: known.kind, id: "1", data: "{}" })
    await vi.waitFor(() => expect(controller.snapshot().lastConfirmedCursor).toBe(1))
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => Promise.resolve({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        // Both maps contain the staged fingerprint, but they do not describe
        // the same canonical event as the already committed id=1 pair.
        byId: new Map([[1, unknown.canonicalFingerprint]]),
        byEventId: new Map([[unknown.eventId, unknown.canonicalFingerprint]]),
        events: [],
        token,
        revision: expectedRevision,
        published: true,
      },
    }))
    transport.mock.calls[0]?.[0].onFrame({ eventName: unknown.kind, id: "2", data: "{}" })
    await vi.waitFor(() => expect(failures).toHaveLength(1))
    expect(querySink.onEvent.mock.calls.map((call) => call[0].eventId)).toEqual([known.eventId, unknown.eventId])
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)

    controller.switchBoard("board-b")
    expect(controller.snapshot().seenIds).toBe(0)
    expect(controller.snapshot().seenEventIds).toBe(0)
    controller.stop()
  })

  test("accepts a 1000-event boundary with full metadata accounting and fails closed on byte overflow", async () => {
    const events = Array.from({ length: 1_000 }, (_, index) => businessEvent({
      id: index + 1,
      eventId: `e-${index + 1}`,
      canonicalFingerprint: `fp-${index + 1}`,
      raw: { id: index + 1, payload: "boundary" },
    }))
    const querySink = sink()
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => Promise.resolve({
      confirmedCursor: 1_000,
      noGap: true,
      boundary: {
        highWatermark: 1_000,
        byId: new Map(events.map((event) => [event.id, event.canonicalFingerprint])),
        byEventId: new Map(events.map((event) => [event.eventId, event.canonicalFingerprint])),
        events,
        token,
        revision: expectedRevision,
        published: true,
      },
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(events[0] ?? businessEvent()),
      sink: querySink,
    })
    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledTimes(1_000), { timeout: 10_000 })
    const appliedIds = querySink.onEvent.mock.calls.map((call) => call[0].id)
    const appliedEventIds = querySink.onEvent.mock.calls.map((call) => call[0].eventId)
    expect(appliedIds).toEqual(events.map((event) => event.id))
    expect(appliedEventIds).toEqual(events.map((event) => event.eventId))
    expect(new Set(appliedIds).size).toBe(1_000)
    expect(new Set(appliedEventIds).size).toBe(1_000)
    expect(controller.snapshot().lastConfirmedCursor).toBe(1_000)
    controller.stop()

    const oversizedSink = sink()
    const failures: string[] = []
    const oversizedTransport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const oversizedController = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport: oversizedTransport,
      adapter: adapterFor(businessEvent()),
      sink: oversizedSink,
      maxBoundaryBytes: 64,
      testOnlyLimits: true,
      telemetry: { record: (entry) => { if (entry.type === "recovery-failure") failures.push(entry.type) } },
    })
    oversizedController.start()
    oversizedTransport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(failures).toHaveLength(1))
    expect(oversizedSink.onEvent).not.toHaveBeenCalled()
    expect(oversizedController.snapshot().lastConfirmedCursor).toBe(0)
    oversizedController.stop()
  })

  test("starts a fresh non-budgeted F when an unknown event arrives during recovery", async () => {
    const querySink = sink()
    const known = businessEvent()
    const unknown = businessEvent({ id: 2, eventId: "e-2", kind: "task.attachment.created", canonicalFingerprint: "fp-2", known: false })
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    let firstRecoveryToken: SyncToken | null = null
    querySink.refetchObserved.mockImplementationOnce((_mode, token) => new Promise<RecoveryResult>((resolve) => {
      firstRecoveryToken = token
      resolveBarrier = () => resolve({
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, known.canonicalFingerprint]]),
          byEventId: new Map([[known.eventId, known.canonicalFingerprint]]),
          events: [known],
          token,
          revision: 1,
          published: true,
        },
      })
    }))
    const baseAdapter = adapterFor(known)
    const adapter: StreamContractAdapter = {
      ...baseAdapter,
      validateBusiness: (envelope) => envelope.id === unknown.id
        ? { status: "unknown", event: { ...unknown, raw: envelope.raw, kind: envelope.kind } }
        : baseAdapter.validateBusiness(envelope),
    }
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter,
      sink: querySink,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("R", expect.anything(), 0, 1, expect.any(AbortSignal)))
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    await controller.processFrame({ eventName: unknown.kind, id: "2", data: "{}" }, {
      boardId: "board-a",
      connectionEpoch: controller.snapshot().connectionEpoch,
      generation: controller.snapshot().generation,
    })
    resolveBarrier({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, known.canonicalFingerprint]]),
        byEventId: new Map([[known.eventId, known.canonicalFingerprint]]),
        events: [known],
        token: firstRecoveryToken ?? controller.snapshot(),
        revision: 1,
        published: true,
      },
    })
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("F", expect.anything(), 0, 2, expect.any(AbortSignal)))
    expect(querySink.onEvent).toHaveBeenCalledTimes(2)
    expect(querySink.onEvent.mock.calls.map((entry) => entry[0].eventId)).toEqual([known.eventId, unknown.eventId])
  })

  test("discards a stale recovery revision and reissues under a new token", async () => {
    const querySink = sink()
    const telemetry: { type: string; details?: Readonly<Record<string, unknown>> }[] = []
    let call = 0
    querySink.refetchObserved.mockImplementation(async (_mode, token) => {
      call += 1
      return {
        confirmedCursor: 0,
        noGap: true,
        boundary: {
          highWatermark: 0,
          byId: new Map(),
          byEventId: new Map(),
          events: [],
          token,
          revision: call === 1 ? 99 : 2,
          published: true,
        },
      }
    })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      telemetry: { record: (entry) => telemetry.push({ type: entry.type, details: entry.details }) },
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledTimes(2))
    expect(telemetry.map((entry) => entry.type)).not.toContain("protocol-anomaly")
    expect(telemetry.map((entry) => entry.type)).not.toContain("recovery-failure")
  })

  test("prevalidates the complete boundary before replaying any projection effect", async () => {
    const querySink = sink()
    const first = businessEvent({ id: 1, eventId: "e-1", canonicalFingerprint: "fp-1" })
    const conflicting = businessEvent({ id: 2, eventId: "e-2", canonicalFingerprint: "fp-2" })
    const failures: string[] = []
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => Promise.resolve({
      confirmedCursor: 2,
      noGap: true,
      boundary: {
        highWatermark: 2,
        byId: new Map([[1, first.canonicalFingerprint], [2, "wrong-id-fingerprint"]]),
        byEventId: new Map([[first.eventId, first.canonicalFingerprint], [conflicting.eventId, "wrong-event-fingerprint"]]),
        events: [first, conflicting],
        token,
        revision: expectedRevision,
        published: true,
      },
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(first),
      sink: querySink,
      telemetry: { record: (entry) => { if (entry.type === "recovery-failure") failures.push(entry.type) } },
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(failures).toHaveLength(1))
    expect(querySink.onEvent).not.toHaveBeenCalled()
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
    expect(controller.snapshot().seenIds).toBe(0)
  })

  test("rejects an H advance with an orphan or duplicate boundary event", async () => {
    const querySink = sink()
    const first = businessEvent({ id: 1, eventId: "e-1", canonicalFingerprint: "fp-1" })
    const duplicate = businessEvent({ id: 1, eventId: "e-duplicate", canonicalFingerprint: "fp-1" })
    const failures: string[] = []
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => Promise.resolve({
      confirmedCursor: 2,
      noGap: true,
      boundary: {
        highWatermark: 2,
        byId: new Map([[1, first.canonicalFingerprint], [2, "fp-2"]]),
        byEventId: new Map([[first.eventId, first.canonicalFingerprint], [duplicate.eventId, "fp-2"]]),
        events: [first, duplicate],
        token,
        revision: expectedRevision,
        published: true,
      },
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(first),
      sink: querySink,
      telemetry: { record: (entry) => { if (entry.type === "recovery-failure") failures.push(entry.type) } },
    })
    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(failures).toHaveLength(1))
    expect(querySink.onEvent).not.toHaveBeenCalled()
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
  })

  test("runs polling through the adapter pipeline while SSE is recovering", async () => {
    const querySink = sink()
    querySink.refetchObserved.mockImplementation(async (mode, token, _after, expectedRevision) => {
      querySink.recoveries.push(mode)
      return {
      confirmedCursor: 0,
      noGap: true,
      boundary: {
        highWatermark: 0,
        byId: new Map(),
        byEventId: new Map(),
        events: [],
        token,
        revision: expectedRevision,
        published: true,
      },
      }
    })
    querySink.pollEvents.mockResolvedValue({
      events: [{ id: 1 }],
      nextAfter: 1,
      hasMore: false,
      noGap: true,
    })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const { clock, advance } = fakeClock()
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
      pollingIntervalMs: 5,
      testOnlyLimits: true,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await Promise.resolve()
    advance(5)
    await vi.waitFor(() => expect(querySink.pollEvents).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(querySink.onEvent.mock.calls[0]?.[0].id).toBe(1)
  })

  test("aborts an in-flight poll on stop before it can commit or reopen", async () => {
    const querySink = sink()
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    querySink.refetchObserved.mockImplementationOnce((_mode, token, _after, expectedRevision) => new Promise<RecoveryResult>((resolve) => {
      resolveBarrier = () => resolve({
        confirmedCursor: 0,
        noGap: true,
        boundary: { highWatermark: 0, byId: new Map(), byEventId: new Map(), events: [], token, revision: expectedRevision, published: true },
      })
    }))
    let resolvePoll: (page: PollEventsPage) => void = () => undefined
    querySink.pollEvents.mockImplementationOnce((_query, signal) => new Promise<PollEventsPage>((resolve) => {
      signal.addEventListener("abort", () => undefined, { once: true })
      resolvePoll = resolve
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const { clock, advance } = fakeClock()
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
      pollingIntervalMs: 5,
      testOnlyLimits: true,
    })
    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledOnce())
    advance(5)
    await vi.waitFor(() => expect(querySink.pollEvents).toHaveBeenCalledOnce())
    controller.stop()
    resolveBarrier({ confirmedCursor: 0, noGap: true, boundary: { highWatermark: 0, byId: new Map(), byEventId: new Map(), events: [], token: querySink.refetchObserved.mock.calls[0]?.[1] ?? { boardId: "board-a", connectionEpoch: 0, generation: 0 }, revision: querySink.refetchObserved.mock.calls[0]?.[3] ?? 1, published: true } })
    resolvePoll({ events: [{ id: 1 }], nextAfter: 1, hasMore: false, noGap: true })
    await Promise.resolve()
    expect(controller.snapshot().state).toBe("stopped")
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
    expect(querySink.onEvent).not.toHaveBeenCalled()
  })

  test("does not remember known or unknown poll-boundary events after a board switch race", async () => {
    const runRace = async (event: ValidatedBusinessEvent): Promise<void> => {
      const querySink = sink()
      let releaseEffect: () => void = () => undefined
      const effect = new Promise<void>((resolve) => { releaseEffect = resolve })
      querySink.onEvent.mockImplementation(async () => effect)
      const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
      const { clock, advance } = fakeClock()
      const controller = new WebSyncController({
        boardId: "board-a",
        streamUrl: "http://127.0.0.1/api/v1/stream/events",
        transport,
        adapter: adapterFor(event),
        sink: querySink,
        clock,
      })
      querySink.pollEvents.mockImplementation(async () => {
        const snapshot = controller.snapshot()
        return {
          events: [],
          nextAfter: event.id,
          hasMore: false,
          noGap: true,
          confirmedCursor: event.id,
          boundary: {
            highWatermark: event.id,
            byId: new Map([[event.id, event.canonicalFingerprint]]),
            byEventId: new Map([[event.eventId, event.canonicalFingerprint]]),
            events: [event],
            token: { boardId: snapshot.boardId, connectionEpoch: snapshot.connectionEpoch, generation: snapshot.generation },
            revision: 1,
            published: true,
          },
        }
      })
      controller.start()
      ;(controller as unknown as { openCircuit: (reason: string) => void }).openCircuit("test-poll-race")
      advance(5_000)
      await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
      controller.switchBoard("board-b")
      releaseEffect()
      await Promise.resolve()
      await Promise.resolve()
      expect(controller.snapshot().boardId).toBe("board-b")
      expect(controller.snapshot().seenIds).toBe(0)
      expect(controller.snapshot().seenEventIds).toBe(0)
      expect(querySink.refetchObserved).not.toHaveBeenCalled()
      controller.stop()
    }

    await runRace(businessEvent({ known: true }))
    await runRace(businessEvent({ known: false, kind: "task.attachment.created" }))
  })

  test("does not rewrite dropped recovery maps after an overlapping buffer effect is fenced", async () => {
    const querySink = sink()
    let releaseEffect: () => void = () => undefined
    const effect = new Promise<void>((resolve) => { releaseEffect = resolve })
    querySink.onEvent.mockImplementation(async () => effect)
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
    })
    controller.start()
    const token: SyncToken = {
      boardId: controller.snapshot().boardId,
      connectionEpoch: controller.snapshot().connectionEpoch,
      generation: controller.snapshot().generation,
    }
    const event = businessEvent()
    const internals = controller as unknown as {
      recoveryBuffer: ValidatedBusinessEvent[]
      recoveryBufferBytes: number
      replayRecoveryBuffer: (token: SyncToken, boundary: RecoveryResult["boundary"], replayed: ReadonlySet<string>) => Promise<RecoveryResult["boundary"]>
      recoveryAppliedEvents: Map<string, ValidatedBusinessEvent>
    }
    internals.recoveryBuffer = [event]
    internals.recoveryBufferBytes = 1
    const boundary: RecoveryResult["boundary"] = {
      highWatermark: event.id,
      byId: new Map([[event.id, event.canonicalFingerprint]]),
      byEventId: new Map([[event.eventId, event.canonicalFingerprint]]),
      events: [],
      token,
      revision: 1,
      published: true,
    }
    const replay = internals.replayRecoveryBuffer(token, boundary, new Set())
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    controller.stop()
    releaseEffect()
    await replay
    expect(internals.recoveryAppliedEvents.size).toBe(0)
  })

  test("dedupes the same event when SSE and polling overlap", async () => {
    const querySink = sink()
    let resolveBarrier: (result: RecoveryResult) => void = () => undefined
    querySink.refetchObserved.mockImplementationOnce((_mode, token) => new Promise<RecoveryResult>((resolve) => {
      resolveBarrier = () => resolve({
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, "fp-1"]]),
          byEventId: new Map([["e-1", "fp-1"]]),
          events: [businessEvent()],
          token,
          revision: 1,
          published: true,
        },
      })
    }))
    querySink.pollEvents.mockResolvedValue({ events: [{ id: 1 }], nextAfter: 1, hasMore: false, noGap: true })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const { clock, advance } = fakeClock()
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter: adapterFor(businessEvent()),
      sink: querySink,
      clock,
      pollingIntervalMs: 5,
      testOnlyLimits: true,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onError(new Error("disconnect"))
    await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(2))
    transport.mock.calls[1]?.[0].onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await Promise.resolve()
    advance(5)
    await vi.waitFor(() => expect(querySink.pollEvents).toHaveBeenCalledOnce())
    resolveBarrier({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, "fp-1"]]),
        byEventId: new Map([["e-1", "fp-1"]]),
        events: [businessEvent()],
        token: {
          boardId: "board-a",
          connectionEpoch: controller.snapshot().connectionEpoch,
          generation: controller.snapshot().generation,
        },
        revision: 1,
        published: true,
      },
    })
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)
  })

  test("opens an anomaly circuit after three retries and keeps only polling", async () => {
    const querySink = sink()
    const { clock, advance } = fakeClock()
    const recoveryStarts: number[] = []
    querySink.refetchObserved.mockImplementation(async (mode) => {
      querySink.recoveries.push(mode)
      recoveryStarts.push(clock.now())
      throw new Error("boundary unavailable")
    })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const adapter: StreamContractAdapter = {
      ...adapterFor(businessEvent()),
      parseEnvelope: () => ({ status: "invalid", code: "malformed" }),
    }
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter,
      sink: querySink,
      clock,
      pollingIntervalMs: 5,
      testOnlyLimits: true,
    })

    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await Promise.resolve()
    advance(250)
    await Promise.resolve()
    advance(1_000)
    await Promise.resolve()
    advance(5_000)
    await vi.waitFor(() => expect(controller.snapshot().circuitOpen).toBe(true))
    expect(querySink.refetchObserved).toHaveBeenCalledTimes(3)
    expect(recoveryStarts).toEqual([250, 1_250, 6_250])
    const streamCalls = transport.mock.calls.length
    advance(5)
    await vi.waitFor(() => expect(querySink.pollEvents).toHaveBeenCalled())
    expect(transport.mock.calls.length).toBe(streamCalls)
  })

  test("keeps circuit polling fail-closed until a compatible boundary is committed", async () => {
    const querySink = sink()
    const { clock, advance } = fakeClock()
    const recoveryStarts: number[] = []
    querySink.refetchObserved.mockImplementation(async (mode) => {
      querySink.recoveries.push(mode)
      recoveryStarts.push(clock.now())
      throw new Error("boundary unavailable")
    })
    let circuitPolls = 0
    let allowBoundary = false
    const pollAnomalies: string[] = []
    querySink.pollEvents.mockImplementation(async () => {
      if (!controller.snapshot().circuitOpen) return { events: [], nextAfter: 0, hasMore: false, noGap: true }
      circuitPolls += 1
      if (!allowBoundary) return { events: [{ id: 1 }], nextAfter: 1, hasMore: false, noGap: true }
      const event = businessEvent()
      const token: SyncToken = {
        boardId: controller.snapshot().boardId,
        connectionEpoch: controller.snapshot().connectionEpoch,
        generation: controller.snapshot().generation,
      }
      return {
        events: [],
        nextAfter: 1,
        hasMore: false,
        noGap: true,
        confirmedCursor: 1,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, event.canonicalFingerprint]]),
          byEventId: new Map([[event.eventId, event.canonicalFingerprint]]),
          events: [event],
          token,
          revision: 1,
          published: true,
        },
      }
    })
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const adapter: StreamContractAdapter = {
      ...adapterFor(businessEvent()),
      parseEnvelope: () => ({ status: "invalid", code: "malformed" }),
    }
    const controller = new WebSyncController({
      boardId: "board-a",
      streamUrl: "http://127.0.0.1/api/v1/stream/events",
      transport,
      adapter,
      sink: querySink,
      clock,
      pollingIntervalMs: 5,
      testOnlyLimits: true,
      telemetry: { record: (entry) => { if (entry.type === "poll-protocol-anomaly") pollAnomalies.push(String(entry.details?.reason)) } },
    })

    controller.start()
    transport.mock.calls[0]?.[0].onFrame({ eventName: "task.updated", id: "1", data: "{}" })
    await Promise.resolve()
    advance(250)
    await Promise.resolve()
    advance(1_000)
    await Promise.resolve()
    advance(5_000)
    await vi.waitFor(() => expect(controller.snapshot().circuitOpen).toBe(true))
    advance(5)
    await vi.waitFor(() => expect(circuitPolls).toBeGreaterThan(0))
    expect(controller.snapshot().lastConfirmedCursor).toBe(0)
    expect(querySink.onEvent).not.toHaveBeenCalled()
    const pollCountBeforeBoundary = circuitPolls
    const anomalyCountBeforeBoundary = pollAnomalies.length
    allowBoundary = true
    for (let step = 0; step < 20 && querySink.onEvent.mock.calls.length === 0; step += 1) {
      await Promise.resolve()
      advance(5)
    }
    await vi.waitFor(() => expect(circuitPolls).toBeGreaterThan(pollCountBeforeBoundary))
    expect(pollAnomalies).toHaveLength(anomalyCountBeforeBoundary)
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(controller.snapshot().circuitOpen).toBe(false))
    expect(controller.snapshot().circuitOpen).toBe(false)
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)
  })
})
