import type { DtoEventPayload } from "../../generated/rpc/kanban/v1/dto_pb"
import { int64, record, RpcCodecError } from "./value-codec"

type EventCase = Exclude<DtoEventPayload["value"]["case"], undefined>

const cases: Readonly<Record<string, EventCase>> = {
  "board.archived": "empty", "task.archived": "empty", "task.updated": "empty",
  "dependency.added": "dependency", "dependency.removed": "dependency",
  "label.created": "labelCreated", "label.deleted": "labelDeleted",
  "label.ontology.action.created": "labelOntologyActionCreated",
  "label.ontology.observation.recorded": "labelOntologyObservationRecorded",
  "label.ontology.signal.reviewed": "labelOntologySignalReviewed",
  "signal.recorded": "signalRecorded", "signal.reviewed": "signalReviewed",
  "task.claimed": "taskClaimed", "task.comment.created": "taskCommentCreated",
  "task.completed": "taskResult", "task.submitted_for_review": "taskResult",
  "task.created": "taskStatus", "task.promoted": "taskToStatus", "task.recomputed": "taskToStatus",
  "task.released": "taskToStatus", "task.specified": "taskToStatus", "task.unblocked": "taskToStatus",
  "task.execution_plan.not_required": "executionPlan", "task.execution_plan.planned": "executionPlan", "task.execution_plan.unplanned": "executionPlan",
  "task.heartbeat": "heartbeat", "task.label.added": "taskLabel", "task.label.removed": "taskLabel",
  "task.label_proposal.accepted": "labelProposal", "task.label_proposal.proposed": "labelProposal", "task.label_proposal.rejected": "labelProposal",
  "task.reopened": "taskReopened", "task.retry_policy.updated": "retryPolicy",
  "task.step.created": "taskStep", "task.step.reopened": "taskStep", "task.step.done": "taskStep",
  "task.step.skipped": "taskStep", "task.step.removed": "taskStep", "task.step.updated": "taskStep",
  "task.export_sanitized": "taskExportSanitized",
}

const constrainedFields: Readonly<Record<string, readonly [string, string]>> = {
  "task.execution_plan.not_required": ["state", "not_required"],
  "task.execution_plan.planned": ["state", "planned"],
  "task.execution_plan.unplanned": ["state", "unplanned"],
  "task.label_proposal.accepted": ["status", "accepted"],
  "task.label_proposal.proposed": ["status", "proposed"],
  "task.label_proposal.rejected": ["status", "rejected"],
  "task.step.created": ["status", "todo"], "task.step.reopened": ["status", "todo"],
  "task.step.done": ["status", "done"], "task.step.skipped": ["status", "skipped"],
}

export function eventCase(kind: string, payload: unknown): EventCase {
  let result: EventCase
  if (kind === "board.created") result = Object.hasOwn(record(payload), "slug") ? "boardCreated" : "empty"
  else if (kind === "task.blocked") {
    const value = record(payload)
    result = Object.hasOwn(value, "reason") ? "taskReason" : Object.hasOwn(value, "retry_count") ? "taskRetry" : "taskResult"
  } else if (kind === "task.reclaimed") result = Object.hasOwn(record(payload), "reason") ? "taskReclaimed" : "taskRetry"
  else result = Object.hasOwn(cases, kind) ? cases[kind] : "unknown"
  const constraint = constrainedFields[kind]
  if (constraint !== undefined && record(payload)[constraint[0]] !== constraint[1]) throw new RpcCodecError("RPC event kind 与 payload 的状态不一致。")
  return result
}

export function validateEventCase(kind: string, branch: EventCase, payload: unknown): void {
  if (eventCase(kind, payload) !== branch) throw new RpcCodecError("RPC event kind 与 typed payload 分支不一致。")
}

export function validateStreamEvent(event: Record<string, unknown>): void {
  if (int64(event.id) < 0n || [event.event_id, event.board_id, event.kind].some((value) => typeof value !== "string" || value.length === 0)) throw new RpcCodecError("RPC event identity 无效。")
  const kind = event.kind as string
  const known = Object.hasOwn(cases, kind) || ["board.created", "task.blocked", "task.reclaimed"].includes(kind)
  const taskScoped = known && (kind.startsWith("task.") || kind.startsWith("dependency.") || kind === "label.ontology.observation.recorded")
  if (taskScoped && (typeof event.task_id !== "string" || event.task_id.length === 0)) throw new RpcCodecError("RPC task event 缺少 task_id。")
}
