import type { ApiGetTaskResponseContract } from "../../lib/api/generated/contracts/api-get-task-response"
import type { ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response"
import type { ApiSuggestTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-suggest-task-labels-response"
import type {
  InspectorMutationOutcome,
  InspectorSuggestTaskLabelsQuery,
  TaskInspectorMutationHandlers,
} from "./task-inspector-mutation-state"

export type InspectorAssetLabel = ApiGetTaskResponseContract["data"]["labels"][number]
export type InspectorAssetAttachment = ApiListAttachmentsResponseContract["data"][number]
export type InspectorLabelSuggestionResult = ApiSuggestTaskLabelsResponseContract["data"]

export type SuggestLabelsHandler = (
  query?: InspectorSuggestTaskLabelsQuery,
) => Promise<ApiSuggestTaskLabelsResponseContract | null>

export type InspectorAssetsMutationHandlers = Pick<
  TaskInspectorMutationHandlers,
  "addLabel" | "removeLabel" | "applySuggestedLabel" | "uploadAttachment" | "downloadAttachment" | "deleteAttachment" | "retry"
> & {
  /** 05C controller-owned read handler; no transport is created in this panel. */
  readonly suggestLabels: SuggestLabelsHandler
}

/**
 * JSON number[] upload budget for the localhost Axum route.
 *
 * The service can store much larger attachments, but the default 2 MiB Axum
 * JSON body limit is reached well before that when binary bytes are encoded as
 * JSON numbers. Keep this wire budget conservative so the client fails before
 * issuing a request that the host cannot accept.
 */
export const MAX_ATTACHMENT_UPLOAD_BYTES = 384 * 1024

const ATTACHMENT_UPLOAD_LIMIT_MESSAGE = "附件超过 384 KiB 上传上限（JSON 数组请求体预算）。"

export async function createAttachmentUploadIntent(file: File): Promise<{
  readonly filename: string
  readonly content_type: string | null
  readonly content: number[]
}> {
  if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > MAX_ATTACHMENT_UPLOAD_BYTES) {
    throw new Error(ATTACHMENT_UPLOAD_LIMIT_MESSAGE)
  }
  const bytes = new Uint8Array(await file.arrayBuffer())
  if (bytes.byteLength > MAX_ATTACHMENT_UPLOAD_BYTES) {
    throw new Error(ATTACHMENT_UPLOAD_LIMIT_MESSAGE)
  }
  return {
    filename: file.name,
    content_type: file.type || null,
    content: Array.from(bytes),
  }
}

/**
 * Enter the promise chain before invoking a controller callback so a callback
 * that throws synchronously is handled by the same rejection path as an async
 * failure.
 */
export function requestSuggestedLabels(
  handler: SuggestLabelsHandler,
  query: InspectorSuggestTaskLabelsQuery = { limit: 5 },
): Promise<ApiSuggestTaskLabelsResponseContract | null> {
  return Promise.resolve().then(() => handler(query))
}

export function formatAttachmentSize(size: number): string {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  if (size < 1024 * 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)} MB`
  return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

export interface InspectorAssetsActions {
  readonly addLabel: (name: string) => Promise<InspectorMutationOutcome>
  readonly removeLabel: (labelId: string) => Promise<InspectorMutationOutcome>
  readonly applySuggestedLabel: (name: string) => Promise<InspectorMutationOutcome>
  readonly uploadAttachment: (input: { readonly filename: string; readonly content_type: string | null; readonly content: number[] }) => Promise<InspectorMutationOutcome>
  readonly downloadAttachment: (attachmentId: string) => ReturnType<InspectorAssetsMutationHandlers["downloadAttachment"]>
  readonly deleteAttachment: (attachmentId: string) => Promise<InspectorMutationOutcome>
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

/** Return only the bytes represented by a Uint8Array view before creating a Blob. */
export function exactAttachmentBytes(content: Uint8Array): ArrayBuffer {
  if (content.byteOffset === 0 && content.byteLength === content.buffer.byteLength) return content.buffer as ArrayBuffer
  return content.slice().buffer
}

/** Drafts are safe to clear only after the server accepted the write. */
export function shouldClearAssetDraft(outcome: InspectorMutationOutcome | null): boolean {
  return outcome?.committed === true
}

/** Retry drafts are the original in-memory File object; same metadata is not enough. */
export function isAttachmentRetryDraftCurrent(current: File | null, attempted: File | null): boolean {
  return current !== null && attempted !== null && current === attempted
}

export interface InspectorAssetsScopeIdentity {
  readonly taskId: string
  readonly epoch: number
  readonly generation: number
}

/** Advance the local fence when task or controller generation changes. */
export function advanceInspectorAssetsScope(
  previous: InspectorAssetsScopeIdentity,
  taskId: string,
  generation: number,
): InspectorAssetsScopeIdentity {
  if (previous.taskId === taskId && previous.generation === generation) return previous
  return {
    taskId,
    epoch: previous.taskId === taskId ? previous.epoch : previous.epoch + 1,
    generation,
  }
}

export function isInspectorAssetsScopeCurrent(
  current: InspectorAssetsScopeIdentity,
  captured: InspectorAssetsScopeIdentity,
): boolean {
  return current.taskId === captured.taskId && current.epoch === captured.epoch && current.generation === captured.generation
}
