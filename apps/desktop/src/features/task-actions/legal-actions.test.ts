import { describe, expect, it, vi } from "vitest"

import type { KanbanApi, Task } from "@/lib/api"

import { legalActions } from "./legal-actions"

describe("legal task actions", () => {
  it("disables specify until a triage task has real description text", () => {
    expect(actionFor(task({ status: "triage", description: null }), "Specify").enabled).toBe(false)
    expect(actionFor(task({ status: "triage", description: "" }), "Specify").enabled).toBe(false)
    expect(actionFor(task({ status: "triage", description: " ready enough " }), "Specify").enabled).toBe(true)
  })

  it("sends existing description for specify without a synthetic fallback", async () => {
    const api = apiStub()
    const item = task({ status: "triage", description: " ready enough " })

    await actionFor(item, "Specify").run(api, item)

    expect(api.transition).toHaveBeenCalledWith(item, "specify", { description: "ready enough" })
  })

  it("force-confirms and force archives running tasks even with a claim token", async () => {
    const api = apiStub()
    const item = task({ status: "running" })
    const action = actionFor(item, "Archive", "claim_123")

    expect(action.enabled).toBe(true)
    expect(action.confirmation).toEqual({ key: "Force archive running task #{seq}?", values: { seq: 1 } })

    await action.run(api, item)

    expect(api.transition).toHaveBeenCalledWith(item, "archive", { force: true })
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
