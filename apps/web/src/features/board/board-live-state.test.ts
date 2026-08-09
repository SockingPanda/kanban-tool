import { describe, expect, test } from "vitest"

import { boardSyncStatusForTelemetry } from "./board-live-state"

describe("Board live telemetry presentation", () => {
  test("maps connection-live to live and all degraded paths to visible stale/recovery states", () => {
    expect(boardSyncStatusForTelemetry("connection-live")).toBe("live")
    expect(boardSyncStatusForTelemetry("stalled")).toBe("stale")
    expect(boardSyncStatusForTelemetry("transport-failure")).toBe("stale")
    expect(boardSyncStatusForTelemetry("detached-async-failure")).toBe("stale")
    expect(boardSyncStatusForTelemetry("protocol-anomaly")).toBe("stale")
    expect(boardSyncStatusForTelemetry("poll-failure")).toBe("stale")
    expect(boardSyncStatusForTelemetry("recovery-start")).toBe("recovering")
    expect(boardSyncStatusForTelemetry("recovery-complete")).toBe("recovering")
    expect(boardSyncStatusForTelemetry("poll-boundary-complete")).toBe("recovering")
    expect(boardSyncStatusForTelemetry("circuit-open")).toBe("circuit-open")
    expect(boardSyncStatusForTelemetry("unrelated-event")).toBeNull()
  })
})
