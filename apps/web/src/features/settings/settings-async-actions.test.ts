import { describe, expect, test } from "vitest"

import { callSettingsAction } from "./settings-async-actions"

describe("Settings async action seam", () => {
  test("converts synchronous clipboard/reconnect throws into rejected promises", async () => {
    await expect(callSettingsAction(() => { throw new Error("clipboard denied") })).rejects.toThrow("clipboard denied")
    await expect(callSettingsAction(() => { throw new Error("reconnect failed") })).rejects.toThrow("reconnect failed")
  })
})
