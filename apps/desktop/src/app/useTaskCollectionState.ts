import { useEffect, useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"

import { queueCountsFromStats, queueCountsFromTasks } from "@/app/queue-counts"
import { shouldLoadTaskCollection } from "@/app/task-selection"
import { fallbackColumns } from "@/features/board/board-config"
import { sortBoardColumnTasks } from "@/features/board/board-card-state"
import { BOARD_COLUMN_TASK_LIMIT, useBoardTasks } from "@/features/board/useBoardTasks"
import type { OperatorView } from "@/features/navigation/view-types"
import { defaultListSort, listSortToApiSort, type ListSortState, type TaskPlanFilter } from "@/features/list/table-state"
import type { BoardColumn, KanbanApi, Task, TaskStatus } from "@/lib/api"
import { hasNextPage, hasPreviousPage, lastPageOffset } from "@/lib/pagination"
import { queryKeys } from "@/lib/query-keys"
import { useDebouncedValue } from "@/lib/use-debounced-value"

const DEFAULT_PAGE_SIZE = 100
const EMPTY_TASKS: Task[] = []

export function useTaskCollectionState(api: KanbanApi | null, view: OperatorView, reportError: (error: unknown) => void) {
  const [search, setSearch] = useState("")
  const debouncedSearch = useDebouncedValue(search, 250)
  const [statusFilter, setStatusFilter] = useState<TaskStatus | "all">("all")
  const [priorityFilters, setPriorityFilters] = useState<number[]>([])
  const [planFilters, setPlanFilters] = useState<TaskPlanFilter[]>([])
  const [listSort, setListSort] = useState<ListSortState>(defaultListSort)
  const [showArchived, setShowArchived] = useState(false)
  const [pageOffset, setPageOffset] = useState(0)
  const [rowsPerPage, setRowsPerPage] = useState(DEFAULT_PAGE_SIZE)
  const [lastRefreshAt, setLastRefreshAt] = useState<number | null>(null)

  useEffect(() => {
    setPageOffset(0)
  }, [debouncedSearch, showArchived, statusFilter, priorityFilters, planFilters, listSort])

  const columnsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.columns(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API 客户端尚未就绪")
      return api.listBoardColumns({ signal })
    },
  })

  const boardsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.boards(),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API 客户端尚未就绪")
      return api.listBoards({ signal })
    },
  })

  const visibleColumns = useMemo(
    () => (columnsQuery.data ?? fallbackColumns).filter((column) => showArchived || (!column.hidden && column.status !== "archived")),
    [columnsQuery.data, showArchived],
  )
  const visibleColumnStatuses = useMemo(() => visibleColumns.map((column) => column.status), [visibleColumns])
  const enabled = shouldLoadTaskCollection(view)
  const statsEnabled = view === "board" || view === "list" || view === "map" || view === "runs"

  const statsQuery = useQuery({
    enabled: Boolean(api && statsEnabled),
    queryKey: queryKeys.stats(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API 客户端尚未就绪")
      return api.stats({ signal })
    },
    staleTime: 30_000,
  })

  const tasksQuery = useBoardTasks({
    api,
    enabled,
    boardStatuses: view === "board" ? visibleColumnStatuses : [],
    search: debouncedSearch,
    statusFilter,
    priorityFilters: view === "list" ? priorityFilters : [],
    planFilters: view === "list" ? planFilters : [],
    sort: view === "list" ? listSortToApiSort(listSort) : "-updated_at",
    mode: view === "list" ? "list" : "board",
    showArchived,
    limit: view === "list" ? rowsPerPage : BOARD_COLUMN_TASK_LIMIT,
    offset: view === "list" ? pageOffset : 0,
  })

  const taskData = enabled ? tasksQuery.data : undefined
  const tasks = taskData?.tasks ?? EMPTY_TASKS
  const fallbackPage = useMemo(() => ({ limit: rowsPerPage, offset: pageOffset, total: null }), [pageOffset, rowsPerPage])
  const page = taskData?.page ?? fallbackPage
  const searchMeta = taskData?.searchMeta ?? null
  const hasNext = hasNextPage(page, tasks.length)
  const hasPrevious = hasPreviousPage(page)
  const lastOffset = lastPageOffset(page)

  useEffect(() => {
    if (enabled && tasksQuery.dataUpdatedAt) setLastRefreshAt(tasksQuery.dataUpdatedAt)
  }, [enabled, tasksQuery.dataUpdatedAt])

  useEffect(() => {
    if (columnsQuery.error) reportError(columnsQuery.error)
  }, [columnsQuery.error, reportError])

  useEffect(() => {
    if (boardsQuery.error) reportError(boardsQuery.error)
  }, [boardsQuery.error, reportError])

  useEffect(() => {
    if (enabled && tasksQuery.error) reportError(tasksQuery.error)
  }, [enabled, reportError, tasksQuery.error])

  useEffect(() => {
    if (statsQuery.error) reportError(statsQuery.error)
  }, [reportError, statsQuery.error])

  const groupedTasks = useMemo(() => {
    const map = new Map<TaskStatus, Task[]>()
    for (const column of visibleColumns) map.set(column.status, [])
    for (const task of tasks) {
      if (map.has(task.status)) map.get(task.status)!.push(task)
    }
    for (const [status, columnTasks] of map) {
      map.set(status, sortBoardColumnTasks(columnTasks, status))
    }
    return map
  }, [tasks, visibleColumns])

  const fallbackQueueCounts = useMemo(() => queueCountsFromTasks(tasks), [tasks])
  const queueCounts = useMemo(
    () => queueCountsFromStats(statsQuery.data?.status_counts, fallbackQueueCounts),
    [fallbackQueueCounts, statsQuery.data?.status_counts],
  )

  return useMemo(
    () => ({
      boardsQuery,
      columns: visibleColumns as BoardColumn[],
      enabled,
      groupedTasks,
      hasNext,
      hasPrevious,
      lastOffset,
      lastRefreshAt,
      listSort,
      page,
      pageOffset,
      planFilters,
      priorityFilters,
      queueCounts,
      rowsPerPage,
      search,
      debouncedSearch,
      searchMeta,
      setLastRefreshAt,
      setListSort,
      setPageOffset,
      setPlanFilters,
      setPriorityFilters,
      setRowsPerPage,
      setSearch,
      setShowArchived,
      setStatusFilter,
      showArchived,
      statsQuery,
      statusFilter,
      tasks,
      tasksQuery,
    }),
    [
      boardsQuery,
      debouncedSearch,
      enabled,
      groupedTasks,
      hasNext,
      hasPrevious,
      lastOffset,
      lastRefreshAt,
      listSort,
      page,
      pageOffset,
      planFilters,
      priorityFilters,
      queueCounts,
      rowsPerPage,
      search,
      searchMeta,
      showArchived,
      statsQuery,
      statusFilter,
      tasks,
      tasksQuery,
      visibleColumns,
    ],
  )
}
