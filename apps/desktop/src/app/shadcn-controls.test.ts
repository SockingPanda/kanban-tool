import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"

import { describe, expect, it } from "vitest"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")

function source(path: string) {
  return readFileSync(resolve(root, path), "utf8")
}

describe("desktop shadcn control convergence", () => {
  it("backs tooltips with the Radix/shadcn trigger/content structure", () => {
    const content = source("components/ui/tooltip.tsx")

    expect(content).toContain('@radix-ui/react-tooltip')
    expect(content).toContain("TooltipTrigger")
    expect(content).toContain("TooltipContent")
    expect(content).not.toContain("cloneElement")
  })

  it("keeps task cards, table task links, dependency chips, and shell nav off ad hoc buttons", () => {
    const files = [
      "app/AppShell.tsx",
      "features/board/BoardView.tsx",
      "features/list/ListView.tsx",
      "features/task-detail/TaskDetail.tsx",
    ]

    for (const file of files) {
      expect(source(file), file).not.toContain("<button")
    }
  })

  it("keeps scoped toolbar, list, and detail controls off native select inputs", () => {
    const files = [
      "app/AppShell.tsx",
      "features/list/ListView.tsx",
      "features/task-detail/TaskDetail.tsx",
    ]

    for (const file of files) {
      const content = source(file)
      expect(content, file).not.toContain("NativeSelect")
      expect(content, file).not.toContain("<select")
      expect(content, file).not.toContain('type="checkbox"')
    }
  })

  it("routes compact single-value menus through a shadcn-compatible Select primitive", () => {
    const content = source("components/ui/menu-select.tsx")

    expect(content).toContain("Select")
    expect(content).toContain("SelectTrigger")
    expect(content).toContain("SelectContent")
    expect(content).not.toContain("DropdownMenuRadioGroup")
  })

  it("keeps List filters, rows-per-page, and button pagination controls", () => {
    const content = source("features/list/ListView.tsx")

    expect(content).toContain('ariaLabel="List status filter"')
    expect(content).toContain("Priority")
    expect(content).toContain('ariaLabel="Rows per page"')
    expect(content).toContain("Pagination")
    expect(content).toContain("PaginationContent")
    expect(content).toContain("onFirstPage")
    expect(content).toContain("onPreviousPage")
    expect(content).toContain("onNextPage")
    expect(content).toContain("onLastPage")
  })

  it("routes shell navigation through the local sidebar primitive", () => {
    const shell = source("app/AppShell.tsx")
    const sidebar = source("components/ui/sidebar.tsx")

    expect(shell).toContain("Sidebar")
    expect(shell).toContain("SidebarMenuButton")
    expect(shell).not.toContain("function NavItem")
    expect(sidebar).toContain("SidebarMenuButton")
  })

  it("keeps the archived shell control as a direct two-state button", () => {
    const content = source("app/AppShell.tsx")

    expect(content).toContain('aria-pressed={showArchived}')
    expect(content).toContain("onShowArchivedChange(!showArchived)")
    expect(content).not.toContain("Include archived tasks")
  })
})
