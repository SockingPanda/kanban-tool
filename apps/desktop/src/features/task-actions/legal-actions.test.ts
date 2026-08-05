import { describe, expect, it, vi } from "vitest"

import type { KanbanApi, Task } from "@/lib/api"

import { legalActions } from "./legal-actions"

describe("legal task actions", () => {
  it("does not expose unsupported transition actions", () => {
    const labels = legalActions(task({ status: "blocked" }), null, "waiting").map((action) => action.label)

    expect(labels).not.toEqual(expect.arrayContaining(["Specify", "Unblock", "Archive"]))
  })

  it("enables non-running block actions with a reason body and no force confirmation", async () => {
    const api = apiStub()
    const item = task({ status: "review" })
    const action = actionFor(item, "Block", null, " needs changes ")

    expect(action.enabled).toBe(true)
    expect(action.confirmation).toBeUndefined()

    await action.run(api, item)

    expect(api.transition).toHaveBeenCalledWith(item, "block", { reason: "needs changes" })
  })

  it("keeps non-running block actions disabled until a reason is present", () => {
    expect(actionFor(task({ status: "ready" }), "Block", null, "  ").enabled).toBe(false)
  })
})

function actionFor(task: Task, label: string, claimToken: string | null = null, blockReason = "waiting") {
  const action = legalActions(task, claimToken, blockReason).find((candidate) => candidate.label === label)
  expect(action).toBeDefined()
  return action!
}

function apiStub() {
  return {
    transition: vi.fn(async () => ({})),
  } as unknown as KanbanApi & { transition: ReturnType<typeof vi.fn> }
}

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
    result: null,
    metadata: {},
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
