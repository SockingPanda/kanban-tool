import type { SseFrame } from "./sse-parser"
import type { ApiListEventsQueryContract } from "../api/generated/contracts/api-list-events-query"

/**
 * Canonical `boards.id` returned by the board query. This is deliberately
 * distinct from the runtime board selector/slug used in request URLs; the
 * Stage 03 query boundary is responsible for resolving that selector.
 */
declare const canonicalBoardIdBrand: unique symbol

export type CanonicalBoardId = string & { readonly [canonicalBoardIdBrand]: "CanonicalBoardId" }

export function asCanonicalBoardId(value: string): CanonicalBoardId {
  if (value.length === 0) throw new RangeError("canonical board ID must not be empty")
  return value as CanonicalBoardId
}

/** Raw frame emitted by the transport before any generated contract validation. */
export type RawSseFrame = SseFrame

/**
 * The transport-neutral envelope metadata needed by sync ordering.
 * The adapter owns the generated schema, payload union, scope cross-validator,
 * and canonical fingerprint; Web code never recreates those field inventories.
 */
export interface EnvelopeCandidate {
  readonly id: number
  readonly eventId: string
  readonly boardId: string
  readonly taskId: string | null
  readonly runId: string | null
  readonly kind: string
  readonly createdAt: number
  readonly raw: unknown
}

export interface ScopeMetadata {
  readonly taskId: string | null
  readonly parentTaskId?: string | null
  readonly linkedTaskId?: string | null
  readonly signalId?: string | null
  readonly atomRef?: string | null
}

export interface ValidatedBusinessEvent extends EnvelopeCandidate {
  readonly scope: ScopeMetadata
  readonly canonicalFingerprint: string
  readonly known: boolean
}

export interface ValidatedControlEvent {
  readonly raw: unknown
}

export type QueryRoot =
  | "boards"
  | "columns"
  | "events"
  | "tasks"
  | "stats"
  | "search-status"
  | "board-task-map"
  | "task-detail"
  | "task-dependencies"
  | "task-neighborhood"
  | "task-steps"
  | "task-runs"
  | "task-run-log"
  | "task-events"
  | "task-comments"
  | "task-attachments"
  | "task-label-suggestions"
  | "signals"
  | "signal"
  | "label-ontology"
  | "label-ontology-signal"
  | "label-ontology-atom"
  | "maintenance-status"

export interface QueryTarget {
  readonly root: QueryRoot
  readonly boardId?: string
  readonly taskId?: string
  readonly runId?: string
  readonly signalId?: string
  readonly atomRef?: string
  readonly observedOnly?: boolean
}

export interface InvalidationPlan {
  readonly kind: "known" | "unknown"
  readonly eventKind: string
  readonly timeline: boolean
  readonly fullRefetch: boolean
  readonly targets: readonly QueryTarget[]
  readonly reason?: string
}

export type EnvelopeParseResult =
  | { readonly status: "valid"; readonly envelope: EnvelopeCandidate }
  | { readonly status: "invalid"; readonly code: string }

export type BusinessValidationResult =
  | { readonly status: "known"; readonly event: ValidatedBusinessEvent }
  | { readonly status: "unknown"; readonly event: ValidatedBusinessEvent }
  | { readonly status: "invalid"; readonly code: string }

export type ControlValidationResult =
  | { readonly status: "valid"; readonly control: ValidatedControlEvent }
  | { readonly status: "invalid"; readonly code: string }

/**
 * Explicit seam for generated protocol helpers. A concrete implementation is
 * supplied once the server/protocol contract milestone is frozen; tests and
 * the controller can inject a fixture adapter without unsafe casts.
 */
export interface StreamContractAdapter {
  parseEnvelope(frame: RawSseFrame): EnvelopeParseResult
  /** Parse a canonical JSON event returned by polling/catch-up without SSE headers. */
  parsePollingEnvelope(value: unknown): EnvelopeParseResult
  validateBusiness(envelope: EnvelopeCandidate): BusinessValidationResult
  /** Return true only for generated protocol control event names. */
  isControlFrame(frame: RawSseFrame): boolean
  validateControl(frame: RawSseFrame): ControlValidationResult
}

export interface SyncToken {
  readonly boardId: CanonicalBoardId
  readonly connectionEpoch: number
  readonly generation: number
}

export type RecoveryMode = "F" | "R"

export type SyncTimer = number

export interface SyncClock {
  now(): number
  setTimeout(callback: () => void, delayMs: number): SyncTimer
  clearTimeout(timer: SyncTimer): void
}

export interface SyncTelemetryEntry {
  readonly type: string
  readonly boardId: CanonicalBoardId
  readonly cursor: number
  readonly details?: Readonly<Record<string, unknown>>
}

export interface SyncTelemetry {
  record(entry: SyncTelemetryEntry): void
}

export interface RecoveryBoundary {
  readonly highWatermark: number
  readonly byId: ReadonlyMap<number, string>
  readonly byEventId: ReadonlyMap<string, string>
  /** Validated accepted events in server order, used to replay query effects. */
  readonly events: readonly ValidatedBusinessEvent[]
  readonly token: SyncToken
  readonly revision: number
  readonly published: boolean
}

export interface RecoveryResult {
  readonly confirmedCursor: number
  readonly noGap: boolean
  readonly boundary: RecoveryBoundary
}

export interface PollEventsPage {
  readonly events: readonly unknown[]
  readonly nextAfter: number
  readonly hasMore: boolean
  readonly noGap: boolean
  /** Optional published boundary returned by catch-up polling. */
  readonly boundary?: RecoveryBoundary
  readonly confirmedCursor?: number
}

export interface SyncQuerySink<TEvent = ValidatedBusinessEvent> {
  onEvent(event: TEvent, plan: InvalidationPlan, token: SyncToken, signal?: AbortSignal): Promise<void>
  refetchObserved(mode: RecoveryMode, token: SyncToken, after: number, expectedRevision: number, signal: AbortSignal): Promise<RecoveryResult>
  pollEvents(query: ApiListEventsQueryContract, signal: AbortSignal): Promise<PollEventsPage>
}

export interface SseTransportRequest {
  readonly url: string
  readonly signal: AbortSignal
  readonly headers?: HeadersInit
  readonly onFrame: (frame: RawSseFrame) => void
  readonly onError: (error: unknown) => void
  readonly onEof: () => void
}

export interface SseTransportConnection {
  readonly closed: boolean
  close(): void
}

export type SseTransport = (request: SseTransportRequest) => SseTransportConnection
