import { describe, expect, it } from "vitest"

import { graphNodeStatusClass, graphNodeStatusMiniMapColor, graphNodeStepProgressClass } from "./task-map-colors"

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

  it("uses dependency blocked colors for non-blocked graph nodes", () => {
    expect(graphNodeStatusClass("ready", false, false, true)).toContain("red")
    expect(graphNodeStatusClass("ready", false, false, true)).not.toContain("emerald")
    expect(graphNodeStatusClass("blocked", false, false, false)).toContain("red")
  })

  it("uses status colors in the minimap", () => {
    expect(graphNodeStatusMiniMapColor("ready")).not.toBe(graphNodeStatusMiniMapColor("blocked"))
    expect(graphNodeStatusMiniMapColor("running")).toBe("#0284c7")
    expect(graphNodeStatusMiniMapColor("done")).toBe("#65a30d")
  })

  it("uses blocked minimap color for dependency blocked graph nodes", () => {
    expect(graphNodeStatusMiniMapColor("ready", true)).toBe(graphNodeStatusMiniMapColor("blocked"))
    expect(graphNodeStatusMiniMapColor("done", false)).toBe("#65a30d")
  })

  it("uses done colors for graph node step progress fill", () => {
    expect(graphNodeStepProgressClass()).toContain("lime")
  })
})
