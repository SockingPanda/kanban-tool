import { describe, expect, test, vi } from "vitest"

import type { WebRuntimeConfig } from "../runtime"
import { BoardReadError, createBoardReadQuery, loadBoardReadModel } from "./board-read-model"

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test-actor",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "sha256:test",
} satisfies WebRuntimeConfig

type ResponseBody = Record<string, unknown>

function jsonResponse(body: ResponseBody, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })
}

function board(id: string, slug: string, name: string, archivedAt: number | null = null) {
  return {
    id,
    slug,
    name,
    description: null,
    created_at: 1,
    updated_at: 2,
    archived_at: archivedAt,
  }
}

function column(id: string, boardId: string, status: string, position: number, hidden = false) {
  return {
    id,
    board_id: boardId,
    status,
    title: status,
    position,
    hidden,
    wip_limit: null,
    created_at: 1,
    updated_at: 2,
  }
}

function task(id: string, boardId: string, boardSlug: string, status: string, position: number) {
  return {
    id,
    board_id: boardId,
    board_slug: boardSlug,
    ref: `${boardSlug}#${position}`,
    seq: position,
    title: id,
    description: null,
    status,
    status_reason: null,
    assignee: null,
    priority: 0,
    position,
    scheduled_at: null,
    due_at: null,
    created_by: "test",
    created_at: 1,
    updated_at: 2,
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
    result: null,
    metadata: {},
    lock_version: 1,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "unplanned",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
  }
}

function routeResponse(url: string): Response {
  const parsed = new URL(url)
  if (parsed.pathname === "/api/v1/boards") {
    return jsonResponse({ data: [board("b_default", "default", "Default"), board("b_other", "other", "Other")] })
  }
  if (parsed.pathname === "/api/v1/boards/b_default/columns") {
    return jsonResponse({
      data: [
        column("c_ready", "b_default", "ready", 20),
        column("c_todo", "b_default", "todo", 10),
        column("c_hidden", "b_default", "done", 30, true),
      ],
    })
  }
  if (parsed.pathname === "/api/v1/boards/b_default/tasks/by-status") {
    const status = parsed.searchParams.get("status")
    if (status === "ready") return jsonResponse({ data: { statuses: [{ status, tasks: [task("t_ready", "b_default", "default", status, 20)], page: { limit: 1000, offset: 0, total: 1 } }] }, meta: { limit: 1000, offset: 0 } })
    if (status === "todo") return jsonResponse({ data: { statuses: [{ status, tasks: [task("t_todo", "b_default", "default", status, 10)], page: { limit: 1000, offset: 0, total: 1 } }] }, meta: { limit: 1000, offset: 0 } })
    if (status === "done") return jsonResponse({ data: { statuses: [{ status, tasks: [], page: { limit: 1000, offset: 0, total: 0 } }] }, meta: { limit: 1000, offset: 0 } })
  }
  throw new Error(`unexpected URL ${url}`)
}

describe("board read model", () => {
  test("resolves the exact selector, preserves server columns, and groups one request per unique status", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) => routeResponse(String(input)))

    const model = await loadBoardReadModel(runtime, undefined, { fetcher })

    expect(model.identity).toEqual({
      selector: "default",
      canonicalBoardId: "b_default",
      slug: "default",
      name: "Default",
    })
    expect(model.columns.map(({ status, position, hidden }) => ({ status, position, hidden }))).toEqual([
      { status: "ready", position: 20, hidden: false },
      { status: "todo", position: 10, hidden: false },
      { status: "done", position: 30, hidden: true },
    ])
    expect(model.tasksByStatus.get("ready")?.map(({ id }) => id)).toEqual(["t_ready"])
    expect(model.tasksByStatus.get("todo")?.map(({ id }) => id)).toEqual(["t_todo"])
    expect(model.tasksByStatus.get("done")).toEqual([])
    expect(fetcher).toHaveBeenCalledTimes(5)
    expect(fetcher.mock.calls.filter(([input]) => String(input).includes("/tasks/by-status")).map(([input]) => String(input))).toHaveLength(3)
    expect(fetcher.mock.calls.every(([, init]) => init?.credentials === "same-origin")).toBe(true)
  })

  test("fails closed when a status window disagrees with the requested server status", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) => {
      const url = String(input)
      if (url.includes("/api/v1/boards?")) return jsonResponse({ data: [board("b_default", "default", "Default")] })
      if (url.endsWith("/columns")) return jsonResponse({ data: [column("c_ready", "b_default", "ready", 10)] })
      return jsonResponse({ data: { statuses: [{ status: "todo", tasks: [], page: { limit: 1000, offset: 0, total: 0 } }] }, meta: { limit: 1000, offset: 0 } })
    })

    await expect(loadBoardReadModel(runtime, "default", { fetcher })).rejects.toMatchObject({
      name: "BoardReadError",
      kind: "anomaly",
    })
  })

  test("fails closed when canonical columns repeat a status", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) => {
      const url = String(input)
      if (url.includes("/api/v1/boards?")) return jsonResponse({ data: [board("b_default", "default", "Default")] })
      return jsonResponse({
        data: [
          column("c_ready", "b_default", "ready", 10),
          column("c_ready_hidden", "b_default", "ready", 20, true),
        ],
      })
    })

    await expect(loadBoardReadModel(runtime, "default", { fetcher })).rejects.toMatchObject({
      name: "BoardReadError",
      kind: "anomaly",
    })
  })

  test("classifies network failures as offline without fallback data", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => {
      throw new TypeError("Failed to fetch")
    })

    await expect(loadBoardReadModel(runtime, "default", { fetcher })).rejects.toMatchObject({
      name: "BoardReadError",
      kind: "offline",
    })
  })

  test("reads every task page and rejects a total that is not fully covered", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) => {
      const url = new URL(String(input))
      if (url.pathname === "/api/v1/boards") return jsonResponse({ data: [board("b_default", "default", "Default")] })
      if (url.pathname.endsWith("/columns")) return jsonResponse({ data: [column("c_ready", "b_default", "ready", 10)] })
      const offset = Number(url.searchParams.get("offset"))
      const page = offset === 0
        ? { tasks: [task("t_0", "b_default", "default", "ready", 0)], total: 1001 }
        : { tasks: [task("t_1000", "b_default", "default", "ready", 1000)], total: 1001 }
      return jsonResponse({ data: { statuses: [{ status: "ready", tasks: page.tasks, page: { limit: 1000, offset, total: page.total } }] }, meta: { limit: 1000, offset } })
    })

    await expect(loadBoardReadModel(runtime, "default", { fetcher })).rejects.toMatchObject({
      name: "BoardReadError",
      kind: "anomaly",
    })
    expect(fetcher.mock.calls.filter(([input]) => String(input).includes("/tasks/by-status"))).toHaveLength(2)
  })

  test("concatenates complete task pages without losing server order", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) => {
      const url = new URL(String(input))
      if (url.pathname === "/api/v1/boards") return jsonResponse({ data: [board("b_default", "default", "Default")] })
      if (url.pathname.endsWith("/columns")) return jsonResponse({ data: [column("c_ready", "b_default", "ready", 10)] })
      const offset = Number(url.searchParams.get("offset"))
      const tasks = offset === 0
        ? [task("t_0", "b_default", "default", "ready", 0), task("t_1", "b_default", "default", "ready", 1)]
        : [task("t_2", "b_default", "default", "ready", 2)]
      return jsonResponse({ data: { statuses: [{ status: "ready", tasks, page: { limit: 2, offset, total: 3 } }] }, meta: { limit: 2, offset } })
    })

    const model = await loadBoardReadModel(runtime, "default", { fetcher, taskLimit: 2 })

    expect(model.tasksByStatus.get("ready")?.map(({ id }) => id)).toEqual(["t_0", "t_1", "t_2"])
  })

  test("rejects cross-origin runtime API bases before issuing a request", async () => {
    const fetcher = vi.fn<typeof fetch>()
    const crossOriginRuntime = { ...runtime, apiBaseUrl: "https://evil.test/api" }

    await expect(
      loadBoardReadModel(crossOriginRuntime, "default", {
        fetcher,
        documentBaseURI: "https://kanban.test/app/",
      }),
    ).rejects.toMatchObject({ name: "BoardReadError", kind: "cross_origin" })
    expect(fetcher).not.toHaveBeenCalled()
  })

  test("uses include_archived only when explicitly requested", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) => {
      const url = new URL(String(input))
      if (url.pathname === "/api/v1/boards") return jsonResponse({ data: [board("b_default", "default", "Default")] })
      if (url.pathname.endsWith("/columns")) return jsonResponse({ data: [] })
      throw new Error(`unexpected URL ${url}`)
    })

    await loadBoardReadModel(runtime, "default", { fetcher, includeArchived: true })
    expect(String(fetcher.mock.calls[0]?.[0])).toContain("include_archived=true")
  })

  test("deduplicates pending loads and invalidation prevents a stale result from being cached", async () => {
    let resolveFirst: ((response: Response) => void) | undefined
    const firstResponse = new Promise<Response>((resolve) => { resolveFirst = resolve })
    const fetcher = vi.fn<typeof fetch>()
      .mockReturnValueOnce(firstResponse)
      .mockImplementation(async (input) => routeResponse(String(input)))
    const query = createBoardReadQuery(runtime, "default", { fetcher })

    const first = query.load()
    expect(query.load()).toBe(first)
    query.invalidate()
    const second = query.load()
    expect(fetcher).toHaveBeenCalledTimes(2)
    resolveFirst?.(routeResponse("http://127.0.0.1/api/v1/boards"))
    await expect(second).resolves.toMatchObject({ identity: { canonicalBoardId: "b_default" } })
    await expect(first).resolves.toMatchObject({ identity: { canonicalBoardId: "b_default" } })
    await expect(query.load()).resolves.toMatchObject({ identity: { canonicalBoardId: "b_default" } })
  })

  test("exposes an empty-board error for an exact selector miss", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => jsonResponse({ data: [board("b_other", "other", "Other")] }))

    await expect(loadBoardReadModel(runtime, "default", { fetcher })).rejects.toMatchObject({
      name: "BoardReadError",
      kind: "empty",
      reason: "board-not-found",
    } satisfies Partial<BoardReadError>)
  })
})
