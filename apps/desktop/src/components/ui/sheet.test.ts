import { describe, expect, it } from "vitest"

import { sheetContentClassName, sheetOverlayClassName } from "./sheet-motion"

describe("sheet motion classes", () => {
  it("marks the overlay for fade motion", () => {
    expect(sheetOverlayClassName()).toContain("kb-sheet-overlay")
  })

  it("marks side-specific content for slide motion", () => {
    expect(sheetContentClassName("right")).toContain("kb-sheet-content-right")
    expect(sheetContentClassName("left")).toContain("kb-sheet-content-left")
    expect(sheetContentClassName("top")).toContain("kb-sheet-content-top")
    expect(sheetContentClassName("bottom")).toContain("kb-sheet-content-bottom")
  })
})
