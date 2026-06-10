import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"

import { parseDateInput, taskToDraft } from "./task-draft"

describe("task draft helpers", () => {
  it("converts nullable task fields into editable strings", () => {
    expect(
      taskToDraft(task({
        title: "Keep boundary behavior",
        description: null,
        assignee: null,
        priority: 7,
        scheduled_at: null,
        due_at: null,
      })),
    ).toEqual({
      title: "Keep boundary behavior",
      description: "",
      assignee: "",
      priority: "7",
      scheduledAt: "",
      dueAt: "",
    })
  })

  it("parses empty and invalid date inputs as null", () => {
    expect(parseDateInput("")).toBeNull()
    expect(parseDateInput("not-a-date")).toBeNull()
  })
})

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "t_1",
    board_id: "b_1",
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
    created_by: "seed",
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
    ...overrides,
  }
}
