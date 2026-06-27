import { describe, expect, it } from "vitest"

import { graphNodeStatusClass, graphNodeStatusMiniMapColor } from "./task-map-colors"

describe("task map colors", () => {
  it("colors graph nodes by task status", () => {
    expect(graphNodeStatusClass("ready", false, false)).toContain("emerald")
    expect(graphNodeStatusClass("running", false, false)).toContain("sky")
    expect(graphNodeStatusClass("blocked", false, false)).toContain("red")
    expect(graphNodeStatusClass("review", false, false)).toContain("amber")
  })

  it("keeps selected and context states additive", () => {
    const selected = graphNodeStatusClass("running", true, false)
    const context = graphNodeStatusClass("running", false, true)

    expect(selected).toContain("sky")
    expect(selected).toContain("ring-2")
    expect(context).toContain("sky")
    expect(context).toContain("border-dashed")
  })

  it("uses status colors in the minimap", () => {
    expect(graphNodeStatusMiniMapColor("ready")).not.toBe(graphNodeStatusMiniMapColor("blocked"))
    expect(graphNodeStatusMiniMapColor("running")).toBe("#0284c7")
    expect(graphNodeStatusMiniMapColor("done")).toBe("#65a30d")
    expect(graphNodeStatusMiniMapColor("done", true)).toBe("#65a30d")
  })
})
