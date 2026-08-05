import { ApiError, expectArray, expectRecord, expectExactKeys, expectString, expectSafeInteger, expectNullableString } from "../../parsers"
import type { CommentRecord } from "../../types"
export function parseApiComment(value: unknown, label: string): CommentRecord {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ["id", "board_id", "task_id", "author", "author_type", "agent_type", "body", "kind", "metadata", "created_at"], label)
  if (record.author_type !== "user" && record.author_type !== "agent") throw new ApiError("invalid_response", `${label}.author_type is unknown`)
  if (record.kind !== "note" && record.kind !== "decision" && record.kind !== "signal") throw new ApiError("invalid_response", `${label}.kind is unknown`)
  return { id: expectString(record.id, `${label}.id`), board_id: expectString(record.board_id, `${label}.board_id`), task_id: expectString(record.task_id, `${label}.task_id`), author: expectString(record.author, `${label}.author`), author_type: record.author_type, agent_type: expectNullableString(record.agent_type, `${label}.agent_type`), body: expectString(record.body, `${label}.body`), kind: record.kind, metadata: expectRecord<Record<string, unknown>>(record.metadata, `${label}.metadata`), created_at: expectSafeInteger(record.created_at, `${label}.created_at`, true) }
}

export function parseListCommentsEnvelope(value: unknown): { data: CommentRecord[] } {
  const envelope = expectRecord<Record<string, unknown>>(value, "list comments response")
  expectExactKeys(envelope, ["data"], "list comments response")
  return { data: expectArray<unknown>(envelope.data, "list comments response data").map((entry, index) => parseApiComment(entry, `list comments response data[${index}]`)) }
}

export function parseCreateCommentEnvelope(value: unknown): { data: CommentRecord } {
  const envelope = expectRecord<Record<string, unknown>>(value, "create comment response")
  expectExactKeys(envelope, ["data"], "create comment response")
  return { data: parseApiComment(envelope.data, "create comment response data") }
}
