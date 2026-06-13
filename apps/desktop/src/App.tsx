import { FormEvent, useCallback, useEffect, useMemo, useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { AppShell } from "@/app/AppShell"
import { createBoardSwitchReset } from "@/app/board-switch-state"
import { parseSidebarOpen, serializeSidebarOpen, SIDEBAR_OPEN_STORAGE_KEY } from "@/app/sidebar-state"
import {
  applyRootTheme,
  effectiveTheme,
  nextThemeMode,
  parseThemeMode,
  THEME_STORAGE_KEY,
  type ThemeMode,
} from "@/app/theme"
import { reconcileSelectedTaskId } from "@/app/task-selection"
import { fallbackColumns } from "@/features/board/board-config"
import { sortBoardColumnTasks } from "@/features/board/board-card-state"
import { executeDragTransition, planDragTransition } from "@/features/board/drag-policy"
import { useBoardTasks } from "@/features/board/useBoardTasks"
import { useEventPoller } from "@/features/events/useEventPoller"
import type { OperatorView } from "@/features/navigation/view-types"
import { defaultListSort, listSortToApiSort, type ListSortState } from "@/features/list/table-state"
import { invalidateTaskDetailAndBoard } from "@/features/task-detail/detail-invalidation"
import { taskDetailOrEmpty, useTaskDetail } from "@/features/task-detail/useTaskDetail"
import {
  parseDateInput,
  reconcileSavedTaskDraft,
  reconcileTaskDraft,
  type TaskDraftState,
  type TaskEditDraft,
} from "@/features/task-detail/task-draft"
import {
  ApiError,
  BoardColumn,
  ClaimResponse,
  KanbanApi,
  RuntimeConfig,
  Task,
  TaskStatus,
  loadRuntimeConfig,
} from "@/lib/api"
import { switchRuntimeBoard } from "@/lib/runtime-board"
import { queryKeys } from "@/lib/query-keys"
import { hasNextPage, hasPreviousPage, lastPageOffset } from "@/lib/pagination"
import { useDebouncedValue } from "@/lib/use-debounced-value"
import { reconcileClaimTokenForTask, reconcileClaimTokensForTasks } from "@/lib/claim-tokens"

const DEFAULT_PAGE_SIZE = 100
const EMPTY_TASKS: Task[] = []

type RunActionOptions = {
  label?: string
  fallbackTaskId?: string | null
}

function App() {
  const queryClient = useQueryClient()
  const [config, setConfig] = useState<RuntimeConfig | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [view, setView] = useState<OperatorView>("board")
  const [themeMode, setThemeMode] = useState<ThemeMode>(() =>
    typeof window === "undefined" ? "system" : parseThemeMode(window.localStorage.getItem(THEME_STORAGE_KEY)),
  )
  const [sidebarOpen, setSidebarOpen] = useState(() =>
    typeof window === "undefined" ? true : parseSidebarOpen(window.localStorage.getItem(SIDEBAR_OPEN_STORAGE_KEY)),
  )
  const [search, setSearch] = useState("")
  const debouncedSearch = useDebouncedValue(search, 250)
  const [statusFilter, setStatusFilter] = useState<TaskStatus | "all">("all")
  const [priorityFilters, setPriorityFilters] = useState<number[]>([])
  const [listSort, setListSort] = useState<ListSortState>(defaultListSort)
  const [showArchived, setShowArchived] = useState(false)
  const [pageOffset, setPageOffset] = useState(0)
  const [rowsPerPage, setRowsPerPage] = useState(DEFAULT_PAGE_SIZE)
  const [newTitle, setNewTitle] = useState("")
  const [newDescription, setNewDescription] = useState("")
  const [blockReason, setBlockReason] = useState("")
  const [dependencyInput, setDependencyInput] = useState("")
  const [commentBody, setCommentBody] = useState("")
  const [draftState, setDraftState] = useState<TaskDraftState | null>(null)
  const [claimTokens, setClaimTokens] = useState<Record<string, string>>({})
  const [lastRefreshAt, setLastRefreshAt] = useState<number | null>(null)
  const [pendingAction, setPendingAction] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadRuntimeConfig()
      .then(setConfig)
      .catch((err: unknown) => setError(errorMessage(err)))
  }, [])

  useEffect(() => {
    if (typeof window === "undefined") return
    const media = window.matchMedia("(prefers-color-scheme: dark)")
    const apply = () => applyRootTheme(document.documentElement.classList, effectiveTheme(themeMode, media.matches))
    apply()
    window.localStorage.setItem(THEME_STORAGE_KEY, themeMode)
    media.addEventListener("change", apply)
    return () => media.removeEventListener("change", apply)
  }, [themeMode])

  useEffect(() => {
    if (typeof window === "undefined") return
    window.localStorage.setItem(SIDEBAR_OPEN_STORAGE_KEY, serializeSidebarOpen(sidebarOpen))
  }, [sidebarOpen])

  const api = useMemo(() => (config ? new KanbanApi(config) : null), [config])

  useEffect(() => {
    setPageOffset(0)
  }, [debouncedSearch, showArchived, statusFilter, priorityFilters, listSort])

  const columnsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.columns(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.listBoardColumns({ signal })
    },
  })

  const boardsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.boards(),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.listBoards({ signal })
    },
  })

  const tasksQuery = useBoardTasks({
    api,
    search: debouncedSearch,
    statusFilter,
    priorityFilters: view === "list" ? priorityFilters : [],
    sort: view === "list" ? listSortToApiSort(listSort) : "-updated_at",
    mode: view === "list" ? "list" : "board",
    showArchived,
    limit: rowsPerPage,
    offset: pageOffset,
  })

  const tasks = tasksQuery.data?.tasks ?? EMPTY_TASKS
  const page = tasksQuery.data?.page ?? { limit: rowsPerPage, offset: pageOffset, total: null }
  const searchMeta = tasksQuery.data?.searchMeta ?? null

  useEffect(() => {
    if (tasksQuery.dataUpdatedAt) setLastRefreshAt(tasksQuery.dataUpdatedAt)
  }, [tasksQuery.dataUpdatedAt])

  useEffect(() => {
    setClaimTokens((current) => reconcileClaimTokensForTasks(current, tasks, config?.actor ?? null))
  }, [config?.actor, tasks])

  useEffect(() => {
    setSelectedId((current) => reconcileSelectedTaskId(current, tasks))
  }, [tasks])

  const detailQuery = useTaskDetail(api, selectedId)
  const boardSelectedTask = useMemo(
    () => (selectedId ? tasks.find((task) => task.id === selectedId) ?? null : null),
    [selectedId, tasks],
  )
  const selectedTask = selectedId ? detailQuery.data?.task ?? boardSelectedTask : null
  const detail = taskDetailOrEmpty(detailQuery.data)
  const dependencySnapshot = useMemo(
    () => ({
      selectedTaskId: selectedId,
      detailTaskId: detailQuery.data?.task.id ?? null,
      dependencies: detailQuery.data?.detail.dependencies ?? null,
      loading: Boolean(selectedId && detailQuery.isFetching),
    }),
    [detailQuery.data?.detail.dependencies, detailQuery.data?.task.id, detailQuery.isFetching, selectedId],
  )

  useEffect(() => {
    setDraftState((current) => reconcileTaskDraft(current, selectedTask))
  }, [selectedTask])

  useEffect(() => {
    if (!selectedTask) return
    setClaimTokens((current) => reconcileClaimTokenForTask(current, selectedTask, config?.actor ?? null))
  }, [config?.actor, selectedTask])

  useEffect(() => {
    setBlockReason("")
  }, [selectedTask?.id, selectedTask?.status])

  useEffect(() => {
    if (columnsQuery.error) setError(errorMessage(columnsQuery.error))
  }, [columnsQuery.error])

  useEffect(() => {
    if (boardsQuery.error) setError(errorMessage(boardsQuery.error))
  }, [boardsQuery.error])

  useEffect(() => {
    if (tasksQuery.error) setError(errorMessage(tasksQuery.error))
  }, [tasksQuery.error])

  useEffect(() => {
    if (detailQuery.error) setError(errorMessage(detailQuery.error))
  }, [detailQuery.error])

  const handlePollError = useCallback((err: unknown) => setError(errorMessage(err)), [])
  useEventPoller({
    api,
    enabled: Boolean(api),
    selectedTaskId: selectedId,
    onError: handlePollError,
  })

  const visibleColumns = useMemo(
    () => (columnsQuery.data ?? fallbackColumns).filter((column) => showArchived || (!column.hidden && column.status !== "archived")),
    [columnsQuery.data, showArchived],
  )

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

  const tasksById = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks])

  const actionMutation = useMutation({
    mutationFn: (action: () => Promise<unknown>) => action(),
  })

  const activeRun = detail.runs.find((run) => run.status === "running") ?? detail.runs[0]
  const claimToken = selectedTask ? claimTokens[selectedTask.id] ?? null : null
  const queueCounts = {
    ready: tasks.filter((task) => task.status === "ready").length,
    running: tasks.filter((task) => task.status === "running").length,
    blocked: tasks.filter((task) => task.status === "blocked").length,
  }

  const invalidateTaskData = useCallback(
    async (taskId: string | null) => {
      if (!api) return
      await invalidateTaskDetailAndBoard(queryClient, api.board, taskId)
    },
    [api, queryClient],
  )

  async function runAction(action: () => Promise<unknown>, options: RunActionOptions | string = "action") {
    const label = typeof options === "string" ? options : options.label ?? "action"
    const fallbackTaskId = typeof options === "string" ? selectedId : options.fallbackTaskId
    setPendingAction(label)
    setError(null)
    try {
      const result = await actionMutation.mutateAsync(action)
      if (isClaimResponse(result)) {
        setClaimTokens((current) => ({ ...current, [result.task.id]: result.claim_token }))
        await invalidateTaskData(result.task.id)
        return result
      }
      if (isTask(result)) {
        setClaimTokens((current) => reconcileClaimTokenForTask(current, result, config?.actor ?? null))
        await invalidateTaskData(result.id)
        return result
      }
      await invalidateTaskData(fallbackTaskId ?? null)
      return result
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setPendingAction(null)
    }
  }

  async function createTask(event: FormEvent) {
    event.preventDefault()
    if (!api || !newTitle.trim()) return
    await runAction(async () => {
      const task = await api.createTask({
        title: newTitle.trim(),
        description: newDescription.trim() || undefined,
      })
      setSelectedId(task.id)
      setNewTitle("")
      setNewDescription("")
      return task
    }, "create")
  }

  async function addDependency() {
    if (!api || !selectedTask || !dependencyInput.trim()) return
    const taskId = selectedTask.id
    await runAction(async () => {
      const result = await api.addDependency(taskId, dependencyInput.trim())
      setDependencyInput("")
      return result
    }, { label: "dependency", fallbackTaskId: taskId })
  }

  async function removeDependency(parentTaskId: string) {
    if (!api || !selectedTask) return
    const taskId = selectedTask.id
    await runAction(async () => api.removeDependency(taskId, parentTaskId), { label: "dependency", fallbackTaskId: taskId })
  }

  async function dropTask(taskId: string, targetStatus: TaskStatus) {
    if (!api) return
    const task = tasksById.get(taskId)
    if (!task) return
    const token = claimTokens[task.id] ?? null
    let plan = planDragTransition(task, targetStatus, token)
    if (!plan.ok) {
      setError(plan.reason)
      return
    }
    if (plan.promptReason) {
      const reason = window.prompt("Block reason")
      if (!reason?.trim()) {
        setError("A block reason is required.")
        return
      }
      plan = { ...plan, body: { ...plan.body, reason: reason.trim() }, promptReason: false }
    }
    if (plan.confirm && !window.confirm(plan.confirm)) return
    await runAction(() => executeDragTransition(api, task, plan), { label: "transition", fallbackTaskId: task.id })
  }

  async function saveTask() {
    if (!api || !selectedTask || !draftState) return false
    if (draftState.taskId !== selectedTask.id) return false
    if (!draftState.draft.title.trim()) return false
    const taskId = selectedTask.id
    const draft = draftState.draft
    const result = await runAction(async () => {
      const updated = await api.updateTask(taskId, {
        title: draft.title.trim(),
        description: draft.description.trim() || null,
        assignee: draft.assignee.trim() || null,
        priority: Number(draft.priority),
        due_at: parseDateInput(draft.dueAt),
        scheduled_at: parseDateInput(draft.scheduledAt),
      })
      setDraftState((current) => reconcileSavedTaskDraft(current, updated))
      return updated
    }, { label: "save", fallbackTaskId: taskId })
    return isTask(result)
  }

  function cancelTaskEdit() {
    setDraftState((current) => reconcileTaskDraft(current, selectedTask, { force: true }))
  }

  async function addComment() {
    if (!api || !selectedTask || !commentBody.trim()) return
    const taskId = selectedTask.id
    await runAction(async () => {
      const result = await api.createComment(taskId, commentBody.trim())
      setCommentBody("")
      return result
    }, { label: "comment", fallbackTaskId: taskId })
  }

  function updateDraft(draft: TaskEditDraft) {
    setDraftState((current) => {
      if (current) return { ...current, draft, dirty: true }
      if (!selectedTask) return null
      return { taskId: selectedTask.id, draft, dirty: true }
    })
  }

  async function switchBoard(board: string) {
    if (!config || board === config.board) return
    setPendingAction("board")
    setError(null)
    try {
      const nextConfig = await switchRuntimeBoard(config, board)
      const reset = createBoardSwitchReset({
        config: nextConfig,
        selectedId,
        pageOffset,
        newTitle,
        newDescription,
        blockReason,
        dependencyInput,
        commentBody,
        draftState,
        claimTokens,
        lastRefreshAt,
        error,
      })
      setConfig(reset.config)
      setSelectedId(reset.selectedId)
      setPageOffset(reset.pageOffset)
      setNewTitle(reset.newTitle)
      setNewDescription(reset.newDescription)
      setBlockReason(reset.blockReason)
      setDependencyInput(reset.dependencyInput)
      setCommentBody(reset.commentBody)
      setDraftState(reset.draftState)
      setClaimTokens(reset.claimTokens)
      setLastRefreshAt(reset.lastRefreshAt)
      setError(reset.error)
      await queryClient.invalidateQueries()
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setPendingAction(null)
    }
  }

  const hasNext = hasNextPage(page, tasks.length)
  const hasPrevious = hasPreviousPage(page)
  const lastOffset = lastPageOffset(page)

  return (
    <AppShell
      config={config}
      api={api}
      boards={boardsQuery.data ?? []}
      boardsLoading={boardsQuery.isLoading}
      boardsError={boardsQuery.error ? errorMessage(boardsQuery.error) : null}
      view={view}
      themeMode={themeMode}
      sidebarOpen={sidebarOpen}
      columns={visibleColumns as BoardColumn[]}
      tasks={tasks}
      groupedTasks={groupedTasks}
      selectedTask={selectedTask}
      selectedId={selectedId}
      dependencySnapshot={dependencySnapshot}
      detail={detail}
      activeRun={activeRun}
      search={search}
      debouncedSearch={debouncedSearch}
      searchMeta={searchMeta}
      statusFilter={statusFilter}
      priorityFilters={priorityFilters}
      listSort={listSort}
      showArchived={showArchived}
      page={page}
      visibleTaskCount={tasks.length}
      hasNextPage={hasNext}
      hasPreviousPage={hasPrevious}
      canGoLastPage={lastOffset !== null && lastOffset !== page.offset}
      rowsPerPage={rowsPerPage}
      newTitle={newTitle}
      newDescription={newDescription}
      blockReason={blockReason}
      dependencyInput={dependencyInput}
      commentBody={commentBody}
      editDraft={draftState?.draft ?? null}
      draftDirty={draftState?.dirty ?? false}
      claimToken={claimToken}
      tasksLoading={tasksQuery.isLoading}
      tasksRefreshing={tasksQuery.isFetching}
      detailLoading={detailQuery.isFetching}
      pendingAction={pendingAction}
      error={error}
      lastRefreshAt={lastRefreshAt}
      queueCounts={queueCounts}
      onSearchChange={setSearch}
      onBoardChange={(board) => void switchBoard(board)}
      onViewChange={setView}
      onThemeModeChange={setThemeMode}
      onCycleThemeMode={() => setThemeMode((current) => nextThemeMode(current))}
      onSidebarOpenChange={setSidebarOpen}
      onStatusFilterChange={setStatusFilter}
      onPriorityFiltersChange={setPriorityFilters}
      onListSortChange={setListSort}
      onResetListFilters={() => {
        setSearch("")
        setStatusFilter("all")
        setPriorityFilters([])
        setPageOffset(0)
      }}
      onShowArchivedChange={setShowArchived}
      onRefreshTasks={() => void tasksQuery.refetch()}
      onFirstPage={() => setPageOffset(0)}
      onPreviousPage={() => setPageOffset((current) => Math.max(0, current - rowsPerPage))}
      onNextPage={() => setPageOffset((current) => current + rowsPerPage)}
      onLastPage={() => setPageOffset(lastOffset ?? pageOffset)}
      onRowsPerPageChange={(value) => {
        setRowsPerPage(value)
        setPageOffset(0)
      }}
      onCreateTask={(event) => void createTask(event)}
      onNewTitleChange={setNewTitle}
      onNewDescriptionChange={setNewDescription}
      onSelectTask={setSelectedId}
      onCloseTaskDetail={() => setSelectedId(null)}
      onDropTask={(taskId, status) => void dropTask(taskId, status)}
      onBlockReasonChange={setBlockReason}
      onDependencyInputChange={setDependencyInput}
      onCommentBodyChange={setCommentBody}
      onEditDraftChange={updateDraft}
      onAction={runAction}
      onAddDependency={addDependency}
      onRemoveDependency={removeDependency}
      onSaveTask={saveTask}
      onCancelTaskEdit={cancelTaskEdit}
      onAddComment={addComment}
    />
  )
}

function isClaimResponse(value: unknown): value is ClaimResponse {
  return Boolean(value && typeof value === "object" && "claim_token" in value)
}

function isTask(value: unknown): value is Task {
  return Boolean(value && typeof value === "object" && "id" in value && "status" in value)
}

function errorMessage(err: unknown) {
  if (err instanceof ApiError) return `${err.code}: ${err.message}`
  if (err instanceof Error) return err.message
  return String(err)
}

export default App
