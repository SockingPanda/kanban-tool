import type { BoardSyncStatus } from "./types"

export interface BrowserConnectivityTarget {
  addEventListener(type: "online" | "offline", listener: () => void): void
  removeEventListener(type: "online" | "offline", listener: () => void): void
}

/** Register connectivity listeners even while the board read is bootstrapping. */
export function subscribeBrowserConnectivity(
  target: BrowserConnectivityTarget,
  onOffline: () => void,
  onOnline: () => void,
): () => void {
  target.addEventListener("offline", onOffline)
  target.addEventListener("online", onOnline)
  return () => {
    target.removeEventListener("offline", onOffline)
    target.removeEventListener("online", onOnline)
  }
}

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
    case "detached-async-failure":
      return "stale"
    case "circuit-open":
      return "circuit-open"
    default:
      return null
  }
}
