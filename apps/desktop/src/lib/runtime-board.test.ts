import { describe, expect, it, vi } from "vitest"

import { switchRuntimeBoard } from "./runtime-board"
import type { RuntimeConfig } from "./api"

const config = {
  apiBaseUrl: "http://127.0.0.1:8721",
  actor: "desktop-test",
  board: "default",
} satisfies RuntimeConfig

describe("switchRuntimeBoard", () => {
  it("returns the current config without invoking runtime when board is unchanged", async () => {
    const invokeRuntime = vi.fn()

    const result = await switchRuntimeBoard(config, "default", invokeRuntime)

    expect(result).toBe(config)
    expect(invokeRuntime).not.toHaveBeenCalled()
  })

  it("uses the Tauri runtime command when available", async () => {
    const updated = { ...config, board: "ops" }
    const invokeRuntime = vi.fn(async () => updated)

    const result = await switchRuntimeBoard(config, "ops", invokeRuntime)

    expect(result).toEqual(updated)
    expect(invokeRuntime).toHaveBeenCalledWith("set_runtime_board", { board: "ops" })
  })

  it("returns an updated web-mode config when no runtime invoker is available", async () => {
    const result = await switchRuntimeBoard(config, "ops", null)

    expect(result).toEqual({ ...config, board: "ops" })
  })
})
