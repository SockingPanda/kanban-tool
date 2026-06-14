import { describe, expect, it } from "vitest"

import { boardGridStyle, boardScrollerClassName } from "./board-layout"

describe("board layout", () => {
  it("keeps columns reachable through horizontal board scrolling", () => {
    expect(boardScrollerClassName).toContain("overflow-x-auto")
    expect(boardScrollerClassName).toContain("overflow-y-hidden")

    expect(boardGridStyle(8)).toEqual({
      gridTemplateColumns: "repeat(8, minmax(18rem, 1fr))",
      minWidth: "max(100%, 144rem)",
    })
  })

  it("keeps a stable single-column grid for empty column sets", () => {
    expect(boardGridStyle(0)).toEqual({
      gridTemplateColumns: "repeat(1, minmax(18rem, 1fr))",
      minWidth: "max(100%, 18rem)",
    })
  })
})
