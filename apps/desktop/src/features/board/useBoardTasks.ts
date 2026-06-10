import { useQuery } from "@tanstack/react-query"

import type { KanbanApi, SearchTasksResult, TaskPageResult, TaskStatus } from "@/lib/api"
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
  showArchived,
  limit,
  offset,
}: {
  api: KanbanApi | null
  search: string
  statusFilter: TaskStatus | "all"
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
      showArchived,
      limit,
      offset,
    }),
    queryFn: async ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      if (normalizedSearch) {
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
        limit,
        offset,
        signal,
      })
      return { tasks: result.tasks, page: result.page, searchMeta: null } satisfies BoardTasksData
    },
  })
}
