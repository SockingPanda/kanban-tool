import { ApiError, expectArray, expectBoolean, expectExactKeys, expectNullableString, expectRecord, expectSafeInteger, expectString } from "../parsers"
import type { Attachment, CreateAttachmentInput, DownloadedAttachment, RequestOptions } from "../types"
import { ApiTransport } from "../transport"

const ATTACHMENT_KEYS = ["id", "board_id", "task_id", "filename", "rel_path", "content_type", "size_bytes", "sha256", "created_by", "created_at"] as const

export async function listAttachments(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
  return parseAttachmentListEnvelope(await api.requestRaw(`/api/v1/tasks/${encodePathSegment(taskId)}/attachments`, options)).data
}

export async function createAttachment(api: ApiTransport, taskId: string, input: CreateAttachmentInput, options: RequestOptions = {}) {
  return parseAttachmentEnvelope(await api.requestRaw(`/api/v1/tasks/${encodePathSegment(taskId)}/attachments`, {
    method: "POST",
    body: input,
    actorHeader: true,
    signal: options.signal,
  })).data
}

export async function downloadAttachment(api: ApiTransport, taskId: string, attachmentId: string, options: RequestOptions = {}): Promise<DownloadedAttachment> {
  const response = await api.requestBytes(`/api/v1/tasks/${encodePathSegment(taskId)}/attachments/${encodePathSegment(attachmentId)}`, options)
  return {
    content_type: response.contentType,
    attachment_id: response.attachmentId,
    sha256: response.sha256,
    content: response.bytes,
  }
}

export async function deleteAttachment(api: ApiTransport, taskId: string, attachmentId: string, options: RequestOptions = {}) {
  return parseDeleteAttachmentEnvelope(await api.requestRaw(`/api/v1/tasks/${encodePathSegment(taskId)}/attachments/${encodePathSegment(attachmentId)}`, {
    method: "DELETE",
    actorHeader: true,
    signal: options.signal,
  })).data.deleted
}

function parseAttachment(value: unknown, label: string): Attachment {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ATTACHMENT_KEYS, label)
  return {
    id: expectString(record.id, `${label}.id`),
    board_id: expectString(record.board_id, `${label}.board_id`),
    task_id: expectString(record.task_id, `${label}.task_id`),
    filename: expectString(record.filename, `${label}.filename`),
    rel_path: expectString(record.rel_path, `${label}.rel_path`),
    content_type: expectNullableString(record.content_type, `${label}.content_type`),
    size_bytes: expectSafeInteger(record.size_bytes, `${label}.size_bytes`, true),
    sha256: expectNullableString(record.sha256, `${label}.sha256`),
    created_by: expectString(record.created_by, `${label}.created_by`),
    created_at: expectSafeInteger(record.created_at, `${label}.created_at`, true),
  }
}

function parseAttachmentEnvelope(value: unknown) {
  const envelope = expectRecord<Record<string, unknown>>(value, "create attachment response")
  expectExactKeys(envelope, ["data"], "create attachment response")
  return { data: parseAttachment(envelope.data, "create attachment response.data") }
}

function parseAttachmentListEnvelope(value: unknown) {
  const envelope = expectRecord<Record<string, unknown>>(value, "list attachments response")
  expectExactKeys(envelope, ["data"], "list attachments response")
  return {
    data: expectArray<unknown>(envelope.data, "list attachments response.data").map((entry, index) =>
      parseAttachment(entry, `list attachments response.data[${index}]`),
    ),
  }
}

function parseDeleteAttachmentEnvelope(value: unknown) {
  const envelope = expectRecord<Record<string, unknown>>(value, "delete attachment response")
  expectExactKeys(envelope, ["data"], "delete attachment response")
  const data = expectRecord<Record<string, unknown>>(envelope.data, "delete attachment response.data")
  expectExactKeys(data, ["deleted"], "delete attachment response.data")
  return { data: { deleted: expectBoolean(data.deleted, "delete attachment response.data.deleted") } }
}

function encodePathSegment(value: string) {
  const trimmed = value.trim()
  if (!trimmed) throw new ApiError("invalid_input", "attachment path segment must not be empty")
  return encodeURIComponent(trimmed)
}
