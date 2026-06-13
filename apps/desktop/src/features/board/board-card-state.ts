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

export function sortBoardColumnTasks(tasks: Task[], status: TaskStatus) {
  const sorted = [...tasks]
  if (status !== "todo") return sorted

  return sorted.sort((left, right) => {
    const dependencyRank = Number(Boolean(left.dependency_blocked)) - Number(Boolean(right.dependency_blocked))
    if (dependencyRank !== 0) return dependencyRank
    if (left.position !== right.position) return left.position - right.position
    if (left.created_at !== right.created_at) return left.created_at - right.created_at
    return left.seq - right.seq
  })
}

export function dependencyBlockedTodoClass(task: Task) {
  if (task.status === "todo" && task.dependency_blocked) {
    return "border-red-500 hover:border-red-600"
  }
  return null
}
