import { describe, expect, it, vi } from "vitest"

import type { KanbanApi, Task } from "@/lib/api"

import { applySuggestedTaskLabel } from "./TaskDetail"

describe("label suggestions", () => {
  it("applies a suggested label through the task label API and existing action refresh path", async () => {
    const updatedTask = {
      ...task,
      labels: [
        { id: "l_backend", board_id: task.board_id, name: "backend", color: null, created_at: 1, updated_at: 1 },
      ],
    }
    const api: Pick<KanbanApi, "addTaskLabel"> = {
      addTaskLabel: vi.fn(async () => updatedTask),
    }
    const onAction = vi.fn(async (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null }) => {
      expect(options).toEqual({ fallbackTaskId: task.id, label: "label" })
      return action()
    })

    await expect(applySuggestedTaskLabel(api, task.id, "backend", onAction)).resolves.toBe(updatedTask)

    expect(api.addTaskLabel).toHaveBeenCalledWith(task.id, "backend")
    expect(onAction).toHaveBeenCalledTimes(1)
  })
})

const task: Task = {
  id: "t_1",
  board_id: "b_1",
  board_slug: "kanban-tool",
  ref: "kanban-tool#1",
  seq: 1,
  title: "Apply suggested label",
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
