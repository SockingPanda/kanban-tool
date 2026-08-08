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
    expect(() => new WebSyncController({ ...base, maxSeenEntries: 1 })).toThrow(/at least 2048/)
    expect(() => new WebSyncController({ ...base, maxAnomalyAttempts: 4 })).toThrow(/at most 3/)
    expect(() => new WebSyncController({ ...base, livenessTimeoutMs: 120_001 })).toThrow(/SLO/)
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
