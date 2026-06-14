import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"

import { SIDEBAR_WIDTH_TRANSITION_MS, isSidebarWidthTransition } from "./sidebar-animation"
import { boardGridStyle, boardScrollerClassName } from "@/features/board/board-layout"
import { sheetContentClassName } from "@/components/ui/sheet-motion"

const sourceRoot = fileURLToPath(new URL("../", import.meta.url))

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
}

describe("desktop layout and scroll contracts", () => {
  it("keeps the board as the only horizontal overflow region", () => {
    expect(boardScrollerClassName.split(" ")).toEqual(
      expect.arrayContaining(["min-h-0", "min-w-0", "flex-1", "overflow-x-auto", "overflow-y-hidden"]),
    )
    expect(boardGridStyle(9)).toMatchObject({
      gridTemplateColumns: "repeat(9, minmax(18rem, 1fr))",
      minWidth: "max(100%, 162rem)",
    })

    const boardView = source("features/board/BoardView.tsx")
    expect(boardView).toContain('className="grid h-full min-h-0 gap-px"')
    expect(boardView).toContain("style={boardGridStyle(columns.length)}")
  })

  it("confines board column overflow to each column body", () => {
    const boardView = source("features/board/BoardView.tsx")

    expect(boardView).toContain('"flex min-h-0 min-w-0 flex-col overflow-hidden bg-muted/60"')
    expect(boardView).toContain('<ScrollArea className="flex-1"')
    expect(boardView).toContain('viewportClassName="p-2 pb-8 scroll-pb-8"')
    expect(boardView).toContain("getScrollElement: () => parentRef.current")
  })

  it("keeps task detail sheet chrome fixed while the body scrolls", () => {
    const appShell = source("app/AppShell.tsx")
    const taskDetail = source("features/task-detail/TaskDetail.tsx")

    expect(sheetContentClassName("right").split(" ")).toEqual(
      expect.arrayContaining(["fixed", "flex", "flex-col"]),
    )
    expect(appShell).toContain('className="w-[min(620px,calc(100vw-32px))] p-0"')
    expect(taskDetail).toContain('className="flex h-full min-h-0 flex-col"')
    expect(taskDetail).toContain('<ScrollArea className="flex-1 p-4">')
  })

  it("clips sidebar content during width transitions and waits for width completion", () => {
    const appShell = source("app/AppShell.tsx")

    expect(SIDEBAR_WIDTH_TRANSITION_MS).toBe(200)
    expect(isSidebarWidthTransition("width")).toBe(true)
    expect(isSidebarWidthTransition("opacity")).toBe(false)
    expect(appShell).toContain('"flex shrink-0 flex-col overflow-hidden border-r border-border bg-sidebar transition-[width] duration-200"')
    expect(appShell).toContain('open ? "w-60 max-sm:w-14" : "w-14"')
    expect(appShell).toContain('!contentOpen && "hidden"')
    expect(appShell).toContain("onTransitionEnd={handleTransitionEnd}")
  })

  it("tracks the manual narrow-width smoke checklist beside the automatic contracts", () => {
    const checklist = readFileSync(
      new URL("../../../../docs/plans/desktop-layout-smoke.md", import.meta.url),
      "utf8",
    )

    expect(checklist).toContain("Automatic contracts")
    expect(checklist).toContain("Manual narrow-width smoke")
    expect(checklist).toContain("Board horizontal overflow")
    expect(checklist).toContain("Column vertical scroll")
    expect(checklist).toContain("Task detail sheet body scroll")
    expect(checklist).toContain("Sidebar transition and clipping")
  })
})
