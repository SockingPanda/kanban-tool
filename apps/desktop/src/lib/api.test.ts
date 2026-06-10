import { afterEach, describe, expect, it, vi } from "vitest"

import { KanbanApi, Task } from "./api"

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
    const fetchMock = mockFetch({
      data: {
        hits: [{ task_id: hitTask.id, seq: hitTask.seq, score: 4.2, snippet: "hydrated", task: hitTask }],
        meta: {
          backend: "tantivy",
          stale: true,
          index_version: "v1",
          last_event_id: 12,
          index_lag_events: 2,
        },
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
    expect(result.searchMeta.backend).toBe("tantivy")
    expect(result.searchMeta.stale).toBe(true)
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

  it("preserves unknown totals while keeping numeric limit and offset", async () => {
    const fetchMock = mockFetch({
      data: {
        hits: [],
        meta: {
          backend: "sqlite",
          stale: false,
          index_version: null,
          last_event_id: null,
          index_lag_events: null,
        },
      },
      meta: { limit: 10, offset: 20 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.searchTasks({ query: "missing total", limit: 10, offset: 20 })

    expect(result.page).toEqual({ limit: 10, offset: 20, total: null })
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

  it("uses board-scoped maintenance and status endpoints", async () => {
    const fetchMock = mockFetch({
      data: {
        backend: "sqlite",
        stale: false,
        index_version: null,
        last_event_id: null,
        index_lag_events: null,
      },
    })
    const api = new KanbanApi(runtimeConfig)

    await api.searchStatus()

    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/search/status")
    expect(calledUrl(fetchMock).searchParams.get("board")).toBe("default")

    fetchMock.mockClear()
    await api.doctor()
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/maintenance/doctor")
    expect(calledInit(fetchMock).method).toBe("POST")
    expect(JSON.parse(String(calledInit(fetchMock).body))).toEqual({
      actor: "desktop-test",
      board: "default",
    })

    fetchMock.mockClear()
    await api.checkpoint()
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/maintenance/checkpoint")
    expect(calledInit(fetchMock).method).toBe("POST")
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
    ...overrides,
  }
}
