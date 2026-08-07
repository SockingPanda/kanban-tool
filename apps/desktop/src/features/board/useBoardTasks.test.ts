import { describe, expect, it, vi } from "vitest"
import { keepPreviousData } from "@tanstack/react-query"

import type { KanbanApi, Task, TaskPageResult, TaskStatus } from "@/lib/api"

import { BOARD_COLUMN_TASK_LIMIT, boardTasksQueryOptions, loadBoardTasks, resolveBoardTaskRequest } from "./useBoardTasks"

function task(id: string, status: TaskStatus) {
  return { id, status } as Task
}

describe("resolveBoardTaskRequest", () => {
  it("keeps board queries on visible statuses and the first page", () => {
    const request = resolveBoardTaskRequest({
      mode: "board",
      boardStatuses: ["triage", "blocked", "triage"],
      search: "  blocked parent  ",
      statusFilter: "ready",
      priorityFilters: [0, 2],
      planFilters: ["plan_needed"],
      sort: "priority",
      showArchived: true,
      limit: BOARD_COLUMN_TASK_LIMIT,
      offset: 75,
    })

    expect(request).toMatchObject({
      search: "blocked parent",
      statusFilter: "all",
      statuses: ["triage", "blocked"],
      priorityFilters: [],
      planFilters: [],
      sort: "-updated_at",
      limit: 50,
      offset: 0,
    })
  })

  it("keeps list filters, sorting, search, and pagination intact", () => {
    const request = resolveBoardTaskRequest({
      mode: "list",
      search: "  dashboard  ",
      statusFilter: "blocked",
      priorityFilters: [1, 3],
      planFilters: ["plan_needed", "has_steps"],
      sort: "priority",
      showArchived: false,
      limit: 50,
      offset: 100,
    })

    expect(request).toMatchObject({
      search: "dashboard",
      statusFilter: "blocked",
      statuses: ["blocked"],
      priorityFilters: [1, 3],
      planFilters: ["plan_needed", "has_steps"],
      sort: "priority",
      limit: 50,
      offset: 100,
    })
  })
})

describe("boardTasksQueryOptions", () => {
  it("keeps previous board data while board query parameters fetch the next result", () => {
    const api = { board: "default" } as unknown as KanbanApi
    const firstQuery = boardTasksQueryOptions({
      api,
      mode: "board",
      boardStatuses: ["triage", "ready"],
      search: "",
      statusFilter: "all",
      showArchived: false,
      limit: BOARD_COLUMN_TASK_LIMIT,
      offset: 0,
    })
    const nextQuery = boardTasksQueryOptions({
      api,
      mode: "board",
      boardStatuses: ["triage", "ready"],
      search: "  blocked  ",
      statusFilter: "all",
      showArchived: false,
      limit: BOARD_COLUMN_TASK_LIMIT,
      offset: 0,
    })

    expect(firstQuery.queryKey).not.toEqual(nextQuery.queryKey)
    expect(firstQuery.placeholderData).toBe(keepPreviousData)
    expect(nextQuery.placeholderData).toBe(keepPreviousData)
  })

  it("keeps previous list data while filters, sort, or pagination fetch the next result", () => {
    const api = { board: "default" } as unknown as KanbanApi
    const firstQuery = boardTasksQueryOptions({
      api,
      mode: "list",
      search: "",
      statusFilter: "all",
      priorityFilters: [],
      planFilters: [],
      sort: "-updated_at",
      showArchived: false,
      limit: 100,
      offset: 0,
    })
    const nextQuery = boardTasksQueryOptions({
      api,
      mode: "list",
      search: "",
      statusFilter: "blocked",
      priorityFilters: [1],
      planFilters: ["plan_needed"],
      sort: "priority",
      showArchived: false,
      limit: 50,
      offset: 50,
    })

    expect(firstQuery.queryKey).not.toEqual(nextQuery.queryKey)
    expect(firstQuery.placeholderData).toBe(keepPreviousData)
    expect(nextQuery.placeholderData).toBe(keepPreviousData)
  })
})

describe("loadBoardTasks", () => {
  it("loads visible board statuses through the canonical batch endpoint", async () => {
    const listTasksByStatus = vi.fn(async () => ({
      statuses: [
        { status: "triage", tasks: [task("task-triage", "triage")], page: { limit: 50, offset: 0, total: 1 } },
        { status: "blocked", tasks: [task("task-blocked", "blocked")], page: { limit: 50, offset: 0, total: 1 } },
      ],
    }))
    const searchTasksByStatus = vi.fn()
    const searchTasks = vi.fn()
    const listTasks = vi.fn()
    const api = { board: "default", listTasks, listTasksByStatus, searchTasksByStatus, searchTasks } as unknown as KanbanApi
    const request = resolveBoardTaskRequest({
      mode: "board",
      boardStatuses: ["triage", "blocked"],
      search: "",
      statusFilter: "all",
      showArchived: false,
      limit: BOARD_COLUMN_TASK_LIMIT,
      offset: 0,
    })

    const result = await loadBoardTasks(api, request)

    expect(listTasksByStatus).toHaveBeenCalledTimes(1)
    expect(listTasksByStatus).toHaveBeenCalledWith(expect.objectContaining({
      query: "",
      statuses: ["triage", "blocked"],
      limit: 50,
      offset: 0,
    }))
    expect(listTasks).not.toHaveBeenCalled()
    expect(searchTasksByStatus).not.toHaveBeenCalled()
    expect(searchTasks).not.toHaveBeenCalled()
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
    expect(result.page).toEqual({ limit: 100, offset: 0, total: 2 })
    expect(result.searchMeta).toBeNull()
  })

  it("passes board search through the canonical batch endpoint", async () => {
    const listTasksByStatus = vi.fn(async () => ({
      statuses: [
        { status: "triage", tasks: [task("search-triage", "triage")], page: { limit: 50, offset: 0, total: 1 } },
        { status: "blocked", tasks: [task("search-blocked", "blocked")], page: { limit: 50, offset: 0, total: 1 } },
      ],
    }))
    const searchTasksByStatus = vi.fn()
    const searchTasks = vi.fn()
    const listTasks = vi.fn()
    const api = { board: "default", listTasks, listTasksByStatus, searchTasksByStatus, searchTasks } as unknown as KanbanApi
    const request = resolveBoardTaskRequest({
      mode: "board",
      boardStatuses: ["triage", "blocked"],
      search: "blocked parent",
      statusFilter: "all",
      showArchived: false,
      limit: BOARD_COLUMN_TASK_LIMIT,
      offset: 0,
    })

    const result = await loadBoardTasks(api, request)

    expect(listTasksByStatus).toHaveBeenCalledTimes(1)
    expect(listTasksByStatus).toHaveBeenCalledWith(expect.objectContaining({
      query: "blocked parent",
      statuses: ["triage", "blocked"],
      limit: 50,
      offset: 0,
    }))
    expect(listTasks).not.toHaveBeenCalled()
    expect(searchTasksByStatus).not.toHaveBeenCalled()
    expect(searchTasks).not.toHaveBeenCalled()
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
    expect(result.page).toEqual({ limit: 100, offset: 0, total: 2 })
    expect(result.searchMeta).toBeNull()
  })

  it("passes list execution plan filters to the task list endpoint", async () => {
    const listTasks = vi.fn(async () => ({
      tasks: [task("task-ready", "ready")],
      page: { limit: 50, offset: 25, total: 1 },
    }) satisfies TaskPageResult)
    const api = { board: "default", listTasks } as unknown as KanbanApi
    const request = resolveBoardTaskRequest({
      mode: "list",
      search: "",
      statusFilter: "ready",
      priorityFilters: [1],
      planFilters: ["plan_needed", "incomplete_required_steps"],
      sort: "priority",
      showArchived: false,
      limit: 50,
      offset: 25,
    })

    await loadBoardTasks(api, request)

    expect(listTasks).toHaveBeenCalledWith(
      expect.objectContaining({
        statuses: ["ready"],
        priorities: [1],
        planFilters: ["plan_needed", "incomplete_required_steps"],
        sort: "priority",
        limit: 50,
        offset: 25,
      }),
    )
  })

})
