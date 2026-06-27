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
    expect(shellHeader).toContain("canCreateTask={canCreateTask}")
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
    expect(addTaskDialog).toContain("!canCreateTask || !newTitle.trim() || creating")
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

  it("keeps global search out of List reset while sizing Board by column status", () => {
    const collection = source("app/useTaskCollectionState.ts")
    const app = source("App.tsx")
    const taskQuery = collection.slice(collection.indexOf("const tasksQuery = useBoardTasks"), collection.indexOf("const taskData ="))
    const resetListFilters = app.slice(app.indexOf("const resetListFilters = useCallback"), app.indexOf("const requestLabelSuggestionsCommand"))

    expect(collection).toContain("const enabled = shouldLoadTaskCollection(view)")
    expect(taskQuery).toContain("enabled,")
    expect(taskQuery).toContain('boardStatuses: view === "board" ? visibleColumnStatuses : []')
    expect(taskQuery).toContain('limit: view === "list" ? rowsPerPage : BOARD_COLUMN_TASK_LIMIT')
    expect(taskQuery).toContain('offset: view === "list" ? pageOffset : 0')
    expect(resetListFilters).not.toContain("setSearch")
    expect(resetListFilters).toContain('setStatusFilter("all")')
    expect(resetListFilters).toContain("setPriorityFilters([])")
    expect(resetListFilters).toContain("setPageOffset(0)")
  })

  it("prunes stale List row selection when task rows change", () => {
    const list = source("features/list/ListView.tsx")

    expect(list).toContain("const selectTaskRef = useRef(onSelectTask)")
    expect(list).toContain("const taskIds = new Set(tasks.map((task) => task.id))")
    expect(list).toContain("setRowSelection((current) => {")
    expect(list).toContain("if (!taskIds.has(taskId))")
    expect(list).toContain("return changed ? next : current")
    expect(list).toContain("[listSort, onListSortChange]")
    expect(list).not.toContain("[listSort, onListSortChange, onSelectTask]")
  })

  it("memoizes the disabled task collection fallback page identity", () => {
    const collection = source("app/useTaskCollectionState.ts")
    const fallbackPage = collection.slice(collection.indexOf("const fallbackPage = useMemo"), collection.indexOf("const page ="))

    expect(fallbackPage).toContain("useMemo(")
    expect(fallbackPage).toContain("limit: rowsPerPage")
    expect(fallbackPage).toContain("offset: pageOffset")
    expect(fallbackPage).toContain("total: null")
    expect(collection).toContain("const page = taskData?.page ?? fallbackPage")
    expect(collection).not.toContain("const page = taskData?.page ?? {")
  })
})
