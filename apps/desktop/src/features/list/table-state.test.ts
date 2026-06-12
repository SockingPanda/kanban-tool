import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"

import { filterListTasks, selectedRowCount } from "./table-state"

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
}

function task(id: string, status: Task["status"], priority: number): Task {
  return { ...baseTask, id, seq: Number(id.slice(2)), ref: `default#${id.slice(2)}`, status, priority }
}

describe("list table state helpers", () => {
  it("filters the current page by status and priority", () => {
    const tasks = [
      task("t_1", "ready", 5),
      task("t_2", "ready", 0),
      task("t_3", "blocked", -1),
      task("t_4", "review", 2),
    ]

    expect(filterListTasks(tasks, "ready", "all").map((item) => item.id)).toEqual(["t_1", "t_2"])
    expect(filterListTasks(tasks, "all", "positive").map((item) => item.id)).toEqual(["t_1", "t_4"])
    expect(filterListTasks(tasks, "blocked", "negative").map((item) => item.id)).toEqual(["t_3"])
  })

  it("counts selected rows without treating false entries as selected", () => {
    expect(selectedRowCount({ t_1: true, t_2: false, t_3: true })).toBe(2)
  })
})
