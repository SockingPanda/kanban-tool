export const priorityLevels = [0, 1, 2, 3] as const

export type PriorityLevel = (typeof priorityLevels)[number]
export type PriorityFilter = "all" | PriorityLevel

export function isPriorityLevel(value: number): value is PriorityLevel {
  return priorityLevels.includes(value as PriorityLevel)
}

export function priorityLabel(priority: number) {
  return isPriorityLevel(priority) ? `P${priority}` : "P3"
}

export function priorityBadgeClass(priority: number) {
  if (priority === 0) return "bg-red-50 text-red-700 ring-red-200"
  if (priority === 1) return "bg-orange-50 text-orange-700 ring-orange-200"
  if (priority === 2) return "bg-amber-50 text-amber-800 ring-amber-200"
  return "bg-neutral-100 text-neutral-700 ring-neutral-200"
}

export function prioritySelectValue(priority: number) {
  return String(isPriorityLevel(priority) ? priority : 3)
}
