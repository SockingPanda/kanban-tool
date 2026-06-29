import type { Task, TaskListSort, TaskPlanFilter, TaskStatus } from "@/lib/api"
import type { PriorityFilter } from "@/lib/priority"

export type { PriorityFilter }

export type { TaskPlanFilter }


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
  | "execution_plan"
  | "step_progress"
  | "dependency_blocked"
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
  execution_plan: "Execution plan",
  step_progress: "Step progress",
  dependency_blocked: "Dependency blocked",
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
  execution_plan: true,
  step_progress: true,
  dependency_blocked: true,
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

export function togglePlanFilter(current: TaskPlanFilter[], filter: TaskPlanFilter) {
  return current.includes(filter) ? current.filter((value) => value !== filter) : [...current, filter]
}

export function hasActiveListFilters(search: string, status: TaskStatus | "all", priorities: number[], planFilters: TaskPlanFilter[] = []) {
  return Boolean(search.trim()) || status !== "all" || priorities.length > 0 || planFilters.length > 0
}

export function filterListTasks(tasks: Task[], status: TaskStatus | "all", priority: PriorityFilter, planFilters: TaskPlanFilter[] = []) {
  return tasks.filter((task) => matchesStatus(task, status) && matchesPriority(task, priority) && matchesPlanFilters(task, planFilters))
}

export function selectedRowCount(selection: Record<string, boolean>) {
  return Object.values(selection).filter(Boolean).length
}

export function stepProgressForTask(task: Task) {
  if (task.required_step_count <= 0) return null

  return {
    completed: task.completed_required_step_count,
    percent: Math.min(100, Math.max(0, Math.round((task.completed_required_step_count / task.required_step_count) * 100))),
    total: task.required_step_count,
  }
}

function matchesStatus(task: Task, status: TaskStatus | "all") {
  return status === "all" || task.status === status
}

function matchesPriority(task: Task, priority: PriorityFilter) {
  if (priority !== "all") return task.priority === priority
  return true
}


function matchesPlanFilters(task: Task, filters: TaskPlanFilter[]) {
  return filters.every((filter) => {
    if (filter === "plan_needed") return task.execution_plan_state === "unplanned" && task.status !== "done" && task.status !== "archived"
    if (filter === "has_steps") return task.required_step_count + task.optional_step_count > 0
    if (filter === "incomplete_required_steps") return task.completed_required_step_count < task.required_step_count
    return true
  })
}
