import type { Dependencies, Task, TaskStatus } from "@/lib/api"
export { priorityBadgeClass, priorityLabel as priorityBadgeLabel } from "@/lib/priority"

export type SelectedDependencySnapshot = {
  selectedTaskId?: string | null
  detailTaskId?: string | null
  dependencies?: Dependencies | null
  loading: boolean
}

export function selectedDependencyCountForTask(taskId: string, snapshot: SelectedDependencySnapshot) {
  if (taskId !== snapshot.selectedTaskId) return undefined
  if (snapshot.loading) return undefined
  if (snapshot.detailTaskId !== snapshot.selectedTaskId) return undefined
  if (!snapshot.dependencies) return undefined

  return snapshot.dependencies.parents.length + snapshot.dependencies.children.length
}

export function selectedUnlockCountForTask(taskId: string, snapshot: SelectedDependencySnapshot) {
  if (taskId !== snapshot.selectedTaskId) return undefined
  if (snapshot.loading) return undefined
  if (snapshot.detailTaskId !== snapshot.selectedTaskId) return undefined
  if (!snapshot.dependencies) return undefined

  return snapshot.dependencies.children.length
}

export function taskNeedsExecutionPlan(task: Task) {
  return task.execution_plan_state === "unplanned" && task.status !== "done" && task.status !== "archived"
}

export function requiredStepProgressLabel(task: Task) {
  if (task.required_step_count <= 0) return null
  return `steps ${task.completed_required_step_count}/${task.required_step_count}`
}

export function sortBoardColumnTasks(tasks: Task[], status: TaskStatus) {
  if (status !== "todo") return [...tasks]

  const unblocked = tasks.filter((task) => !task.dependency_blocked)
  const blocked = tasks.filter((task) => task.dependency_blocked)
  return [...unblocked, ...blocked]
}

export function dependencyBlockedTodoClass(task: Task) {
  if (task.status === "todo" && task.dependency_blocked) {
    return "border-[var(--dependency-blocked-border)] hover:border-[var(--dependency-blocked-hover-border)]"
  }
  return null
}
