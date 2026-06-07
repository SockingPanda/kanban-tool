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

    const tasks = await api.listTasks({ includeArchived: true, statuses: ["ready"] })

    expect(tasks).toHaveLength(1)
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks")
    expect(url.searchParams.get("q")).toBeNull()
    expect(url.searchParams.get("include_archived")).toBe("true")
    expect(url.searchParams.getAll("status")).toEqual(["ready"])
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
    })

    expect(result.tasks).toEqual([hitTask])
    expect(result.meta.backend).toBe("tantivy")
    expect(result.meta.stale).toBe(true)
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/search/tasks")
    expect(url.searchParams.get("board")).toBe("default")
    expect(url.searchParams.get("q")).toBe("hydrated")
    expect(url.searchParams.get("limit")).toBe("100")
    expect(url.searchParams.getAll("status")).toEqual(["ready", "review"])
  })
})

function mockFetch(envelope: unknown) {
  const fetchMock = vi.fn(async (input: string) => {
    void input
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

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "t_1",
    board_id: "b_1",
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
