import {
  canonicalSseEventFingerprint,
  parseSseEvent,
  parseSseHeartbeat,
  sseEventEnvelopeFieldOrder,
  sseHeartbeatEventName,
  taskScopedSseEventKinds,
} from "../api/generated/sse"
import type { SseEventDataContract } from "../api/generated/contracts/sse-event-data"
import type { ParsedSseEvent, UnknownSseEvent } from "../api/generated/sse"
import type {
  BusinessValidationResult,
  ControlValidationResult,
  EnvelopeCandidate,
  EnvelopeParseResult,
  RawSseFrame,
  ScopeMetadata,
  StreamContractAdapter,
} from "./contracts"

type RecordValue = Record<string, unknown>

function isRecord(value: unknown): value is RecordValue {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function record(value: unknown): RecordValue | null {
  if (!isRecord(value)) return null
  return value
}

function isUnknownEvent(value: ParsedSseEvent): value is UnknownSseEvent {
  return "reason" in value
}

function safeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value)
}

function safeCursor(value: unknown): value is number {
  return safeInteger(value) && value >= 0
}

function parseHeaderCursor(value: string | null): number | null {
  if (value === null || !/^\d+$/.test(value)) return null
  const cursor = Number(value)
  return safeCursor(cursor) ? cursor : null
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalValue)
  const object = record(value)
  if (object === null) return value
  const result = Object.create(null) as RecordValue
  for (const key of Object.keys(object).sort()) {
    Object.defineProperty(result, key, {
      value: canonicalValue(object[key]),
      enumerable: true,
      writable: true,
      configurable: true,
    })
  }
  return result
}

/** Unknown fallback uses the generated envelope order; no Web-owned field inventory is duplicated. */
function canonicalUnknown(value: RecordValue): string {
  const ordered = Object.create(null) as RecordValue
  for (const field of sseEventEnvelopeFieldOrder) {
    Object.defineProperty(ordered, field, {
      value: canonicalValue(value[field]),
      enumerable: true,
      writable: true,
      configurable: true,
    })
  }
  return JSON.stringify(ordered)
}

function envelopeFromValue(value: unknown, frame: RawSseFrame | null): EnvelopeParseResult {
  const object = record(value)
  if (object === null) return { status: "invalid", code: "envelope-not-object" }
  for (const field of sseEventEnvelopeFieldOrder) {
    if (!(field in object)) return { status: "invalid", code: `envelope-missing-${field}` }
  }

  const id = object.id
  const eventId = object.event_id
  const boardId = object.board_id
  const taskId = object.task_id
  const runId = object.run_id
  const kind = object.kind
  const actor = object.actor
  const createdAt = object.created_at
  if (!safeCursor(id)) return { status: "invalid", code: "envelope-invalid-id" }
  if (typeof eventId !== "string" || eventId.length === 0) return { status: "invalid", code: "envelope-invalid-event-id" }
  if (typeof boardId !== "string" || boardId.length === 0) return { status: "invalid", code: "envelope-invalid-board-id" }
  if (typeof taskId !== "string" && taskId !== null) return { status: "invalid", code: "envelope-invalid-task-id" }
  if (typeof runId !== "string" && runId !== null) return { status: "invalid", code: "envelope-invalid-run-id" }
  if (typeof kind !== "string" || kind.length === 0) return { status: "invalid", code: "envelope-invalid-kind" }
  if (typeof actor !== "string" && actor !== null) return { status: "invalid", code: "envelope-invalid-actor" }
  if (!safeInteger(createdAt)) return { status: "invalid", code: "envelope-invalid-created-at" }

  if (frame !== null) {
    const headerId = parseHeaderCursor(frame.id)
    if (headerId === null) return { status: "invalid", code: "sse-invalid-header-id" }
    if (headerId !== id) return { status: "invalid", code: "sse-id-data-mismatch" }
    if (frame.eventName !== kind) return { status: "invalid", code: "sse-event-kind-mismatch" }
  }

  return {
    status: "valid",
    envelope: {
      id,
      eventId,
      boardId,
      taskId,
      runId,
      kind,
      createdAt,
      raw: object,
    },
  }
}

function scopeForKnown(envelope: EnvelopeCandidate, parsed: SseEventDataContract): ScopeMetadata | { status: "invalid"; code: string } {
  if (taskScopedSseEventKinds.some((kind) => kind === envelope.kind) && envelope.taskId === null) {
    return { status: "invalid", code: "task-scope-missing-task-id" }
  }
  const payload = record(parsed.payload)
  const parentTaskId = envelope.kind === "dependency.added" || envelope.kind === "dependency.removed" ? payload?.parent_task_id : undefined
  if ((envelope.kind === "dependency.added" || envelope.kind === "dependency.removed") && typeof parentTaskId !== "string") {
    return { status: "invalid", code: "dependency-parent-task-id-missing" }
  }
  const linkedTaskId = payload?.linked_task_id
  const signalId = payload?.signal_id
  return {
    taskId: envelope.taskId,
    ...(typeof parentTaskId === "string" ? { parentTaskId } : {}),
    ...(typeof linkedTaskId === "string" || linkedTaskId === null ? { linkedTaskId } : {}),
    ...(typeof signalId === "string" ? { signalId } : {}),
  }
}

function scopeForUnknown(envelope: EnvelopeCandidate): ScopeMetadata {
  return { taskId: envelope.taskId }
}

function parseJson(value: string): unknown {
  try {
    return JSON.parse(value)
  } catch {
    return null
  }
}

export function createGeneratedStreamContractAdapter(): StreamContractAdapter {
  return {
    parseEnvelope(frame: RawSseFrame): EnvelopeParseResult {
      return envelopeFromValue(parseJson(frame.data), frame)
    },
    parsePollingEnvelope(value: unknown): EnvelopeParseResult {
      return envelopeFromValue(value, null)
    },
    validateBusiness(envelope: EnvelopeCandidate): BusinessValidationResult {
      const parsed = parseSseEvent(envelope.raw)
      if (isUnknownEvent(parsed)) {
        if (parsed.reason === "invalid_envelope" || parsed.reason === "known_payload_invalid") {
          return { status: "invalid", code: parsed.reason }
        }
        return {
          status: "unknown",
          event: {
            ...envelope,
            scope: scopeForUnknown(envelope),
            canonicalFingerprint: canonicalUnknown(record(envelope.raw) ?? {}),
            known: false,
          },
        }
      }
      const scope = scopeForKnown(envelope, parsed)
      if ("status" in scope) return scope
      return {
        status: "known",
        event: {
          ...envelope,
          scope,
          canonicalFingerprint: canonicalSseEventFingerprint(parsed),
          known: true,
        },
      }
    },
    isControlFrame(frame: RawSseFrame): boolean {
      return frame.eventName === sseHeartbeatEventName
    },
    validateControl(frame: RawSseFrame): ControlValidationResult {
      if (frame.eventName !== sseHeartbeatEventName) return { status: "invalid", code: "heartbeat-event-name-mismatch" }
      if (frame.id !== null) return { status: "invalid", code: "heartbeat-id-forbidden" }
      const data = parseJson(frame.data)
      try {
        return { status: "valid", control: { raw: parseSseHeartbeat(data) } }
      } catch {
        return { status: "invalid", code: "heartbeat-invalid-data" }
      }
    },
  }
}
