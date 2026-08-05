import { ApiError, expectArray, expectRecord, expectExactKeys, expectString, expectBoolean, expectSafeInteger, expectNullableString, expectNullableInteger } from "../../parsers"
import type { Run } from "../../types"

const RUN_STATUSES = new Set(["running", "succeeded", "failed", "canceled", "expired"])
export function parseApiRun(value: unknown, label: string): Run {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ["id", "task_id", "status", "worker_profile", "worker_pid", "claim_owner", "started_at", "finished_at", "exit_code", "summary", "error", "has_log", "metadata"], label)
  if (!RUN_STATUSES.has(record.status as string)) throw new ApiError("invalid_response", `${label}.status is unknown`)
  return { id: expectString(record.id, `${label}.id`), task_id: expectString(record.task_id, `${label}.task_id`), status: record.status as Run["status"], worker_profile: expectNullableString(record.worker_profile, `${label}.worker_profile`), worker_pid: expectNullableInteger(record.worker_pid, `${label}.worker_pid`), claim_owner: expectString(record.claim_owner, `${label}.claim_owner`), started_at: expectSafeInteger(record.started_at, `${label}.started_at`, true), finished_at: expectNullableInteger(record.finished_at, `${label}.finished_at`), exit_code: expectNullableInteger(record.exit_code, `${label}.exit_code`), summary: expectNullableString(record.summary, `${label}.summary`), error: expectNullableString(record.error, `${label}.error`), has_log: expectBoolean(record.has_log, `${label}.has_log`), metadata: record.metadata }
}
export function parseListRunsEnvelope(value: unknown): { data: Run[] } {
  const envelope = expectRecord<Record<string, unknown>>(value, "list runs response"); expectExactKeys(envelope, ["data"], "list runs response")
  return { data: expectArray<unknown>(envelope.data, "list runs response data").map((entry, index) => parseApiRun(entry, `list runs response data[${index}]`)) }
}
export function parseGetRunEnvelope(value: unknown): { data: Run } {
  const envelope = expectRecord<Record<string, unknown>>(value, "get run response"); expectExactKeys(envelope, ["data"], "get run response")
  return { data: parseApiRun(envelope.data, "get run response data") }
}
