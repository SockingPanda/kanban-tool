import { useQuery } from "@tanstack/react-query"

import type { KanbanApi, SearchTasksResult, TaskListSort, TaskPageResult, TaskPlanFilter, TaskStatus } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export const BOARD_COLUMN_TASK_LIMIT = 50

export type BoardTasksData = {
  tasks: TaskPageResult["tasks"]
  page: TaskPageResult["page"]
  searchMeta: SearchTasksResult["searchMeta"] | null
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

export function useBoardTasks({ api, ...input }: BoardTaskRequestInput & { api: KanbanApi | null }) {
  const request = resolveBoardTaskRequest(input)

  return useQuery({
    enabled: Boolean(api),
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
    queryFn: async ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return loadBoardTasks(api, request, signal)
    },
  })
}

export async function loadBoardTasks(api: KanbanApi, request: BoardTaskRequest, signal?: AbortSignal): Promise<BoardTasksData> {
  if (request.mode === "board") {
    return request.search ? searchBoardStatuses(api, request, signal) : listBoardStatuses(api, request, signal)
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

async function listBoardStatuses(api: KanbanApi, request: BoardTaskRequest, signal?: AbortSignal): Promise<BoardTasksData> {
  if (request.statuses.length === 0) return emptyBoardWindow()

  const results = await Promise.all(
    request.statuses.map((status) =>
      api.listTasks({
        includeArchived: request.showArchived,
        statuses: [status],
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

async function searchBoardStatuses(api: KanbanApi, request: BoardTaskRequest, signal?: AbortSignal): Promise<BoardTasksData> {
  if (request.statuses.length === 0) return emptyBoardWindow()

  const results = await Promise.all(
    request.statuses.map((status) =>
      api.searchTasks({
        query: request.search,
        includeArchived: request.showArchived,
        statuses: [status],
        limit: request.limit,
        offset: 0,
        signal,
      }),
    ),
  )

  return {
    tasks: results.flatMap((result) => result.tasks),
    page: aggregateStatusPages(results.map((result) => result.page)),
    searchMeta: mergeSearchMeta(results.map((result) => result.searchMeta)),
  } satisfies BoardTasksData
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

function mergeSearchMeta(metas: SearchTasksResult["searchMeta"][]) {
  if (metas.length === 0) return null
  return {
    backend: commonValue(metas.map((meta) => meta.backend)) ?? "mixed",
    stale: metas.some((meta) => meta.stale),
    index_version: commonValue(metas.map((meta) => meta.index_version)),
    last_event_id: maxNullable(metas.map((meta) => meta.last_event_id)),
    index_lag_events: maxNullable(metas.map((meta) => meta.index_lag_events)),
  } satisfies SearchTasksResult["searchMeta"]
}

function commonValue<T>(values: T[]) {
  const [first] = values
  return values.every((value) => value === first) ? first : null
}

function maxNullable(values: Array<number | null>) {
  const numbers = values.filter((value): value is number => value !== null)
  return numbers.length ? Math.max(...numbers) : null
}
