import { describe, expect, it, vi } from "vitest"

import type { KanbanApi } from "@/lib/api"

import { fetchTaskDetail, requestTaskLabelSuggestions, resolveTaskDetailQueryEnablement } from "./useTaskDetail"

describe("task detail loading", () => {
  it("enables only the selected task query by default", () => {
    expect(resolveTaskDetailQueryEnablement()).toEqual({
      task: true,
      dependencies: false,
      neighborhood: false,
      steps: false,
      runs: false,
      events: false,
      comments: false,
      runLog: false,
    })
  })

  it("enables subqueries only from explicit panel state", () => {
    expect(
      resolveTaskDetailQueryEnablement({
        enabled: true,
        dependenciesEnabled: true,
        neighborhoodEnabled: true,
        stepsEnabled: true,
        runsEnabled: true,
        eventsEnabled: true,
        commentsEnabled: true,
        runLogEnabled: true,
      }),
    ).toEqual({
      task: true,
      dependencies: true,
      neighborhood: true,
      steps: true,
      runs: true,
      events: true,
      comments: true,
      runLog: true,
    })
  })

  it("can disable the selected task query while preserving panel intent", () => {
    expect(resolveTaskDetailQueryEnablement({ enabled: false, runsEnabled: true, runLogEnabled: true })).toEqual({
      task: false,
      dependencies: false,
      neighborhood: false,
      steps: false,
      runs: true,
      events: false,
      comments: false,
      runLog: true,
    })
  })

  it("keeps the legacy aggregate fetch helper lazy for label suggestions and run logs", async () => {
    const api = {
      getTask: vi.fn(async () => task),
      listDependencies: vi.fn(async () => ({ parents: [], children: [] })),
      getTaskNeighborhood: vi.fn(async () => ({
        center_task_id: task.id,
        nodes: [],
        edges: [],
        meta: { generated_at: 1, truncated: false, node_count: 0, edge_count: 0, depth: 1 },
      })),
      listSteps: vi.fn(async () => ({
        task_id: task.id,
        steps: [],
        execution_plan: { board_id: task.board_id, task_id: task.id, state: "unplanned", reason: null, updated_by: "system", updated_at: 1 },
      })),
      listRuns: vi.fn(async () => []),
      getRunLog: vi.fn(async () => {
        throw new Error("run log should be lazy")
      }),
      listEvents: vi.fn(async () => ({ events: [], next_after: 0 })),
      listComments: vi.fn(async () => []),
      suggestTaskLabels: vi.fn(async () => {
        throw new Error("suggestions should be explicit")
      }),
    } as unknown as KanbanApi

    const result = await fetchTaskDetail(api, task.id)

    expect(result.task).toBe(task)
    expect(result.detail.labelSuggestions).toBeNull()
    expect(result.detail.neighborhood?.center_task_id).toBe(task.id)
    expect(result.detail.steps?.task_id).toBe(task.id)
    expect(api.getTaskNeighborhood).toHaveBeenCalledWith(task.id, { depth: 1, limitNodes: 40, signal: undefined })
    expect(api.listSteps).toHaveBeenCalledWith(task.id, { signal: undefined })
    expect(api.getRunLog).not.toHaveBeenCalled()
    expect(api.suggestTaskLabels).not.toHaveBeenCalled()
  })

  it("requests label suggestions only through the explicit suggestion action", async () => {
    const suggestions = {
      task_id: task.id,
      board_id: task.board_id,
      selected_labels: [],
      candidates: [],
      coverage: 0,
      coverage_cosine: 0,
      residual_norm: 1,
      needs_new_label: false,
      reason_codes: ["degraded_result", "vector_store_disabled"],
      degraded: true,
      diagnostics: ["vector_store_disabled"],
    }
    const api = {
      suggestTaskLabels: vi.fn(async () => suggestions),
    } as unknown as KanbanApi

    await expect(requestTaskLabelSuggestions(api, task.id)).resolves.toBe(suggestions)
    expect(api.suggestTaskLabels).toHaveBeenCalledWith(task.id, { signal: undefined })
  })
})

const task = {
  id: "t_1",
  board_id: "b_1",
  board_slug: "kanban-tool",
  ref: "default#123",
  seq: 1,
  title: "Manual label suggestions",
  description: null,
  status: "ready",
  status_reason: null,
  assignee: null,
  priority: 1,
  position: 0,
  scheduled_at: null,
  due_at: null,
  created_by: "codex",
  created_at: 1_781_441_329_826,
  updated_at: 1_781_441_329_826,
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
}
