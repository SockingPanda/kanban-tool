import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react"

import { AppShell } from "@/app/AppShell"
import { fallbackColumns } from "@/features/board/board-config"
import { emptyDetail, type DetailState } from "@/features/task-detail/detail-state"
import { parseDateInput, taskToDraft, type TaskEditDraft } from "@/features/task-detail/task-draft"
import {
  ApiError,
  BoardColumn,
  ClaimResponse,
  KanbanApi,
  RuntimeConfig,
  SearchMeta,
  Task,
  TaskStatus,
  loadRuntimeConfig,
} from "@/lib/api"
import { createLatestRequestGuard, runLatestRequest } from "@/lib/latest-request"

function App() {
  const [config, setConfig] = useState<RuntimeConfig | null>(null)
  const [api, setApi] = useState<KanbanApi | null>(null)
  const [columns, setColumns] = useState<BoardColumn[]>(fallbackColumns)
  const [tasks, setTasks] = useState<Task[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [detail, setDetail] = useState<DetailState>(emptyDetail)
  const [search, setSearch] = useState("")
  const [searchMeta, setSearchMeta] = useState<SearchMeta | null>(null)
  const [statusFilter, setStatusFilter] = useState<TaskStatus | "all">("all")
  const [showArchived, setShowArchived] = useState(false)
  const [newTitle, setNewTitle] = useState("")
  const [newDescription, setNewDescription] = useState("")
  const [blockReason, setBlockReason] = useState("")
  const [dependencyInput, setDependencyInput] = useState("")
  const [commentBody, setCommentBody] = useState("")
  const [editDraft, setEditDraft] = useState<TaskEditDraft | null>(null)
  const [claimTokens, setClaimTokens] = useState<Record<string, string>>({})
  const [lastEventId, setLastEventId] = useState(0)
  const [lastRefreshAt, setLastRefreshAt] = useState<number | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const taskRefreshGuard = useRef(createLatestRequestGuard())

  useEffect(() => {
    loadRuntimeConfig()
      .then((runtime) => {
        const client = new KanbanApi(runtime)
        setConfig(runtime)
        setApi(client)
      })
      .catch((err: unknown) => setError(errorMessage(err)))
  }, [])

  const visibleColumns = useMemo(
    () => columns.filter((column) => showArchived || (!column.hidden && column.status !== "archived")),
    [columns, showArchived],
  )

  const refreshTasks = useCallback(async () => {
    if (!api) return
    const query = search.trim()
    const statuses = statusFilter === "all" ? [] : [statusFilter]
    return runLatestRequest(
      taskRefreshGuard.current,
      async () => {
        if (query) {
          const result = await api.searchTasks({
            query,
            includeArchived: showArchived,
            statuses,
          })
          return { tasks: result.tasks, searchMeta: result.meta }
        }

        const tasks = await api.listTasks({
          includeArchived: showArchived,
          statuses,
        })
        return { tasks, searchMeta: null }
      },
      ({ tasks: nextTasks, searchMeta: nextSearchMeta }) => {
        setSearchMeta(nextSearchMeta)
        setTasks(nextTasks)
        setSelectedId((current) =>
          current && nextTasks.some((task) => task.id === current) ? current : nextTasks[0]?.id ?? null,
        )
        setLastRefreshAt(Date.now())
      },
    )
  }, [api, search, showArchived, statusFilter])

  const refreshDetail = useCallback(
    async (taskId: string) => {
      if (!api) return
      const [dependencies, runs, events, comments] = await Promise.all([
        api.listDependencies(taskId),
        api.listRuns(taskId),
        api.listEvents(taskId),
        api.listComments(taskId),
      ])
      const runWithLog = runs.find((run) => Boolean(run.log_path)) ?? null
      const runLog = runWithLog
        ? await api.getRunLog(runWithLog.id).catch(() => null)
        : null
      setDetail({ dependencies, runs, events, comments, runLog })
      setLastEventId((current) => Math.max(current, ...events.map((event) => event.id), current))
    },
    [api],
  )

  const refreshColumns = useCallback(async () => {
    if (!api) return
    const nextColumns = await api.listBoardColumns()
    setColumns(nextColumns)
  }, [api])

  useEffect(() => {
    if (!api) return
    refreshColumns().catch((err: unknown) => setError(errorMessage(err)))
  }, [api, refreshColumns])

  useEffect(() => {
    if (!api) return
    setBusy(true)
    refreshTasks()
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setBusy(false))
  }, [api, refreshTasks])

  useEffect(() => {
    if (!selectedId) {
      setDetail(emptyDetail)
      return
    }
    refreshDetail(selectedId).catch((err: unknown) => setError(errorMessage(err)))
  }, [refreshDetail, selectedId])

  const selectedTask = useMemo(
    () => tasks.find((task) => task.id === selectedId) ?? tasks[0] ?? null,
    [selectedId, tasks],
  )

  useEffect(() => {
    if (!selectedTask) {
      setEditDraft(null)
      return
    }
    setEditDraft(taskToDraft(selectedTask))
  }, [selectedTask])

  useEffect(() => {
    if (!api) return
    const interval = window.setInterval(() => {
      api
        .listEventsAfter(lastEventId)
        .then((events) => {
          if (!events.length) return
          setLastEventId((current) => Math.max(current, ...events.map((event) => event.id)))
          void refreshTasks()
          if (selectedId) void refreshDetail(selectedId)
        })
        .catch((err: unknown) => setError(errorMessage(err)))
    }, 5_000)
    return () => window.clearInterval(interval)
  }, [api, lastEventId, refreshDetail, refreshTasks, selectedId])

  const groupedTasks = useMemo(() => {
    const map = new Map<TaskStatus, Task[]>()
    for (const column of visibleColumns) map.set(column.status, [])
    for (const task of tasks) {
      if (map.has(task.status)) map.get(task.status)!.push(task)
    }
    return map
  }, [tasks, visibleColumns])

  const activeRun = detail.runs.find((run) => run.status === "running") ?? detail.runs[0]
  const claimToken = selectedTask ? claimTokens[selectedTask.id] ?? null : null
  const queueCounts = {
    ready: tasks.filter((task) => task.status === "ready").length,
    running: tasks.filter((task) => task.status === "running").length,
    blocked: tasks.filter((task) => task.status === "blocked").length,
  }

  async function runAction(action: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    try {
      const result = await action()
      if (isClaimResponse(result)) {
        setClaimTokens((current) => ({ ...current, [result.task.id]: result.claim_token }))
      }
      await refreshTasks()
      const taskId = selectedId ?? selectedTask?.id
      if (taskId) await refreshDetail(taskId)
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setBusy(false)
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
    })
  }

  async function addDependency() {
    if (!api || !selectedTask || !dependencyInput.trim()) return
    await runAction(async () => {
      await api.addDependency(selectedTask.id, dependencyInput.trim())
      setDependencyInput("")
    })
  }

  async function saveTask() {
    if (!api || !selectedTask || !editDraft) return
    await runAction(async () => {
      const updated = await api.updateTask(selectedTask.id, {
        title: editDraft.title.trim(),
        description: editDraft.description.trim() || null,
        assignee: editDraft.assignee.trim() || null,
        priority: Number(editDraft.priority) || 0,
        due_at: parseDateInput(editDraft.dueAt),
        scheduled_at: parseDateInput(editDraft.scheduledAt),
      })
      setEditDraft(taskToDraft(updated))
    })
  }

  async function addComment() {
    if (!api || !selectedTask || !commentBody.trim()) return
    await runAction(async () => {
      await api.createComment(selectedTask.id, commentBody.trim())
      setCommentBody("")
    })
  }

  return (
    <AppShell
      config={config}
      api={api}
      columns={visibleColumns}
      groupedTasks={groupedTasks}
      selectedTask={selectedTask}
      selectedId={selectedId}
      detail={detail}
      activeRun={activeRun}
      search={search}
      searchMeta={searchMeta}
      statusFilter={statusFilter}
      showArchived={showArchived}
      newTitle={newTitle}
      newDescription={newDescription}
      blockReason={blockReason}
      dependencyInput={dependencyInput}
      commentBody={commentBody}
      editDraft={editDraft}
      claimToken={claimToken}
      busy={busy}
      error={error}
      lastRefreshAt={lastRefreshAt}
      queueCounts={queueCounts}
      onSearchChange={setSearch}
      onStatusFilterChange={setStatusFilter}
      onShowArchivedChange={setShowArchived}
      onRefreshTasks={() => void refreshTasks()}
      onCreateTask={(event) => void createTask(event)}
      onNewTitleChange={setNewTitle}
      onNewDescriptionChange={setNewDescription}
      onSelectTask={setSelectedId}
      onBlockReasonChange={setBlockReason}
      onDependencyInputChange={setDependencyInput}
      onCommentBodyChange={setCommentBody}
      onEditDraftChange={setEditDraft}
      onAction={runAction}
      onAddDependency={addDependency}
      onSaveTask={saveTask}
      onAddComment={addComment}
    />
  )
}

function isClaimResponse(value: unknown): value is ClaimResponse {
  return Boolean(value && typeof value === "object" && "claim_token" in value)
}

function errorMessage(err: unknown) {
  if (err instanceof ApiError) return `${err.code}: ${err.message}`
  if (err instanceof Error) return err.message
  return String(err)
}

export default App
