import { describe, expect, it } from "vitest"

import { parseSidebarOpen, serializeSidebarOpen } from "./sidebar-state"

describe("sidebar state helpers", () => {
  it("parses persisted sidebar state", () => {
    expect(parseSidebarOpen("true")).toBe(true)
    expect(parseSidebarOpen("false")).toBe(false)
    expect(parseSidebarOpen("missing")).toBe(true)
    expect(parseSidebarOpen(null, false)).toBe(false)
  })

  it("serializes sidebar state for localStorage", () => {
    expect(serializeSidebarOpen(true)).toBe("true")
    expect(serializeSidebarOpen(false)).toBe("false")
  })
})
