import { expectArray, expectRecord, expectExactKeys, expectString, expectSafeInteger, expectNullableString } from "../../parsers"
import type { Board } from "../../types"
export function parseApiBoard(value: unknown, label: string): Board {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ["id", "slug", "name", "description", "created_at", "updated_at", "archived_at"], label)
  return {
    id: expectString(record.id, `${label}.id`),
    slug: expectString(record.slug, `${label}.slug`),
    name: expectString(record.name, `${label}.name`),
    description: expectNullableString(record.description, `${label}.description`),
    created_at: expectSafeInteger(record.created_at, `${label}.created_at`, true),
    updated_at: expectSafeInteger(record.updated_at, `${label}.updated_at`, true),
    archived_at: record.archived_at === null ? null : expectSafeInteger(record.archived_at, `${label}.archived_at`, true),
  }
}

export function parseListBoardsEnvelope(value: unknown): { data: Board[] } {
  const envelope = expectRecord<Record<string, unknown>>(value, "boards response")
  expectExactKeys(envelope, ["data"], "boards response")
  return {
    data: expectArray<unknown>(envelope.data, "boards response data")
      .map((entry, index) => parseApiBoard(entry, `boards response data[${index}]`)),
  }
}
export function parseCreateBoardEnvelope(value: unknown): { data: Board } {
  const envelope = expectRecord<Record<string, unknown>>(value, "create board response")
  expectExactKeys(envelope, ["data"], "create board response")
  return { data: parseApiBoard(envelope.data, "create board response data") }
}
export function parseGetBoardEnvelope(value: unknown): { data: Board } {
  const envelope = expectRecord<Record<string, unknown>>(value, "get board response")
  expectExactKeys(envelope, ["data"], "get board response")
  return { data: parseApiBoard(envelope.data, "get board response data") }
}
export function parseArchiveBoardEnvelope(value: unknown): { data: Board } {
  const envelope = expectRecord<Record<string, unknown>>(value, "archive board response")
  expectExactKeys(envelope, ["data"], "archive board response")
  return { data: parseApiBoard(envelope.data, "archive board response data") }
}
