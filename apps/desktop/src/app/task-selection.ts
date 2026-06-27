import type { OperatorView } from "@/features/navigation/view-types"
import type { Task } from "@/lib/api"

const detailCapableViews = new Set<OperatorView>(["board", "list", "map", "runs"])

export function reconcileSelectedTaskId(selectedId: string | null, tasks: Pick<Task, "id">[]): string | null {
  if (!selectedId) return null
  return tasks.some((task) => task.id === selectedId) ? selectedId : null
}

export function shouldOpenTaskDetailSheet(view: OperatorView, selectedTask: Pick<Task, "id"> | null): boolean {
  return detailCapableViews.has(view) && Boolean(selectedTask)
}
