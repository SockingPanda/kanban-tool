import { parseTaskListQuery } from "../../application/data/explorer-read-model";
import { describe, expect, test, vi } from "vitest"

import { buildTaskListRequest, buildTaskInspectorRequests, buildTaskMapRequest, defaultTaskListQuery, ExplorerReadError, buildRunLogRequest, buildTaskRunsRequest, serializeTaskListQuery } from "../../application/data/explorer-read-model";
import { loadTaskMap, loadTaskRuns, loadBoardEvents, loadExplorerBoardIdentity, loadTaskInspector } from "./explorer-read-model";
import type { ApiBoardTaskMapResponseContract } from "../../lib/api/generated/contracts/api-board-task-map-response"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { assertCanonicalBoardSlug } from "../../domain/board-slug"
import { asCanonicalBoardId } from "../../domain/board-id"
import type { RpcTransportResponse, RpcCall } from "../../application/data/rpc-transport";
import { RpcTransportError } from "../../application/data/rpc-transport";
import type { ApiListEventsResponseContract } from "../../lib/api/generated/contracts/api-list-events-response"

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v2",
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

  test("builds a named RPC request through generated path/query parsers", () => {
    const request = buildTaskListRequest("default", {
      ...defaultTaskListQuery,
      status: ["ready"],
      search: "needle",
      page: 2,
      limit: 25,
    })

    expect(request).toEqual(
      { method: "ListTasks", path: { board: "default" }, query: { status: ["ready"], priority: [], label: [], plan_filter: [], assignee: null, q: "needle", include_archived: false, limit: 25, offset: 25, sort: "updated_at" } },
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
    })).toEqual({ method: "BoardTaskMap", path: { board: "default" }, query: { active_only: true, context_depth: 1, include_done_context: true, include_archived_context: false, hide_isolated: false, limit_nodes: 240 } })
    expect(buildTaskInspectorRequests("t_1")).toEqual({
      task: { method: "GetTask", path: { task_id: "t_1" } },
      labels: { method: "ListTaskLabels", path: { task_id: "t_1" } },
      neighborhood: { method: "TaskNeighborhood", path: { task_id: "t_1" }, query: { depth: 1, include_archived_context: false, limit_nodes: 40 } },
      dependencies: { method: "ListDependencies", path: { task_id: "t_1" } },
      steps: { method: "ListSteps", path: { task_id: "t_1" } },
      runs: { method: "ListRuns", path: { task_id: "t_1" } },
      comments: { method: "ListComments", path: { task_id: "t_1" } },
      attachments: { method: "ListAttachments", path: { task_id: "t_1" } },
    })
  })

  test("hydrates inspector labels from the canonical task-labels read", async () => {
    const requests: RpcCall[] = []
    const labels = [{ id: "l_release", board_id: "b_default", name: "Stage09 release", color: "#4F46E5", created_at: 1, updated_at: 1 }]
    const dependencyTask = { id: "t_1", board_id: "b_default", board_slug: "default", ref: "default#1", title: "t_1", status: "ready" as const }
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        requests.push(request)
        if (request.method === "ListBoards") return { payload: { data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }] }, bytes: 1 }
        if (request.method === "GetTask") return { payload: { data: mapTask("t_1") }, bytes: 1 }
        if (request.method === "ListTaskLabels") return { payload: { data: labels }, bytes: 1 }
        if (request.method === "ListDependencies") return { payload: { data: { task: dependencyTask, parents: [], children: [], edges: [] } }, bytes: 1 }
        if (request.method === "ListSteps") return { payload: { data: { task_id: "t_1", steps: [], execution_plan: { board_id: "b_default", task_id: "t_1", state: "unplanned", reason: null, updated_by: "test", updated_at: 1 } } }, bytes: 1 }
        if (request.method === "ListComments") return { payload: { data: [] }, bytes: 1 }
        throw new Error(`unexpected inspector RPC: ${request.method}`)
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
    expect(requests).toContainEqual(expect.objectContaining({ method: "ListTaskLabels", path: { task_id: "t_1" } }))
  })

  test("rejects inspector labels that cross the canonical board scope", async () => {
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        if (request.method === "ListBoards") return { payload: { data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }] }, bytes: 1 }
        if (request.method === "GetTask") return { payload: { data: mapTask("t_1") }, bytes: 1 }
        if (request.method === "ListTaskLabels") return { payload: { data: [{ id: "l_other", board_id: "b_other", name: "foreign", color: null, created_at: 1, updated_at: 1 }] }, bytes: 1 }
        if (request.method === "ListDependencies") return { payload: { data: { task: { id: "t_1", board_id: "b_default", board_slug: "default", ref: "default#1", title: "t_1", status: "ready" }, parents: [], children: [], edges: [] } }, bytes: 1 }
        if (request.method === "ListSteps") return { payload: { data: { task_id: "t_1", steps: [], execution_plan: { board_id: "b_default", task_id: "t_1", state: "unplanned", reason: null, updated_by: "test", updated_at: 1 } } }, bytes: 1 }
        if (request.method === "ListComments") return { payload: { data: [] }, bytes: 1 }
        throw new Error(`unexpected inspector RPC: ${request.method}`)
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
    expect(() => buildTaskInspectorRequests("task-1")).toThrow(/canonical t_ identity/)
    expect(() => buildTaskRunsRequest("task-1")).toThrow(/canonical t_ identity/)
    expect(() => buildRunLogRequest("run-1")).toThrow(/canonical r_ identity/)
  })

  test("rejects invalid task deep links locally without a board request", async () => {
    const requests: RpcCall[] = []
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        requests.push(request)
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
    expect(requests).toEqual([])
  })

  test("surfaces a typed task-not-found error before attempting detail children", async () => {
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        if (request.method === "ListBoards") {
          return {
            payload: {
              data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }],
            },
            bytes: 1,
          }
        }
        throw new RpcTransportError("http", "task missing", { status: 404 })
      },
    }

    await expect(loadTaskInspector(runtime, "default", "t_missing", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "http",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
  })

  test("fails closed with a typed task-not-found reason for a cross-board task response", async () => {
    const requests: RpcCall[] = []
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        requests.push(request)
        if (request.method === "ListBoards") {
          return {
            payload: {
              data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }],
            },
            bytes: 1,
          }
        }
        if (request.method === "GetTask") {
          return { payload: { data: mapTask("t_other", "b_other", "other"), meta: null }, bytes: 1 }
        }
        throw new Error(`cross-board task response must not fetch child detail: ${request.method}`)
      },
    }

    await expect(loadTaskInspector(runtime, "default", "t_other", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "anomaly",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
    expect(requests).toEqual([
      expect.objectContaining({ method: "ListBoards", query: { include_archived: false } }),
      expect.objectContaining({ method: "GetTask", path: { task_id: "t_other" } }),
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
      call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board("b_default", " default ")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "default", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "anomaly",
    })
  })

  test("rejects path-like server identity values before any board request path is built", async () => {
    const transport = {
      call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board("b_/escape", "../escape")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "../escape", { transport })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("rejects duplicate canonical board id or slug before selecting a board", async () => {
    const duplicateSlugTransport = {
      call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board("b_one", "same"), board("b_two", "same")] }, bytes: 1 }),
    }
    const duplicateIdTransport = {
      call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board("b_same", "one"), board("b_same", "two")] }, bytes: 1 }),
    }

    await expect(loadExplorerBoardIdentity(runtime, "same", { transport: duplicateSlugTransport })).rejects.toMatchObject({ kind: "anomaly" })
    await expect(loadExplorerBoardIdentity(runtime, "one", { transport: duplicateIdTransport })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("rejects non-canonical board ids and C1 controls", async () => {
    const noPrefix = { call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board("default", "default")] }, bytes: 1 }) }
    const c1 = { call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board(`b_bad\u0085`, "default")] }, bytes: 1 }) }

    await expect(loadExplorerBoardIdentity(runtime, "default", { transport: noPrefix })).rejects.toMatchObject({ kind: "anomaly" })
    await expect(loadExplorerBoardIdentity(runtime, "default", { transport: c1 })).rejects.toMatchObject({ kind: "anomaly" })
  })

  test("fails closed for cross-type selector collisions", async () => {
    const transport = {
      call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [board("b_one", "one"), board("b_two", "b_one")] }, bytes: 1 }),
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
    const requests: RpcCall[] = []
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        requests.push(request)
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
    expect(requests).toEqual([expect.objectContaining({ method: "BoardTaskMap", path: { board: "default" }, query: { active_only: true, context_depth: 1, include_done_context: false, include_archived_context: false, hide_isolated: false, limit_nodes: 240 } })])
  })

  test("rejects a parsed identity that does not belong to the route selector", async () => {
    const transport = { call: async (): Promise<RpcTransportResponse> => ({ payload: mapResponse(), bytes: 1 }) }

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
      call: async (): Promise<RpcTransportResponse> => ({ payload, bytes: 1 }),
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

  function queryTransport(events: ApiListEventsResponseContract['data'], nextAfter = events.at(-1)?.id ?? 0) {
    return {
      call: vi.fn(async (request: RpcCall): Promise<RpcTransportResponse> => {
        expect(request.method).toBe('ListBoards')
        return { payload: { data: [board()] }, bytes: 1 }
      }),
      recentEvents: vi.fn(async (): Promise<RpcTransportResponse> => ({ payload: { data: events, meta: { next_after: nextAfter } }, bytes: 1 })),
    }
  }

  test('请求 canonical board 的完整最近窗口，仅订阅有界 RecentEvents', async () => {
    const transport = queryTransport([event(1), event(2)])
    await expect(loadBoardEvents(runtime, 'default', { transport })).resolves.toMatchObject({
      board: { id: 'b_default', slug: 'default' }, taskId: null, events: [{ id: 1 }, { id: 2 }], meta: { count: 2, nextAfter: 2, limit: 150 },
    })
    expect(transport.recentEvents).toHaveBeenCalledExactlyOnceWith({ board: 'b_default', limit: 150 }, undefined)
    expect(transport.call).toHaveBeenCalledOnce()
  })

  test('服务端给出最新 150 条后不再向前翻页', async () => {
    const transport = queryTransport(Array.from({ length: 150 }, (_, index) => event(index + 1_000_000)))
    const result = await loadBoardEvents(runtime, 'default', { transport })
    expect(result.events).toHaveLength(150)
    expect(result.events[0]?.id).toBe(1_000_000)
    expect(result.events.at(-1)?.id).toBe(1_000_149)
    expect(result.meta.nextAfter).toBe(1_000_149)
    expect(transport.call).toHaveBeenCalledOnce()
    expect(transport.recentEvents).toHaveBeenCalledOnce()
  })

  test('完整窗口拒绝跨看板、倒序、重复 ID 和 event_id', async () => {
    for (const events of [[event(1, 'b_other')], [event(2), event(1)], [event(1), event(1)], [event(1), event(2, 'b_default', 'event-1')]]) {
      await expect(loadBoardEvents(runtime, 'default', { transport: queryTransport(events) })).rejects.toMatchObject({ kind: 'anomaly' })
    }
  })

  test('任务筛选传入当前 task scope 并拒绝混入其他任务', async () => {
    const valid = queryTransport([{ ...event(1), task_id: 't_1' }])
    await loadBoardEvents(runtime, 'default', { transport: valid, taskId: 't_1' })
    expect(valid.recentEvents).toHaveBeenCalledExactlyOnceWith({ board: 'b_default', task_id: 't_1', limit: 150 }, undefined)
    const foreign = queryTransport([{ ...event(1), task_id: 't_1' }, { ...event(2), task_id: 't_other' }])
    await expect(loadBoardEvents(runtime, 'default', { transport: foreign, taskId: 't_1' })).rejects.toMatchObject({ kind: 'anomaly' })
  })

  test('拒绝超过窗口、落后于内容或空窗口前进的审计游标', async () => {
    for (const transport of [queryTransport([event(1), event(2)], 1), queryTransport([], 3), queryTransport(Array.from({ length: 151 }, (_, index) => event(index + 1)))]) {
      await expect(loadBoardEvents(runtime, 'default', { transport })).rejects.toMatchObject({ kind: 'anomaly' })
    }
  })

  test('迟到查询结果不越过已中止的读取', async () => {
    const controller = new AbortController(), transport = queryTransport([event(1)])
    transport.recentEvents.mockImplementationOnce(async () => {
      controller.abort()
      return { payload: { data: [event(1)], meta: { next_after: 1 } }, bytes: 1 }
    })
    await expect(loadBoardEvents(runtime, 'default', { transport, signal: controller.signal })).rejects.toMatchObject({ name: 'AbortError' })
  })

  test('缺少 QueryService 能力时直接报错，不回退审计历史读取', async () => {
    const transport = queryTransport([])
    await expect(loadBoardEvents(runtime, 'default', { transport: { call: transport.call } })).rejects.toThrow('QueryService')
    expect(transport.call).toHaveBeenCalledOnce()
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
    expect(buildTaskRunsRequest("t_1")).toEqual({ method: "ListRuns", path: { task_id: "t_1" } })
    expect(buildRunLogRequest("r_1")).toEqual({ method: "GetRunLog", path: { run_id: "r_1" } })
  })

  test("loads runs and fetches the first run with has_log only", async () => {
    const requests: RpcCall[] = []
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        requests.push(request)
        if (request.method === "ListRuns") return { payload: { data: [run("r_no_log"), run("r_first", "t_1", true), run("r_second", "t_1", true)] }, bytes: 1 }
        if (request.method === "GetRunLog") return { payload: { data: { run_id: "r_first", content: "hello", truncated: false } }, bytes: 1 }
        throw new Error(`unexpected path: ${request.method}`)
      },
    }

    await expect(loadTaskRuns(runtime, "t_1", { transport })).resolves.toMatchObject({
      taskId: "t_1",
      selectedRunId: "r_first",
      log: { run_id: "r_first", content: "hello", truncated: false },
    })
    expect(requests).toEqual([{ method: "ListRuns", path: { task_id: "t_1" } }, { method: "GetRunLog", path: { run_id: "r_first" } }])
  })

  test("keeps no-log state without requesting a log endpoint", async () => {
    const requests: RpcCall[] = []
    const transport = {
      call: async (request: RpcCall): Promise<RpcTransportResponse> => {
        requests.push(request)
        return { payload: { data: [run("r_no_log")] }, bytes: 1 }
      },
    }

    await expect(loadTaskRuns(runtime, "t_1", { transport })).resolves.toMatchObject({ taskId: "t_1", selectedRunId: null, log: null })
    expect(requests).toEqual([{ method: "ListRuns", path: { task_id: "t_1" } }])
  })

  test("fails closed when a run belongs to another task", async () => {
    const transport = {
      call: async (): Promise<RpcTransportResponse> => ({ payload: { data: [run("r_cross", "t_other", true)] }, bytes: 1 }),
    }

    await expect(loadTaskRuns(runtime, "t_1", { transport })).rejects.toMatchObject({ name: "ExplorerReadError", kind: "anomaly" })
  })
})
