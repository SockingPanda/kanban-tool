import { describe, expect, test, vi } from "vitest"

import type { BoardReadModel, BoardReadQuery } from "../api/board-read-model"
import type { ApiListEventsResponseContract } from "../api/generated/contracts/api-list-events-response"
import type { EventsApiClient } from "./events-api"
import { createBoardSyncSink, type BoardSyncSinkIdentity } from "./board-sync-sink"
import { asCanonicalBoardId } from "./contracts"
import type {
  BusinessValidationResult,
  ControlValidationResult,
  EnvelopeParseResult,
  InvalidationPlan,
  StreamContractAdapter,
  SyncToken,
  ValidatedBusinessEvent,
} from "./contracts"

const BOARD_ID = asCanonicalBoardId("board-a")
const IDENTITY: BoardSyncSinkIdentity = { canonicalBoardId: BOARD_ID, selector: "board-slug" }
const MODEL: BoardReadModel = {
  identity: { selector: IDENTITY.selector, canonicalBoardId: BOARD_ID, slug: "board-slug", name: "Board A" },
  columns: [],
  tasksByStatus: {},
}

function token(generation = 1, boardId = BOARD_ID): SyncToken {
  return { boardId, connectionEpoch: 1, generation }
}

function businessEvent(id = 1, overrides: Partial<ValidatedBusinessEvent> = {}): ValidatedBusinessEvent {
  return {
    id,
    eventId: `event-${id}`,
    boardId: BOARD_ID,
    taskId: `task-${id}`,
    runId: null,
    kind: "task.updated",
    createdAt: 1_700_000_000 + id,
    raw: { id },
    scope: { taskId: `task-${id}` },
    canonicalFingerprint: `fingerprint-${id}`,
    known: true,
    ...overrides,
  }
}

function rawEvent(id: number, overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id,
    event_id: `event-${id}`,
    board_id: BOARD_ID,
    task_id: `task-${id}`,
    run_id: null,
    kind: "task.updated",
    actor: null,
    payload: { id },
    created_at: 1_700_000_000 + id,
    ...overrides,
  }
}

function envelope(value: unknown): EnvelopeParseResult {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return { status: "invalid", code: "not-an-object" }
  }
  const raw = value as Record<string, unknown>
  if (
    typeof raw.id !== "number"
    || !Number.isSafeInteger(raw.id)
    || raw.id < 0
    || typeof raw.event_id !== "string"
    || typeof raw.board_id !== "string"
    || (typeof raw.task_id !== "string" && raw.task_id !== null)
    || (typeof raw.run_id !== "string" && raw.run_id !== null)
    || typeof raw.kind !== "string"
    || typeof raw.created_at !== "number"
  ) {
    return { status: "invalid", code: "invalid-envelope" }
  }
  return {
    status: "valid",
    envelope: {
      id: raw.id,
      eventId: raw.event_id,
      boardId: raw.board_id,
      taskId: raw.task_id,
      runId: raw.run_id,
      kind: raw.kind,
      createdAt: raw.created_at,
      raw: value,
    },
  }
}

function adapter(): StreamContractAdapter {
  return {
    parseEnvelope: (): EnvelopeParseResult => ({ status: "invalid", code: "unused" }),
    parsePollingEnvelope: envelope,
    validateBusiness: (candidate): BusinessValidationResult => {
      const raw = candidate.raw as Record<string, unknown>
      const payload = JSON.stringify(raw.payload)
      const known = candidate.kind !== "future.event"
      const event: ValidatedBusinessEvent = {
        ...candidate,
        scope: { taskId: candidate.taskId },
        canonicalFingerprint: `${candidate.eventId}:${payload}`,
        known,
      }
      return known ? { status: "known", event } : { status: "unknown", event }
    },
    isControlFrame: (): boolean => false,
    validateControl: (): ControlValidationResult => ({ status: "invalid", code: "unused" }),
  }
}

function response(events: readonly unknown[], nextAfter: number): ApiListEventsResponseContract {
  return { data: events, meta: { next_after: nextAfter } } as unknown as ApiListEventsResponseContract
}

interface Harness {
  readonly sink: ReturnType<typeof createBoardSyncSink>
  readonly query: BoardReadQuery
  readonly reload: ReturnType<typeof vi.fn>
  readonly publish: ReturnType<typeof vi.fn>
  readonly listEvents: ReturnType<typeof vi.fn<EventsApiClient["listEvents"]>>
}

function harness(): Harness {
  const reload = vi.fn(async () => MODEL)
  const query: BoardReadQuery = {
    load: vi.fn(async () => MODEL),
    reload,
    invalidate: vi.fn(),
  }
  const publish = vi.fn(async () => undefined)
  const listEvents = vi.fn<EventsApiClient["listEvents"]>(async () => response([], 0))
  const eventsApi: EventsApiClient = { listEvents }
  const sink = createBoardSyncSink({ identity: IDENTITY, query, publish, eventsApi, adapter: adapter() })
  return { sink, query, reload, publish, listEvents }
}

const relevantPlan: InvalidationPlan = {
  kind: "known",
  eventKind: "task.updated",
  timeline: true,
  fullRefetch: false,
  targets: [{ root: "columns", boardId: BOARD_ID }],
}

describe("BoardSyncSink", () => {
  test("reloads and publishes only relevant active-board invalidations", async () => {
    const { sink, reload, publish } = harness()
    await sink.onEvent(businessEvent(), { ...relevantPlan, targets: [{ root: "stats", boardId: BOARD_ID }] }, token())
    expect(reload).not.toHaveBeenCalled()
    await sink.onEvent(businessEvent(), relevantPlan, token())
    expect(reload).toHaveBeenCalledTimes(1)
    expect(publish).toHaveBeenCalledWith(MODEL)
  })

  test("aborts an older generation and never publishes its late reload", async () => {
    const { sink, reload, publish } = harness()
    let release!: (model: BoardReadModel) => void
    const blocked = new Promise<BoardReadModel>((resolve) => { release = resolve })
    let oldSignal: AbortSignal | undefined
    reload.mockImplementationOnce(async (signal?: AbortSignal) => {
      oldSignal = signal
      return blocked
    })
    const old = sink.onEvent(businessEvent(), relevantPlan, token(1))
    await Promise.resolve()
    const newer = sink.onEvent(businessEvent(), relevantPlan, token(2))
    await newer
    expect(oldSignal?.aborted).toBe(true)
    release(MODEL)
    await old
    expect(publish).toHaveBeenCalledTimes(1)
  })

  test("ignores a same-epoch lower generation as stale", async () => {
    const { sink, reload } = harness()
    await sink.onEvent(businessEvent(), relevantPlan, token(2))
    await sink.onEvent(businessEvent(), relevantPlan, token(1))
    expect(reload).toHaveBeenCalledTimes(1)
  })

  test("publishes an empty recovery and confirms an unchanged cursor", async () => {
    const { sink, listEvents, publish } = harness()
    listEvents.mockResolvedValue(response([], 0))
    const result = await sink.refetchObserved("F", token(), 0, 1, new AbortController().signal)
    expect(result).toMatchObject({ confirmedCursor: 0, noGap: true })
    expect(result.boundary).toMatchObject({ highWatermark: 0, published: true })
    expect(result.boundary.events).toEqual([])
    expect(publish).toHaveBeenCalledTimes(1)
  })

  test("handles a full page followed by an empty tail page", async () => {
    const { sink, listEvents } = harness()
    const firstPage = Array.from({ length: 100 }, (_, index) => rawEvent(index + 1))
    listEvents.mockImplementation(async (query) => query.after === 0 ? response(firstPage, 100) : response([], 100))
    const result = await sink.refetchObserved("R", token(), 0, 7, new AbortController().signal)
    expect(result.noGap).toBe(true)
    expect(result.confirmedCursor).toBe(100)
    expect(result.boundary.events).toHaveLength(100)
    expect(listEvents.mock.calls.map(([query]) => query.after)).toEqual([0, 100])
  })

  test("keeps an unknown valid event in the recovery boundary", async () => {
    const { sink, listEvents } = harness()
    listEvents.mockResolvedValue(response([rawEvent(1, { kind: "future.event" })], 1))
    const result = await sink.refetchObserved("R", token(), 0, 2, new AbortController().signal)
    expect(result.boundary.events[0]?.known).toBe(false)
    expect(result.boundary.byId.get(1)).toBe(result.boundary.events[0]?.canonicalFingerprint)
  })

  test("does not create a boundary when publishing the snapshot fails", async () => {
    const { sink, publish, listEvents } = harness()
    publish.mockRejectedValueOnce(new Error("publish failed"))
    await expect(sink.refetchObserved("F", token(), 0, 1, new AbortController().signal)).rejects.toThrow("publish failed")
    listEvents.mockResolvedValue(response([], 0))
    const page = await sink.pollEvents({ board: IDENTITY.selector, after: 0, limit: 100 }, new AbortController().signal)
    expect(page.boundary).toBeUndefined()
  })

  test("forwards the operation signal and preserves an API AbortError", async () => {
    const { sink, listEvents } = harness()
    const abortError = new Error("aborted by transport")
    abortError.name = "AbortError"
    let apiSignal: AbortSignal | undefined
    listEvents.mockImplementation(async (_query, signal) => {
      apiSignal = signal
      throw abortError
    })
    await expect(sink.refetchObserved("R", token(), 0, 1, new AbortController().signal)).rejects.toBe(abortError)
    expect(apiSignal).toBeDefined()
  })

  test.each([
    ["invalid next_after", () => response([], -1), "invalid_cursor"],
    ["empty page advances cursor", () => response([], 1), "invalid_cursor"],
    ["foreign board", () => response([rawEvent(1, { board_id: "board-b" })], 1), "board_isolation"],
    ["out-of-order ids", () => response([rawEvent(2), rawEvent(1)], 1), "invalid_page"],
    ["duplicate event", () => response([rawEvent(1), rawEvent(1)], 1), "duplicate_event"],
    ["fingerprint conflict", () => response([rawEvent(1), rawEvent(1, { payload: { changed: true } })], 1), "fingerprint_conflict"],
  ] as const)("rejects %s recovery data", async (_name, makeResponse, code) => {
    const { sink, listEvents } = harness()
    listEvents.mockResolvedValue(makeResponse())
    const result = sink.refetchObserved("R", token(), 0, 1, new AbortController().signal)
    await expect(result).rejects.toMatchObject({ code })
  })

  test("enforces event-count and raw-byte recovery budgets", async () => {
    const tooMany = harness()
    const allEvents = Array.from({ length: 2_049 }, (_, index) => rawEvent(index + 1))
    tooMany.listEvents.mockImplementation(async (query) => {
      const after = query.after ?? 0
      const page = allEvents.slice(after, after + 100)
      return response(page, after + page.length)
    })
    await expect(tooMany.sink.refetchObserved("R", token(), 0, 1, new AbortController().signal))
      .rejects.toMatchObject({ code: "recovery_budget" })

    const tooLarge = harness()
    tooLarge.listEvents.mockResolvedValue(response([rawEvent(1, { payload: "x".repeat(8 * 1024 * 1024) })], 1))
    await expect(tooLarge.sink.refetchObserved("R", token(), 0, 1, new AbortController().signal))
      .rejects.toMatchObject({ code: "recovery_budget" })
  })

  test("returns the published boundary with a validated polling page", async () => {
    const { sink, listEvents } = harness()
    listEvents.mockResolvedValueOnce(response([rawEvent(1)], 1))
    const recovery = await sink.refetchObserved("R", token(), 0, 3, new AbortController().signal)
    listEvents.mockResolvedValueOnce(response([rawEvent(2)], 2))
    const page = await sink.pollEvents({ board: IDENTITY.selector, after: recovery.confirmedCursor, limit: 100 }, new AbortController().signal)
    expect(page).toMatchObject({ noGap: true, nextAfter: 2, confirmedCursor: 2 })
    expect(page.boundary).not.toBe(recovery.boundary)
    expect(page.boundary?.highWatermark).toBe(2)
    expect(page.boundary?.events.map((event) => event.id)).toEqual([2])
    expect(page.boundary?.byId.has(1)).toBe(false)
  })

  test("does not publish a boundary for a different polling selector", async () => {
    const { sink, listEvents } = harness()
    listEvents.mockResolvedValueOnce(response([rawEvent(1)], 1))
    await sink.refetchObserved("R", token(), 0, 3, new AbortController().signal)
    listEvents.mockResolvedValueOnce(response([rawEvent(2)], 2))
    const page = await sink.pollEvents({ board: "other-board", after: 1, limit: 100 }, new AbortController().signal)
    expect(page.noGap).toBe(false)
    expect(page.boundary).toBeUndefined()
  })
})
