import type { OperatorView } from "@/features/navigation/view-types"
import type { Task } from "@/lib/api"

const detailCapableViews = new Set<OperatorView>(["board", "list", "map", "runs"])
const taskCollectionViews = new Set<OperatorView>(["board", "list"])

export function reconcileSelectedTaskId(selectedId: string | null, tasks: Pick<Task, "id">[]): string | null {
  if (!selectedId) return null
  return tasks.some((task) => task.id === selectedId) ? selectedId : null
}

export function shouldOpenTaskDetailSheet(view: OperatorView, selectedTask: Pick<Task, "id"> | null): boolean {
  return shouldLoadTaskDetail(view, selectedTask?.id ?? null)
}

export function shouldLoadTaskCollection(view: OperatorView): boolean {
  return taskCollectionViews.has(view)
}

export function shouldLoadTaskDetail(view: OperatorView, selectedTaskId: string | null): boolean {
  return detailCapableViews.has(view) && Boolean(selectedTaskId)
}
