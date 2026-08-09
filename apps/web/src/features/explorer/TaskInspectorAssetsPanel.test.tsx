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
} from "./TaskInspectorAssetsPanel.logic"
import type { TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"

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
  return {
    addLabel: vi.fn(async () => undefined),
    removeLabel: vi.fn(async () => undefined),
    applySuggestedLabel: vi.fn(async () => undefined),
    uploadAttachment: vi.fn(async () => undefined),
    downloadAttachment: vi.fn(async () => null),
    deleteAttachment: vi.fn(async () => undefined),
    ...overrides,
  }
}

describe("TaskInspectorAssetsPanel", () => {
  test("does not request suggestions during render; explicit state exposes the action", () => {
    const suggestLabels = vi.fn(async () => suggestions)
    const markup = renderToStaticMarkup(
      <TaskInspectorAssetsPanel
        taskId="t_1"
        labels={[]}
        attachments={[]}
        suggestionResult={null}
        suggestionRequested={false}
        suggestLabels={suggestLabels}
        handlers={handlers()}
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
        handlers={handlers()}
        snapshot={snapshot()}
      />,
    )

    expect(markup).toContain("existing")
    expect(markup).toContain("report.txt")
    expect(markup).toContain("text/plain")
    expect(markup).toContain("12 B")
    expect(markup).toContain("sha256-report")
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
        handlers={handlers()}
        snapshot={snapshot({ pending: new Set(["uploadAttachment:t_1", "addLabel:t_1"]) })}
        attachmentLoading
        attachmentError="读取附件失败"
      />,
    )

    expect(pending).toContain("正在上传")
    expect(pending).toContain("读取附件失败")
    expect(pending).toContain("暂无附件")
    expect(pending).toContain('role="alert"')
    expect(pending).toContain('role="status"')
  })

  test("creates the typed upload intent once and enforces the host-sized limit", async () => {
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
    expect(MAX_ATTACHMENT_UPLOAD_BYTES).toBe(256 * 1024 * 1024)

    const oversized = { ...file, size: MAX_ATTACHMENT_UPLOAD_BYTES + 1, arrayBuffer: vi.fn(async () => bytes.buffer) } as unknown as File
    await expect(createAttachmentUploadIntent(oversized)).rejects.toThrow("256 MiB")
    expect(oversized.arrayBuffer).toHaveBeenCalledTimes(0)
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
