import { describe, expect, it } from "vitest"

import type { BoardTaskMap, Task } from "@/lib/api"

import { __test } from "./BoardTaskMapView"

function task(id: string, status: Task["status"], overrides: Partial<Task> = {}): Task {
  return {
    id,
    ref: "kanban-tool#" + id,
    seq: Number(id.replace(/\D/g, "")) || 1,
    board_id: "b_1",
    board_slug: "kanban-tool",
    title: id,
    description: "",
    status,
    status_reason: null,
    assignee: null,
    priority: 2,
    position: 0,
    due_at: null,
    scheduled_at: null,
    started_at: null,
    completed_at: null,
    archived_at: null,
    created_at: 1,
    updated_at: 1,
    created_by: "test",
    labels: [],
    metadata_json: "{}",
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "planned",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    max_retries: null,
    retry_count: 0,
    current_run_id: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    lock_version: 0,
    result_summary: null,
    result_json: null,
    ...overrides,
  }
}

const graph: BoardTaskMap = {
  nodes: [
    { task: task("ready", "ready"), role: "active", context_only: false },
    { task: task("blocked", "todo", { dependency_blocked: true }), role: "active", context_only: false },
    { task: task("running", "running"), role: "active", context_only: false },
    { task: task("done", "done"), role: "context", context_only: true },
    { task: task("unplanned", "todo", { execution_plan_state: "unplanned" }), role: "active", context_only: false },
    { task: task("parent", "todo", { required_step_count: 2, completed_required_step_count: 1 }), role: "active", context_only: false },
  ],
  edges: [
    { id: "dep:done:ready", source_task_id: "done", target_task_id: "ready", kind: "dependency", required: true, blocking: false },
    { id: "step:parent:running", source_task_id: "parent", target_task_id: "running", kind: "step", required: true, blocking: false },
  ],
  meta: { generated_at: 1, truncated: false, node_count: 6, edge_count: 2 },
}

describe("BoardTaskMapView filters", () => {
  it("keeps done context in all results when the API includes it", () => {
    const result = __test.filterBoardMap(graph, "all", false)

    expect(result?.nodes.map((node) => node.task.id)).toContain("done")
    expect(result?.edges.map((edge) => edge.id)).toContain("dep:done:ready")
  })

  it("filters operational states without including context-only done nodes", () => {
    expect(__test.filterBoardMap(graph, "ready", false)?.nodes.map((node) => node.task.id)).toEqual(["ready"])
    expect(__test.filterBoardMap(graph, "blocked", false)?.nodes.map((node) => node.task.id)).toEqual(["blocked"])
    expect(__test.filterBoardMap(graph, "running", false)?.nodes.map((node) => node.task.id)).toEqual(["running"])
    expect(__test.filterBoardMap(graph, "unplanned", false)?.nodes.map((node) => node.task.id)).toEqual(["unplanned"])
    expect(__test.filterBoardMap(graph, "incomplete-steps", false)?.nodes.map((node) => node.task.id)).toEqual(["parent"])
  })

  it("can hide isolated nodes after filtering", () => {
    const result = __test.filterBoardMap(graph, "all", true)

    expect(result?.nodes.map((node) => node.task.id).sort()).toEqual(["done", "parent", "ready", "running"])
  })

  it("bounds deterministic zoom steps", () => {
    expect(__test.clampMapZoom(0)).toBe(0.65)
    expect(__test.clampMapZoom(2)).toBe(1.5)
    expect(__test.clampMapZoom(Number.NaN)).toBe(1)
    expect(__test.stepMapZoom(1, 1)).toBe(1.15)
    expect(__test.stepMapZoom(0.7, -1)).toBe(0.65)
  })
})
