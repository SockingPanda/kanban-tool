import type { SseFrame } from "./sse-parser"

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
  validateBusiness(envelope: EnvelopeCandidate): BusinessValidationResult
  validateControl(frame: RawSseFrame): ControlValidationResult
}

export interface SyncToken {
  readonly boardId: string
  readonly connectionEpoch: number
  readonly generation: number
}

export type RecoveryMode = "F" | "R"

export interface SyncQuerySink<TEvent = ValidatedBusinessEvent> {
  onEvent(event: TEvent, token: SyncToken): Promise<void>
  refetchObserved(mode: RecoveryMode, token: SyncToken, after: number): Promise<{
    readonly confirmedCursor: number
    readonly noGap: boolean
  }>
  pollEvents(query: unknown, signal: AbortSignal): Promise<unknown>
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
