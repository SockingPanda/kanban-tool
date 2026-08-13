import { describe, expect, test } from "vitest"

import {
  BOARD_EVENTS_PAGE_LIMIT,
  buildBoardEventsRequest,
  buildTaskListRequest,
  buildTaskInspectorRequests,
  buildTaskMapRequest,
  loadTaskMap,
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
import type { ApiBoardTaskMapResponseContract } from "./generated/contracts/api-board-task-map-response"
import type { WebRuntimeConfig } from "../runtime"
import { assertCanonicalBoardSlug } from "../board-slug"
import { asCanonicalBoardId } from "../sync/contracts"
import type { HttpTransportResponse } from "./http-transport"
import { HttpTransportError } from "./http-transport"
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

type MapTask = ApiBoardTaskMapResponseContract["data"]["nodes"][number]["task"]

function mapTask(id: string, boardId = "b_default", boardSlug = "default"): MapTask {
  return {
    id,
    board_id: boardId,
    board_slug: boardSlug,
    ref: `${boardSlug}#${id}`,
    seq: 1,
    title: id,
    description: "",
    status: "ready",
    status_reason: null,
    assignee: null,
    priority: 1,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "test",
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
    result: null,
    metadata: {},
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "planned",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
  }
}

function mapResponse(overrides: Partial<ApiBoardTaskMapResponseContract["data"]> = {}): ApiBoardTaskMapResponseContract {
  const nodes = [{ task: mapTask("t_1"), role: "active" as const, context_only: false }]
  const edges: ApiBoardTaskMapResponseContract["data"]["edges"] = []
  return {
    data: {
      nodes,
      edges,
      meta: {
        depth: 0,
        context_depth: 1,
        generated_at: 1,
        node_count: nodes.length,
        edge_count: edges.length,
        truncated: false,
        active_statuses: ["ready"],
        active_only: true,
        include_done_context: false,
        include_archived_context: false,
        hide_isolated: false,
        limit_nodes: 240,
      },
      ...overrides,
    },
  }
}

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
      labels: "/api/v1/tasks/t_1/labels",
      neighborhood: "/api/v1/tasks/t_1/neighborhood?depth=1&include_archived_context=false&limit_nodes=40",
      dependencies: "/api/v1/tasks/t_1/dependencies",
      steps: "/api/v1/tasks/t_1/steps",
      runs: "/api/v1/tasks/t_1/runs",
      comments: "/api/v1/tasks/t_1/comments",
      attachments: "/api/v1/tasks/t_1/attachments",
      events: "/api/v1/events?board=default&task_id=t_1&after=0&limit=50",
    })
  })

  test("hydrates inspector labels from the canonical task-labels read", async () => {
    const paths: string[] = []
    const labels = [{ id: "l_release", board_id: "b_default", name: "Stage09 release", color: "#4F46E5", created_at: 1, updated_at: 1 }]
    const dependencyTask = { id: "t_1", board_id: "b_default", board_slug: "default", ref: "default#1", title: "t_1", status: "ready" as const }
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }] }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1") return { payload: { data: mapTask("t_1") }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/labels") return { payload: { data: labels }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/dependencies") return { payload: { data: { task: dependencyTask, parents: [], children: [], edges: [] } }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/steps") return { payload: { data: { task_id: "t_1", steps: [], execution_plan: { board_id: "b_default", task_id: "t_1", state: "unplanned", reason: null, updated_by: "test", updated_at: 1 } } }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/comments") return { payload: { data: [] }, bytes: 1 }
        throw new Error(`unexpected inspector path: ${path}`)
      },
    }

    const result = await loadTaskInspector(runtime, "default", "t_1", {
      transport,
      includeNeighborhood: false,
      includeRuns: false,
      includeEvents: false,
      includeAttachments: false,
    })

    expect(result.task.labels).toEqual(labels)
    expect(paths).toContain("/api/v1/tasks/t_1/labels")
  })

  test("rejects inspector labels that cross the canonical board scope", async () => {
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }] }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1") return { payload: { data: mapTask("t_1") }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/labels") return { payload: { data: [{ id: "l_other", board_id: "b_other", name: "foreign", color: null, created_at: 1, updated_at: 1 }] }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/dependencies") return { payload: { data: { task: { id: "t_1", board_id: "b_default", board_slug: "default", ref: "default#1", title: "t_1", status: "ready" }, parents: [], children: [], edges: [] } }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/steps") return { payload: { data: { task_id: "t_1", steps: [], execution_plan: { board_id: "b_default", task_id: "t_1", state: "unplanned", reason: null, updated_by: "test", updated_at: 1 } } }, bytes: 1 }
        if (path === "/api/v1/tasks/t_1/comments") return { payload: { data: [] }, bytes: 1 }
        throw new Error(`unexpected inspector path: ${path}`)
      },
    }

    await expect(loadTaskInspector(runtime, "default", "t_1", {
      transport,
      includeNeighborhood: false,
      includeRuns: false,
      includeEvents: false,
      includeAttachments: false,
    })).rejects.toMatchObject({ name: "ExplorerReadError", kind: "anomaly" })
  })

  test("rejects non-canonical task deep links before building requests", () => {
    expect(() => buildTaskInspectorRequests("default", "task-1")).toThrow(/canonical t_ identity/)
    expect(() => buildTaskRunsRequest("task-1")).toThrow(/canonical t_ identity/)
    expect(() => buildBoardEventsRequest("default", "task-1")).toThrow(/canonical t_ identity/)
    expect(() => buildRunLogRequest("run-1")).toThrow(/canonical r_ identity/)
  })

  test("rejects invalid task deep links locally without a board request", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        return { payload: { data: [] }, bytes: 1 }
      },
    }

    await expect(loadTaskInspector(runtime, "default", "task-1", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "anomaly",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
    await expect(loadBoardEvents(runtime, "default", { transport, taskId: "task-1" })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "anomaly",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
    expect(paths).toEqual([])
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

  test("fails closed with a typed task-not-found reason for a cross-board task response", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        if (path.startsWith("/api/v1/boards?")) {
          return {
            payload: {
              data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }],
            },
            bytes: 1,
          }
        }
        if (path === "/api/v1/tasks/t_other") {
          return { payload: { data: mapTask("t_other", "b_other", "other"), meta: null }, bytes: 1 }
        }
        throw new Error(`cross-board task response must not fetch child detail: ${path}`)
      },
    }

    await expect(loadTaskInspector(runtime, "default", "t_other", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "anomaly",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
    expect(paths).toEqual([
      "/api/v1/boards?include_archived=false",
      "/api/v1/tasks/t_other",
    ])
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

  test("rejects non-canonical board ids and C1 controls", async () => {
    const noPrefix = { get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board("default", "default")] }, bytes: 1 }) }
    const c1 = { get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board(`b_bad\u0085`, "default")] }, bytes: 1 }) }

    await expect(loadExplorerBoardIdentity(runtime, "default", { transport: noPrefix })).rejects.toMatchObject({ kind: "anomaly" })
    await expect(loadExplorerBoardIdentity(runtime, "default", { transport: c1 })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("fails closed for cross-type selector collisions", async () => {
    const transport = {
      get: async (): Promise<HttpTransportResponse> => ({ payload: { data: [board("b_one", "one"), board("b_two", "b_one")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "b_one", { transport })).rejects.toMatchObject({ kind: "anomaly" })
  })
})

describe("explorer task map response boundary", () => {
  const identity = {
    selector: "default",
    id: asCanonicalBoardId("b_default"),
    slug: assertCanonicalBoardSlug("default"),
    name: "Default",
  }

  test("uses the parsed board identity without refetching the board list", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        return { payload: mapResponse(), bytes: 1 }
      },
    }

    await expect(loadTaskMap(runtime, "default", {
      transport,
      boardIdentity: identity,
      activeOnly: true,
      contextDepth: 1,
      includeDoneContext: false,
      includeArchivedContext: false,
      hideIsolated: false,
      limitNodes: 240,
    })).resolves.toMatchObject({ board: identity })
    expect(paths).toEqual(["/api/v1/boards/default/task-map?active_only=true&context_depth=1&include_done_context=false&include_archived_context=false&hide_isolated=false&limit_nodes=240"])
  })

  test("rejects a parsed identity that does not belong to the route selector", async () => {
    const transport = { get: async (): Promise<HttpTransportResponse> => ({ payload: mapResponse(), bytes: 1 }) }

    await expect(loadTaskMap(runtime, "other", { transport, boardIdentity: identity, limitNodes: 240 })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("rejects dangling edge endpoints and inconsistent map metadata", async () => {
    const dangling = mapResponse({
      edges: [{ id: "e_1", source_task_id: "t_1", target_task_id: "t_missing", kind: "dependency", required: true, blocking: true }],
    })
    const mismatch = mapResponse({ meta: { ...mapResponse().data.meta, node_count: 2 } })
    const overLimitNodes = Array.from({ length: 241 }, (_, index) => ({ task: mapTask(`t_${index}`), role: "active" as const, context_only: false }))
    const overLimit = mapResponse({ nodes: overLimitNodes, meta: { ...mapResponse().data.meta, node_count: overLimitNodes.length } })
    const transport = (payload: ApiBoardTaskMapResponseContract) => ({
      get: async (): Promise<HttpTransportResponse> => ({ payload, bytes: 1 }),
    })

    await expect(loadTaskMap(runtime, "default", { transport: transport(dangling), boardIdentity: identity, limitNodes: 240 })).rejects.toMatchObject({ kind: "anomaly" })
    await expect(loadTaskMap(runtime, "default", { transport: transport(mismatch), boardIdentity: identity, limitNodes: 240 })).rejects.toMatchObject({ kind: "anomaly" })
    await expect(loadTaskMap(runtime, "default", { transport: transport(overLimit), boardIdentity: identity, limitNodes: 240 })).rejects.toMatchObject({ kind: "anomaly" })
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

  test("starts a catch-up read from the supplied cursor", async () => {
    const paths: string[] = []
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        paths.push(path)
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [board()] }, bytes: 1 }
        expect(new URLSearchParams(path.split("?", 2)[1]).get("after")).toBe("42")
        return { payload: { data: [event(43)], meta: { next_after: 43 } }, bytes: 1 }
      },
    }
    await expect(loadBoardEvents(runtime, "default", { transport, after: 42 })).resolves.toMatchObject({ meta: { nextAfter: 43 } })
    expect(paths.filter((path) => path.includes("/api/v1/events?")).length).toBe(1)
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
    expect(() => mergeBoardEvents([], [event(0)], boardId)).toThrow(/id 必须严格递增/)
  })

  test("reuses frozen event identities across incremental merges", () => {
    const boardId = asCanonicalBoardId("b_default")
    const first = mergeBoardEvents([], [event(1), event(2)], boardId)
    const second = mergeBoardEvents(first, [event(3)], boardId)

    expect(second[0]).toBe(first[0])
    expect(second[1]).toBe(first[1])
    expect(second[2]).not.toBe(first[1])
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

  test("fails fast when an ignored transport resolves after abort", async () => {
    const controller = new AbortController()
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) return { payload: { data: [board()] }, bytes: 1 }
        controller.abort()
        return { payload: { data: [event(1)], meta: { next_after: 1 } }, bytes: 1 }
      },
    }

    await expect(loadBoardEvents(runtime, "default", { transport, signal: controller.signal })).rejects.toMatchObject({ name: "AbortError" })
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
