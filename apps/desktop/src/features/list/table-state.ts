import type { Task, TaskListSort, TaskStatus } from "@/lib/api"
import type { PriorityFilter } from "@/lib/priority"

export type { PriorityFilter }

export type ListSortField =
  | "seq"
  | "title"
  | "status"
  | "priority"
  | "assignee"
  | "scheduled_at"
  | "due_at"
  | "updated_at"

export type ListSortDirection = "asc" | "desc"

export type ListSortState = {
  field: ListSortField
  direction: ListSortDirection
}

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

export const defaultListSort: ListSortState = { field: "updated_at", direction: "desc" }

export function listSortToApiSort(sort: ListSortState) {
  return `${sort.direction === "desc" ? "-" : ""}${sort.field}` as TaskListSort
}

export function sortForColumn(column: ListColumnId): ListSortField | null {
  if (column === "ref") return "seq"
  if (column === "title") return "title"
  if (column === "status") return "status"
  if (column === "priority") return "priority"
  if (column === "assignee") return "assignee"
  if (column === "schedule") return "scheduled_at"
  if (column === "updated") return "updated_at"
  return null
}

export function togglePriorityFilter(current: number[], priority: number) {
  return current.includes(priority)
    ? current.filter((value) => value !== priority)
    : [...current, priority].sort((left, right) => left - right)
}

export function hasActiveListFilters(search: string, status: TaskStatus | "all", priorities: number[]) {
  return Boolean(search.trim()) || status !== "all" || priorities.length > 0
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
