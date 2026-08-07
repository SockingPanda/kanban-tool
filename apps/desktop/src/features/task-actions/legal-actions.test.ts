import { describe, expect, it, vi } from "vitest"

import type { KanbanApi, Task } from "@/lib/api"

import { legalActions } from "./legal-actions"

describe("legal task actions", () => {
  it("只在 triage 暴露 Specify，并通过 typed API 发送现有描述", async () => {
    const unsupported = legalActions(task({ status: "ready" }), null, "waiting").map((action) => action.label)
    expect(unsupported).not.toContain("Specify")

    const api = apiStub()
    const item = task({ status: "triage", description: " ready enough " })
    const action = actionFor(item, "Specify")

    expect(action.enabled).toBe(true)
    await action.run(api, item)

    expect(api.transition).toHaveBeenCalledWith(item, "specify", { description: "ready enough" })
  })

  it("要求 triage 任务有真实描述后才启用 Specify", () => {
    expect(actionFor(task({ status: "triage", description: null }), "Specify").enabled).toBe(false)
    expect(actionFor(task({ status: "triage", description: "" }), "Specify").enabled).toBe(false)
    expect(actionFor(task({ status: "triage", description: " ready enough " }), "Specify").enabled).toBe(true)
  })

  it("只在 blocked 暴露 Unblock，并通过 typed API 请求服务重算", async () => {
    const unsupported = legalActions(task({ status: "ready" }), null, "waiting").map((action) => action.label)
    expect(unsupported).not.toContain("Unblock")

    const api = apiStub()
    const item = task({ status: "blocked" })
    const action = actionFor(item, "Unblock")

    expect(action.enabled).toBe(true)
    await action.run(api, item)

    expect(api.transition).toHaveBeenCalledWith(item, "unblock")
  })

  it("仅从非 archived 源状态暴露 Archive，并为 running 使用 force", async () => {
    expect(legalActions(task({ status: "archived" }), null, "waiting")).toEqual([])

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
