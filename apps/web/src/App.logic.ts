import { mergeBoardEvents, type ExplorerEvent } from "./lib/api/explorer-read-model"
import type { CanonicalBoardId } from "./lib/sync/contracts"

const explorerBoundaryTelemetry = new Set([
  "recovery-complete",
  "poll-complete",
  "poll-boundary-complete",
  "protocol-anomaly",
  "isolation-anomaly",
  "poll-protocol-anomaly",
])

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
    if (byIdMatch || byEventIdMatch) continue
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
