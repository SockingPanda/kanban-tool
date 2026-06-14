import { describe, expect, it } from "vitest"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

import { boardGridStyle, boardScrollerClassName, clampBoardScrollLeft } from "./board-layout"

const sourceRoot = fileURLToPath(new URL("../../", import.meta.url))

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
}

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

  it("keeps shadcn task cards from expanding beyond their column", () => {
    const boardView = source("features/board/BoardView.tsx")

    expect(boardView).toContain("<Button")
    expect(boardView).toContain("h-auto min-w-0 w-full shrink overflow-hidden")
    expect(boardView).toContain('className="flex min-w-0 w-full items-start gap-2"')
    expect(boardView).toContain('className={cn("mt-1.5 h-2 w-2 shrink-0 rounded-full", statusAccent[task.status])}')
    expect(boardView).toContain('className="min-w-0 flex-1"')
  })
})
