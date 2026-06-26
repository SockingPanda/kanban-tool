import type { TaskGraph, TaskGraphLayout, TaskGraphLayoutEdge, TaskGraphLayoutNode, TaskGraphMode, TaskGraphNode } from "./task-graph-types"

const NODE_WIDTH = 176
const NODE_HEIGHT = 72
const DETAIL_COLUMN_GAP = 112
const DETAIL_ROW_GAP = 24
const BOARD_COLUMN_GAP = 40
const BOARD_ROW_GAP = 20
const BOARD_STATUSES = ["ready", "running", "blocked", "review", "todo", "scheduled", "triage", "done", "archived"] as const

type LayoutOptions = {
  mode: TaskGraphMode
  selectedTaskId?: string | null
}

export function layoutTaskGraph(graph: TaskGraph, options: LayoutOptions): TaskGraphLayout {
  const nodes = options.mode === "detail"
    ? layoutDetailNodes(graph.nodes, options.selectedTaskId)
    : layoutBoardMapNodes(graph.nodes)
  const nodeById = new Map(nodes.map((node) => [node.id, node]))
  const edges = graph.edges.flatMap((edge): TaskGraphLayoutEdge[] => {
    const source = nodeById.get(edge.sourceTaskId)
    const target = nodeById.get(edge.targetTaskId)
    if (!source || !target) return []
    return [{ ...edge, source, target, path: edgePath(source, target) }]
  })
  return {
    nodes,
    edges,
    width: Math.max(...nodes.map((node) => node.x + node.width), NODE_WIDTH) + 24,
    height: Math.max(...nodes.map((node) => node.y + node.height), NODE_HEIGHT) + 24,
  }
}

function layoutDetailNodes(nodes: TaskGraphNode[], selectedTaskId?: string | null) {
  const sorted = [...nodes].sort(compareNodes)
  const center = sorted.find((node) => node.role === "center") ?? sorted.find((node) => node.id === selectedTaskId) ?? sorted[0]
  const groups = {
    parents: sorted.filter((node) => node.id !== center?.id && (node.role === "dependency_parent" || node.role === "subtask_parent")),
    children: sorted.filter((node) => node.id !== center?.id && node.role === "dependency_child"),
    subtasks: sorted.filter((node) => node.id !== center?.id && node.role === "subtask_child"),
    context: sorted.filter((node) => node.id !== center?.id && !["dependency_parent", "subtask_parent", "dependency_child", "subtask_child"].includes(node.role ?? "")),
  }
  const result: TaskGraphLayoutNode[] = []
  const centerX = 24 + NODE_WIDTH + DETAIL_COLUMN_GAP
  const centerY = 120
  if (center) result.push(toLayoutNode(center, centerX, centerY))
  result.push(...stack(groups.parents, 24, 72))
  result.push(...stack(groups.children, centerX + NODE_WIDTH + DETAIL_COLUMN_GAP, 72))
  result.push(...stack(groups.subtasks, centerX, centerY + NODE_HEIGHT + 88))
  result.push(...stack(groups.context, centerX + NODE_WIDTH + DETAIL_COLUMN_GAP, 72 + groups.children.length * (NODE_HEIGHT + DETAIL_ROW_GAP)))
  return result
}

function layoutBoardMapNodes(nodes: TaskGraphNode[]) {
  const byStatus = new Map<string, TaskGraphNode[]>()
  for (const node of [...nodes].sort(compareNodes)) {
    const bucket = node.contextOnly ? "context" : node.status
    byStatus.set(bucket, [...(byStatus.get(bucket) ?? []), node])
  }
  const statuses = [...BOARD_STATUSES, "context"].filter((status) => byStatus.has(status))
  return statuses.flatMap((status, column) => {
    const x = 24 + column * (NODE_WIDTH + BOARD_COLUMN_GAP)
    return stack(byStatus.get(status) ?? [], x, 56, BOARD_ROW_GAP)
  })
}

function stack(nodes: TaskGraphNode[], x: number, startY: number, rowGap = DETAIL_ROW_GAP) {
  return nodes.map((node, index) => toLayoutNode(node, x, startY + index * (NODE_HEIGHT + rowGap)))
}

function toLayoutNode(node: TaskGraphNode, x: number, y: number): TaskGraphLayoutNode {
  return { ...node, x, y, width: NODE_WIDTH, height: NODE_HEIGHT }
}

function edgePath(source: TaskGraphLayoutNode, target: TaskGraphLayoutNode) {
  const startX = source.x + source.width
  const startY = source.y + source.height / 2
  const endX = target.x
  const endY = target.y + target.height / 2
  const midX = startX + (endX - startX) / 2
  return `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`
}

function compareNodes(a: TaskGraphNode, b: TaskGraphNode) {
  const statusDelta = statusRank(a.status) - statusRank(b.status)
  if (statusDelta !== 0) return statusDelta
  return `${a.ref}:${a.id}`.localeCompare(`${b.ref}:${b.id}`)
}

function statusRank(status: string) {
  const index = BOARD_STATUSES.indexOf(status as (typeof BOARD_STATUSES)[number])
  return index === -1 ? BOARD_STATUSES.length : index
}
