import type { Dependencies } from "@/lib/api"
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
