import {
  Background,
  BaseEdge,
  Controls,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  getBezierPath,
  useReactFlow,
  type Edge,
  type EdgeProps,
  type EdgeTypes,
  type Node,
  type NodeProps,
  type NodeTypes,
} from "@xyflow/react"
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react"

import { cn } from "@/lib/utils"

import { TaskGraphNodeCard } from "./TaskGraphNodeCard"
import { clampTaskGraphScale } from "./task-graph-scale"
import { layoutTaskGraphFallback, layoutTaskGraphWithElk } from "./task-graph-layout"
import { graphNodeStatusMiniMapColor } from "./task-map-colors"
import type { TaskGraph, TaskGraphEdgeKind, TaskGraphLayout, TaskGraphLayoutNode, TaskGraphMode } from "./task-graph-types"

type TaskGraphCanvasProps = {
  graph: TaskGraph
  selectedTaskId?: string | null
  onSelectTask?: (taskId: string) => void
  onOpenTask?: (taskId: string) => void
  mode?: TaskGraphMode
  scale?: number
  className?: string
}

type TaskFlowNodeData = {
  node: TaskGraphLayoutNode
  selected: boolean
  onSelectTask?: (taskId: string) => void
  onOpenTask?: (taskId: string) => void
} & Record<string, unknown>

type TaskFlowEdgeData = {
  kind: TaskGraphEdgeKind
  blocking?: boolean
  required?: boolean
} & Record<string, unknown>

type TaskFlowNode = Node<TaskFlowNodeData, "taskNode" | "centerTaskNode" | "contextTaskNode">
type TaskFlowEdge = Edge<TaskFlowEdgeData, "dependencyEdge" | "stepEdge">

export function TaskGraphCanvas(props: TaskGraphCanvasProps) {
  return (
    <ReactFlowProvider>
      <TaskGraphCanvasInner {...props} />
    </ReactFlowProvider>
  )
}

function TaskGraphCanvasInner({
  graph,
  selectedTaskId,
  onSelectTask,
  onOpenTask,
  mode = "detail",
  scale = 1,
  className,
}: TaskGraphCanvasProps) {
  const reactFlow = useReactFlow<TaskFlowNode, TaskFlowEdge>()
  const onSelectTaskRef = useRef(onSelectTask)
  const onOpenTaskRef = useRef(onOpenTask)
  const selectedTaskIdRef = useRef<string | null>(selectedTaskId ?? null)
  const latestGraphRef = useRef(graph)
  const initialLayoutRef = useRef<TaskGraphLayout | null>(null)
  if (!initialLayoutRef.current) initialLayoutRef.current = layoutTaskGraphFallback(graph, { mode })
  const initialLayout = initialLayoutRef.current
  const handleSelectTask = useCallback((taskId: string) => {
    onSelectTaskRef.current?.(taskId)
  }, [])
  const handleOpenTask = useCallback((taskId: string) => {
    ;(onOpenTaskRef.current ?? onSelectTaskRef.current)?.(taskId)
  }, [])
  const [layout, setLayout] = useState<TaskGraphLayout>(initialLayout)
  const [nodes, setNodes] = useState<TaskFlowNode[]>(() =>
    buildTaskFlowNodes(initialLayout, graph, selectedTaskId ?? null, handleSelectTask, handleOpenTask),
  )
  const [edges, setEdges] = useState<TaskFlowEdge[]>(() => buildTaskFlowEdges(initialLayout))
  const safeScale = clampTaskGraphScale(scale)
  const interaction = taskGraphInteraction(mode)
  const layoutKey = useMemo(() => taskGraphLayoutKey(graph, mode), [graph, mode])
  const graphDataKey = useMemo(() => taskGraphDataKey(graph), [graph])

  onSelectTaskRef.current = onSelectTask
  onOpenTaskRef.current = onOpenTask
  latestGraphRef.current = graph

  useEffect(() => {
    let cancelled = false
    const layoutGraph = latestGraphRef.current
    const fallback = layoutTaskGraphFallback(layoutGraph, { mode })
    setLayout(fallback)
    void layoutTaskGraphWithElk(layoutGraph, { mode })
      .then((nextLayout) => {
        if (!cancelled) setLayout(nextLayout)
      })
      .catch(() => {
        if (!cancelled) setLayout(fallback)
      })
    return () => {
      cancelled = true
    }
  }, [layoutKey, mode])

  useEffect(() => {
    const layoutGraph = latestGraphRef.current
    setNodes(buildTaskFlowNodes(layout, layoutGraph, selectedTaskIdRef.current, handleSelectTask, handleOpenTask))
    setEdges(buildTaskFlowEdges(layout))
  }, [graphDataKey, handleOpenTask, handleSelectTask, layout])

  useEffect(() => {
    const previousTaskId = selectedTaskIdRef.current
    const nextTaskId = selectedTaskId ?? null
    if (previousTaskId === nextTaskId) return
    selectedTaskIdRef.current = nextTaskId
    setNodes((currentNodes) => patchTaskFlowNodeSelection(currentNodes, previousTaskId, nextTaskId))
  }, [selectedTaskId])

  useEffect(() => {
    if (!nodes.length) return
    const frame = window.requestAnimationFrame(() => {
      void reactFlow.fitView({ padding: interaction.fitViewPadding, duration: 160 })
    })
    return () => window.cancelAnimationFrame(frame)
  }, [interaction.fitViewPadding, layout.height, layout.width, nodes.length, reactFlow])

  useEffect(() => {
    if (mode !== "board-map") return
    void reactFlow.setViewport({ x: 24, y: 24, zoom: safeScale }, { duration: 120 })
  }, [mode, reactFlow, safeScale])

  return (
    <div
      className={cn("relative h-full min-h-[320px] w-full overflow-hidden rounded-md border border-border bg-muted/20", className)}
      aria-label={`Task graph with ${layout.nodes.length} nodes and ${layout.edges.length} edges`}
    >
      <ReactFlow<TaskFlowNode, TaskFlowEdge>
        className="task-graph-flow"
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        minZoom={interaction.minZoom}
        maxZoom={interaction.maxZoom}
        nodesConnectable={false}
        nodesDraggable={false}
        elementsSelectable
        panOnDrag={interaction.panOnDrag}
        zoomOnScroll={interaction.zoomOnScroll}
        zoomOnPinch={interaction.zoomOnPinch}
        zoomOnDoubleClick={interaction.zoomOnDoubleClick}
        connectOnClick={false}
        proOptions={{ hideAttribution: true }}
        onNodeClick={(_, node) => handleSelectTask(node.id)}
        onNodeDoubleClick={(_, node) => handleOpenTask(node.id)}
      >
        <Background gap={24} size={1} className="opacity-40" />
        {interaction.showMiniMap ? (
          <MiniMap pannable zoomable nodeColor={miniMapNodeColor} nodeStrokeWidth={3} className="!bg-background/95" />
        ) : null}
        {interaction.showControls ? <Controls showInteractive={false} /> : null}
      </ReactFlow>
    </div>
  )
}

const TaskGraphFlowNode = memo(function TaskGraphFlowNode({ data, selected }: NodeProps<TaskFlowNode>) {
  return (
    <div className="nodrag">
      <Handle type="target" position={Position.Left} isConnectable={false} className="opacity-0" />
      <Handle type="target" position={Position.Top} isConnectable={false} className="opacity-0" />
      <TaskGraphNodeCard
        node={data.node}
        selected={data.selected || selected}
        onSelectTask={data.onSelectTask}
        onOpenTask={data.onOpenTask}
      />
      <Handle type="source" position={Position.Right} isConnectable={false} className="opacity-0" />
      <Handle type="source" position={Position.Bottom} isConnectable={false} className="opacity-0" />
    </div>
  )
})

function TaskGraphFlowEdge(props: EdgeProps<TaskFlowEdge>) {
  const [edgePath, labelX, labelY] = getBezierPath(props)
  const kind = props.data?.kind ?? "dependency"
  const blocking = props.data?.blocking
  const color = graphEdgeStroke(kind, blocking)
  const label = kind === "step" ? (props.data?.required ? "required step" : "optional step") : blocking ? "blocks" : "unlocks"

  return (
    <>
      <BaseEdge id={props.id} path={edgePath} markerEnd={props.markerEnd} style={{ stroke: color, strokeWidth: kind === "step" ? 2.5 : 2 }} />
      <EdgeLabelRenderer>
        <span
          className="pointer-events-none absolute rounded-full border border-border bg-background/90 px-1.5 py-0.5 text-[10px] text-muted-foreground shadow-sm"
          style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}
        >
          {label}
        </span>
      </EdgeLabelRenderer>
    </>
  )
}

const nodeTypes: NodeTypes = {
  taskNode: TaskGraphFlowNode,
  centerTaskNode: TaskGraphFlowNode,
  contextTaskNode: TaskGraphFlowNode,
}

const edgeTypes: EdgeTypes = {
  dependencyEdge: TaskGraphFlowEdge,
  stepEdge: TaskGraphFlowEdge,
}

function flowNodeType(role: TaskGraphLayoutNode["role"], contextOnly?: boolean): TaskFlowNode["type"] {
  if (role === "center") return "centerTaskNode"
  if (contextOnly || role === "context") return "contextTaskNode"
  return "taskNode"
}

function graphEdgeStroke(kind: TaskGraphEdgeKind, blocking?: boolean) {
  if (kind === "step") return "#7c3aed"
  return blocking ? "#dc2626" : "#059669"
}

function taskGraphLayoutKey(graph: TaskGraph, mode: TaskGraphMode) {
  const nodes = graph.nodes.map((node) => node.id).sort().join(",")
  const edges = graph.edges
    .map((edge) => `${edge.sourceTaskId}>${edge.targetTaskId}:${edge.kind}:${edge.id}`)
    .sort()
    .join(",")
  return `${mode}|${nodes}|${edges}`
}

function taskGraphDataKey(graph: TaskGraph) {
  const nodes = graph.nodes
    .map((node) =>
      [
        node.id,
        node.ref,
        node.title,
        node.status,
        node.priority ?? "",
        node.role ?? "",
        node.contextOnly ? "context" : "active",
        node.dependencyBlocked ? "blocked" : "clear",
        node.unfinishedParentCount ?? 0,
        node.stepCounts?.completed ?? "",
        node.stepCounts?.total ?? "",
      ].join(":"),
    )
    .sort()
    .join(",")
  const edges = graph.edges
    .map((edge) => `${edge.id}:${edge.sourceTaskId}>${edge.targetTaskId}:${edge.kind}:${edge.blocking ? "blocking" : "clear"}:${edge.required ? "required" : "optional"}`)
    .sort()
    .join(",")
  return `${nodes}|${edges}`
}

function buildTaskFlowNodes(
  layout: TaskGraphLayout,
  graph: TaskGraph,
  selectedTaskId: string | null,
  onSelectTask?: (taskId: string) => void,
  onOpenTask?: (taskId: string) => void,
) {
  const graphNodeById = new Map(graph.nodes.map((node) => [node.id, node]))
  return layout.nodes.map((layoutNode): TaskFlowNode => {
    const graphNode = graphNodeById.get(layoutNode.id)
    const node = graphNode ? { ...graphNode, x: layoutNode.x, y: layoutNode.y, width: layoutNode.width, height: layoutNode.height } : layoutNode
    const selected = taskFlowNodeSelected(node, selectedTaskId)
    return {
      id: node.id,
      type: flowNodeType(node.role, node.contextOnly),
      position: { x: node.x, y: node.y },
      data: { node, selected, onSelectTask, onOpenTask },
      draggable: false,
      selectable: true,
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      ariaLabel: `Task graph node ${node.ref} ${node.title}`,
    }
  })
}

function buildTaskFlowEdges(layout: TaskGraphLayout) {
  return layout.edges.map((edge): TaskFlowEdge => {
    const color = graphEdgeStroke(edge.kind, edge.blocking)
    return {
      id: edge.id,
      source: edge.sourceTaskId,
      target: edge.targetTaskId,
      type: edge.kind === "step" ? "stepEdge" : "dependencyEdge",
      data: { kind: edge.kind, blocking: edge.blocking, required: edge.required },
      markerEnd: { type: MarkerType.ArrowClosed, color },
      style: { stroke: color, strokeWidth: edge.kind === "step" ? 2.5 : 2 },
      selectable: true,
    }
  })
}

function patchTaskFlowNodeSelection(nodes: TaskFlowNode[], previousTaskId: string | null, nextTaskId: string | null) {
  if (previousTaskId === nextTaskId) return nodes
  let changed = false
  const nextNodes = nodes.map((node) => {
    if (node.id !== previousTaskId && node.id !== nextTaskId && node.data.node.role !== "center") return node
    const selected = taskFlowNodeSelected(node.data.node, nextTaskId)
    if (node.selected === selected && node.data.selected === selected) return node
    changed = true
    return { ...node, selected, data: { ...node.data, selected } }
  })
  return changed ? nextNodes : nodes
}

function taskFlowNodeSelected(node: Pick<TaskGraphLayoutNode, "id" | "role">, selectedTaskId: string | null) {
  return node.id === selectedTaskId || node.role === "center"
}

function taskGraphInteraction(mode: TaskGraphMode) {
  const detailMode = mode === "detail"
  return {
    fitViewPadding: detailMode ? 0.28 : 0.16,
    minZoom: detailMode ? 0.4 : 0.35,
    maxZoom: detailMode ? 2.5 : 1.75,
    panOnDrag: true,
    zoomOnScroll: true,
    zoomOnPinch: true,
    zoomOnDoubleClick: !detailMode,
    showControls: true,
    showMiniMap: !detailMode,
  }
}

function miniMapNodeColor(node: Node) {
  const data = node.data as Partial<TaskFlowNodeData>
  return graphNodeStatusMiniMapColor(data.node?.status)
}

export const __test = {
  buildTaskFlowNodes,
  layoutTaskGraphFallback,
  patchTaskFlowNodeSelection,
  taskGraphDataKey,
  taskGraphLayoutKey,
  taskGraphInteraction,
}
