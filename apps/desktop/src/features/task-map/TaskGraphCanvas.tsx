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
import { memo, useEffect, useMemo } from "react"

import { cn } from "@/lib/utils"

import { TaskGraphNodeCard } from "./TaskGraphNodeCard"
import { clampTaskGraphScale } from "./task-graph-scale"
import { layoutTaskGraph } from "./task-graph-layout"
import type { TaskGraph, TaskGraphEdgeKind, TaskGraphLayoutNode, TaskGraphMode } from "./task-graph-types"

type TaskGraphCanvasProps = {
  graph: TaskGraph
  selectedTaskId?: string | null
  onSelectTask?: (taskId: string) => void
  mode?: TaskGraphMode
  scale?: number
  className?: string
}

type TaskFlowNodeData = {
  node: TaskGraphLayoutNode
  selected: boolean
  onSelectTask?: (taskId: string) => void
} & Record<string, unknown>

type TaskFlowEdgeData = {
  kind: TaskGraphEdgeKind
  blocking?: boolean
  required?: boolean
} & Record<string, unknown>

type TaskFlowNode = Node<TaskFlowNodeData, "taskNode" | "centerTaskNode" | "contextTaskNode">
type TaskFlowEdge = Edge<TaskFlowEdgeData, "dependencyEdge" | "subtaskEdge">

export function TaskGraphCanvas(props: TaskGraphCanvasProps) {
  return (
    <ReactFlowProvider>
      <TaskGraphCanvasInner {...props} />
    </ReactFlowProvider>
  )
}

function TaskGraphCanvasInner({ graph, selectedTaskId, onSelectTask, mode = "detail", scale = 1, className }: TaskGraphCanvasProps) {
  const reactFlow = useReactFlow<TaskFlowNode, TaskFlowEdge>()
  const layout = useMemo(() => layoutTaskGraph(graph, { mode, selectedTaskId }), [graph, mode, selectedTaskId])
  const safeScale = clampTaskGraphScale(scale)
  const nodes = useMemo(
    () =>
      layout.nodes.map((node): TaskFlowNode => {
        const selected = node.id === selectedTaskId || node.role === "center"
        return {
          id: node.id,
          type: flowNodeType(node.role, node.contextOnly),
          position: { x: node.x, y: node.y },
          data: { node, selected, onSelectTask },
          draggable: false,
          selectable: true,
          sourcePosition: mode === "detail" && node.role === "subtask_child" ? Position.Top : Position.Right,
          targetPosition: mode === "detail" && node.role === "subtask_child" ? Position.Top : Position.Left,
          ariaLabel: `Task graph node ${node.ref} ${node.title}`,
        }
      }),
    [layout.nodes, mode, onSelectTask, selectedTaskId],
  )
  const edges = useMemo(
    () =>
      layout.edges.map((edge): TaskFlowEdge => {
        const color = graphEdgeStroke(edge.kind, edge.blocking)
        return {
          id: edge.id,
          source: edge.sourceTaskId,
          target: edge.targetTaskId,
          type: edge.kind === "subtask" ? "subtaskEdge" : "dependencyEdge",
          data: { kind: edge.kind, blocking: edge.blocking, required: edge.required },
          markerEnd: { type: MarkerType.ArrowClosed, color },
          style: { stroke: color, strokeWidth: edge.kind === "subtask" ? 2.5 : 2 },
          selectable: true,
        }
      }),
    [layout.edges],
  )

  useEffect(() => {
    if (!nodes.length) return
    const frame = window.requestAnimationFrame(() => {
      void reactFlow.fitView({ padding: mode === "detail" ? 0.28 : 0.16, duration: 160 })
    })
    return () => window.cancelAnimationFrame(frame)
  }, [layout.height, layout.width, mode, nodes.length, reactFlow, selectedTaskId])

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
        fitView
        minZoom={mode === "detail" ? 0.4 : 0.35}
        maxZoom={1.75}
        nodesConnectable={false}
        nodesDraggable={false}
        elementsSelectable
        panOnDrag={mode === "board-map"}
        zoomOnScroll={mode === "board-map"}
        zoomOnPinch={mode === "board-map"}
        zoomOnDoubleClick={mode === "board-map"}
        connectOnClick={false}
        proOptions={{ hideAttribution: true }}
        onNodeClick={(_, node) => onSelectTask?.(node.id)}
        onNodeDoubleClick={(_, node) => onSelectTask?.(node.id)}
      >
        <Background gap={24} size={1} className="opacity-40" />
        {mode === "board-map" ? (
          <>
            <MiniMap pannable zoomable nodeColor={miniMapNodeColor} nodeStrokeWidth={3} className="!bg-background/95" />
            <Controls showInteractive={false} />
          </>
        ) : null}
      </ReactFlow>
    </div>
  )
}

const TaskGraphFlowNode = memo(function TaskGraphFlowNode({ data, selected }: NodeProps<TaskFlowNode>) {
  return (
    <div className="nodrag nowheel">
      <Handle type="target" position={Position.Left} isConnectable={false} className="opacity-0" />
      <Handle type="target" position={Position.Top} isConnectable={false} className="opacity-0" />
      <TaskGraphNodeCard node={data.node} selected={data.selected || selected} onSelectTask={data.onSelectTask} />
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
  const label = kind === "subtask" ? (props.data?.required ? "required step" : "optional step") : blocking ? "blocks" : "unlocks"

  return (
    <>
      <BaseEdge id={props.id} path={edgePath} markerEnd={props.markerEnd} style={{ stroke: color, strokeWidth: kind === "subtask" ? 2.5 : 2 }} />
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
  subtaskEdge: TaskGraphFlowEdge,
}

function flowNodeType(role: TaskGraphLayoutNode["role"], contextOnly?: boolean): TaskFlowNode["type"] {
  if (role === "center") return "centerTaskNode"
  if (contextOnly || role === "context") return "contextTaskNode"
  return "taskNode"
}

function graphEdgeStroke(kind: TaskGraphEdgeKind, blocking?: boolean) {
  if (kind === "subtask") return "#7c3aed"
  return blocking ? "#dc2626" : "#059669"
}

function miniMapNodeColor(node: Node) {
  const data = node.data as Partial<TaskFlowNodeData>
  if (data.node?.role === "center") return "#111827"
  if (data.node?.contextOnly) return "#9ca3af"
  if (data.node?.role === "dependency_parent") return "#dc2626"
  if (data.node?.role === "dependency_child") return "#059669"
  if (data.node?.role === "subtask_child" || data.node?.role === "subtask_parent") return "#7c3aed"
  return "#64748b"
}
