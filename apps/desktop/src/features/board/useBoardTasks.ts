import { keepPreviousData, useQuery } from "@tanstack/react-query"

import { type KanbanApi, type SearchTasksMeta, type TaskListSort, type TaskPageResult, type TaskPlanFilter, type TaskStatus } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export const BOARD_COLUMN_TASK_LIMIT = 50

export type BoardTasksData = {
  tasks: TaskPageResult["tasks"]
  page: TaskPageResult["page"]
  searchMeta: SearchTasksMeta | null
}

export type BoardTaskRequestInput = {
  mode?: "board" | "list"
  boardStatuses?: TaskStatus[]
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters?: number[]
  planFilters?: TaskPlanFilter[]
  sort?: TaskListSort
  showArchived: boolean
  limit: number
  offset: number
}

type UseBoardTasksInput = BoardTaskRequestInput & {
  api: KanbanApi | null
  enabled?: boolean
}

export type BoardTaskRequest = {
  mode: "board" | "list"
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  planFilters: TaskPlanFilter[]
  sort: TaskListSort
  statuses: TaskStatus[]
  showArchived: boolean
  limit: number
  offset: number
}

export function resolveBoardTaskRequest({
  mode = "board",
  boardStatuses = [],
  search,
  statusFilter,
  priorityFilters = [],
  planFilters = [],
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
      statuses: uniqueStatuses(boardStatuses),
      priorityFilters: [],
      planFilters: [],
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
    planFilters,
    sort,
    showArchived,
    limit,
    offset,
  }
}

export function useBoardTasks({ api, enabled = true, ...input }: UseBoardTasksInput) {
  return useQuery(boardTasksQueryOptions({ api, enabled, ...input }))
}

export function boardTasksQueryOptions({ api, enabled = true, ...input }: UseBoardTasksInput) {
  const request = resolveBoardTaskRequest(input)

  return {
    enabled: Boolean(enabled && api),
    queryKey: queryKeys.boardTasks({
      board: api?.board ?? "pending",
      search: request.search,
      status: request.statusFilter,
      priorities: request.priorityFilters,
      planFilters: request.planFilters,
      sort: request.sort,
      mode: request.mode,
      statuses: request.statuses,
      showArchived: request.showArchived,
      limit: request.limit,
      offset: request.offset,
    }),
    queryFn: async ({ signal }: { signal?: AbortSignal }) => {
      if (!api) throw new Error("API client is not ready")
      return loadBoardTasks(api, request, signal)
    },
    placeholderData: keepPreviousData,
  }
}

export async function loadBoardTasks(api: KanbanApi, request: BoardTaskRequest, signal?: AbortSignal): Promise<BoardTasksData> {
  if (request.mode === "board") {
    if (request.statuses.length === 0) return emptyBoardWindow()

    const results = await Promise.all(
      request.statuses.map((status) =>
        api.listTasks({
          includeArchived: request.showArchived,
          statuses: [status],
          priorities: request.priorityFilters,
          planFilters: request.planFilters,
          query: request.search,
          sort: request.sort,
          limit: request.limit,
          offset: 0,
          signal,
        }),
      ),
    )

    return {
      tasks: results.flatMap((result) => result.tasks),
      page: aggregateStatusPages(results.map((result) => result.page)),
      searchMeta: null,
    } satisfies BoardTasksData
  }

  const result = await api.listTasks({
    includeArchived: request.showArchived,
    statuses: request.statuses,
    priorities: request.priorityFilters,
    planFilters: request.planFilters,
    query: request.search,
    sort: request.sort,
    limit: request.limit,
    offset: request.offset,
    signal,
  })
  return { tasks: result.tasks, page: result.page, searchMeta: null } satisfies BoardTasksData
}

function uniqueStatuses(statuses: TaskStatus[]) {
  return Array.from(new Set(statuses))
}

function emptyBoardWindow() {
  return {
    tasks: [],
    page: { limit: 0, offset: 0, total: 0 },
    searchMeta: null,
  } satisfies BoardTasksData
}

function aggregateStatusPages(pages: TaskPageResult["page"][]) {
  return {
    limit: pages.reduce((total, page) => total + page.limit, 0),
    offset: 0,
    total: pages.every((page) => page.total !== null) ? pages.reduce((total, page) => total + (page.total ?? 0), 0) : null,
  } satisfies TaskPageResult["page"]
}
