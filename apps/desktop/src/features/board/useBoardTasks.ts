import { useQuery } from "@tanstack/react-query"

import type { KanbanApi, SearchTasksResult, TaskListSort, TaskPageResult, TaskStatus } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type BoardTasksData = {
  tasks: TaskPageResult["tasks"]
  page: TaskPageResult["page"]
  searchMeta: SearchTasksResult["searchMeta"] | null
}

export function useBoardTasks({
  api,
  search,
  statusFilter,
  priorityFilters = [],
  sort = "-updated_at",
  mode = "board",
  showArchived,
  limit,
  offset,
}: {
  api: KanbanApi | null
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters?: number[]
  sort?: TaskListSort
  mode?: "board" | "list"
  showArchived: boolean
  limit: number
  offset: number
}) {
  const normalizedSearch = search.trim()
  const statuses = statusFilter === "all" ? [] : [statusFilter]

  return useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.boardTasks({
      board: api?.board ?? "pending",
      search: normalizedSearch,
      status: statusFilter,
      priorities: priorityFilters,
      sort,
      mode,
      showArchived,
      limit,
      offset,
    }),
    queryFn: async ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      if (mode === "board" && normalizedSearch) {
        const result = await api.searchTasks({
          query: normalizedSearch,
          includeArchived: showArchived,
          statuses,
          limit,
          offset,
          signal,
        })
        return {
          tasks: result.tasks,
          page: result.page,
          searchMeta: result.searchMeta,
        } satisfies BoardTasksData
      }

      const result = await api.listTasks({
        includeArchived: showArchived,
        statuses,
        priorities: priorityFilters,
        query: normalizedSearch,
        sort,
        limit,
        offset,
        signal,
      })
      return { tasks: result.tasks, page: result.page, searchMeta: null } satisfies BoardTasksData
    },
  })
}
