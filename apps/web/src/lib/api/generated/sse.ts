// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { SseEventDataContract } from "./contracts/sse-event-data";
import { sseEventDataValidator } from "./contracts/sse-event-data";
import type { SseEventHeartbeatContract } from "./contracts/sse-event-heartbeat";
import { parseSseEventHeartbeat, sseEventHeartbeatValidator } from "./contracts/sse-event-heartbeat";

export const sseHeartbeatEventName = "kb-heartbeat" as const;
export const sseEventEnvelopeFieldOrder = ["id","event_id","board_id","task_id","run_id","kind","actor","payload","created_at"] as const;
export const taskScopedSseEventKinds = ["dependency.added","dependency.removed","task.archived","task.blocked","task.claimed","task.comment.created","task.completed","task.created","task.execution_plan.not_required","task.execution_plan.planned","task.execution_plan.unplanned","task.export_sanitized","task.heartbeat","task.label.added","task.label.removed","task.label_proposal.accepted","task.label_proposal.proposed","task.label_proposal.rejected","task.promoted","task.reclaimed","task.recomputed","task.released","task.reopened","task.retry_policy.updated","task.specified","task.step.created","task.step.done","task.step.removed","task.step.reopened","task.step.skipped","task.step.updated","task.submitted_for_review","task.unblocked","task.updated"] as const;

export type SseEventEnvelopeField = (typeof sseEventEnvelopeFieldOrder)[number];
export type TaskScopedSseEventKind = (typeof taskScopedSseEventKinds)[number];
export type SseHeartbeatDataContract = SseEventHeartbeatContract;
export const sseHeartbeatDataValidator = sseEventHeartbeatValidator;
export const isSseHeartbeat = sseEventHeartbeatValidator;
export function parseSseHeartbeat(value: unknown): SseHeartbeatDataContract {
  return parseSseEventHeartbeat(value);
}

export const knownSseEventKinds = [
  "board.created",
  "board.archived",
  "dependency.added",
  "dependency.removed",
  "label.created",
  "label.deleted",
  "signal.recorded",
  "signal.reviewed",
  "task.archived",
  "task.blocked",
  "task.claimed",
  "task.comment.created",
  "task.completed",
  "task.created",
  "task.execution_plan.not_required",
  "task.execution_plan.planned",
  "task.execution_plan.unplanned",
  "task.heartbeat",
  "task.label.added",
  "task.label.removed",
  "task.label_proposal.accepted",
  "task.label_proposal.proposed",
  "task.label_proposal.rejected",
  "task.promoted",
  "task.reclaimed",
  "task.recomputed",
  "task.released",
  "task.reopened",
  "task.retry_policy.updated",
  "task.specified",
  "task.step.created",
  "task.step.done",
  "task.step.removed",
  "task.step.reopened",
  "task.step.skipped",
  "task.step.updated",
  "task.submitted_for_review",
  "task.unblocked",
  "task.updated",
  "task.export_sanitized",
] as const;

export type KnownSseEventKind = (typeof knownSseEventKinds)[number];
export type KnownSseEvent = { [K in KnownSseEventKind]: Extract<SseEventDataContract, { kind: K }> }[KnownSseEventKind];

export interface UnknownSseEvent {
  readonly kind: string | null;
  readonly raw: unknown;
  readonly envelope: Record<string, unknown> | null;
  readonly reason: "unknown_kind" | "known_payload_invalid" | "invalid_envelope";
}

export type ParsedSseEvent = KnownSseEvent | UnknownSseEvent;

export function canonicalizeSseEventEnvelope(value: SseEventDataContract): Record<string, unknown> {
  const result = Object.create(null) as Record<string, unknown>;
  for (const field of sseEventEnvelopeFieldOrder) Object.defineProperty(result, field, { value: canonicalizeSseValue(value[field]), enumerable: true, writable: true, configurable: true });
  return result;
}

export function canonicalSseEventFingerprint(value: SseEventDataContract): string {
  return JSON.stringify(canonicalizeSseEventEnvelope(value));
}

function canonicalizeSseValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalizeSseValue);
  if (!isRecord(value)) return value;
  const result = Object.create(null) as Record<string, unknown>;
  for (const key of Object.keys(value).sort()) Object.defineProperty(result, key, { value: canonicalizeSseValue(value[key]), enumerable: true, writable: true, configurable: true });
  return result;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function parseSseEvent(value: unknown): ParsedSseEvent {
  if (!isRecord(value)) return invalidEnvelope(value, null);
  const kind = typeof value.kind === "string" ? value.kind : null;
  if (!sseEventDataValidator(value)) {
    return kind !== null && isKnownKind(kind)
      ? { kind, raw: value, envelope: value, reason: "known_payload_invalid" }
      : invalidEnvelope(value, value);
  }
  if (kind !== null && isKnownSseEvent(value)) return value;
  if (kind !== null) return { kind, raw: value, envelope: value, reason: "unknown_kind" };
  return invalidEnvelope(value, value);
}

function isKnownSseEvent(value: SseEventDataContract): value is KnownSseEvent {
  return typeof value.kind === "string" && isKnownKind(value.kind);
}

function isKnownKind(value: string): value is KnownSseEventKind {
  return knownSseEventKinds.some((kind) => kind === value);
}

function invalidEnvelope(raw: unknown, envelope: Record<string, unknown> | null): UnknownSseEvent {
  return { kind: envelope && typeof envelope.kind === "string" ? envelope.kind : null, raw, envelope, reason: "invalid_envelope" };
}
