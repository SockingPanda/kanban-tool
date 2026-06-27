import ELK, { type ElkExtendedEdge, type ElkNode } from "elkjs/lib/elk.bundled.js"

import type { TaskGraph, TaskGraphLayout, TaskGraphLayoutEdge, TaskGraphLayoutNode, TaskGraphMode, TaskGraphNode } from "./task-graph-types"

const NODE_WIDTH = 176
const NODE_HEIGHT = 72
const DETAIL_LAYER_GAP = 112
const DETAIL_ROW_GAP = 28
const BOARD_LAYER_GAP = 96
const BOARD_ROW_GAP = 24
const LAYOUT_PADDING = 24

const elk = new ELK()

type LayoutOptions = {
  mode: TaskGraphMode
  selectedTaskId?: string | null
}

export async function layoutTaskGraph(graph: TaskGraph, options: LayoutOptions): Promise<TaskGraphLayout> {
  return layoutTaskGraphWithElk(graph, options)
}

export async function layoutTaskGraphWithElk(graph: TaskGraph, options: LayoutOptions): Promise<TaskGraphLayout> {
  if (!graph.nodes.length) return emptyLayout()
  const sortedNodes = [...graph.nodes].sort(compareNodes)
  const visibleNodeIds = new Set(sortedNodes.map((node) => node.id))
  const visibleEdges = graph.edges.filter((edge) => visibleNodeIds.has(edge.sourceTaskId) && visibleNodeIds.has(edge.targetTaskId))
  const elkGraph: ElkNode = {
    id: "task-graph-root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.edgeRouting": "SPLINES",
      "elk.spacing.nodeNode": `${options.mode === "detail" ? DETAIL_ROW_GAP : BOARD_ROW_GAP}`,
      "elk.layered.spacing.nodeNodeBetweenLayers": `${options.mode === "detail" ? DETAIL_LAYER_GAP : BOARD_LAYER_GAP}`,
      "elk.layered.cycleBreaking.strategy": "GREEDY",
    },
    children: sortedNodes.map((node) => ({
      id: node.id,
      width: NODE_WIDTH,
      height: NODE_HEIGHT,
    })),
    edges: visibleEdges.map((edge): ElkExtendedEdge => ({
      id: edge.id,
      sources: [edge.sourceTaskId],
      targets: [edge.targetTaskId],
    })),
  }

  const result = await elk.layout(elkGraph)
  const children = result.children ?? []
  const minX = children.length ? Math.min(...children.map((child) => child.x ?? 0)) : 0
  const minY = children.length ? Math.min(...children.map((child) => child.y ?? 0)) : 0
  const childById = new Map(children.map((child) => [child.id, child]))
  const layoutNodes = sortedNodes.map((node) => {
    const child = childById.get(node.id)
    return toLayoutNode(node, Math.round((child?.x ?? 0) - minX + LAYOUT_PADDING), Math.round((child?.y ?? 0) - minY + LAYOUT_PADDING))
  })

  return finalizeLayout(layoutNodes, graph.edges)
}

export function layoutTaskGraphFallback(graph: TaskGraph, options: LayoutOptions): TaskGraphLayout {
  if (!graph.nodes.length) return emptyLayout()
  const nodes = layoutFallbackNodes(graph.nodes, graph.edges, options.mode)
  return finalizeLayout(nodes, graph.edges)
}

function finalizeLayout(nodes: TaskGraphLayoutNode[], graphEdges: TaskGraph["edges"]): TaskGraphLayout {
  const nodeById = new Map(nodes.map((node) => [node.id, node]))
  const edges = graphEdges.flatMap((edge): TaskGraphLayoutEdge[] => {
    const source = nodeById.get(edge.sourceTaskId)
    const target = nodeById.get(edge.targetTaskId)
    if (!source || !target) return []
    return [{ ...edge, source, target, path: edgePath(source, target) }]
  })
  const maxX = nodes.length ? Math.max(...nodes.map((node) => node.x + node.width)) : NODE_WIDTH
  const maxY = nodes.length ? Math.max(...nodes.map((node) => node.y + node.height)) : NODE_HEIGHT
  return {
    nodes,
    edges,
    width: maxX + 24,
    height: maxY + 24,
  }
}

function layoutFallbackNodes(nodes: TaskGraphNode[], edges: TaskGraph["edges"], mode: TaskGraphMode) {
  const sorted = [...nodes].sort(compareNodes)
  const visibleIds = new Set(sorted.map((node) => node.id))
  const ranks = fallbackRanks(sorted, edges.filter((edge) => visibleIds.has(edge.sourceTaskId) && visibleIds.has(edge.targetTaskId)))
  const layers = new Map<number, TaskGraphNode[]>()
  for (const node of sorted) {
    const rank = ranks.get(node.id) ?? 0
    layers.set(rank, [...(layers.get(rank) ?? []), node])
  }
  const layerGap = mode === "detail" ? DETAIL_LAYER_GAP : BOARD_LAYER_GAP
  const rowGap = mode === "detail" ? DETAIL_ROW_GAP : BOARD_ROW_GAP
  return [...layers.keys()].sort((a, b) => a - b).flatMap((rank) => {
    const x = LAYOUT_PADDING + rank * (NODE_WIDTH + layerGap)
    return stack(layers.get(rank) ?? [], x, LAYOUT_PADDING, rowGap)
  })
}

function fallbackRanks(nodes: TaskGraphNode[], edges: TaskGraph["edges"]) {
  const indegree = new Map(nodes.map((node) => [node.id, 0]))
  const adjacency = new Map(nodes.map((node) => [node.id, [] as string[]]))
  for (const edge of edges) {
    indegree.set(edge.targetTaskId, (indegree.get(edge.targetTaskId) ?? 0) + 1)
    adjacency.set(edge.sourceTaskId, [...(adjacency.get(edge.sourceTaskId) ?? []), edge.targetTaskId])
  }

  const ranks = new Map(nodes.map((node) => [node.id, 0]))
  const queue = nodes.filter((node) => (indegree.get(node.id) ?? 0) === 0).map((node) => node.id)
  const visited = new Set<string>()
  for (let index = 0; index < queue.length; index += 1) {
    const current = queue[index]
    visited.add(current)
    for (const next of adjacency.get(current) ?? []) {
      ranks.set(next, Math.max(ranks.get(next) ?? 0, (ranks.get(current) ?? 0) + 1))
      indegree.set(next, (indegree.get(next) ?? 0) - 1)
      if ((indegree.get(next) ?? 0) === 0) queue.push(next)
    }
  }

  for (const node of nodes) {
    if (!visited.has(node.id)) ranks.set(node.id, ranks.get(node.id) ?? 0)
  }
  return ranks
}

function stack(nodes: TaskGraphNode[], x: number, startY: number, rowGap = DETAIL_ROW_GAP) {
  return nodes.map((node, index) => toLayoutNode(node, x, startY + index * (NODE_HEIGHT + rowGap)))
}

function toLayoutNode(node: TaskGraphNode, x: number, y: number): TaskGraphLayoutNode {
  return { ...node, x, y, width: NODE_WIDTH, height: NODE_HEIGHT }
}

function edgePath(source: TaskGraphLayoutNode, target: TaskGraphLayoutNode) {
  if (target.x >= source.x + source.width) {
    return horizontalEdgePath(source.x + source.width, source.y + source.height / 2, target.x, target.y + target.height / 2)
  }
  if (source.x >= target.x + target.width) {
    return horizontalEdgePath(source.x, source.y + source.height / 2, target.x + target.width, target.y + target.height / 2)
  }

  const sourceBelowTarget = source.y > target.y
  const startX = source.x + source.width / 2
  const startY = sourceBelowTarget ? source.y : source.y + source.height
  const endX = target.x + target.width / 2
  const endY = sourceBelowTarget ? target.y + target.height : target.y
  const midY = startY + (endY - startY) / 2
  return `M ${startX} ${startY} C ${startX} ${midY}, ${endX} ${midY}, ${endX} ${endY}`
}

function horizontalEdgePath(startX: number, startY: number, endX: number, endY: number) {
  const midX = startX + (endX - startX) / 2
  return `M ${startX} ${startY} C ${midX} ${startY}, ${midX} ${endY}, ${endX} ${endY}`
}

function compareNodes(a: TaskGraphNode, b: TaskGraphNode) {
  return `${a.ref}:${a.id}`.localeCompare(`${b.ref}:${b.id}`)
}

function emptyLayout(): TaskGraphLayout {
  return {
    nodes: [],
    edges: [],
    width: NODE_WIDTH,
    height: NODE_HEIGHT,
  }
}
