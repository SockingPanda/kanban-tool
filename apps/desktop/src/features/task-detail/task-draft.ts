import type { Task } from "@/lib/api"

export type TaskEditDraft = {
  title: string
  description: string
  assignee: string
  priority: string
  scheduledAt: string
  dueAt: string
}

export function taskToDraft(task: Task): TaskEditDraft {
  return {
    title: task.title,
    description: task.description ?? "",
    assignee: task.assignee ?? "",
    priority: String(task.priority),
    scheduledAt: formatDateInput(task.scheduled_at),
    dueAt: formatDateInput(task.due_at),
  }
}

export function formatDateInput(value: number | null) {
  if (!value) return ""
  const date = new Date(value)
  const offset = date.getTimezoneOffset() * 60_000
  return new Date(date.getTime() - offset).toISOString().slice(0, 16)
}

export function parseDateInput(value: string) {
  if (!value) return null
  const time = new Date(value).getTime()
  return Number.isFinite(time) ? time : null
}
