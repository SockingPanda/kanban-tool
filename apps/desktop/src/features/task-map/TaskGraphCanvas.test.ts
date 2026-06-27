import { describe, expect, it } from "vitest"

import { __test } from "./TaskGraphCanvas"

describe("TaskGraphCanvas interactions", () => {
  it("shows zoom controls and viewport pan in detail mode", () => {
    const interaction = __test.taskGraphInteraction("detail")

    expect(interaction.showControls).toBe(true)
    expect(interaction.showMiniMap).toBe(false)
    expect(interaction.panOnDrag).toBe(true)
    expect(interaction.zoomOnPinch).toBe(true)
    expect(interaction.zoomOnScroll).toBe(false)
    expect(interaction.zoomOnDoubleClick).toBe(false)
    expect(interaction.maxZoom).toBeGreaterThan(1.75)
  })

  it("keeps board map controls and map interactions enabled", () => {
    const interaction = __test.taskGraphInteraction("board-map")

    expect(interaction.showControls).toBe(true)
    expect(interaction.showMiniMap).toBe(true)
    expect(interaction.panOnDrag).toBe(true)
    expect(interaction.zoomOnScroll).toBe(true)
    expect(interaction.zoomOnPinch).toBe(true)
    expect(interaction.zoomOnDoubleClick).toBe(true)
    expect(interaction.maxZoom).toBe(1.75)
  })
})
