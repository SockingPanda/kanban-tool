import { describe, expect, test, vi } from "vitest"

import { boardSyncStatusForTelemetry, subscribeBrowserConnectivity } from "./board-live-state"

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

  test("registers browser connectivity listeners during bootstrap and cleans them up", () => {
    const listeners = new Map<string, () => void>()
    const target = {
      addEventListener: (type: "online" | "offline", listener: () => void) => listeners.set(type, listener),
      removeEventListener: (type: "online" | "offline") => listeners.delete(type),
    }
    const onOffline = vi.fn()
    const onOnline = vi.fn()
    const unsubscribe = subscribeBrowserConnectivity(target, onOffline, onOnline)

    listeners.get("offline")?.()
    listeners.get("online")?.()
    expect(onOffline).toHaveBeenCalledOnce()
    expect(onOnline).toHaveBeenCalledOnce()
    unsubscribe()
    expect(listeners.size).toBe(0)
  })
})
