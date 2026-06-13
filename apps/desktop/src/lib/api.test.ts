import { afterEach, describe, expect, it, vi } from "vitest"

import { KanbanApi, type ApiError, type Board, type SearchIndexStatus, type SearchTasksMeta, type Task } from "./api"

const runtimeConfig = {
  apiBaseUrl: "http://127.0.0.1:8721",
  dbPath: "test.db",
  actor: "desktop-test",
  board: "default",
}

describe("KanbanApi task search", () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("keeps the task list endpoint query-free for empty search flows", async () => {
    const fetchMock = mockFetch({ data: [task({ id: "t_list", title: "plain list" })] })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listTasks({ includeArchived: true, statuses: ["ready"], limit: 25, offset: 50 })

    expect(result.tasks).toHaveLength(1)
    expect(result.page).toEqual({ limit: 25, offset: 50, total: null })
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks")
    expect(url.searchParams.get("q")).toBeNull()
    expect(url.searchParams.get("include_archived")).toBe("true")
    expect(url.searchParams.get("limit")).toBe("25")
    expect(url.searchParams.get("offset")).toBe("50")
    expect(url.searchParams.getAll("status")).toEqual(["ready"])
  })

  it("passes list search, priority filters, and sort to the task list endpoint", async () => {
    const fetchMock = mockFetch({ data: [] })
    const api = new KanbanApi(runtimeConfig)

    await api.listTasks({
      query: " dashboard ",
      statuses: ["ready", "blocked"],
      priorities: [0, 2],
      sort: "priority",
      limit: 25,
      offset: 0,
    })

    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks")
    expect(url.searchParams.get("q")).toBe("dashboard")
    expect(url.searchParams.get("sort")).toBe("priority")
    expect(url.searchParams.getAll("status")).toEqual(["ready", "blocked"])
    expect(url.searchParams.getAll("priority")).toEqual(["0", "2"])
  })

  it("keeps list task pagination metadata from the response envelope", async () => {
    const fetchMock = mockFetch({
      data: [task({ id: "t_list", title: "plain list" })],
      meta: { limit: 25, offset: 50, total: 225 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listTasks({ includeArchived: true, limit: 25, offset: 50 })

    expect(result.page).toEqual({ limit: 25, offset: 50, total: 225 })
    expect(calledInit(fetchMock).signal).toBeUndefined()
  })

  it("uses the search endpoint and returns hydrated task rows", async () => {
    const hitTask = task({ id: "t_search", title: "hydrated search result" })
    const searchMeta = {
      backend: "tantivy",
      stale: true,
      index_version: "v1",
      last_event_id: 12,
      index_lag_events: 2,
    } satisfies SearchTasksMeta
    const fetchMock = mockFetch({
      data: {
        hits: [{ task_id: hitTask.id, seq: hitTask.seq, score: 4.2, snippet: "hydrated", task: hitTask }],
        meta: searchMeta,
      },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.searchTasks({
      query: " hydrated ",
      includeArchived: false,
      statuses: ["ready", "review"],
      limit: 20,
      offset: 40,
    })

    expect(result.tasks).toEqual([hitTask])
    expect(result.searchMeta).toEqual(searchMeta)
    expect("derived_index" in result.searchMeta).toBe(false)
    expect("message" in result.searchMeta).toBe(false)
    expect(result.page).toEqual({ limit: 20, offset: 40, total: null })
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/search/tasks")
    expect(url.searchParams.get("board")).toBe("default")
    expect(url.searchParams.get("q")).toBe("hydrated")
    expect(url.searchParams.get("limit")).toBe("20")
    expect(url.searchParams.get("offset")).toBe("40")
    expect(url.searchParams.getAll("status")).toEqual(["ready", "review"])
  })

  it("passes AbortSignal through queryable API requests", async () => {
    const fetchMock = mockFetch({ data: [], meta: { limit: 10, offset: 0, total: 0 } })
    const api = new KanbanApi(runtimeConfig)
    const controller = new AbortController()

    await api.listTasks({ signal: controller.signal, limit: 10 })

    expect(calledInit(fetchMock).signal).toBe(controller.signal)
  })

  it("rejects malformed task list envelopes before React consumes them", async () => {
    mockFetch({ data: { not: "an array" }, meta: { limit: 10, offset: 0, total: 1 } })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.listTasks({ limit: 10 })).rejects.toMatchObject({
      code: "invalid_response",
      message: "tasks response data must be an array",
    } satisfies Partial<ApiError>)
  })

  it("rejects malformed search envelopes before returning hydrated rows", async () => {
    mockFetch({
      data: {
        meta: {
          backend: "tantivy",
          stale: false,
          index_version: "v1",
          last_event_id: 12,
          index_lag_events: 0,
        },
      },
    })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.searchTasks({ query: "broken" })).rejects.toMatchObject({
      code: "invalid_response",
      message: "search hits must be an array",
    } satisfies Partial<ApiError>)
  })

  it("preserves unknown totals while keeping numeric limit and offset", async () => {
    const searchMeta = {
      backend: "sqlite",
      stale: false,
      index_version: null,
      last_event_id: null,
      index_lag_events: null,
    } satisfies SearchTasksMeta
    const fetchMock = mockFetch({
      data: {
        hits: [],
        meta: searchMeta,
      },
      meta: { limit: 10, offset: 20 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.searchTasks({ query: "missing total", limit: 10, offset: 20 })

    expect(result.page).toEqual({ limit: 10, offset: 20, total: null })
    expect(result.searchMeta).toEqual(searchMeta)
    expect(calledUrl(fetchMock).searchParams.get("q")).toBe("missing total")
  })

  it("uses event envelope cursor metadata instead of deriving only from row ids", async () => {
    const fetchMock = mockFetch({
      data: [eventRecord({ id: 123, task_id: "t_1", kind: "task.claimed" })],
      meta: { next_after: 130 },
    })
    const api = new KanbanApi(runtimeConfig)

    const page = await api.listEventsAfter(120)

    expect(page.events).toHaveLength(1)
    expect(page.meta.next_after).toBe(130)
    const url = calledUrl(fetchMock)
    expect(url.searchParams.get("after")).toBe("120")
  })

  it("uses /health outside the API v1 prefix", async () => {
    const fetchMock = mockFetch({ data: { ok: true, db: "ok", version: "1.1.0" } })
    const api = new KanbanApi(runtimeConfig)

    const health = await api.health()

    expect(health).toEqual({ ok: true, db: "ok", version: "1.1.0" })
    expect(calledUrl(fetchMock).pathname).toBe("/health")
  })

  it("lists active boards through the boards endpoint", async () => {
    const boards = [
      board({ id: "b_default", slug: "default", name: "Default" }),
      board({ id: "b_ops", slug: "ops", name: "Ops" }),
    ]
    const fetchMock = mockFetch({ data: boards })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listBoards()

    expect(result).toEqual(boards)
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/boards")
    expect(calledUrl(fetchMock).searchParams.get("include_archived")).toBe("false")
  })

  it("can include archived boards when listing boards", async () => {
    const fetchMock = mockFetch({ data: [board({ archived_at: 10 })] })
    const api = new KanbanApi(runtimeConfig)

    await api.listBoards({ includeArchived: true })

    expect(calledUrl(fetchMock).searchParams.get("include_archived")).toBe("true")
  })

  it("rejects malformed board list envelopes before React consumes them", async () => {
    mockFetch({ data: { not: "an array" } })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.listBoards()).rejects.toMatchObject({
      code: "invalid_response",
      message: "boards response data must be an array",
    } satisfies Partial<ApiError>)
  })

  it("uses backend-shaped maintenance and status envelopes", async () => {
    const searchStatusEnvelope = {
      backend: "sqlite",
      derived_index: false,
      stale: false,
      index_version: null,
      last_event_id: null,
      index_lag_events: 0,
      message: "SQLite fallback search is active",
    } satisfies SearchIndexStatus
    const fetchMock = mockFetchSequence([
      {
        data: {
          board_id: "b_1",
          generated_at: 10,
          status_counts: [{ status: "ready", count: 3 }],
          stale_claims: [
            {
              task_id: "t_stale",
              seq: 7,
              title: "stale worker",
              claim_owner: "dispatcher",
              claim_expires_at: 8,
              last_heartbeat_at: 5,
              current_run_id: "r_1",
              retry_count: 1,
              max_retries: 3,
            },
          ],
          blocked_reasons: [{ reason: "waiting", count: 2 }],
        },
      },
      {
        data: searchStatusEnvelope,
      },
      {
        data: {
          ok: true,
          integrity_check: "ok",
          migration_version: 1,
          user_version: 0,
          expired_running_tasks: 0,
          running_tasks_without_active_run: 0,
          orphan_running_runs: 0,
          dependency_cycles: 0,
          archived_dependency_edges: 0,
          missing_run_logs: 0,
          suspicious_run_log_paths: 0,
          executable_dependency_violations: 0,
          executable_spec_violations: 0,
          executable_schedule_violations: 0,
          outbox_pending: 0,
          outbox_running: 0,
          outbox_failed: 0,
          derived_dirty_stores: 0,
          derived_error_stores: 0,
          derived_stores: [],
        },
      },
      {
        data: {
          busy: 0,
          log_frames: 4,
          checkpointed_frames: 4,
        },
      },
    ])
    const api = new KanbanApi(runtimeConfig)

    const stats = await api.stats()
    expect(stats.status_counts).toEqual([{ status: "ready", count: 3 }])
    expect(stats.stale_claims[0].task_id).toBe("t_stale")
    expect(stats.blocked_reasons).toEqual([{ reason: "waiting", count: 2 }])
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/stats")
    expect(calledUrl(fetchMock).searchParams.get("board")).toBe("default")

    const searchStatus = await api.searchStatus()
    expect(searchStatus).toEqual(searchStatusEnvelope)
    expect(searchStatus.derived_index).toBe(false)
    expect(searchStatus.message).toBe("SQLite fallback search is active")
    expect(fetchMock.mock.calls[1] ? new URL(String(fetchMock.mock.calls[1][0])).pathname : "").toBe("/api/v1/search/status")

    const doctor = await api.doctor()
    expect(doctor.integrity_check).toBe("ok")
    expect(fetchMock.mock.calls[2] ? new URL(String(fetchMock.mock.calls[2][0])).pathname : "").toBe("/api/v1/maintenance/doctor")
    expect(fetchMock.mock.calls[2]?.[1]?.method).toBe("POST")
    expect(JSON.parse(String(fetchMock.mock.calls[2]?.[1]?.body))).toEqual({
      actor: "desktop-test",
      board: "default",
    })

    const checkpoint = await api.checkpoint()
    expect(checkpoint).toEqual({ busy: 0, log_frames: 4, checkpointed_frames: 4 })
    expect(fetchMock.mock.calls[3] ? new URL(String(fetchMock.mock.calls[3][0])).pathname : "").toBe("/api/v1/maintenance/checkpoint")
    expect(fetchMock.mock.calls[3]?.[1]?.method).toBe("POST")
  })

  it("deletes parent dependencies through the child scoped endpoint", async () => {
    const fetchMock = mockFetch({ data: { parents: [], children: [] } })
    const api = new KanbanApi(runtimeConfig)

    await api.removeDependency("t_child", "t_parent")

    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/tasks/t_child/dependencies/t_parent")
    expect(calledInit(fetchMock).method).toBe("DELETE")
  })
})

function mockFetch(envelope: unknown) {
  const fetchMock = vi.fn(async (input: string, init?: RequestInit) => {
    void input
    void init
    return {
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => JSON.stringify(envelope),
    }
  })
  vi.stubGlobal("fetch", fetchMock)
  return fetchMock
}

function mockFetchSequence(envelopes: unknown[]) {
  const fetchMock = vi.fn(async (input: string, init?: RequestInit) => {
    void input
    void init
    const envelope = envelopes.shift()
    expect(envelope).toBeDefined()
    return {
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => JSON.stringify(envelope),
    }
  })
  vi.stubGlobal("fetch", fetchMock)
  return fetchMock
}

function calledUrl(fetchMock: ReturnType<typeof mockFetch>) {
  const url = fetchMock.mock.calls[0]?.[0]
  expect(url).toBeDefined()
  return new URL(url)
}

function calledInit(fetchMock: ReturnType<typeof mockFetch>) {
  const init = fetchMock.mock.calls[0]?.[1]
  expect(init).toBeDefined()
  return init!
}

function eventRecord(overrides: Partial<import("./api").EventRecord> = {}): import("./api").EventRecord {
  return {
    id: 1,
    event_id: "e_1",
    board_id: "b_1",
    task_id: "t_1",
    run_id: null,
    kind: "task.created",
    actor: "seed",
    payload: {},
    created_at: 1,
    ...overrides,
  }
}

function board(overrides: Partial<Board> = {}): Board {
  return {
    id: "b_1",
    slug: "default",
    name: "Default",
    description: null,
    created_at: 1,
    updated_at: 1,
    archived_at: null,
    ...overrides,
  }
}

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "t_1",
    board_id: "b_1",
    board_slug: "default",
    ref: "default#1",
    seq: 1,
    title: "Task",
    description: null,
    status: "ready",
    status_reason: null,
    assignee: null,
    priority: 0,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "seed",
    created_at: 1,
    updated_at: 1,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result_json: null,
    metadata_json: "{}",
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    ...overrides,
  }
}
