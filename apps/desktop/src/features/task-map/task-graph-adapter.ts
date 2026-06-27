import type { BoardTaskMap, TaskGraphNodeRole as ApiTaskGraphNodeRole, TaskNeighborhood } from "@/lib/api"

import type { TaskGraph, TaskGraphNodeRole } from "./task-graph-types"

type ApiGraph = Pick<TaskNeighborhood | BoardTaskMap, "nodes" | "edges">

export function apiTaskGraphToCanvasGraph(graph: ApiGraph | null | undefined): TaskGraph {
  if (!graph) return { nodes: [], edges: [] }
  return {
    nodes: graph.nodes.map((node) => ({
      id: node.task.id,
      ref: node.task.ref,
      title: node.task.title,
      status: node.task.status,
      priority: node.task.priority,
      role: canvasNodeRole(node.role),
      contextOnly: node.context_only,
      dependencyBlocked: node.task.dependency_blocked,
      unfinishedParentCount: node.task.unfinished_parent_count,
      subtaskCounts: {
        total: node.task.required_subtask_count + node.task.optional_subtask_count,
        incomplete: Math.max(0, node.task.required_subtask_count - node.task.completed_required_subtask_count),
      },
    })),
    edges: graph.edges.map((edge) => ({
      id: edge.id,
      sourceTaskId: edge.source_task_id,
      targetTaskId: edge.target_task_id,
      kind: edge.kind,
      blocking: edge.blocking,
      required: edge.required,
    })),
  }
}

function canvasNodeRole(role: ApiTaskGraphNodeRole): TaskGraphNodeRole {
  return role === "active" ? "context" : role
}
