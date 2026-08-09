import { mergeBoardEvents, type ExplorerEvent } from "./lib/api/explorer-read-model"
import { createGeneratedStreamContractAdapter } from "./lib/sync/generated-adapter"
import { classifyEvent, fullRefetchPlan } from "./lib/sync/invalidation"
import type { CanonicalBoardId, InvalidationPlan, QueryRoot, ValidatedBusinessEvent } from "./lib/sync/contracts"

const explorerBoundaryTelemetry = new Set([
  "recovery-complete",
  "poll-complete",
  "poll-boundary-complete",
  "protocol-anomaly",
  "isolation-anomaly",
  "poll-protocol-anomaly",
  "protocol-anomaly-suppressed",
  "stalled",
  "transport-failure",
  "sink-effect-failure",
  "recovery-failure",
  "poll-failure",
  "circuit-open",
  "detached-async-failure",
])

export interface ExplorerEventInvalidation {
  readonly board: boolean
  readonly inspector: boolean
  readonly runs: boolean
  readonly fullRefetch: boolean
}

const boardRoots = new Set<QueryRoot>(["columns", "tasks", "stats", "board-task-map"])
const inspectorRoots = new Set<QueryRoot>([
  "task-detail",
  "task-dependencies",
  "task-neighborhood",
  "task-steps",
  "task-comments",
  "task-attachments",
  "task-label-suggestions",
])
const runRoots = new Set<QueryRoot>(["task-runs", "task-run-log"])
const generatedStreamAdapter = createGeneratedStreamContractAdapter()

function projectExplorerInvalidation(plan: InvalidationPlan): ExplorerEventInvalidation {
  const roots = new Set(plan.targets.map((target) => target.root))
  return {
    board: plan.fullRefetch || [...boardRoots].some((root) => roots.has(root)),
    inspector: plan.fullRefetch || [...inspectorRoots].some((root) => roots.has(root)),
    runs: plan.fullRefetch || [...runRoots].some((root) => roots.has(root)),
    fullRefetch: plan.fullRefetch,
  }
}

function validatedEventForExplorer(event: ExplorerEvent, boardId: CanonicalBoardId): ValidatedBusinessEvent | null {
  const envelope = generatedStreamAdapter.parsePollingEnvelope(event)
  if (envelope.status !== "valid" || envelope.envelope.boardId !== boardId) return null
  const business = generatedStreamAdapter.validateBusiness(envelope.envelope)
  if (business.status === "invalid" || business.event.boardId !== boardId) return null
  return business.event
}

/** Rebuild the canonical plan from the generated event payload before projecting it. */
export function explorerEventInvalidationPlan(event: ExplorerEvent, boardId: CanonicalBoardId): InvalidationPlan {
  const validated = validatedEventForExplorer(event, boardId)
  return validated === null ? fullRefetchPlan(event.kind, "unknown", boardId) : classifyEvent(validated)
}

/** Project an existing server invalidation plan onto mounted Explorer views. */
export function explorerEventInvalidation(event: ExplorerEvent, boardId: CanonicalBoardId): ExplorerEventInvalidation {
  return projectExplorerInvalidation(explorerEventInvalidationPlan(event, boardId))
}

function stableEventValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableEventValue)
  if (typeof value !== "object" || value === null) return value
  const record = value as Record<string, unknown>
  return Object.fromEntries(Object.keys(record).sort().map((key) => [key, stableEventValue(record[key])]))
}

function eventFingerprint(event: ExplorerEvent): string {
  return JSON.stringify(stableEventValue(event))
}

/** Append a telemetry burst with numeric-id/event-id dedupe and ASC ordering. */
export function appendExplorerEventBatch(
  existing: readonly ExplorerEvent[],
  incoming: readonly ExplorerEvent[],
  boardId: CanonicalBoardId,
): readonly ExplorerEvent[] {
  const byId = new Map<number, ExplorerEvent>()
  const byEventId = new Map<string, ExplorerEvent>()
  for (const event of [...existing, ...incoming]) {
    const byIdMatch = byId.get(event.id)
    const byEventIdMatch = byEventId.get(event.event_id)
    if ((byIdMatch && byIdMatch.event_id !== event.event_id) || (byEventIdMatch && byEventIdMatch.id !== event.id)) {
      throw new Error("event batch identity conflict")
    }
    if (byIdMatch || byEventIdMatch) {
      const previous = byIdMatch ?? byEventIdMatch
      if (previous && eventFingerprint(previous) !== eventFingerprint(event)) {
        throw new Error("event batch duplicate fingerprint conflict")
      }
      continue
    }
    byId.set(event.id, event)
    byEventId.set(event.event_id, event)
  }
  return mergeBoardEvents([], [...byId.values()].sort((left, right) => left.id - right.id), boardId)
}

/**
 * Poll boundary and poll completion describe one published read boundary.
 * The UI schedules one bounded commit for the whole telemetry burst instead
 * of incrementing visible read revisions once per control frame.
 */
export function coalesceExplorerBoundary(types: readonly string[]): { readonly invalidationDelta: 0 | 1; readonly eventsRefreshDelta: 0 | 1 } {
  const hasBoundary = types.some((type) => explorerBoundaryTelemetry.has(type))
  return hasBoundary ? { invalidationDelta: 1, eventsRefreshDelta: 1 } : { invalidationDelta: 0, eventsRefreshDelta: 0 }
}
