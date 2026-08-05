import { createElement } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it, vi } from "vitest"

import { Sheet } from "@/components/ui/sheet"
import type { KanbanApi, LabelSuggestionResult, Task } from "@/lib/api"

import { emptyDetail } from "./detail-state"
import { TaskDetail } from "./TaskDetail"
import { applySuggestedTaskLabel } from "./TaskLabelsPanel"

describe("label suggestions cutline", () => {
  it("keeps the legacy helper on the existing task label action path", async () => {
    const updatedTask = {
      ...task,
      labels: [{ id: "l_backend", board_id: task.board_id, name: "backend", color: null, created_at: 1, updated_at: 1 }],
    }
    const api: Pick<KanbanApi, "addTaskLabel"> = {
      addTaskLabel: vi.fn(async () => updatedTask),
    }
    const onAction = vi.fn(async (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null; invalidate?: string }) => {
      expect(options).toEqual({ fallbackTaskId: task.id, label: "label", invalidate: "task" })
      return action()
    })

    await expect(applySuggestedTaskLabel(api, task.id, "backend", onAction)).resolves.toBe(updatedTask)

    expect(api.addTaskLabel).toHaveBeenCalledWith(task.id, "backend")
    expect(onAction).toHaveBeenCalledTimes(1)
  })

  it("does not render legacy labels or suggestions controls", () => {
    const html = renderTaskDetailWithSuggestions()

    expect(html).not.toContain("Labels")
    expect(html).not.toContain("Suggest labels")
    expect(html).not.toContain("Suggestions")
    expect(html).not.toContain("backend")
    expect(html).not.toContain(">Edit<")
  })
})

function renderTaskDetailWithSuggestions() {
  const suggestions: LabelSuggestionResult = {
    task_id: task.id,
    board_id: task.board_id,
    selected_labels: [{
      label_id: "l_backend",
      label_name: "backend",
      score: 0.9,
      weight: 0.9,
      already_applied: false,
      evidence_atoms: [],
      negative_evidence_atoms: [],
    }],
    candidates: [],
    coverage: 0.9,
    coverage_cosine: 0.9,
    residual_norm: 0.1,
    needs_new_label: false,
    reason_codes: [],
    degraded: false,
    diagnostics: [],
  }

  return renderToStaticMarkup(
    createElement(
      Sheet,
      { open: true },
      createElement(
        TaskDetail,
        {
          api: null,
          task,
          detail: emptyDetail,
          labelSuggestions: suggestions,
          labelSuggestionsRequested: true,
          blockReason: "",
          setBlockReason: () => undefined,
          dependencyInput: "",
          setDependencyInput: () => undefined,
          claimToken: null,
          commentBody: "",
          setCommentBody: () => undefined,
          editDraft: null,
          draftDirty: false,
          setEditDraft: () => undefined,
          detailLoading: false,
          pendingAction: null,
          onAction: async () => undefined,
          onAddDependency: async () => undefined,
          onRemoveDependency: async () => undefined,
          onSelectTask: () => undefined,
          onSaveTask: async () => true,
          onCancelEdit: () => undefined,
          onAddComment: async () => undefined,
        },
      ),
    ),
  )
}

const task: Task = {
  id: "t_1",
  board_id: "b_1",
  board_slug: "kanban-tool",
  ref: "default#1",
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
}
