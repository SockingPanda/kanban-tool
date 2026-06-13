import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"
import { reconcileClaimTokenForTask, reconcileClaimTokensForTasks } from "@/lib/claim-tokens"

describe("claim token reconciliation", () => {
  it("keeps a token only while the task is running under the desktop actor", () => {
    const tokens = { t_1: "claim_1" }

    expect(reconcileClaimTokenForTask(tokens, task({ status: "running", claim_owner: "desktop" }), "desktop")).toBe(tokens)
    expect(reconcileClaimTokenForTask(tokens, task({ status: "review", claim_owner: null }), "desktop")).toEqual({})
    expect(reconcileClaimTokenForTask(tokens, task({ status: "done", claim_owner: null }), "desktop")).toEqual({})
    expect(reconcileClaimTokenForTask(tokens, task({ status: "running", claim_owner: "dispatcher" }), "desktop")).toEqual({})
    expect(reconcileClaimTokenForTask(tokens, task({ status: "running", claim_owner: null }), "desktop")).toEqual({})
  })

  it("reconciles every returned task without dropping unknown tokens", () => {
    expect(
      reconcileClaimTokensForTasks(
        { t_1: "claim_1", t_2: "claim_2", t_hidden: "claim_hidden" },
        [
          task({ id: "t_1", status: "running", claim_owner: "desktop" }),
          task({ id: "t_2", status: "blocked", claim_owner: null }),
        ],
        "desktop",
      ),
    ).toEqual({ t_1: "claim_1", t_hidden: "claim_hidden" })
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
    ...overrides,
  }
}
