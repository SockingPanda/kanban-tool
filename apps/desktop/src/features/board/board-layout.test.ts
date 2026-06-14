import { describe, expect, it } from "vitest"

import { boardGridStyle, boardScrollerClassName, clampBoardScrollLeft } from "./board-layout"

describe("board layout", () => {
  it("keeps columns reachable through horizontal board scrolling", () => {
    expect(boardScrollerClassName).toContain("overflow-x-auto")
    expect(boardScrollerClassName).toContain("overflow-y-hidden")
    expect(boardScrollerClassName).toContain("kb-native-scrollbar-fade")

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

  it("recomputes grid width from the current visible columns", () => {
    expect(boardGridStyle(7)).toEqual({
      gridTemplateColumns: "repeat(7, minmax(18rem, 1fr))",
      minWidth: "max(100%, 126rem)",
    })
  })

  it("clamps stale horizontal scroll after a column is hidden", () => {
    const scroller = {
      clientWidth: 900,
      scrollLeft: 1404,
      scrollWidth: 2304,
    }

    clampBoardScrollLeft(scroller, 7)

    expect(scroller.scrollLeft).toBe(1116)
  })
})
