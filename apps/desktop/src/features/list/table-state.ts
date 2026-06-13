import type { Task, TaskStatus } from "@/lib/api"
import type { PriorityFilter } from "@/lib/priority"

export type { PriorityFilter }

export type ListColumnId =
  | "select"
  | "ref"
  | "title"
  | "status"
  | "priority"
  | "assignee"
  | "schedule"
  | "updated"
  | "actions"

export const listColumnLabels: Record<ListColumnId, string> = {
  select: "Select",
  ref: "Ref",
  title: "Title",
  status: "Status",
  priority: "Priority",
  assignee: "Assignee",
  schedule: "Scheduled / due",
  updated: "Updated",
  actions: "Actions",
}

export const defaultListColumnVisibility: Record<ListColumnId, boolean> = {
  select: true,
  ref: true,
  title: true,
  status: true,
  priority: true,
  assignee: true,
  schedule: true,
  updated: true,
  actions: true,
}

export function filterListTasks(tasks: Task[], status: TaskStatus | "all", priority: PriorityFilter) {
  return tasks.filter((task) => matchesStatus(task, status) && matchesPriority(task, priority))
}

export function selectedRowCount(selection: Record<string, boolean>) {
  return Object.values(selection).filter(Boolean).length
}

function matchesStatus(task: Task, status: TaskStatus | "all") {
  return status === "all" || task.status === status
}

function matchesPriority(task: Task, priority: PriorityFilter) {
  if (priority !== "all") return task.priority === priority
  return true
}
