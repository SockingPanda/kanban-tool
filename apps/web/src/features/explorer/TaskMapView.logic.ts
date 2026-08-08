import type { ExplorerTaskMap } from "../../lib/api/explorer-read-model"

export type BoardMapFilter = "all" | "blocked" | "ready" | "running" | "unplanned" | "incomplete-steps"

type MapNode = ExplorerTaskMap["nodes"][number]

export const MIN_MAP_ZOOM = 0.65
export const MAX_MAP_ZOOM = 1.5
const MAP_ZOOM_STEP = 0.15

export interface TaskMapUrlState {
  readonly filter: BoardMapFilter
  readonly showDoneContext: boolean
  readonly hideIsolated: boolean
  readonly zoom: number
  readonly taskId: string | null
}

export const defaultTaskMapUrlState: TaskMapUrlState = Object.freeze({
  filter: "all",
  showDoneContext: false,
  hideIsolated: false,
  zoom: 1,
  taskId: null,
})

const mapFilters = new Set<BoardMapFilter>(["all", "blocked", "ready", "running", "unplanned", "incomplete-steps"])

function safeTaskSelector(value: string | null): string | null {
  if (value === null || value.trim() !== value || !value.startsWith("t_") || value.length <= 2 || /[\\/?#]/.test(value)) return null
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) return null
  }
  return value
}

export function parseTaskMapUrlState(input: string | URLSearchParams): TaskMapUrlState {
  const params = typeof input === "string"
    ? new URLSearchParams(input.startsWith("?") ? input.slice(1) : input)
    : input
  const filterValue = params.get("filter")
  const rawZoom = params.get("zoom")
  const zoomValue = rawZoom !== null && rawZoom.trim() === rawZoom && rawZoom.length > 0 ? Number(rawZoom) : Number.NaN
  const zoom = rawZoom !== null && Number.isFinite(zoomValue) ? clampMapZoom(zoomValue) : defaultTaskMapUrlState.zoom
  return Object.freeze({
    filter: filterValue && mapFilters.has(filterValue as BoardMapFilter) ? filterValue as BoardMapFilter : defaultTaskMapUrlState.filter,
    showDoneContext: params.get("show_done") === "true",
    hideIsolated: params.get("hide_isolated") === "true",
    zoom,
    taskId: safeTaskSelector(params.get("task")),
  })
}

export function serializeTaskMapUrlState(state: TaskMapUrlState): string {
  const params = new URLSearchParams()
  const filter = mapFilters.has(state.filter) ? state.filter : defaultTaskMapUrlState.filter
  if (filter !== defaultTaskMapUrlState.filter) params.set("filter", filter)
  if (state.showDoneContext === true) params.set("show_done", "true")
  if (state.hideIsolated === true) params.set("hide_isolated", "true")
  const zoom = typeof state.zoom === "number" ? clampMapZoom(state.zoom) : defaultTaskMapUrlState.zoom
  if (zoom !== defaultTaskMapUrlState.zoom) params.set("zoom", String(zoom))
  const taskId = safeTaskSelector(state.taskId)
  if (taskId) params.set("task", taskId)
  return params.toString()
}

function incompleteRequiredSteps(task: MapNode["task"]): number {
  return Math.max(0, task.required_step_count - task.completed_required_step_count)
}

function nodeMatchesFilter(node: MapNode, filter: BoardMapFilter): boolean {
  if (filter === "all") return !node.task.archived_at
  if (node.context_only) return false
  if (filter === "blocked") return node.task.status === "blocked" || node.task.dependency_blocked
  if (filter === "ready") return node.task.status === "ready"
  if (filter === "running") return node.task.status === "running"
  if (filter === "unplanned") return node.task.execution_plan_state === "unplanned"
  return incompleteRequiredSteps(node.task) > 0
}

export function filterTaskMap(
  graph: ExplorerTaskMap,
  filter: BoardMapFilter,
  hideIsolated: boolean,
): Pick<ExplorerTaskMap, "nodes" | "edges"> {
  const matchingIds = new Set(graph.nodes.filter((node) => nodeMatchesFilter(node, filter)).map((node) => node.task.id))
  const edges = graph.edges.filter((edge) => matchingIds.has(edge.source_task_id) && matchingIds.has(edge.target_task_id))
  if (!hideIsolated) {
    return {
      nodes: graph.nodes.filter((node) => matchingIds.has(node.task.id)),
      edges,
    }
  }
  const connected = new Set<string>()
  for (const edge of edges) {
    connected.add(edge.source_task_id)
    connected.add(edge.target_task_id)
  }
  return {
    nodes: graph.nodes.filter((node) => matchingIds.has(node.task.id) && connected.has(node.task.id)),
    edges,
  }
}

export function resolveSelectedNode(
  graph: ExplorerTaskMap | null,
  inspectedTaskId: string | null,
  taskId: string | null,
): MapNode | null {
  if (!graph) return null
  return graph.nodes.find((node) => node.task.id === inspectedTaskId)
    ?? graph.nodes.find((node) => node.task.id === taskId)
    ?? null
}

export function clampMapZoom(value: number): number {
  if (!Number.isFinite(value)) return 1
  return Math.min(MAX_MAP_ZOOM, Math.max(MIN_MAP_ZOOM, value))
}

export function stepMapZoom(current: number, direction: -1 | 1): number {
  return clampMapZoom(Number((current + direction * MAP_ZOOM_STEP).toFixed(2)))
}

export const __test = { clampMapZoom, filterTaskMap, resolveSelectedNode, stepMapZoom }
