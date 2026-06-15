import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"

const sourceRoot = fileURLToPath(new URL("../", import.meta.url))

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
}

describe("task explorer chrome", () => {
  it("keeps the shell header free of task status filters and inert command buttons", () => {
    const content = source("app/AppShell.tsx")
    const shellHeader = content.slice(content.indexOf("function ShellHeader"), content.indexOf("function MainView"))

    expect(shellHeader).toContain('placeholder="Search tasks"')
    expect(shellHeader).toContain('aria-label={tasksRefreshing ? "Refreshing tasks" : "Refresh tasks"}')
    expect(shellHeader).toContain('title={tasksRefreshing ? "Refreshing tasks" : "Refresh tasks"}')
    expect(shellHeader).toContain("shouldShowTaskExplorerToolbar(view)")
    expect(shellHeader).toContain("<AddTaskDialog")
    expect(shellHeader).not.toContain("statusFilterOptions")
    expect(shellHeader).not.toContain("onStatusFilterChange")
    expect(shellHeader).not.toContain("<Command")
  })

  it("moves task creation into a shell-header Add task dialog without the inline row", () => {
    const content = source("app/AppShell.tsx")
    const appShell = content.slice(content.indexOf("export function AppShell"), content.indexOf("function ShellSidebar"))
    const addTaskDialog = content.slice(content.indexOf("function AddTaskDialog"), content.indexOf("function MainView"))

    expect(content).not.toContain("function TaskExplorerToolbar")
    expect(content).not.toContain("<TaskExplorerToolbar")
    expect(appShell).not.toContain("<AddTaskDialog")
    expect(appShell).not.toContain('placeholder="New task title"')
    expect(appShell).not.toContain("grid-cols-[1fr_1.4fr_auto]")
    expect(addTaskDialog).toContain("<Dialog")
    expect(addTaskDialog).toContain("<DialogContent")
    expect(addTaskDialog).toContain("<DialogTitle>Add task</DialogTitle>")
    expect(addTaskDialog).toContain("<DialogDescription>")
    expect(addTaskDialog).toContain('aria-label="Add task"')
    expect(addTaskDialog).toContain("<Plus")
    expect(addTaskDialog).toContain("name=\"new-task-title\"")
    expect(addTaskDialog).toContain("name=\"new-task-description\"")
    expect(addTaskDialog).not.toContain('placeholder="New task title"')
    expect(addTaskDialog).not.toContain("pageRangeLabel")
    expect(addTaskDialog).not.toContain("Previous")
    expect(addTaskDialog).not.toContain("Next")
  })

  it("keeps List-specific filters and pagination while removing its duplicate search box", () => {
    const content = source("features/list/ListView.tsx")

    expect(content).not.toContain('placeholder="Filter tasks"')
    expect(content).not.toContain("onSearchChange")
    expect(content).toContain('ariaLabel="List status filter"')
    expect(content).toContain("Priority")
    expect(content).toContain('ariaLabel="Rows per page"')
    expect(content).toContain("Previous")
    expect(content).toContain("Next")
  })

  it("keeps global search out of List reset and Board pagination sizing", () => {
    const content = source("App.tsx")
    const taskQuery = content.slice(content.indexOf("const tasksQuery = useBoardTasks"), content.indexOf("const tasks = tasksQuery.data"))
    const resetListFilters = content.slice(content.indexOf("onResetListFilters={() =>"), content.indexOf("onShowArchivedChange"))

    expect(taskQuery).toContain('limit: view === "list" ? rowsPerPage : DEFAULT_PAGE_SIZE')
    expect(taskQuery).toContain('offset: view === "list" ? pageOffset : 0')
    expect(resetListFilters).not.toContain("setSearch")
    expect(resetListFilters).toContain('setStatusFilter("all")')
    expect(resetListFilters).toContain("setPriorityFilters([])")
    expect(resetListFilters).toContain("setPageOffset(0)")
  })
})
