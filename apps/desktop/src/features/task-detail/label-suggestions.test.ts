import { createElement } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it, vi } from "vitest"

import { Sheet } from "@/components/ui/sheet"
import { sheetContentClassName } from "@/components/ui/sheet-motion"
import type { KanbanApi, LabelSuggestionResult, Task } from "@/lib/api"

import { emptyDetail } from "./detail-state"
import { applySuggestedTaskLabel, TaskDetail } from "./TaskDetail"

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
    const onAction = vi.fn(async (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null; invalidate?: string }) => {
      expect(options).toEqual({ fallbackTaskId: task.id, label: "label", invalidate: "task" })
      return action()
    })

    await expect(applySuggestedTaskLabel(api, task.id, "backend", onAction)).resolves.toBe(updatedTask)

    expect(api.addTaskLabel).toHaveBeenCalledWith(task.id, "backend")
    expect(onAction).toHaveBeenCalledTimes(1)
  })

  it("keeps long suggestion content constrained inside the task detail sheet", () => {
    const html = renderTaskDetailWithSuggestions(longSuggestionResult)

    expect(classListForElementWithAttribute(html, 'data-test-sheet="task-detail"')).toEqual(
      expect.arrayContaining(["fixed", "flex", "flex-col", "w-[min(1100px,calc(100vw-24px))]", "p-0"]),
    )
    expect(classListForPanelRoot(html)).toEqual(
      expect.arrayContaining(["min-w-0", "w-full", "max-w-full", "overflow-hidden"]),
    )
    expect(classListForText(html, longSuggestionResult.selected_labels[0].label_name)).toEqual(
      expect.arrayContaining(["min-w-0", "max-w-full", "truncate"]),
    )
    expect(classListForText(html, longSuggestionResult.selected_labels[0].evidence_atoms[0].text)).toEqual(
      expect.arrayContaining(["min-w-0", "max-w-full", "truncate"]),
    )
    expect(classListForText(html, longSuggestionResult.diagnostics[0])).toContain("break-words")
    expect(classListForLastTextInTag(html, "Apply", "button")).toContain("shrink-0")
  })

  it("renders coverage review without making a new-label recommendation", () => {
    const html = renderTaskDetailWithSuggestions(coverageReviewSuggestionResult)

    expect(html).toContain("Existing label coverage needs review: coverage gap, no selected labels, unexplained residual")
    expect(html).not.toContain("new label may be needed")
  })
})

function renderTaskDetailWithSuggestions(suggestions: LabelSuggestionResult) {
  return renderToStaticMarkup(
    createElement(
      Sheet,
      { open: true },
      createElement(
        "div",
        {
          "data-test-sheet": "task-detail",
          className: `${sheetContentClassName("right")} w-[min(1100px,calc(100vw-24px))] p-0`,
        },
        createElement(TaskDetail, {
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
        }),
      ),
    ),
  )
}

function classListForElementWithAttribute(html: string, attribute: string) {
  const attributeIndex = html.indexOf(attribute)
  expect(attributeIndex).toBeGreaterThanOrEqual(0)
  const tagStart = html.lastIndexOf("<", attributeIndex)
  const tagEnd = html.indexOf(">", attributeIndex)
  return classListFromTag(html.slice(tagStart, tagEnd + 1))
}

function classListForPanelRoot(html: string) {
  const suggestionsIndex = html.indexOf(">Suggestions<")
  expect(suggestionsIndex).toBeGreaterThanOrEqual(0)
  let searchEnd = suggestionsIndex

  while (searchEnd > 0) {
    const tagStart = html.lastIndexOf("<div", searchEnd)
    expect(tagStart).toBeGreaterThanOrEqual(0)
    const tagEnd = html.indexOf(">", tagStart)
    const classList = classListFromTag(html.slice(tagStart, tagEnd + 1))
    if (classList.includes("space-y-2") && classList.includes("border")) {
      return classList
    }
    searchEnd = tagStart - 1
  }

  throw new Error("Could not find label suggestions panel root")
}

function classListForText(html: string, text: string) {
  const textIndex = html.indexOf(text)
  expect(textIndex).toBeGreaterThanOrEqual(0)
  const tagStart = html.lastIndexOf("<", textIndex)
  const tagEnd = html.indexOf(">", tagStart)
  return classListFromTag(html.slice(tagStart, tagEnd + 1))
}

function classListForLastTextInTag(html: string, text: string, tagName: string) {
  const textIndex = html.lastIndexOf(text)
  expect(textIndex).toBeGreaterThanOrEqual(0)
  const tagStart = html.lastIndexOf(`<${tagName}`, textIndex)
  expect(tagStart).toBeGreaterThanOrEqual(0)
  const tagEnd = html.indexOf(">", tagStart)
  return classListFromTag(html.slice(tagStart, tagEnd + 1))
}

function classListFromTag(tag: string) {
  const classAttribute = tag.match(/\bclass="([^"]*)"/)?.[1] ?? ""
  return classAttribute.split(/\s+/).filter(Boolean)
}

const longSuggestionResult: LabelSuggestionResult = {
  task_id: "t_1",
  board_id: "b_1",
  selected_labels: [
    {
      label_id: "l_long",
      label_name: "desktop-task-detail-label-suggestion-with-a-very-long-unbroken-name",
      score: 0.91,
      weight: 0.91,
      already_applied: false,
      evidence_atoms: [
        {
          atom_id: "la_long",
          label_id: "l_long",
          label_name: "desktop-task-detail-label-suggestion-with-a-very-long-unbroken-name",
          polarity: "positive",
          kind: "applies_when",
          text: "longunbrokenevidenceatomtextthatmustnotwidenthetaskdetailsheet",
          score: 0.86,
        },
      ],
      negative_evidence_atoms: [],
    },
  ],
  candidates: [],
  coverage: 0.91,
  coverage_cosine: 0.88,
  residual_norm: 0.12,
  needs_new_label: false,
  reason_codes: ["degraded_result"],
  degraded: true,
  diagnostics: ["longunbrokendiagnosticmessagethatmustwrapinplace"],
}

const coverageReviewSuggestionResult: LabelSuggestionResult = {
  task_id: "t_1",
  board_id: "b_1",
  selected_labels: [],
  candidates: [],
  coverage: 0.2,
  coverage_cosine: 0.31,
  residual_norm: 0.8,
  needs_new_label: true,
  reason_codes: ["coverage_below_threshold", "no_selected_labels", "residual_above_threshold"],
  degraded: false,
  diagnostics: [],
}

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
