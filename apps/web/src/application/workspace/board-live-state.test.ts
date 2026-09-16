import { describe, expect, test, vi } from "vitest"

import { subscribeBrowserConnectivity } from "./board-live-state"

describe("Board 浏览器连接生命周期", () => {
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
