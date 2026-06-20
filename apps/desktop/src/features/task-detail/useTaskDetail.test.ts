import { describe, expect, it, vi } from "vitest"

import type { KanbanApi } from "@/lib/api"

import { fetchTaskDetail, requestTaskLabelSuggestions } from "./useTaskDetail"

describe("task detail loading", () => {
  it("does not request label suggestions during default task detail loading", async () => {
    const api = {
      getTask: vi.fn(async () => task),
      listDependencies: vi.fn(async () => ({ parents: [], children: [] })),
      listRuns: vi.fn(async () => []),
      listEvents: vi.fn(async () => ({ events: [], next_after: 0 })),
      listComments: vi.fn(async () => []),
      suggestTaskLabels: vi.fn(async () => {
        throw new Error("suggestions should be explicit")
      }),
    } as unknown as KanbanApi

    const result = await fetchTaskDetail(api, task.id)

    expect(result.task).toBe(task)
    expect(result.detail.labelSuggestions).toBeNull()
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
  ref: "kanban-tool#1",
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
  labels: [],
}
