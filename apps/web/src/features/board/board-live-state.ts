import type { BoardSyncStatus } from "./types"

/** Map sync telemetry to the small set of user-visible Board states. */
export function boardSyncStatusForTelemetry(type: string): BoardSyncStatus | null {
  switch (type) {
    case "connection-live":
      return "live"
    case "recovery-start":
    case "recovery-connection-retry":
    case "recovery-complete":
    case "poll-boundary-complete":
      return "recovering"
    case "stalled":
    case "transport-failure":
    case "protocol-anomaly":
    case "protocol-anomaly-suppressed":
    case "isolation-anomaly":
    case "sink-effect-failure":
    case "recovery-failure":
    case "poll-failure":
    case "poll-protocol-anomaly":
      return "stale"
    case "circuit-open":
      return "circuit-open"
    default:
      return null
  }
}
