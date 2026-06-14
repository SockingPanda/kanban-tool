import { useQuery } from "@tanstack/react-query"

import type { KanbanApi, SearchTasksResult, TaskListSort, TaskPageResult, TaskStatus } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type BoardTasksData = {
  tasks: TaskPageResult["tasks"]
  page: TaskPageResult["page"]
  searchMeta: SearchTasksResult["searchMeta"] | null
}

export type BoardTaskRequestInput = {
  mode?: "board" | "list"
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters?: number[]
  sort?: TaskListSort
  showArchived: boolean
  limit: number
  offset: number
}

export type BoardTaskRequest = {
  mode: "board" | "list"
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  sort: TaskListSort
  statuses: TaskStatus[]
  showArchived: boolean
  limit: number
  offset: number
}

export function resolveBoardTaskRequest({
  mode = "board",
  search,
  statusFilter,
  priorityFilters = [],
  sort = "-updated_at",
  showArchived,
  limit,
  offset,
}: BoardTaskRequestInput): BoardTaskRequest {
  const normalizedSearch = search.trim()
  if (mode === "board") {
    return {
      mode,
      search: normalizedSearch,
      statusFilter: "all",
      statuses: [],
      priorityFilters: [],
      sort: "-updated_at",
      showArchived,
      limit,
      offset: 0,
    }
  }

  return {
    mode,
    search: normalizedSearch,
    statusFilter,
    statuses: statusFilter === "all" ? [] : [statusFilter],
    priorityFilters,
    sort,
    showArchived,
    limit,
    offset,
  }
}

export function useBoardTasks({ api, ...input }: BoardTaskRequestInput & { api: KanbanApi | null }) {
  const request = resolveBoardTaskRequest(input)

  return useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.boardTasks({
      board: api?.board ?? "pending",
      search: request.search,
      status: request.statusFilter,
      priorities: request.priorityFilters,
      sort: request.sort,
      mode: request.mode,
      showArchived: request.showArchived,
      limit: request.limit,
      offset: request.offset,
    }),
    queryFn: async ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      if (request.mode === "board" && request.search) {
        const result = await api.searchTasks({
          query: request.search,
          includeArchived: request.showArchived,
          statuses: request.statuses,
          limit: request.limit,
          offset: request.offset,
          signal,
        })
        return {
          tasks: result.tasks,
          page: result.page,
          searchMeta: result.searchMeta,
        } satisfies BoardTasksData
      }

      const result = await api.listTasks({
        includeArchived: request.showArchived,
        statuses: request.statuses,
        priorities: request.priorityFilters,
        query: request.search,
        sort: request.sort,
        limit: request.limit,
        offset: request.offset,
        signal,
      })
      return { tasks: result.tasks, page: result.page, searchMeta: null } satisfies BoardTasksData
    },
  })
}
