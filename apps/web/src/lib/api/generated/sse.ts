// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { SseEventDataContract } from "./contracts/sse-event-data";
import { sseEventDataValidator } from "./contracts/sse-event-data";

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
