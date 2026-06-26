import type { TaskStatus } from "@/lib/api"

export type TaskGraphMode = "detail" | "board-map"

export type TaskGraphNodeRole =
  | "center"
  | "dependency_parent"
  | "dependency_child"
  | "subtask_parent"
  | "subtask_child"
  | "context"

export type TaskGraphEdgeKind = "dependency" | "subtask"

export type TaskGraphNode = {
  id: string
  ref: string
  title: string
  status: TaskStatus
  priority?: number | null
  role?: TaskGraphNodeRole
  contextOnly?: boolean
  dependencyBlocked?: boolean
  unfinishedParentCount?: number
  subtaskCounts?: {
    total: number
    incomplete: number
  }
}

export type TaskGraphEdge = {
  id: string
  sourceTaskId: string
  targetTaskId: string
  kind: TaskGraphEdgeKind
  blocking?: boolean
  required?: boolean
}

export type TaskGraph = {
  nodes: TaskGraphNode[]
  edges: TaskGraphEdge[]
}

export type TaskGraphLayoutNode = TaskGraphNode & {
  x: number
  y: number
  width: number
  height: number
}

export type TaskGraphLayoutEdge = TaskGraphEdge & {
  source: TaskGraphLayoutNode
  target: TaskGraphLayoutNode
  path: string
}

export type TaskGraphLayout = {
  nodes: TaskGraphLayoutNode[]
  edges: TaskGraphLayoutEdge[]
  width: number
  height: number
}
