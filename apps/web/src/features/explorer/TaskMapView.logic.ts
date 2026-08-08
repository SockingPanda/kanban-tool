import type { ExplorerTaskMap } from "../../lib/api/explorer-read-model"

export type BoardMapFilter = "all" | "blocked" | "ready" | "running" | "unplanned" | "incomplete-steps"

type MapNode = ExplorerTaskMap["nodes"][number]

export const MIN_MAP_ZOOM = 0.65
export const MAX_MAP_ZOOM = 1.5
const MAP_ZOOM_STEP = 0.15

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
    ?? graph.nodes.find((node) => !node.context_only)
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
