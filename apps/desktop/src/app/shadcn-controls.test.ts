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

  it("mounts the TooltipProvider above List tooltip usage", () => {
    const main = source("main.tsx")
    const list = source("features/list/ListView.tsx")

    expect(main).toContain("TooltipProvider")
    expect(main).toContain("<TooltipProvider>")
    expect(main).toContain("<App />")
    expect(list).toContain("TooltipTrigger")
    expect(list).toContain("TooltipContent")
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

  it("replaces browser-native confirmations and prompts with shadcn dialog primitives", () => {
    const files = [
      "App.tsx",
      "features/task-detail/TaskDetail.tsx",
      "features/maintenance/MaintenanceView.tsx",
    ]

    for (const file of files) {
      const content = source(file)
      expect(content, file).not.toContain("window.confirm")
      expect(content, file).not.toContain("window.prompt")
    }

    expect(source("components/ui/alert-dialog.tsx")).toContain("@radix-ui/react-alert-dialog")
    expect(source("components/ui/dialog.tsx")).toContain("@radix-ui/react-dialog")
    expect(source("components/ui/tabs.tsx")).toContain("@radix-ui/react-tabs")
    expect(source("App.tsx")).toContain("AlertDialog")
    expect(source("App.tsx")).toContain("Dialog")
    expect(source("features/task-detail/TaskDetail.tsx")).toContain("AlertDialog")
    expect(source("features/maintenance/MaintenanceView.tsx")).toContain("AlertDialog")
  })

  it("routes shell tabs, input compositions, empty states, and field groups through shadcn primitives", () => {
    expect(source("components/ui/input-group.tsx")).toContain("InputGroup")
    expect(source("components/ui/empty.tsx")).toContain("Empty")
    expect(source("components/ui/field.tsx")).toContain("Field")

    const shell = source("app/AppShell.tsx")
    const detail = source("features/task-detail/TaskDetail.tsx")
    const list = source("features/list/ListView.tsx")
    const events = source("features/events/EventsView.tsx")
    const runs = source("features/runs/RunsView.tsx")
    const maintenance = source("features/maintenance/MaintenanceView.tsx")
    const health = source("features/health/HealthView.tsx")

    expect(shell).toContain("Tabs")
    expect(shell).toContain("TabsList")
    expect(shell).toContain("TabsTrigger")
    expect(shell).toContain("InputGroup")
    expect(shell).toContain("Field")
    expect(detail).toContain("InputGroup")
    expect(detail).toContain("Field")

    for (const content of [list, events, runs, detail, maintenance, health]) {
      expect(content).toContain("Empty")
    }
  })

  it("uses the shadcn-compatible textarea composition for task comments", () => {
    const detail = source("features/task-detail/TaskDetail.tsx")

    expect(source("components/ui/input-group.tsx")).toContain("InputGroupTextarea")
    expect(detail).toContain("InputGroupTextarea")
    expect(detail).toContain('name="comment-body"')
    expect(detail).toContain('aria-label="Comment body"')
    expect(detail).toContain('autoComplete="off"')
  })
})
