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
  if (priority === 0) return "bg-[var(--priority-p0-bg)] text-[var(--priority-p0-fg)] ring-[var(--priority-p0-ring)]"
  if (priority === 1) return "bg-[var(--priority-p1-bg)] text-[var(--priority-p1-fg)] ring-[var(--priority-p1-ring)]"
  if (priority === 2) return "bg-[var(--priority-p2-bg)] text-[var(--priority-p2-fg)] ring-[var(--priority-p2-ring)]"
  return "bg-muted text-muted-foreground ring-border"
}

export function prioritySelectValue(priority: number) {
  return String(isPriorityLevel(priority) ? priority : 3)
}
