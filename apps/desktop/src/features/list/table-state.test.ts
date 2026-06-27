import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"

import {
  defaultListSort,
  filterListTasks,
  hasActiveListFilters,
  listSortToApiSort,
  selectedRowCount,
  sortForColumn,
  togglePlanFilter,
  togglePriorityFilter,
} from "./table-state"

const baseTask: Task = {
  id: "t_1",
  board_id: "b_1",
  board_slug: "default",
  ref: "default#1",
  seq: 1,
  title: "Task",
  description: null,
  status: "ready",
  status_reason: null,
  assignee: null,
  priority: 0,
  position: 0,
  scheduled_at: null,
  due_at: null,
  created_by: "test",
  created_at: 1,
  updated_at: 1,
  started_at: null,
  completed_at: null,
  archived_at: null,
  claim_owner: null,
  claim_expires_at: null,
  last_heartbeat_at: null,
  current_run_id: null,
  retry_count: 0,
  max_retries: null,
  result_summary: null,
  result_json: null,
  metadata_json: "{}",
  lock_version: 0,
  dependency_blocked: false,
  unfinished_parent_count: 0,
  execution_plan_state: "unplanned",
  required_subtask_count: 0,
  completed_required_subtask_count: 0,
  optional_subtask_count: 0,
  labels: [],
}

function task(id: string, status: Task["status"], priority: number, overrides: Partial<Task> = {}): Task {
  return { ...baseTask, id, seq: Number(id.slice(2)), ref: `default#${id.slice(2)}`, status, priority, ...overrides }
}

describe("list table state helpers", () => {
  it("filters the current page by status and priority", () => {
    const tasks = [
      task("t_1", "ready", 3),
      task("t_2", "ready", 0),
      task("t_3", "blocked", 1),
      task("t_4", "review", 2),
    ]

    expect(filterListTasks(tasks, "ready", "all").map((item) => item.id)).toEqual(["t_1", "t_2"])
    expect(filterListTasks(tasks, "all", 2).map((item) => item.id)).toEqual(["t_4"])
    expect(filterListTasks(tasks, "blocked", 1).map((item) => item.id)).toEqual(["t_3"])
  })

  it("counts selected rows without treating false entries as selected", () => {
    expect(selectedRowCount({ t_1: true, t_2: false, t_3: true })).toBe(2)
  })

  it("maps visible column sorting to API sort tokens", () => {
    expect(defaultListSort).toEqual({ field: "updated_at", direction: "desc" })
    expect(listSortToApiSort(defaultListSort)).toBe("-updated_at")
    expect(listSortToApiSort({ field: "priority", direction: "asc" })).toBe("priority")
    expect(sortForColumn("ref")).toBe("seq")
    expect(sortForColumn("schedule")).toBe("scheduled_at")
    expect(sortForColumn("select")).toBeNull()
  })

  it("toggles a multi-priority filter in stable order", () => {
    expect(togglePriorityFilter([], 2)).toEqual([2])
    expect(togglePriorityFilter([2], 0)).toEqual([0, 2])
    expect(togglePriorityFilter([0, 2], 2)).toEqual([0])
  })

  it("toggles execution plan filters", () => {
    expect(togglePlanFilter([], "plan_needed")).toEqual(["plan_needed"])
    expect(togglePlanFilter(["plan_needed"], "has_subtasks")).toEqual(["plan_needed", "has_subtasks"])
    expect(togglePlanFilter(["plan_needed", "has_subtasks"], "plan_needed")).toEqual(["has_subtasks"])
  })

  it("filters the current page by execution plan state and subtask progress", () => {
    const tasks = [
      task("t_1", "ready", 1, { execution_plan_state: "unplanned" }),
      task("t_2", "ready", 1, { execution_plan_state: "planned", required_subtask_count: 2, completed_required_subtask_count: 1 }),
      task("t_3", "ready", 1, { execution_plan_state: "planned", required_subtask_count: 2, completed_required_subtask_count: 2 }),
      task("t_4", "done", 1, { execution_plan_state: "unplanned" }),
      task("t_5", "todo", 1, { execution_plan_state: "not_required", optional_subtask_count: 1 }),
    ]

    expect(filterListTasks(tasks, "all", "all", ["plan_needed"]).map((item) => item.id)).toEqual(["t_1"])
    expect(filterListTasks(tasks, "all", "all", ["has_subtasks"]).map((item) => item.id)).toEqual(["t_2", "t_3", "t_5"])
    expect(filterListTasks(tasks, "all", "all", ["incomplete_required_subtasks"]).map((item) => item.id)).toEqual(["t_2"])
    expect(filterListTasks(tasks, "ready", "all", ["has_subtasks", "incomplete_required_subtasks"]).map((item) => item.id)).toEqual(["t_2"])
  })

  it("detects active list filters for reset controls", () => {
    expect(hasActiveListFilters("", "all", [])).toBe(false)
    expect(hasActiveListFilters("ready", "all", [])).toBe(true)
    expect(hasActiveListFilters("", "blocked", [])).toBe(true)
    expect(hasActiveListFilters("", "all", [0])).toBe(true)
    expect(hasActiveListFilters("", "all", [], ["plan_needed"])).toBe(true)
  })
})
