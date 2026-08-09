import type { ApiGetTaskResponseContract } from "../../lib/api/generated/contracts/api-get-task-response"
import type { ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response"
import type { ApiSuggestTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-suggest-task-labels-response"
import type { InspectorSuggestTaskLabelsQuery, TaskInspectorMutationHandlers } from "./task-inspector-mutation-state"

export type InspectorAssetLabel = ApiGetTaskResponseContract["data"]["labels"][number]
export type InspectorAssetAttachment = ApiListAttachmentsResponseContract["data"][number]
export type InspectorLabelSuggestionResult = ApiSuggestTaskLabelsResponseContract["data"]

export type SuggestLabelsHandler = (
  query?: InspectorSuggestTaskLabelsQuery,
) => Promise<InspectorLabelSuggestionResult | void> | InspectorLabelSuggestionResult | void

export type InspectorAssetsMutationHandlers = Pick<
  TaskInspectorMutationHandlers,
  "addLabel" | "removeLabel" | "applySuggestedLabel" | "uploadAttachment" | "downloadAttachment" | "deleteAttachment"
> & {
  /** 05C controller-owned read handler; no transport is created in this panel. */
  readonly suggestLabels?: SuggestLabelsHandler
}

/** Host binary response and attachment write budget; keep in step with kanban-server. */
export const MAX_ATTACHMENT_UPLOAD_BYTES = 256 * 1024 * 1024

export async function createAttachmentUploadIntent(file: File): Promise<{
  readonly filename: string
  readonly content_type: string | null
  readonly content: number[]
}> {
  if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > MAX_ATTACHMENT_UPLOAD_BYTES) {
    throw new Error("附件超过 256 MiB 上传上限。")
  }
  const bytes = new Uint8Array(await file.arrayBuffer())
  if (bytes.byteLength > MAX_ATTACHMENT_UPLOAD_BYTES) {
    throw new Error("附件超过 256 MiB 上传上限。")
  }
  return {
    filename: file.name,
    content_type: file.type || null,
    content: Array.from(bytes),
  }
}

export function formatAttachmentSize(size: number): string {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  if (size < 1024 * 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)} MB`
  return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

export interface InspectorAssetsActions {
  readonly addLabel: (name: string) => Promise<void>
  readonly removeLabel: (labelId: string) => Promise<void>
  readonly applySuggestedLabel: (name: string) => Promise<void>
  readonly uploadAttachment: (input: { readonly filename: string; readonly content_type: string | null; readonly content: number[] }) => Promise<void>
  readonly downloadAttachment: (attachmentId: string) => ReturnType<InspectorAssetsMutationHandlers["downloadAttachment"]>
  readonly deleteAttachment: (attachmentId: string) => Promise<void>
}

/** Maps user-facing asset actions to the exact 05C controller input shapes. */
export function createInspectorAssetsActions(handlers: InspectorAssetsMutationHandlers): InspectorAssetsActions {
  return {
    addLabel: (name) => handlers.addLabel({ name, create_missing: false }),
    removeLabel: (labelId) => handlers.removeLabel({ labelId }),
    applySuggestedLabel: (name) => handlers.applySuggestedLabel({ name, create_missing: false }),
    uploadAttachment: (input) => handlers.uploadAttachment(input),
    downloadAttachment: (attachmentId) => handlers.downloadAttachment({ attachmentId }),
    deleteAttachment: (attachmentId) => handlers.deleteAttachment({ attachmentId }),
  }
}
