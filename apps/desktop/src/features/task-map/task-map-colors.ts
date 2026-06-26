import type { TaskGraphEdgeKind, TaskGraphNodeRole } from "./task-graph-types"

export function graphNodeRoleClass(role: TaskGraphNodeRole | undefined, selected: boolean, contextOnly: boolean | undefined) {
  if (selected || role === "center") return "border-primary bg-card text-card-foreground shadow-sm"
  if (contextOnly) return "border-border bg-muted/40 text-muted-foreground"
  if (role === "dependency_parent") return "border-red-200 bg-red-50 text-red-950 dark:border-red-900 dark:bg-red-950/30 dark:text-red-100"
  if (role === "dependency_child") return "border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-900 dark:bg-emerald-950/30 dark:text-emerald-100"
  if (role === "subtask_child" || role === "subtask_parent") return "border-violet-200 bg-violet-50 text-violet-950 dark:border-violet-900 dark:bg-violet-950/30 dark:text-violet-100"
  return "border-border bg-card text-card-foreground"
}

export function graphEdgeClass(kind: TaskGraphEdgeKind, blocking?: boolean) {
  if (kind === "subtask") return "stroke-violet-500"
  return blocking ? "stroke-red-500" : "stroke-emerald-500"
}
