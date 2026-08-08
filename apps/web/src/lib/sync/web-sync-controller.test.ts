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

function businessEvent(overrides: Partial<ValidatedBusinessEvent> = {}): ValidatedBusinessEvent {
  return {
    id: 1,
    eventId: "e-1",
    boardId: "board-a",
    taskId: "task-a",
    runId: null,
    kind: "task.updated",
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
        raw: event.raw,
      },
    }),
    validateBusiness: (envelope): BusinessValidationResult => {
      const currentEvent = envelope.id === event.id
        ? event
        : { ...event, id: envelope.id, eventId: envelope.eventId, kind: envelope.kind, raw: envelope.raw, canonicalFingerprint: `fp-${envelope.id}` }
      return currentEvent.known ? { status: "known", event: currentEvent } : { status: "unknown", event: currentEvent }
    },
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
type RecoveryHandler = (mode: "F" | "R", token: SyncToken, after: number) => Promise<RecoveryResult>
type PollHandler = (query: unknown, signal: AbortSignal) => Promise<PollEventsPage>

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
    refetchObserved: vi.fn<RecoveryHandler>(async (mode, token) => {
      state.recoveries.push(mode)
      return {
        confirmedCursor: 1,
        noGap: true,
        boundary: {
          highWatermark: 1,
          byId: new Map([[1, "fp-1"]]),
          byEventId: new Map([["e-1", "fp-1"]]),
          token,
          revision: 1,
          published: true,
        },
      }
    }),
    pollEvents: vi.fn<PollHandler>(async () => ({ events: [], nextAfter: 0, hasMore: false, noGap: true })),
  }
}

describe("WebSyncController", () => {
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
    advance(34)
    await Promise.resolve()
    expect(querySink.refetchObserved).not.toHaveBeenCalled()
    advance(1)
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("R", expect.anything(), 0))
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
    await vi.waitFor(() => expect(querySink.refetchObserved).toHaveBeenCalledWith("F", expect.anything(), 0))
    expect(querySink.onEvent).toHaveBeenCalledOnce()
    expect(controller.snapshot().lastConfirmedCursor).toBe(1)
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

    resolveBarrier({
      confirmedCursor: 1,
      noGap: true,
      boundary: {
        highWatermark: 1,
        byId: new Map([[1, "fp-1"]]),
        byEventId: new Map([["e-1", "fp-1"]]),
        token: controller.snapshot().connectionEpoch === 2
          ? { boardId: "board-a", connectionEpoch: 2, generation: 0 }
          : { boardId: "board-a", connectionEpoch: 1, generation: 0 },
        revision: 1,
        published: true,
      },
    })
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(controller.snapshot().lastConfirmedCursor).toBe(2)
  })

  test("runs polling through the adapter pipeline while SSE is recovering", async () => {
    const querySink = sink()
    querySink.refetchObserved.mockImplementation(async (_mode, token) => ({
      confirmedCursor: 0,
      noGap: true,
      boundary: {
        highWatermark: 0,
        byId: new Map(),
        byEventId: new Map(),
        token,
        revision: 1,
        published: true,
      },
    }))
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
    advance(5)
    await vi.waitFor(() => expect(querySink.pollEvents).toHaveBeenCalledOnce())
    await vi.waitFor(() => expect(querySink.onEvent).toHaveBeenCalledOnce())
    expect(querySink.onEvent.mock.calls[0]?.[0].id).toBe(1)
  })

  test("opens an anomaly circuit after three retries and keeps only polling", async () => {
    const querySink = sink()
    querySink.refetchObserved.mockImplementation(async (_mode, token) => ({
      confirmedCursor: 0,
      noGap: true,
      boundary: {
        highWatermark: 0,
        byId: new Map(),
        byEventId: new Map(),
        token,
        revision: 1,
        published: true,
      },
    }))
    const transport = vi.fn<SseTransport>(() => ({ closed: false, close: vi.fn() }))
    const { clock, advance } = fakeClock()
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
    for (let attempt = 0; attempt < 4; attempt += 1) {
      await vi.waitFor(() => expect(transport.mock.calls.length).toBeGreaterThan(attempt))
      transport.mock.calls[attempt]?.[0].onFrame({ eventName: "task.updated", id: String(attempt + 1), data: "{}" })
      await Promise.resolve()
    }
    await vi.waitFor(() => expect(controller.snapshot().circuitOpen).toBe(true))
    const streamCalls = transport.mock.calls.length
    advance(5)
    await vi.waitFor(() => expect(querySink.pollEvents).toHaveBeenCalled())
    expect(transport.mock.calls.length).toBe(streamCalls)
  })
})
