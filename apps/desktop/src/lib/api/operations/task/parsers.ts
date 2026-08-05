import { ApiError, expectArray, expectRecord, expectExactKeys, expectString, expectBoolean, expectSafeInteger, expectNullableString, expectNullableInteger } from "../../parsers"
import type { ErrorBody, LabelRecord, RequiredOffsetPageMeta, RequiredTotalPageMeta, StepPlanState, Task, TaskStatus, TaskStatusWindowsResponse } from "../../types"

const TASK_STATUSES = new Set<TaskStatus>(["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"])
const PLAN_STATES = new Set<StepPlanState>(["unplanned", "planned", "not_required"])
const TASK_KEYS = ["id", "board_id", "board_slug", "ref", "seq", "title", "description", "status", "status_reason", "assignee", "priority", "position", "scheduled_at", "due_at", "created_by", "created_at", "updated_at", "started_at", "completed_at", "archived_at", "claim_owner", "claim_expires_at", "last_heartbeat_at", "current_run_id", "retry_count", "max_retries", "result_summary", "result", "metadata", "lock_version", "dependency_blocked", "unfinished_parent_count", "execution_plan_state", "required_step_count", "completed_required_step_count", "optional_step_count", "labels"] as const
const LABEL_KEYS = ["id", "board_id", "name", "color", "created_at", "updated_at"] as const

export function parseApiLabel(value: unknown, label: string): LabelRecord {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, LABEL_KEYS, label)
  expectString(record.id, `${label}.id`); expectString(record.board_id, `${label}.board_id`); expectString(record.name, `${label}.name`)
  expectNullableString(record.color, `${label}.color`); expectSafeInteger(record.created_at, `${label}.created_at`); expectSafeInteger(record.updated_at, `${label}.updated_at`)
  return record as LabelRecord
}

export function parseApiTask(value: unknown, label: string): Task {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, TASK_KEYS, label)
  for (const key of ["id", "board_id", "board_slug", "ref", "title", "created_by"] as const) expectString(record[key], `${label}.${key}`)
  for (const key of ["description", "status_reason", "assignee", "claim_owner", "current_run_id", "result_summary"] as const) expectNullableString(record[key], `${label}.${key}`)
  for (const key of ["seq", "position", "created_at", "updated_at", "retry_count", "lock_version", "unfinished_parent_count", "required_step_count", "completed_required_step_count", "optional_step_count"] as const) expectSafeInteger(record[key], `${label}.${key}`)
  for (const key of ["scheduled_at", "due_at", "started_at", "completed_at", "archived_at", "claim_expires_at", "last_heartbeat_at", "max_retries"] as const) expectNullableInteger(record[key], `${label}.${key}`)
  if (!TASK_STATUSES.has(record.status as TaskStatus)) throw new ApiError("invalid_response", `${label}.status is unknown`)
  if (!Number.isSafeInteger(record.priority) || (record.priority as number) < 0 || (record.priority as number) > 3) throw new ApiError("invalid_response", `${label}.priority must be an integer in 0..=3`)
  expectBoolean(record.dependency_blocked, `${label}.dependency_blocked`)
  if (!PLAN_STATES.has(record.execution_plan_state as StepPlanState)) throw new ApiError("invalid_response", `${label}.execution_plan_state is unknown`)
  record.labels = expectArray<unknown>(record.labels, `${label}.labels`).map((entry, index) => parseApiLabel(entry, `${label}.labels[${index}]`))
  return record as Task
}

export function parseTransitionTaskEnvelope(value: unknown): Task {
  const envelope = expectRecord<Record<string, unknown>>(value, "task transition response")
  expectExactKeys(envelope, ["data"], "task transition response")
  return parseApiTask(envelope.data, "task transition response.data")
}

export function parseTotalMeta(value: unknown, label: string): RequiredTotalPageMeta {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ["limit", "offset", "total"], label)
  return { limit: expectSafeInteger(record.limit, `${label}.limit`, true), offset: expectSafeInteger(record.offset, `${label}.offset`, true), total: expectSafeInteger(record.total, `${label}.total`, true) }
}

export function parseOffsetMeta(value: unknown, label: string): RequiredOffsetPageMeta {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ["limit", "offset"], label)
  return { limit: expectSafeInteger(record.limit, `${label}.limit`, true), offset: expectSafeInteger(record.offset, `${label}.offset`, true) }
}

export function parseTaskReadErrorEnvelope(value: unknown): ErrorBody {
  const envelope = expectRecord<Record<string, unknown>>(value, "task-read error response")
  expectExactKeys(envelope, ["error"], "task-read error response")
  const error = expectRecord<Record<string, unknown>>(envelope.error, "task-read error response.error")
  const keys = Object.keys(error)
  const hasDetails = keys.includes("details")
  expectExactKeys(error, hasDetails ? ["code", "message", "details"] : ["code", "message"], "task-read error response.error")
  return {
    code: expectString(error.code, "task-read error response.error.code"),
    message: expectString(error.message, "task-read error response.error.message"),
    ...(hasDetails ? { details: error.details } : {}),
  }
}
export function parseTaskListEnvelope(value: unknown): { data: Task[]; meta: RequiredTotalPageMeta } {
  const envelope = expectRecord<Record<string, unknown>>(value, "tasks response"); expectExactKeys(envelope, ["data", "meta"], "tasks response")
  return { data: expectArray<unknown>(envelope.data, "tasks response data").map((entry, index) => parseApiTask(entry, `tasks response data[${index}]`)), meta: parseTotalMeta(envelope.meta, "tasks response meta") }
}

export function parseTaskStatusEnvelope(value: unknown): { data: TaskStatusWindowsResponse; meta: RequiredOffsetPageMeta } {
  const envelope = expectRecord<Record<string, unknown>>(value, "task status windows response"); expectExactKeys(envelope, ["data", "meta"], "task status windows response")
  const data = expectRecord<Record<string, unknown>>(envelope.data, "task status windows response data"); expectExactKeys(data, ["statuses"], "task status windows response data")
  const statuses = expectArray<unknown>(data.statuses, "task status windows").map((entry, index) => {
    const label = `task status windows[${index}]`; const window = expectRecord<Record<string, unknown>>(entry, label); expectExactKeys(window, ["status", "tasks", "page"], label)
    if (!TASK_STATUSES.has(window.status as TaskStatus)) throw new ApiError("invalid_response", `${label}.status is unknown`)
    return { status: window.status as TaskStatus, tasks: expectArray<unknown>(window.tasks, `${label}.tasks`).map((task, taskIndex) => parseApiTask(task, `${label}.tasks[${taskIndex}]`)), page: parseTotalMeta(window.page, `${label}.page`) }
  })
  return { data: { statuses }, meta: parseOffsetMeta(envelope.meta, "task status windows response meta") }
}
