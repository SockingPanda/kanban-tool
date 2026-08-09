import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import type { ApiGetTaskResponseContract } from "../../lib/api/generated/contracts/api-get-task-response"
import type { ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response"
import type { ApiSuggestTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-suggest-task-labels-response"
import {
  TaskInspectorAssetsPanel,
  type InspectorAssetsMutationHandlers,
} from "./TaskInspectorAssetsPanel"
import {
  MAX_ATTACHMENT_UPLOAD_BYTES,
  createAttachmentUploadIntent,
  createInspectorAssetsActions,
  exactAttachmentBytes,
  advanceInspectorAssetsScope,
  isAttachmentRetryDraftCurrent,
  isLabelRetryDraftCurrent,
  isInspectorAssetsScopeCurrent,
  requestSuggestedLabels,
  shouldShowInspectorSnapshotError,
  shouldClearAssetDraft,
  type SuggestLabelsHandler,
} from "./TaskInspectorAssetsPanel.logic"
import type { InspectorMutationOutcome, TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"

type Label = ApiGetTaskResponseContract["data"]["labels"][number]
type Attachment = ApiListAttachmentsResponseContract["data"][number]
type Suggestions = ApiSuggestTaskLabelsResponseContract["data"]

const label = (id: string, name: string): Label => ({
  id,
  board_id: "b_default",
  name,
  color: null,
  created_at: 1,
  updated_at: 1,
})

const attachment = (id: string, filename = "report.txt"): Attachment => ({
  id,
  board_id: "b_default",
  task_id: "t_1",
  filename,
  rel_path: `attachments/${filename}`,
  content_type: "text/plain",
  size_bytes: 12,
  sha256: "sha256-report",
  created_by: "web",
  created_at: 1,
})

const suggestion = (id: string, name: string, alreadyApplied = false) => ({
  label_id: id,
  label_name: name,
  score: 0.82,
  weight: 0.82,
  already_applied: alreadyApplied,
  evidence_atoms: [],
  negative_evidence_atoms: [],
})

const suggestions: Suggestions = {
  task_id: "t_1",
  board_id: "b_default",
  selected_labels: [suggestion("l_backend", "backend", true)],
  candidates: [suggestion("l_frontend", "frontend")],
  coverage: 0.82,
  coverage_cosine: 0.91,
  residual_norm: 0.18,
  needs_new_label: false,
  reason_codes: ["degraded_result", "label_atom_index_dirty"],
  degraded: true,
  diagnostics: ["label_atom_index_dirty"],
}

const mutationScope = {
  identity: "runtime\u0000b_default\u0000t_1",
  boardId: "b_default",
  taskId: "t_1",
}

function snapshot(overrides: Partial<TaskInspectorMutationSnapshot> = {}): TaskInspectorMutationSnapshot {
  return {
    scope: mutationScope,
    generation: 1,
    pending: new Set(),
    errors: new Map(),
    retries: new Map(),
    ...overrides,
  }
}

function handlers(overrides: Partial<InspectorAssetsMutationHandlers> = {}): InspectorAssetsMutationHandlers {
  const committed: InspectorMutationOutcome = { committed: true, reconciled: true }
  return {
    addLabel: vi.fn(async () => committed),
    removeLabel: vi.fn(async () => committed),
    applySuggestedLabel: vi.fn(async () => committed),
    uploadAttachment: vi.fn(async () => committed),
    downloadAttachment: vi.fn(async () => null),
    deleteAttachment: vi.fn(async () => committed),
    suggestLabels: vi.fn(async () => null),
    retry: vi.fn(async () => committed),
    ...overrides,
  }
}

describe("TaskInspectorAssetsPanel", () => {
  test("does not request suggestions during render; explicit state exposes the action", () => {
    const suggestLabels = vi.fn(async () => ({ data: suggestions }))
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested={false}
        handlers={handlers({ suggestLabels })}
        snapshot={snapshot()}
      />,
    )

    expect(suggestLabels).not.toHaveBeenCalled()
    expect(markup).toContain('data-testid="label-suggestion-request"')
    expect(markup).toContain("建议标签")
    expect(markup).not.toContain("label_atom_index_dirty")
  })

  test("renders selected/candidate coverage and degraded diagnostics without treating degraded as success", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[label("l_existing", "existing")]}
        attachments={[]}
        suggestionResult={suggestions}
        suggestionRequested
        locale="en"
        handlers={handlers()}
        snapshot={snapshot()}
      />,
    )

    expect(markup).toContain('data-testid="label-suggestions"')
    expect(markup).toContain("backend")
    expect(markup).toContain("frontend")
    expect(markup).toContain("Coverage 82%")
    expect(markup).toContain("cosine 91%")
    expect(markup).toContain("residual 0.180")
    expect(markup).toContain("degraded_result")
    expect(markup).toContain("label_atom_index_dirty")
    expect(markup).toContain('role="alert"')
    expect(markup).not.toContain("建议已应用")
  })

  test("shows current labels and attachment metadata, including sha256", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[label("l_existing", "existing")]}
        attachments={[attachment("a_1")]}
        suggestionResult={null}
        suggestionRequested={false}
        handlers={handlers()}
        snapshot={snapshot()}
      />,
    )

    expect(markup).toContain("existing")
    expect(markup).toContain("report.txt")
    expect(markup).toContain("text/plain")
    expect(markup).toContain("12 B")
    expect(markup).toContain("sha256-report")
    expect(markup).toContain("文件名")
    expect(markup).toContain('dateTime="1970-01-01T00:00:01.000Z"')
    expect(markup).toContain('aria-label="移除标签 existing"')
    expect(markup).toContain('aria-label="下载附件 report.txt"')
    expect(markup).toContain('aria-label="删除附件 report.txt"')
  })

  test("renders pending/error/empty states with accessible status regions", () => {
    const pending = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested={false}
        handlers={handlers()}
        snapshot={snapshot({ pending: new Set(["uploadAttachment:t_1", "addLabel:t_1", "reload:t_1"]) })}
        attachmentLoading
        attachmentError="读取附件失败"
      />,
    )

    expect(pending).toContain("正在上传")
    expect(pending).toContain('data-testid="inspector-retry-status"')
    expect(pending).toContain("正在重试原提交")
    expect(pending).toContain('data-testid="label-add"')
    expect(pending).toContain('data-testid="attachment-upload"')
    expect(pending).toContain("读取附件失败")
    expect(pending).toContain("暂无附件")
    expect(pending).toContain('role="alert"')
    expect(pending).toContain('role="status"')
  })

  test("creates the typed upload intent once and enforces the wire-sized limit", async () => {
    const bytes = new Uint8Array([1, 2, 255])
    const file = {
      name: "bytes.bin",
      type: "application/octet-stream",
      size: bytes.byteLength,
      arrayBuffer: vi.fn(async () => bytes.buffer),
    } as unknown as File

    await expect(createAttachmentUploadIntent(file)).resolves.toEqual({
      filename: "bytes.bin",
      content_type: "application/octet-stream",
      content: [1, 2, 255],
    })
    expect(file.arrayBuffer).toHaveBeenCalledTimes(1)
    expect(MAX_ATTACHMENT_UPLOAD_BYTES).toBe(384 * 1024)

    const oversized = { ...file, size: MAX_ATTACHMENT_UPLOAD_BYTES + 1, arrayBuffer: vi.fn(async () => bytes.buffer) } as unknown as File
    await expect(createAttachmentUploadIntent(oversized)).rejects.toThrow("384 KiB")
    expect(oversized.arrayBuffer).toHaveBeenCalledTimes(0)
  })

  test("normalizes synchronous suggestion throws into a rejected promise", async () => {
    const suggestLabels: SuggestLabelsHandler = vi.fn(() => {
      throw new Error("同步失败")
    })

    await expect(requestSuggestedLabels(suggestLabels)).rejects.toThrow("同步失败")
    expect(suggestLabels).toHaveBeenCalledWith({ limit: 5 })
  })

  test("keeps a null suggestion response as an explicit empty state", async () => {
    const suggestLabels: SuggestLabelsHandler = vi.fn(async () => null)

    await expect(requestSuggestedLabels(suggestLabels)).resolves.toBeNull()
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested
        handlers={handlers({ suggestLabels })}
        snapshot={snapshot()}
      />,
    )

    expect(markup).toContain('data-testid="label-suggestions"')
    expect(markup).toContain("暂无标签建议")
    expect(markup).toContain("刷新建议")
  })

  test("renders a snapshot suggestion failure inline without an empty state or duplicate alert", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested
        handlers={handlers()}
        snapshot={snapshot({
          errors: new Map([[
            "suggestLabels:t_1",
            { operation: "suggestLabels", taskId: "t_1", kind: "error", message: "建议服务失败", status: 503, code: null, recoverable: true },
          ]]),
        })}
      />,
    )

    expect(markup).toContain("建议服务失败")
    expect(markup).not.toContain("暂无标签建议")
    expect(markup).not.toContain('data-testid="inspector-mutation-errors"')
    expect(markup).toContain("刷新建议")
  })

  test("clears label and file drafts only after a committed write", () => {
    expect(shouldClearAssetDraft({ committed: false, reconciled: false })).toBe(false)
    expect(shouldClearAssetDraft({ committed: false, reconciled: true })).toBe(false)
    expect(shouldClearAssetDraft({ committed: true, reconciled: false })).toBe(true)
    expect(shouldClearAssetDraft({ committed: true, reconciled: true })).toBe(true)
  })

  test("requires the original File object for an upload retry draft", () => {
    const attempted = { name: "same.txt", size: 3 } as File
    const sameMetadata = { name: "same.txt", size: 3 } as File

    expect(isAttachmentRetryDraftCurrent(attempted, attempted)).toBe(true)
    expect(isAttachmentRetryDraftCurrent(sameMetadata, attempted)).toBe(false)
    expect(isAttachmentRetryDraftCurrent(null, attempted)).toBe(false)
  })

  test("locks only an unchanged label retry draft", () => {
    expect(isLabelRetryDraftCurrent(" backend ", "backend")).toBe(true)
    expect(isLabelRetryDraftCurrent("frontend", "backend")).toBe(false)
  })

  test("dedupes a snapshot error when its inline owner already has the same key", () => {
    const error = { operation: "downloadAttachment" as const, taskId: "t_1" }
    const pending = new Set<string>()
    const retryBusy = new Set<string>()

    expect(shouldShowInspectorSnapshotError("downloadAttachment:t_1", error, "t_1", pending, retryBusy, new Map([["downloadAttachment:t_1", "下载失败"]]))).toBe(false)
    expect(shouldShowInspectorSnapshotError("downloadAttachment:t_1", error, "t_1", pending, retryBusy, new Map())).toBe(true)
  })

  test("fences stale promises across t1 to t2 to t1 scope epochs", () => {
    const first = { taskId: "t_1", epoch: 0, generation: 1 }
    const runtimeReplacement = advanceInspectorAssetsScope(first, "t_1", 2)
    const second = advanceInspectorAssetsScope(first, "t_2", 2)
    const third = advanceInspectorAssetsScope(second, "t_1", 3)

    expect(runtimeReplacement.epoch).toBe(0)
    expect(isInspectorAssetsScopeCurrent(runtimeReplacement, first)).toBe(false)
    expect(third.epoch).toBe(2)
    expect(isInspectorAssetsScopeCurrent(third, first)).toBe(false)
    expect(isInspectorAssetsScopeCurrent(third, second)).toBe(false)
    expect(isInspectorAssetsScopeCurrent(third, third)).toBe(true)
  })

  test("renders every snapshot error and its exact retry key", async () => {
    const retry = vi.fn(async (key?: string) => {
      void key
      return { committed: true, reconciled: true }
    })
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested={false}
        handlers={handlers({ retry })}
        snapshot={snapshot({
          errors: new Map([
            ["addLabel:t_1", { operation: "addLabel", taskId: "t_1", kind: "error", message: "mutation_failed", status: 500, code: null, recoverable: true }],
            ["reload:t_1", { operation: "reload", taskId: "t_1", kind: "stale", message: "stale", status: null, code: null, recoverable: true }],
            ["suggestLabels:t_1", { operation: "suggestLabels", taskId: "t_1", kind: "error", message: "unavailable", status: 503, code: null, recoverable: true }],
            ["downloadAttachment:t_1", { operation: "downloadAttachment", taskId: "t_1", kind: "error", message: "unavailable", status: 503, code: null, recoverable: true }],
          ]),
          retries: new Map([
            ["addLabel:t_1", { operation: "addLabel", taskId: "t_1", input: { name: "backend", create_missing: false } }],
            ["reload:t_1", { operation: "reload", taskId: "t_1" }],
          ]),
        })}
      />,
    )

    expect(markup).toContain('data-testid="inspector-mutation-errors"')
    expect(markup).toContain('data-operation-key="addLabel:t_1"')
    expect(markup).toContain('data-retry-key="addLabel:t_1"')
    expect(markup).not.toContain('data-operation-key="reload:t_1"')
    expect(markup).not.toContain("写入已提交，但刷新失败")
    expect(markup).not.toContain('data-retry-key="suggestLabels:t_1"')
    expect(markup).not.toContain('data-retry-key="downloadAttachment:t_1"')
    await retry("addLabel:t_1")
    expect(retry).toHaveBeenCalledWith("addLabel:t_1")
  })

  test("associates asset errors with the editable label and file controls", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[attachment("a_1")]}
        suggestionResult={null}
        suggestionRequested={false}
        handlers={handlers()}
        attachmentError="读取附件失败"
        snapshot={snapshot({
          errors: new Map([[
            "addLabel:t_1",
            { operation: "addLabel", taskId: "t_1", kind: "error", message: "标签写入失败", status: 500, code: null, recoverable: true },
          ]]),
        })}
      />,
    )

    expect(markup).toContain('name="attachment-file"')
    expect(markup).toMatch(/aria-describedby="[^"]+-label-error"/)
    expect(markup).toMatch(/aria-describedby="[^"]+-attachment-error"/)
    expect(markup).toMatch(/id="[^"]+-label-error"/)
    expect(markup).toMatch(/id="[^"]+-attachment-error"/)
    expect((markup.match(/标签写入失败/g) ?? []).length).toBe(1)
  })

  test("does not surface header, relations, or global reload errors in the asset owner", () => {
    const keys = ["saveTask", "transition", "addDependency", "createStep", "addComment", "reload"]
    for (const operation of keys) {
      expect(shouldShowInspectorSnapshotError(
        `${operation}:t_1`,
        { operation: operation as never, taskId: "t_1" },
        "t_1",
        new Set(),
        new Set(),
        new Map(),
      )).toBe(false)
    }
    expect(shouldShowInspectorSnapshotError(
      "suggestLabels:t_1",
      { operation: "suggestLabels", taskId: "t_1" },
      "t_1",
      new Set(),
      new Set(),
      new Map(),
    )).toBe(false)
  })

  test("keeps snapshot pending/error state scoped to the current task", () => {
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_2"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested={false}
        handlers={handlers()}
        snapshot={snapshot({
          pending: new Set(["uploadAttachment:t_1"]),
          errors: new Map([[
            "uploadAttachment:t_1",
            { operation: "uploadAttachment", taskId: "t_1", kind: "error", message: "旧任务错误", status: null, code: null, recoverable: true },
          ]]),
        })}
      />,
    )

    expect(markup).toContain('aria-busy="false"')
    expect(markup).not.toContain("旧任务错误")
    expect(markup).not.toContain("正在上传")
  })

  test("keeps duplicate suggestions visible and reports their label ids", () => {
    const selectedWithEvidence = {
      ...suggestion("l_duplicate", "selected-name"),
      evidence_atoms: [{ atom_id: "atom-positive", label_id: "l_duplicate", label_name: "selected-name", polarity: "positive", kind: "title", text: "positive evidence", score: 0.8 }],
      negative_evidence_atoms: [{ atom_id: "atom-negative", label_id: "l_duplicate", label_name: "selected-name", polarity: "negative", kind: "description", text: "negative evidence", score: 0.2 }],
    }
    const duplicateSuggestions: Suggestions = {
      ...suggestions,
      degraded: false,
      reason_codes: [],
      diagnostics: [],
      selected_labels: [selectedWithEvidence],
      candidates: [suggestion("l_duplicate", "candidate-name")],
    }
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={duplicateSuggestions}
        suggestionRequested
        locale="en"
        handlers={handlers()}
        snapshot={snapshot()}
      />,
    )

    expect(markup).toContain('data-testid="label-suggestion-duplicates"')
    expect(markup).toContain("l_duplicate")
    expect(markup).toContain("selected-name")
    expect(markup).toContain("candidate-name")
    expect(markup).toContain("positive evidence")
    expect(markup).toContain("negative evidence")
    expect((markup.match(/data-testid="label-suggestion-apply"/g) ?? []).length).toBe(2)
    expect((markup.match(/disabled=""/g) ?? []).length).toBeGreaterThanOrEqual(2)
    expect(markup).toContain("Reason codes:</span> —")
    expect(markup).toContain("Diagnostics:</span> —")
  })

  test("preserves the exact bytes represented by an attachment download view", () => {
    const backing = new Uint8Array([1, 2, 3, 4])
    const view = backing.subarray(1, 3)
    const exact = exactAttachmentBytes(view)

    expect(Array.from(new Uint8Array(exact))).toEqual([2, 3])
    expect(exact.byteLength).toBe(2)
    expect(exactAttachmentBytes(backing)).toBe(backing.buffer)
  })

  test("maps every asset action to the exact controller handler input", async () => {
    const actionHandlers = handlers({
      downloadAttachment: vi.fn(async () => ({ content_type: "text/plain", attachment_id: "a_1", sha256: "sha", content: new Uint8Array([1]) })),
    })
    const actions = createInspectorAssetsActions(actionHandlers)
    await actions.addLabel("backend")
    await actions.removeLabel("l_backend")
    await actions.applySuggestedLabel("frontend")
    await actions.uploadAttachment({ filename: "x.txt", content_type: "text/plain", content: [1, 2] })
    await actions.downloadAttachment("a_1")
    await actions.deleteAttachment("a_1")

    expect(actionHandlers.addLabel).toHaveBeenCalledWith({ name: "backend", create_missing: false })
    expect(actionHandlers.removeLabel).toHaveBeenCalledWith({ labelId: "l_backend" })
    expect(actionHandlers.applySuggestedLabel).toHaveBeenCalledWith({ name: "frontend", create_missing: false })
    expect(actionHandlers.uploadAttachment).toHaveBeenCalledWith({ filename: "x.txt", content_type: "text/plain", content: [1, 2] })
    expect(actionHandlers.downloadAttachment).toHaveBeenCalledWith({ attachmentId: "a_1" })
    expect(actionHandlers.deleteAttachment).toHaveBeenCalledWith({ attachmentId: "a_1" })
  })
})
