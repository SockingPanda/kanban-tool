import { describe, expect, test } from "vitest"

import {
  BOARD_EVENTS_PAGE_LIMIT,
  buildBoardEventsRequest,
  buildTaskListRequest,
  buildTaskInspectorRequests,
  buildTaskMapRequest,
  defaultTaskListQuery,
  ExplorerReadError,
  buildRunLogRequest,
  buildTaskRunsRequest,
  loadTaskRuns,
  loadBoardEvents,
  loadExplorerBoardIdentity,
  loadTaskInspector,
  mergeBoardEvents,
  parseTaskListQuery,
  serializeTaskListQuery,
} from "./explorer-read-model"
import type { WebRuntimeConfig } from "../runtime"
import type { HttpTransportResponse } from "./http-transport"
import { HttpTransportError } from "./http-transport"
import { asCanonicalBoardId } from "../sync/contracts"
import type { ApiListEventsResponseContract } from "./generated/contracts/api-list-events-response"

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "test",
} satisfies WebRuntimeConfig

describe("explorer task list URL state", () => {
  test("parses and serializes all list controls without losing them", () => {
    const query = parseTaskListQuery(
      "?status=ready&status=running&priority=1&priority=3&plan=has_steps&plan=incomplete_required_steps&q=needle&sort=-updated_at&page=3&limit=50&include_archived=true",
    )

    expect(query).toEqual({
      ...defaultTaskListQuery,
      status: ["ready", "running"],
      priority: [1, 3],
      plan: ["has_steps", "incomplete_required_steps"],
      search: "needle",
      sort: "-updated_at",
      page: 3,
      limit: 50,
      includeArchived: true,
    })
    expect(serializeTaskListQuery(query)).toBe(
      "?status=ready&status=running&priority=1&priority=3&plan=has_steps&plan=incomplete_required_steps&q=needle&sort=-updated_at&page=3&limit=50&include_archived=true",
    )
  })

  test("normalizes unsafe values back to the generated query contract defaults", () => {
    expect(parseTaskListQuery("?status=unknown&priority=8&plan=unknown&page=-2&limit=9999&sort=wat&q=%00")).toEqual(defaultTaskListQuery)
  })

  test("builds an absolute same-origin request path through generated path/query parsers", () => {
    const request = buildTaskListRequest("default", {
      ...defaultTaskListQuery,
      status: ["ready"],
      search: "needle",
      page: 2,
      limit: 25,
    })

    expect(request).toBe(
      "/api/v1/boards/default/tasks?status=ready&q=needle&include_archived=false&limit=25&offset=25&sort=updated_at",
    )
  })

  test("builds task map and inspector reads from generated path/query contracts", () => {
    expect(buildTaskMapRequest("default", {
      activeOnly: true,
      contextDepth: 1,
      includeDoneContext: true,
      includeArchivedContext: false,
      hideIsolated: false,
      limitNodes: 240,
    })).toBe("/api/v1/boards/default/task-map?active_only=true&context_depth=1&include_done_context=true&include_archived_context=false&hide_isolated=false&limit_nodes=240")
    expect(buildTaskInspectorRequests("default", "t_1")).toEqual({
      task: "/api/v1/tasks/t_1",
      neighborhood: "/api/v1/tasks/t_1/neighborhood?depth=1&include_archived_context=false&limit_nodes=40",
      dependencies: "/api/v1/tasks/t_1/dependencies",
      steps: "/api/v1/tasks/t_1/steps",
      runs: "/api/v1/tasks/t_1/runs",
      comments: "/api/v1/tasks/t_1/comments",
      events: "/api/v1/events?board=default&task_id=t_1&after=0&limit=50",
    })
  })

  test("surfaces a typed task-not-found error before attempting detail children", async () => {
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) {
          return {
            payload: {
              data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }],
            },
            bytes: 1,
          }
        }
        throw new HttpTransportError("http", "task missing", { status: 404 })
      },
    }

    await expect(loadTaskInspector(runtime, "default", "t_missing", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "http",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
  })
})

describe("explorer canonical board identity", () => {
  const board = (id: string, slug: string) => ({
    id,
    slug,
    name: "Board",
    description: null,
    created_at: 1,
    updated_at: 1,
    archived_at: null,
  })

  test("rejects a server slug that only becomes valid after trimming", async () => {
    const transport = {
      get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board("b_default", " default ")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "default", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "anomaly",
    })
  })

  test("rejects path-like server identity values before any board request path is built", async () => {
    const transport = {
      get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board("b_/escape", "../escape")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "../escape", { transport })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("rejects duplicate canonical board id or slug before selecting a board", async () => {
    const duplicateSlugTransport = {
      get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board("b_one", "same"), board("b_two", "same")] }, bytes: 1 }),
    }
    const duplicateIdTransport = {
      get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board("b_same", "one"), board("b_same", "two")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "same", { transport: duplicateSlugTransport })).rejects.toMatchObject({ kind: "anomaly" })
    await expect(loadExplorerBoardIdentity(runtime, "one", { transport: duplicateIdTransport })).rejects.toMatchObject({ kind: "anomaly" })
  })
})

describe("board events read model", () => {
  const board = (id = "b_default", slug = "default") => ({
    id,
    slug,
    name: "Board",
    description: null,
    created_at: 1,
    updated_at: 1,
    archived_at: null,
  })
  const event = (id: number, boardId = "b_default", eventId = `event-${id}`): ApiListEventsResponseContract["data"][number] => ({
    id,
    event_id: eventId,
    board_id: boardId,
    task_id: null,
    run_id: null,
    kind: "board.archived",
    actor: "tester",
    payload: {},
    created_at: 1_700_000_000 + id,
  })

  test("builds the first board page with ASC cursor order and encoded selector", () => {
    expect(buildBoardEventsRequest("board slug")).toBe("/api/v1/events?board=board+slug&after=0&limit=150")
    expect(buildBoardEventsRequest("default", "t_1")).toBe("/api/v1/events?board=default&task_id=t_1&after=0&limit=150")
    expect(buildBoardEventsRequest("default", null, 150)).toBe("/api/v1/events?board=default&after=150&limit=150")
    expect(BOARD_EVENTS_PAGE_LIMIT).toBe(150)
  })

  test("resolves canonical board identity and rejects foreign or non-ascending events", async () => {
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [board()] }, bytes: 1 }
        return { payload: { data: [event(1), event(2)], meta: { next_after: 2 } }, bytes: 1 }
      },
    }
    await expect(loadBoardEvents(runtime, "default", { transport })).resolves.toMatchObject({
      board: { id: "b_default", slug: "default" },
      taskId: null,
      events: [{ id: 1 }, { id: 2 }],
      meta: { count: 2, nextAfter: 2, limit: 150 },
    })

    const foreign = {
      get: async (path: string): Promise<HttpTransportResponse> => path.startsWith("/api/v1/boards?")
        ? { payload: { data: [board()] }, bytes: 1 }
        : { payload: { data: [event(1, "b_other")], meta: { next_after: 1 } }, bytes: 1 },
    }
    await expect(loadBoardEvents(runtime, "default", { transport: foreign })).rejects.toMatchObject({ kind: "anomaly" })

    const outOfOrder = {
      get: async (path: string): Promise<HttpTransportResponse> => path.startsWith("/api/v1/boards?")
        ? { payload: { data: [board()] }, bytes: 1 }
        : { payload: { data: [event(2), event(1)], meta: { next_after: 1 } }, bytes: 1 },
    }
    await expect(loadBoardEvents(runtime, "default", { transport: outOfOrder })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("merges by numeric id and event_id and retains only the final 150 events", () => {
    const first = Array.from({ length: 150 }, (_, index) => event(index + 1))
    const boardId = asCanonicalBoardId("b_default")
    const merged = mergeBoardEvents(first, [event(150), event(151)], boardId)
    expect(merged).toHaveLength(150)
    expect(merged[0]?.id).toBe(2)
    expect(merged.at(-1)?.id).toBe(151)
    expect(() => mergeBoardEvents(first, [event(151, "b_other")], boardId)).toThrow(/board scope/)
    expect(() => mergeBoardEvents(first, [event(151, "b_default", "event-1")], boardId)).toThrow(/多个数字 id/)
  })

  test("walks ASC pages to expose the newest 150 events", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [board()] }, bytes: 1 }
        const after = Number(new URLSearchParams(path.split("?", 2)[1]).get("after"))
        if (after === 0) {
          return { payload: { data: Array.from({ length: 150 }, (_, index) => event(index + 1)), meta: { next_after: 150 } }, bytes: 1 }
        }
        expect(after).toBe(150)
        return { payload: { data: Array.from({ length: 30 }, (_, index) => event(index + 151)), meta: { next_after: 180 } }, bytes: 1 }
      },
    }

    const result = await loadBoardEvents(runtime, "default", { transport })
    expect(result.events).toHaveLength(150)
    expect(result.events[0]?.id).toBe(31)
    expect(result.events.at(-1)?.id).toBe(180)
    expect(result.meta.nextAfter).toBe(180)
    expect(paths).toHaveLength(3)
  })

  test("rejects any task event that crosses the requested task scope", async () => {
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => path.startsWith("/api/v1/boards?")
        ? { payload: { data: [board()] }, bytes: 1 }
        : {
            payload: {
              data: [
                { ...event(1, "b_default", "event-1"), task_id: "t_1" },
                { ...event(2, "b_default", "event-2"), task_id: "t_other" },
              ],
              meta: { next_after: 2 },
            },
            bytes: 1,
          },
    }

    await expect(loadBoardEvents(runtime, "default", { transport, taskId: "t_1" })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("rejects a cursor that lags the page or repeats after pagination", async () => {
    const lagging = {
      get: async (path: string): Promise<HttpTransportResponse> => path.startsWith("/api/v1/boards?")
        ? { payload: { data: [board()] }, bytes: 1 }
        : { payload: { data: Array.from({ length: 150 }, (_, index) => event(index + 1)), meta: { next_after: 149 } }, bytes: 1 },
    }
    await expect(loadBoardEvents(runtime, "default", { transport: lagging })).rejects.toMatchObject({ kind: "anomaly" })

    const repeated = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [board()] }, bytes: 1 }
        const after = Number(new URLSearchParams(path.split("?", 2)[1]).get("after"))
        return after === 0
          ? { payload: { data: Array.from({ length: 150 }, (_, index) => event(index + 1)), meta: { next_after: 150 } }, bytes: 1 }
          : { payload: { data: [event(150)], meta: { next_after: 150 } }, bytes: 1 }
      },
    }
    await expect(loadBoardEvents(runtime, "default", { transport: repeated })).rejects.toMatchObject({ kind: "anomaly" })
  })
})

describe("explorer task runs read", () => {
  const run = (id: string, taskId = "t_1", hasLog = false) => ({
    id,
    task_id: taskId,
    status: "succeeded" as const,
    worker_profile: "worker",
    worker_pid: null,
    claim_owner: "owner",
    started_at: 1,
    finished_at: 2,
    exit_code: 0,
    summary: "done",
    error: null,
    has_log: hasLog,
    metadata: {},
  })

  test("builds generated run and log paths", () => {
    expect(buildTaskRunsRequest("t_1")).toBe("/api/v1/tasks/t_1/runs")
    expect(buildRunLogRequest("r_1")).toBe("/api/v1/runs/r_1/log")
  })

  test("loads runs and fetches the first run with has_log only", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        if (path === "/api/v1/tasks/t_1/runs") return { payload: { data: [run("r_no_log"), run("r_first", "t_1", true), run("r_second", "t_1", true)] }, bytes: 1 }
        if (path === "/api/v1/runs/r_first/log") return { payload: { data: { run_id: "r_first", content: "hello", truncated: false } }, bytes: 1 }
        throw new Error(`unexpected path: ${path}`)
      },
    }

    await expect(loadTaskRuns(runtime, "t_1", { transport })).resolves.toMatchObject({
      taskId: "t_1",
      selectedRunId: "r_first",
      log: { run_id: "r_first", content: "hello", truncated: false },
    })
    expect(paths).toEqual(["/api/v1/tasks/t_1/runs", "/api/v1/runs/r_first/log"])
  })

  test("keeps no-log state without requesting a log endpoint", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        return { payload: { data: [run("r_no_log")] }, bytes: 1 }
      },
    }

    await expect(loadTaskRuns(runtime, "t_1", { transport })).resolves.toMatchObject({ taskId: "t_1", selectedRunId: null, log: null })
    expect(paths).toEqual(["/api/v1/tasks/t_1/runs"])
  })

  test("fails closed when a run belongs to another task", async () => {
    const transport = {
      get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [run("r_cross", "t_other", true)] }, bytes: 1 }),
    }

    await expect(loadTaskRuns(runtime, "t_1", { transport })).rejects.toMatchObject({ name: "ExplorerReadError", kind: "anomaly" })
  })
})
