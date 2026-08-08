import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import type { ExplorerTaskMap, ExplorerTaskMapReadModel } from "../../lib/api/explorer-read-model"
import { assertCanonicalBoardSlug } from "../../lib/board-slug"
import { asCanonicalBoardId } from "../../lib/sync/contracts"
import { TaskMapPresentation, type TaskMapReadState } from "./TaskMapView"
import { __test, defaultTaskMapUrlState, fenceTaskMapReadModel, hasTaskMapSelection, parseTaskMapUrlState, serializeTaskMapUrlState } from "./TaskMapView.logic"

type MapTask = ExplorerTaskMap["nodes"][number]["task"]

function task(id: string, status: MapTask["status"], overrides: Partial<MapTask> = {}): MapTask {
  return {
    id,
    board_id: "b_1",
    board_slug: "default",
    ref: `default#${id}`,
    seq: 1,
    title: id,
    description: "",
    status,
    status_reason: null,
    assignee: null,
    priority: 2,
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
    ...overrides,
  }
}

const graph: ExplorerTaskMap = {
  nodes: [
    { task: task("ready", "ready"), role: "active", context_only: false },
    { task: task("done", "done"), role: "context", context_only: true },
    { task: task("blocked", "todo", { dependency_blocked: true }), role: "active", context_only: false },
  ],
  edges: [
    { id: "dep:done:ready", source_task_id: "done", target_task_id: "ready", kind: "dependency", required: true, blocking: false },
  ],
  meta: {
    depth: 1,
    context_depth: 1,
    generated_at: 1,
    node_count: 3,
    edge_count: 1,
    truncated: false,
    active_statuses: ["ready", "todo"],
    active_only: true,
    include_done_context: true,
    include_archived_context: false,
    hide_isolated: false,
    limit_nodes: 240,
  },
}

const model: ExplorerTaskMapReadModel = {
  board: { selector: "default", id: asCanonicalBoardId("b_1"), slug: assertCanonicalBoardSlug("default"), name: "Default" },
  map: graph,
}

const ready: TaskMapReadState = { data: model, loading: false, error: null }

describe("TaskMapView", () => {
  test("round-trips map controls and task selection through canonical URL state", () => {
    const state = parseTaskMapUrlState("?filter=ready&show_done=true&hide_isolated=true&zoom=1.3&task=t_ready")

    expect(state).toEqual({ filter: "ready", showDoneContext: true, hideIsolated: true, zoom: 1.3, taskId: "t_ready" })
    expect(serializeTaskMapUrlState(state)).toBe("filter=ready&show_done=true&hide_isolated=true&zoom=1.3&task=t_ready")
    expect(serializeTaskMapUrlState({ ...state, taskId: null })).toBe("filter=ready&show_done=true&hide_isolated=true&zoom=1.3")
  })

  test("fails safe and canonicalizes unknown map query values", () => {
    expect(parseTaskMapUrlState("?filter=wat&show_done=yes&hide_isolated=1&zoom=9&task=../escape")).toEqual({
      ...defaultTaskMapUrlState,
      zoom: 1.5,
    })
    expect(parseTaskMapUrlState("?zoom=not-a-number")).toEqual(defaultTaskMapUrlState)
  })

  test("filters graph nodes and edges without mutating typed source", () => {
    const filtered = __test.filterTaskMap(graph, "ready", false)

    expect(filtered.nodes.map((node) => node.task.id)).toEqual(["ready"])
    expect(filtered.edges).toEqual([])
    expect(graph.nodes).toHaveLength(3)
    expect(__test.filterTaskMap(graph, "all", true).nodes.map((node) => node.task.id).sort()).toEqual(["done", "ready"])
    expect(__test.clampMapZoom(0)).toBe(0.65)
    expect(__test.clampMapZoom(2)).toBe(1.5)
    expect(__test.stepMapZoom(1, 1)).toBe(1.15)
  })

  test("renders each discrete zoom level through static CSP-safe classes", () => {
    for (const zoom of [0.65, 0.7, 0.8, 0.85, 0.95, 1, 1.1, 1.15, 1.25, 1.3, 1.4, 1.45, 1.5]) {
      const markup = renderToStaticMarkup(
        <TaskMapPresentation board="default" taskId={null} state={ready} zoom={zoom} onSelectTask={() => undefined} />,
      )

      expect(markup).toContain(`data-zoom="${zoom}"`)
      expect(markup).not.toContain(" style=")
    }
  })

  test("fences stale board data and clears missing task selections", () => {
    const identity = model.board
    const otherIdentity = {
      ...identity,
      id: asCanonicalBoardId("b_other"),
      slug: assertCanonicalBoardSlug("other"),
    }

    expect(fenceTaskMapReadModel("default", identity, model)).toBe(model)
    expect(fenceTaskMapReadModel("other", null, model)).toBeNull()
    expect(fenceTaskMapReadModel("other", identity, model)).toBeNull()
    expect(fenceTaskMapReadModel("other", otherIdentity, model)).toBeNull()
    expect(hasTaskMapSelection(model, "ready")).toBe(true)
    expect(hasTaskMapSelection(model, "t_missing")).toBe(false)
    expect(hasTaskMapSelection(fenceTaskMapReadModel("other", identity, model), "ready")).toBe(false)

    const hidden = renderToStaticMarkup(
      <TaskMapPresentation board="other" taskId="ready" state={{ data: fenceTaskMapReadModel("other", identity, model), loading: true, error: null }} onSelectTask={() => undefined} />,
    )
    expect(hidden).not.toContain('data-task-id="ready"')
    expect(__test.resolveSelectedNode(graph, "t_missing", "t_missing")).toBeNull()
  })

  test("renders keyboard-accessible graph region, nodes, edges and selected inspector", () => {
    const markup = renderToStaticMarkup(
      <TaskMapPresentation board="default" taskId="ready" state={ready} onSelectTask={() => undefined} />,
    )

    expect(markup).toContain('data-testid="task-map"')
    expect(markup).toContain('data-testid="task-map-graph"')
    expect(markup).toContain('role="toolbar"')
    expect(markup).not.toContain("<h1")
    expect(markup).toContain('tabindex="0"')
    expect(markup).toContain('data-testid="task-map-node"')
    expect(markup).toContain('data-task-id="ready"')
    expect(markup).toContain("dep:done:ready")
    expect(markup).toContain("当前选择")
  })

  test("renders the map copy in English", () => {
    const markup = renderToStaticMarkup(
      <TaskMapPresentation locale="en" board="default" taskId="ready" state={ready} onSelectTask={() => undefined} />,
    )

    expect(markup).toContain("Task map")
    expect(markup).toContain("Task map filters and zoom")
    expect(markup).toContain("Current selection")
    expect(markup).not.toContain("任务关系图")
  })

  test("keeps a selected node inspectable when a filter hides it", () => {
    const markup = renderToStaticMarkup(
      <TaskMapPresentation board="default" taskId="done" state={ready} filter="ready" onSelectTask={() => undefined} />,
    )

    expect(markup).toContain("当前节点被筛选隐藏")
    expect(markup).toContain("default#done")
  })

  test("exposes loading, empty, error and board-not-found states", () => {
    const loading = renderToStaticMarkup(<TaskMapPresentation board="default" taskId={null} state={{ data: null, loading: true, error: null }} onSelectTask={() => undefined} />)
    const empty = renderToStaticMarkup(<TaskMapPresentation board="default" taskId={null} state={{ data: { ...model, map: { ...graph, nodes: [], edges: [], meta: { ...graph.meta, node_count: 0, edge_count: 0 } } }, loading: false, error: null }} onSelectTask={() => undefined} />)
    const error = new Error("关系图请求失败")
    const failed = renderToStaticMarkup(<TaskMapPresentation board="default" taskId={null} state={{ data: null, loading: false, error }} onSelectTask={() => undefined} />)
    const notFound = new Error("board not found")
    Object.defineProperty(notFound, "reason", { value: "board-not-found" })
    const missing = renderToStaticMarkup(<TaskMapPresentation board="missing" taskId={null} state={{ data: null, loading: false, error: notFound }} onSelectTask={() => undefined} />)

    expect(loading).toContain('data-testid="task-map-loading"')
    expect(empty).toContain('data-testid="task-map-empty"')
    expect(failed).toContain('data-testid="task-map-error"')
    expect(missing).toContain('data-testid="task-map-not-found"')
  })
})
