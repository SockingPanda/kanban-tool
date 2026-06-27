import { describe, expect, it, vi } from "vitest"
import { keepPreviousData } from "@tanstack/react-query"

import type { KanbanApi, SearchTasksResult, Task, TaskPageResult, TaskStatus } from "@/lib/api"

import { BOARD_COLUMN_TASK_LIMIT, boardTasksQueryOptions, loadBoardTasks, resolveBoardTaskRequest } from "./useBoardTasks"

function task(id: string, status: TaskStatus) {
  return { id, status } as Task
}

describe("resolveBoardTaskRequest", () => {
  it("keeps board queries on a per-visible-status first page", () => {
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
  it("loads visible board statuses through the batch endpoint when available", async () => {
    const listTasksByStatus = vi.fn(async () => ({
      statuses: [
        { status: "triage", tasks: [task("task-triage", "triage")], page: { limit: 50, offset: 0, total: 1 } },
        { status: "blocked", tasks: [task("task-blocked", "blocked")], page: { limit: 50, offset: 0, total: 2 } },
      ],
    }))
    const listTasks = vi.fn()
    const api = { board: "default", listTasksByStatus, listTasks } as unknown as KanbanApi
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

    expect(listTasksByStatus).toHaveBeenCalledWith(expect.objectContaining({
      statuses: ["triage", "blocked"],
      limit: 50,
      offset: 0,
    }))
    expect(listTasks).not.toHaveBeenCalled()
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
    expect(result.page).toEqual({ limit: 100, offset: 0, total: 3 })
  })

  it("falls back to per-status task requests when batch is unavailable", async () => {
    const listTasksByStatus = vi.fn(async () => {
      throw Object.assign(new Error("404 Not Found"), { code: "http_error" })
    })
    const listTasks = vi.fn(async (options: { statuses?: TaskStatus[]; limit?: number; offset?: number }) => {
      const status = options.statuses?.[0] ?? "triage"
      return {
        tasks: [task(`task-${status}`, status)],
        page: { limit: options.limit ?? 0, offset: options.offset ?? 0, total: 1 },
      } satisfies TaskPageResult
    })
    const api = { board: "default", listTasksByStatus, listTasks } as unknown as KanbanApi
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
    expect(listTasks).toHaveBeenCalledTimes(2)
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
  })

  it("loads the first page independently for each visible board status", async () => {
    const listTasks = vi.fn(async (options: { statuses?: TaskStatus[]; limit?: number; offset?: number }) => {
      const status = options.statuses?.[0] ?? "triage"
      return {
        tasks: [task(`task-${status}`, status)],
        page: { limit: options.limit ?? 0, offset: options.offset ?? 0, total: 60 },
      } satisfies TaskPageResult
    })
    const api = { board: "default", listTasks } as unknown as KanbanApi
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

    expect(listTasks).toHaveBeenCalledTimes(2)
    expect(listTasks).toHaveBeenNthCalledWith(1, expect.objectContaining({ statuses: ["triage"], limit: 50, offset: 0 }))
    expect(listTasks).toHaveBeenNthCalledWith(2, expect.objectContaining({ statuses: ["blocked"], limit: 50, offset: 0 }))
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
    expect(result.page).toEqual({ limit: 100, offset: 0, total: 120 })
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

  it("searches each visible board status and merges search metadata", async () => {
    const searchTasks = vi.fn(async (options: { statuses?: TaskStatus[]; limit?: number; offset?: number }) => {
      const status = options.statuses?.[0] ?? "triage"
      return {
        tasks: [task(`search-${status}`, status)],
        page: { limit: options.limit ?? 0, offset: options.offset ?? 0, total: 2 },
        searchMeta: {
          backend: status === "triage" ? "tantivy" : "sqlite",
          stale: status === "blocked",
          index_version: "v1",
          last_event_id: status === "triage" ? 10 : 12,
          index_lag_events: status === "triage" ? 0 : 3,
        },
      } satisfies SearchTasksResult
    })
    const api = { board: "default", searchTasks } as unknown as KanbanApi
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

    expect(searchTasks).toHaveBeenCalledTimes(2)
    expect(searchTasks).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ query: "blocked parent", statuses: ["triage"], limit: 50, offset: 0 }),
    )
    expect(searchTasks).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ query: "blocked parent", statuses: ["blocked"], limit: 50, offset: 0 }),
    )
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
    expect(result.page).toEqual({ limit: 100, offset: 0, total: 4 })
    expect(result.searchMeta).toEqual({
      backend: "mixed",
      stale: true,
      index_version: "v1",
      last_event_id: 12,
      index_lag_events: 3,
    })
  })

  it("searches visible board statuses through the batch endpoint when available", async () => {
    const searchMeta = {
      backend: "sqlite",
      stale: false,
      index_version: null,
      last_event_id: null,
      index_lag_events: null,
    } satisfies SearchTasksResult["searchMeta"]
    const searchTasksByStatus = vi.fn(async () => ({
      statuses: [
        { status: "triage", tasks: [task("search-triage", "triage")], page: { limit: 50, offset: 0, total: 1 }, searchMeta },
        { status: "blocked", tasks: [task("search-blocked", "blocked")], page: { limit: 50, offset: 0, total: 1 }, searchMeta },
      ],
    }))
    const searchTasks = vi.fn()
    const api = { board: "default", searchTasksByStatus, searchTasks } as unknown as KanbanApi
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

    expect(searchTasksByStatus).toHaveBeenCalledWith(expect.objectContaining({
      query: "blocked parent",
      statuses: ["triage", "blocked"],
      limit: 50,
      offset: 0,
    }))
    expect(searchTasks).not.toHaveBeenCalled()
    expect(result.tasks.map((entry) => entry.status)).toEqual(["triage", "blocked"])
  })
})
