import { describe, expect, test, vi } from "vitest"

import { bootstrapWebApp } from "./bootstrap"
import type { WebRuntimeConfig } from "./lib/runtime"

const runtime: WebRuntimeConfig = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "local",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "dev",
}

describe("Web React bootstrap", () => {
  test("mounts only after runtime validation succeeds", async () => {
    const mount = vi.fn()
    const loadRuntime = vi.fn(async () => runtime)
    const root = {} as HTMLElement

    await expect(bootstrapWebApp(root, { loadRuntime, mount })).resolves.toBe(runtime)
    expect(loadRuntime).toHaveBeenCalledTimes(1)
    expect(mount).toHaveBeenCalledWith(root, runtime)
  })

  test("does not mount the app when runtime validation rejects", async () => {
    const mount = vi.fn()
    const loadRuntime = vi.fn(async () => {
      throw new Error("invalid runtime")
    })

    await expect(bootstrapWebApp({} as HTMLElement, { loadRuntime, mount })).rejects.toThrow("invalid runtime")
    expect(mount).not.toHaveBeenCalled()
  })
})
