import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"

import { planDragTransition } from "./drag-policy"

describe("drag transition policy", () => {
  it("maps ready to running drops to claim", () => {
    expect(planDragTransition(task({ status: "ready" }), "running", null)).toMatchObject({
      ok: true,
      action: "claim",
      body: { ttl_ms: 300_000, worker_profile: "manual" },
    })
  })

  it("requires viable triage spec before specifying to todo", () => {
    expect(planDragTransition(task({ status: "triage", description: "" }), "todo", null)).toEqual({
      ok: false,
      reason: "Triage tasks need a description before specify.",
    })
    expect(planDragTransition(task({ status: "triage", description: null }), "todo", null)).toEqual({
      ok: false,
      reason: "Triage tasks need a description before specify.",
    })
    expect(planDragTransition(task({ status: "triage", description: "ready enough" }), "todo", null)).toMatchObject({
      ok: true,
      action: "specify",
      body: { description: "ready enough" },
    })
  })

  it("adds explicit force bodies for running drops without a claim token", () => {
    expect(planDragTransition(task({ status: "running" }), "done", null)).toMatchObject({
      ok: true,
      action: "complete",
      body: { force: true },
      confirm: expect.stringContaining("Force complete"),
    })
    expect(planDragTransition(task({ status: "running" }), "blocked", null)).toMatchObject({
      ok: true,
      action: "block",
      body: { force: true },
      confirm: expect.stringContaining("Force block"),
      promptReason: true,
    })
  })

  it("always prompts for fresh reasons when dropping blockable non-running tasks on blocked", () => {
    expect(planDragTransition(task({ status: "ready" }), "blocked", null)).toMatchObject({
      ok: true,
      action: "block",
      body: {},
      promptReason: true,
    })
    expect(planDragTransition(task({ status: "review" }), "blocked", null)).toMatchObject({
      ok: true,
      action: "block",
      body: {},
      promptReason: true,
    })
  })

  it("prompts for fresh reasons when running tasks with a claim token are dropped on blocked", () => {
    expect(planDragTransition(task({ status: "running" }), "blocked", "claim_123")).toMatchObject({
      ok: true,
      action: "block",
      body: { claim_token: "claim_123" },
      promptReason: true,
    })
  })

  it("does not route already blocked or terminal tasks through block", () => {
    expect(planDragTransition(task({ status: "blocked" }), "blocked", null)).toEqual({
      ok: false,
      reason: "Already in that column.",
    })
    expect(planDragTransition(task({ status: "done" }), "blocked", null)).toEqual({
      ok: false,
      reason: "done cannot be dropped on blocked.",
    })
    expect(planDragTransition(task({ status: "archived" }), "blocked", null)).toEqual({
      ok: false,
      reason: "archived cannot be dropped on blocked.",
    })
  })

  it("always force-confirms running archive drops", () => {
    expect(planDragTransition(task({ status: "running" }), "archived", "claim_123")).toMatchObject({
      ok: true,
      action: "archive",
      body: { force: true },
      confirm: expect.stringContaining("Force archive"),
    })
    expect(planDragTransition(task({ status: "running" }), "archived", null)).toMatchObject({
      ok: true,
      action: "archive",
      body: { force: true },
      confirm: expect.stringContaining("Force archive"),
    })
  })

  it("routes blocked drops through unblock instead of setting a target status", () => {
    expect(planDragTransition(task({ status: "blocked" }), "ready", null)).toMatchObject({
      ok: true,
      action: "unblock",
      body: {},
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
    ...overrides,
  }
}
