import { describe, expect, it, vi } from "vitest"

import type { KanbanApi, SearchTasksResult, Task, TaskPageResult, TaskStatus } from "@/lib/api"

import { BOARD_COLUMN_TASK_LIMIT, loadBoardTasks, resolveBoardTaskRequest } from "./useBoardTasks"

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
      planFilters: ["plan_needed", "has_subtasks"],
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
      planFilters: ["plan_needed", "has_subtasks"],
      sort: "priority",
      limit: 50,
      offset: 100,
    })
  })
})

describe("loadBoardTasks", () => {
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
      planFilters: ["plan_needed", "incomplete_required_subtasks"],
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
        planFilters: ["plan_needed", "incomplete_required_subtasks"],
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
})
