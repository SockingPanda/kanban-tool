import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"

import { parseDateInput, reconcileSavedTaskDraft, reconcileTaskDraft, taskToDraft } from "./task-draft"

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
      priority: "3",
      scheduledAt: "",
      dueAt: "",
    })
  })

  it("parses empty and invalid date inputs as null", () => {
    expect(parseDateInput("")).toBeNull()
    expect(parseDateInput("not-a-date")).toBeNull()
  })

  it("does not overwrite a dirty draft when the same task refreshes in the background", () => {
    const current = {
      taskId: "t_1",
      draft: taskToDraft(task({ title: "Local edit" })),
      dirty: true,
    }

    expect(reconcileTaskDraft(current, task({ title: "Server refresh" }))).toBe(current)
  })

  it("replaces the draft when selection changes or after a forced save sync", () => {
    const current = {
      taskId: "t_1",
      draft: taskToDraft(task({ title: "Local edit" })),
      dirty: true,
    }

    expect(reconcileTaskDraft(current, task({ id: "t_2", title: "Other task" }))).toEqual({
      taskId: "t_2",
      draft: taskToDraft(task({ id: "t_2", title: "Other task" })),
      dirty: false,
    })
    expect(reconcileTaskDraft(current, task({ id: "t_1", title: "Saved server task" }), { force: true })).toEqual({
      taskId: "t_1",
      draft: taskToDraft(task({ id: "t_1", title: "Saved server task" })),
      dirty: false,
    })
  })

  it("only reconciles a saved task when the current draft still belongs to it", () => {
    const current = {
      taskId: "t_2",
      draft: taskToDraft(task({ id: "t_2", title: "Newly selected task" })),
      dirty: true,
    }

    expect(reconcileSavedTaskDraft(current, task({ id: "t_1", title: "Late save result" }))).toBe(current)
    expect(reconcileSavedTaskDraft(current, task({ id: "t_2", title: "Saved current task" }))).toEqual({
      taskId: "t_2",
      draft: taskToDraft(task({ id: "t_2", title: "Saved current task" })),
      dirty: false,
    })
    expect(reconcileSavedTaskDraft(null, task({ id: "t_2", title: "Saved current task" }))).toBeNull()
  })

  it("force-reconciles a dirty draft back to the current task for cancel", () => {
    const current = {
      taskId: "t_1",
      draft: taskToDraft(task({ title: "Unsaved title", description: "Unsaved description" })),
      dirty: true,
    }

    expect(
      reconcileTaskDraft(current, task({ id: "t_1", title: "Server title", description: "Server description" }), {
        force: true,
      }),
    ).toEqual({
      taskId: "t_1",
      draft: taskToDraft(task({ id: "t_1", title: "Server title", description: "Server description" })),
      dirty: false,
    })
  })
})

function task(overrides: Partial<Task> = {}): Task {
  return {
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
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "unplanned",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
    ...overrides,
  }
}
