import { Component, lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from "react"

import { BoardView } from "../board/BoardView"
import { MutationDialog, MutationNotice } from "../board/BoardTaskMutations"
import { boardMessagesForLocale, type BoardViewModel } from "../board/types"
import { useBoardTaskMutationController } from "../board/task-mutation-controller"
import { toBoardViewModel } from "../board/board-adapter"
import type { BoardSyncStatus } from "../board/types"
import type { BoardTaskMutationSurface } from "../board/task-mutation-state"
import {
  ExplorerReadError,
  loadTaskInspectorAttachments,
  loadTaskInspector,
  loadTaskInspectorEvents,
  loadTaskInspectorNeighborhood,
  loadTaskInspectorRuns,
  loadExplorerBoardIdentity,
  loadTaskListPage,
  parseTaskListQuery,
  serializeTaskListQuery,
  type TaskInspectorReadModel,
  type TaskListQueryState,
} from "../../lib/api/explorer-read-model"
import { createAttachmentDownloadClient } from "../../lib/api/attachment-download"
import type { CanonicalBoardSlug } from "../../lib/board-slug"
import type { Locale } from "../../lib/preferences"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { routePath, type AppNavigationTarget, type AppRoute, type BoardRouteView } from "../../lib/router"
import { usePreferences } from "../../lib/use-preferences"
import { restoreExplorerFocus, type ExplorerFocusElement, type ExplorerFocusSnapshot } from "../../lib/explorer-focus"
import { TaskInspector, type InspectorDependency, type TaskInspectorViewModel } from "./TaskInspector"
import { TaskInspectorRelationsPanel, type TaskInspectorCommentView, type TaskInspectorDependenciesView, type TaskInspectorRelationTaskView, type TaskInspectorStepView } from "./TaskInspectorRelationsPanel"
import { TaskInspectorAssetsPanel, type InspectorAssetAttachment, type InspectorAssetLabel } from "./TaskInspectorAssetsPanel"
import { inspectorMutationKey, type TaskInspectorMutationHandlers, type TaskInspectorMutationSnapshot, type TaskInspectorMutationSurface } from "./task-inspector-mutation-state"
import { useTaskInspectorMutationController } from "./task-inspector-mutation-controller"
import { TaskListView, type TaskListRow } from "./TaskListView"
import { TaskRunsView } from "./TaskRunsView"
import {
  asyncReadToken,
  shouldClearMapTaskFromInspector,
  type AsyncReadInternalState,
  type AsyncReadState,
  visibleAsyncReadState,
} from "./ExplorerPage.logic"
import { parseTaskMapUrlState, serializeTaskMapUrlState, type TaskMapUrlState } from "./TaskMapView.logic"
import { EventsView } from "./EventsView"
import type { BoardEventsBatch } from "../../lib/api/explorer-read-model"
import styles from "./ExplorerPage.module.css"

const LazyTaskMapView = lazy(() => import("./TaskMapView").then((module) => ({ default: module.TaskMapView })))

class TaskMapChunkBoundary extends Component<{ readonly locale: "zh" | "en"; readonly children: ReactNode }, { readonly failed: boolean }> {
  state = { failed: false }

  static getDerivedStateFromError(): { readonly failed: boolean } {
    return { failed: true }
  }

  render() {
    if (!this.state.failed) return this.props.children
    const copy = this.props.locale === "en"
      ? { title: "Task map unavailable", description: "The map page chunk could not be loaded.", retry: "Retry" }
      : { title: "关系图暂不可用", description: "关系图页面资源加载失败。", retry: "重试" }
    return (
      <section className={styles.boundary} data-testid="task-map-chunk-error" role="alert">
        <h2>{copy.title}</h2>
        <p>{copy.description}</p>
        <button
          type="button"
          onClick={() => {
            if (typeof window !== "undefined") window.location.reload()
            else this.setState({ failed: false })
          }}
        >{copy.retry}</button>
      </section>
    )
  }
}

export interface ExplorerPageProps {
  readonly runtime: WebRuntimeConfig
  readonly route: Extract<AppRoute, { kind: "board" }>
  readonly onNavigate?: (target: AppNavigationTarget, options?: { readonly replace?: boolean }) => void | Promise<unknown>
  /** 由 ProductShell 持有的响应式浏览器 connectivity 状态。 */
  readonly online?: boolean
  /** 现有 persistent SSE integration 持有的 revision/batch seam。 */
  readonly invalidationRevision?: number
  readonly boardRevision?: number
  readonly inspectorRevision?: number
  readonly runsRevision?: number
  /** 仅 recovery/gap/poll boundaries 触发 Events catch-up read。 */
  readonly eventsRefreshRevision?: number
  readonly eventsBatch?: BoardEventsBatch | null
  readonly syncStatus?: BoardSyncStatus
  readonly taskMutations?: BoardTaskMutationSurface
}

const MAX_EVENT_KIND_FILTER_LENGTH = 128

type ExplorerCopy = {
  readonly eyebrow: string
  readonly tabsLabel: string
  readonly closeInspector: string
  readonly boardLoading: string
  readonly boardError: string
  readonly boardOffline: string
  readonly retry: string
  readonly inspectorLoading: string
  readonly inspectorError: string
  readonly inspectorOffline: string
  readonly board: string
  readonly list: string
  readonly map: string
  readonly runs: string
  readonly events: string
  readonly syncConnecting: string
  readonly syncRecovering: string
  readonly syncStale: string
  readonly syncCircuitOpen: string
  readonly syncOffline: string
}

const explorerCopies: Record<Locale, ExplorerCopy> = {
  zh: {
    eyebrow: "ASTRYX EXPLORER",
    tabsLabel: "看板浏览视图",
    closeInspector: "关闭任务检查器",
    boardLoading: "正在加载看板…",
    boardError: "看板加载失败",
    boardOffline: "当前离线，无法加载看板。",
    retry: "重试",
    inspectorLoading: "正在加载任务检查器…",
    inspectorError: "任务检查器加载失败",
    inspectorOffline: "当前离线，无法加载任务检查器。",
    board: "看板",
    list: "列表",
    map: "关系图",
    runs: "运行记录",
    events: "事件",
    syncConnecting: "正在连接实时同步…",
    syncRecovering: "正在恢复同步",
    syncStale: "同步暂时中断",
    syncCircuitOpen: "同步暂时不可用",
    syncOffline: "当前离线",
  },
  en: {
    eyebrow: "ASTRYX EXPLORER",
    tabsLabel: "Board explorer views",
    closeInspector: "Close Inspector",
    boardLoading: "Loading board…",
    boardError: "Board failed to load",
    boardOffline: "You are offline; the board cannot be loaded.",
    retry: "Retry",
    inspectorLoading: "Loading Task Inspector…",
    inspectorError: "Task Inspector failed to load",
    inspectorOffline: "You are offline; Task Inspector cannot be loaded.",
    board: "Board",
    list: "List",
    map: "Map",
    runs: "Runs",
    events: "Events",
    syncConnecting: "Connecting to live sync…",
    syncRecovering: "Recovering sync",
    syncStale: "Sync is temporarily interrupted",
    syncCircuitOpen: "Sync is temporarily unavailable",
    syncOffline: "You are offline",
  },
}

function normalizeEventKindFilter(value: string | null | undefined): string {
  return (value ?? "").trim().slice(0, MAX_EVENT_KIND_FILTER_LENGTH)
}

function syncStatusLabel(status: BoardSyncStatus, copy: ExplorerCopy): string {
  if (status === "connecting") return copy.syncConnecting
  if (status === "recovering") return copy.syncRecovering
  if (status === "circuit-open") return copy.syncCircuitOpen
  if (status === "offline") return copy.syncOffline
  return copy.syncStale
}

function useAsyncRead<T>(
  enabled: boolean,
  key: string,
  load: (signal: AbortSignal) => Promise<T>,
  refreshRevision = 0,
  online = true,
): AsyncReadState<T> & { readonly retry: () => void } {
  const loadRef = useRef(load)
  loadRef.current = load
  const [generation, setGeneration] = useState(0)
  const { identityKey, requestKey: baseRequestKey } = asyncReadToken(enabled, key, generation)
  // A session event/poll boundary is a new request for the same visible
  // identity. Keep the last usable data while the coalesced refresh is in flight.
  const requestKey = `${baseRequestKey}\u0000${refreshRevision}`
  const [state, setState] = useState<AsyncReadInternalState<T>>(() => ({
    data: null,
    error: null,
    loading: false,
    identityKey,
    requestKey,
  }))

  useEffect(() => {
    if (!enabled) {
      setState({ data: null, error: null, loading: false, identityKey, requestKey })
      return
    }
    if (!online) {
      setState((current) => ({
        data: current.identityKey === identityKey ? current.data : null,
        error: new ExplorerReadError("offline", "当前离线，无法加载 Explorer 数据。"),
        loading: false,
        identityKey,
        requestKey,
      }))
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({
      data: current.identityKey === identityKey ? current.data : null,
      error: null,
      loading: true,
      identityKey,
      requestKey,
    }))
    void loadRef.current(controller.signal).then(
      (data) => {
        if (active) setState({ data, error: null, loading: false, identityKey, requestKey })
      },
      (error: unknown) => {
        if (active && !(error instanceof Error && error.name === "AbortError")) {
          setState((current) => ({
            data: current.identityKey === identityKey ? current.data : null,
            error: error instanceof Error ? error : new Error(String(error)),
            loading: false,
            identityKey,
            requestKey,
          }))
        }
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [enabled, generation, identityKey, key, online, requestKey])

  return {
    ...visibleAsyncReadState(state, { identityKey, requestKey }, enabled),
    retry: () => setGeneration((current) => current + 1),
  }
}

function queryParams(route: Extract<AppRoute, { kind: "board" }>): URLSearchParams {
  return new URLSearchParams(route.query ?? "")
}

function routeTarget(
  boardSlug: CanonicalBoardSlug,
  view: BoardRouteView,
  params: URLSearchParams,
  basePath?: string,
): string {
  const query = params.toString()
  return routePath({ kind: "board", boardSlug, view, ...(query ? { query } : {}) }, { basePath })
}

function boardPriority(value: number): 0 | 1 | 2 | 3 {
  if (value === 0 || value === 1 || value === 2 || value === 3) return value
  throw new Error(`服务端任务 ${String(value)} 的 priority 超出 board contract。`)
}

function taskListRow(task: NonNullable<Awaited<ReturnType<typeof loadTaskListPage>>>["tasks"][number]): TaskListRow {
  return {
    id: task.id,
    ref: task.ref,
    title: task.title,
    status: task.status,
    priority: task.priority,
    assignee: task.assignee,
    executionPlanState: task.execution_plan_state,
    dependencyBlocked: task.dependency_blocked,
    requiredStepCount: task.required_step_count,
    completedRequiredStepCount: task.completed_required_step_count,
    optionalStepCount: task.optional_step_count,
    updatedAt: task.updated_at,
  }
}

function dependencyView(task: NonNullable<TaskInspectorReadModel["dependencies"]["parents"]>[number]): InspectorDependency {
  return { id: task.id, ref: task.ref, title: task.title, status: task.status }
}

function inspectorRunView(run: TaskInspectorReadModel["runs"][number]): TaskInspectorViewModel["runs"][number] {
  return { id: run.id, status: run.status, workerProfile: run.worker_profile, claimOwner: run.claim_owner, startedAt: run.started_at, finishedAt: run.finished_at, exitCode: run.exit_code, error: run.error, hasLog: run.has_log }
}

function inspectorEventView(event: TaskInspectorReadModel["events"][number]): TaskInspectorViewModel["events"][number] {
  return { id: event.id, kind: event.kind, actor: event.actor, createdAt: event.created_at }
}

function inspectorViewModel(model: TaskInspectorReadModel): TaskInspectorViewModel {
  const task = model.task
  return {
    task: {
      id: task.id,
      ref: task.ref,
      title: task.title,
      status: task.status,
      lockVersion: task.lock_version,
      scheduledAt: task.scheduled_at,
      dueAt: task.due_at,
      priority: boardPriority(task.priority),
      description: task.description,
      statusReason: task.status_reason,
      assignee: task.assignee,
      executionPlanState: task.execution_plan_state,
      dependencyBlocked: task.dependency_blocked,
      unfinishedParentCount: task.unfinished_parent_count,
      requiredStepCount: task.required_step_count,
      completedRequiredStepCount: task.completed_required_step_count,
      optionalStepCount: task.optional_step_count,
      metadata: task.metadata,
      claimOwner: task.claim_owner,
      claimExpiresAt: task.claim_expires_at,
      lastHeartbeatAt: task.last_heartbeat_at,
      currentRunId: task.current_run_id,
      retryCount: task.retry_count,
      maxRetries: task.max_retries,
      createdAt: task.created_at,
      updatedAt: task.updated_at,
    },
    steps: model.steps.steps.map((step) => ({ id: step.id, title: step.title, status: step.status, required: step.required, body: step.body })),
    parents: model.dependencies.parents.map(dependencyView),
    children: model.dependencies.children.map(dependencyView),
    comments: model.comments.map((comment) => ({ id: comment.id, author: comment.author, kind: comment.kind, body: comment.body, createdAt: comment.created_at })),
    runs: model.runs.map(inspectorRunView),
    events: model.events.map(inspectorEventView),
    neighborhood: model.neighborhood ? {
      centerTaskId: model.neighborhood.center_task_id,
      nodes: model.neighborhood.nodes.map((node) => ({ id: node.task.id, ref: node.task.ref, title: node.task.title, role: node.role })),
      edges: model.neighborhood.edges.map((edge) => ({ id: edge.id, sourceTaskId: edge.source_task_id, targetTaskId: edge.target_task_id, kind: edge.kind })),
    } : undefined,
    runtime: model.runtime,
  }
}

function relationTaskView(task: TaskInspectorReadModel["dependencies"]["parents"][number]): TaskInspectorRelationTaskView {
  return { id: task.id, ref: task.ref, title: task.title, status: task.status }
}

function linkedTaskView(task: NonNullable<TaskInspectorReadModel["steps"]["steps"][number]["linked_task"]>): TaskInspectorRelationTaskView {
  return { id: task.id, ref: task.ref, title: task.title, status: task.status }
}

function inspectorRelationsView(model: TaskInspectorReadModel): {
  readonly comments: readonly TaskInspectorCommentView[]
  readonly dependencies: TaskInspectorDependenciesView
  readonly steps: { readonly steps: readonly TaskInspectorStepView[]; readonly executionPlan: { readonly state: "unplanned" | "planned" | "not_required"; readonly reason: string | null } }
} {
  return {
    comments: model.comments.map((comment) => ({
      id: comment.id,
      author: comment.author,
      kind: comment.kind,
      body: comment.body,
      createdAt: comment.created_at,
      metadata: comment.metadata,
    })),
    dependencies: {
      parents: model.dependencies.parents.map(relationTaskView),
      children: model.dependencies.children.map(relationTaskView),
    },
    steps: {
      steps: model.steps.steps.map((step) => ({
        id: step.id,
        title: step.title,
        body: step.body,
        required: step.required,
        status: step.status,
        linkedTask: step.linked_task ? linkedTaskView(step.linked_task) : null,
      })),
      executionPlan: {
        state: model.steps.execution_plan.state,
        reason: model.steps.execution_plan.reason,
      },
    },
  }
}

const inspectorWriteOperations = [
  "saveTask",
  "transition",
  "addDependency",
  "removeDependency",
  "createStep",
  "linkStep",
  "markPlanNotRequired",
  "addLabel",
  "removeLabel",
  "applySuggestedLabel",
  "addComment",
  "uploadAttachment",
  "deleteAttachment",
] as const

function inspectorUiSnapshot(snapshot: TaskInspectorMutationSnapshot | null, taskId: string | null): TaskInspectorMutationSnapshot | undefined {
  if (snapshot === null || taskId === null) return undefined
  const reloadKey = inspectorMutationKey("reload", taskId)
  if (!snapshot.pending.has(reloadKey)) return snapshot
  const pending = new Set(snapshot.pending)
  for (const operation of inspectorWriteOperations) pending.add(inspectorMutationKey(operation, taskId))
  return { ...snapshot, pending }
}

function InspectorBoundary({ loading, error, onRetry, copy }: { readonly loading: boolean; readonly error: Error | null; readonly onRetry: () => void; readonly copy: ExplorerCopy }) {
  if (loading) return <aside className={styles.inspectorBoundary} data-testid="task-inspector-loading" role="status"><h2>{copy.inspectorLoading}</h2></aside>
  if (error) {
    const offline = error instanceof ExplorerReadError && error.kind === "offline"
    return <aside className={styles.inspectorBoundary} data-testid={offline ? "task-inspector-offline" : "task-inspector-error"} role={offline ? "status" : "alert"}><h2>{offline ? copy.inspectorOffline : copy.inspectorError}</h2><p>{error.message}</p><button type="button" onClick={onRetry}>{copy.retry}</button></aside>
  }
  return null
}

function ExplorerTabs({ route, basePath, taskId, onNavigate, copy }: { readonly route: Extract<AppRoute, { kind: "board" }>; readonly basePath: string; readonly taskId: string | null; readonly onNavigate?: ExplorerPageProps["onNavigate"]; readonly copy: ExplorerCopy }) {
  const params = queryParams(route)
  const views: readonly [BoardRouteView, string][] = [["board", copy.board], ["list", copy.list], ["map", copy.map], ["runs", copy.runs], ["events", copy.events]]
  return (
    <nav className={styles.tabs} aria-label={copy.tabsLabel}>
      {views.map(([view, label]) => {
        const next = new URLSearchParams(params)
        if (taskId) next.set("task", taskId)
        const href = routeTarget(route.boardSlug, view, next, basePath)
        return <a key={view} href={href} aria-current={(route.view ?? "board") === view ? "page" : undefined} onClick={(event) => { if (!onNavigate) return; event.preventDefault(); void onNavigate(href) }}>{label}</a>
      })}
    </nav>
  )
}

export function ExplorerPage({ runtime, route, onNavigate, online, invalidationRevision = 0, boardRevision = invalidationRevision, inspectorRevision = invalidationRevision, runsRevision = invalidationRevision, eventsRefreshRevision = invalidationRevision, eventsBatch, syncStatus, taskMutations }: ExplorerPageProps) {
  const { locale } = usePreferences()
  const copy = explorerCopies[locale]
  const view = route.view ?? "board"
  const params = queryParams(route)
  const rawTaskId = params.get("task")?.trim() || null
  const mapUrlState = useMemo(() => parseTaskMapUrlState(route.query ?? ""), [route.query])
  const taskId = view === "map" ? mapUrlState.taskId : rawTaskId
  const openerRef = useRef<ExplorerFocusSnapshot | null>(null)
  const previousTaskIdRef = useRef<string | null>(taskId)
  const kindFilter = normalizeEventKindFilter(params.get("kind"))
  const showInspector = Boolean(taskId) && view !== "runs"
  const listQuery = useMemo(() => parseTaskListQuery(new URLSearchParams(route.query ?? "")), [route.query])
  const listKey = `${route.boardSlug}|${serializeTaskListQuery(listQuery)}`
  const boardRead = useAsyncRead(view === "board", route.boardSlug, (signal) => import("../../lib/api/board-read-model").then(({ loadBoardReadModel }) => loadBoardReadModel(runtime, route.boardSlug, { signal })), boardRevision, online !== false)
  const listRead = useAsyncRead(view === "list", listKey, (signal) => loadTaskListPage(runtime, route.boardSlug, listQuery, { signal }), boardRevision, online !== false)
  const listMutationModel = useMemo<BoardViewModel | null>(() => {
    const board = listRead.data?.board
    if (board === undefined) return null
    return {
      board: { id: board.id, slug: board.slug, name: board.name },
      columns: [],
      tasksByStatus: {},
    }
  }, [listRead.data])
  const listMutationController = useBoardTaskMutationController(listMutationModel, taskMutations, [], boardMessagesForLocale(locale))
  const mapIdentityRead = useAsyncRead(view === "map", route.boardSlug, (signal) => loadExplorerBoardIdentity(runtime, route.boardSlug, { signal }), boardRevision, online !== false)
  const inspectorKey = `${route.boardSlug}|${taskId ?? ""}`
  const inspectorIdentity = `${runtime.apiBaseUrl}\u0000${runtime.webBuildId}\u0000${inspectorKey}`
  const inspectorRead = useAsyncRead(Boolean(taskId) && view !== "runs", inspectorKey, (signal) => taskId ? loadTaskInspector(runtime, route.boardSlug, taskId, { signal, includeNeighborhood: false, includeRuns: false, includeEvents: false, includeAttachments: false }) : Promise.reject(new Error("Task Inspector 尚未选择任务")), inspectorRevision, online !== false)
  const attachmentsRead = useAsyncRead(showInspector, `${inspectorKey}\u0000attachments`, (signal) => taskId
    ? loadTaskInspectorAttachments(runtime, route.boardSlug, taskId, { signal })
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), inspectorRevision, online !== false)
  const inspectorModel = useMemo(() => inspectorRead.data ? inspectorViewModel(inspectorRead.data) : null, [inspectorRead.data])
  const inspectorRelations = useMemo(() => inspectorRead.data ? inspectorRelationsView(inspectorRead.data) : null, [inspectorRead.data])
  const attachmentDownload = useMemo(() => {
    try {
      return createAttachmentDownloadClient(runtime)
    } catch {
      return undefined
    }
  }, [runtime])
  const inspectorMutationSurface = useMemo<TaskInspectorMutationSurface | undefined>(() => {
    if (!taskId || !inspectorRead.data || taskMutations?.inspectorClient === undefined) return undefined
    const client = taskMutations.inspectorClient
    const scope = {
      identity: inspectorIdentity,
      boardId: inspectorRead.data.board.id,
      taskId,
    }
    return {
      scope,
      client,
      claimTokens: taskMutations.claimTokens,
      attachmentDownload,
      suggestTaskLabels: (requestTaskId, query, options) => client.suggestTaskLabels(requestTaskId, query, options),
      onMutationCommitted: (event) => {
        taskMutations.onMutationCommitted?.({
          kind: event.kind === "transition" ? "transition" : "edit",
          taskId: event.taskId,
        })
      },
      onCanonicalReload: async (event) => {
        await Promise.resolve(taskMutations.onCanonicalReload?.({
          reason: "retry",
          mutationKind: event.kind === "transition" ? "transition" : "edit",
        }))
        inspectorRead.retry()
        attachmentsRead.retry()
      },
    }
  }, [attachmentDownload, attachmentsRead, inspectorIdentity, inspectorRead, taskId, taskMutations])
  const inspectorMutationController = useTaskInspectorMutationController(inspectorMutationSurface)
  const inspectorMutationHandlers = useMemo<TaskInspectorMutationHandlers | undefined>(() => {
    if (inspectorMutationController === null) return undefined
    return {
      saveTask: inspectorMutationController.saveTask.bind(inspectorMutationController),
      transition: inspectorMutationController.transition.bind(inspectorMutationController),
      addDependency: inspectorMutationController.addDependency.bind(inspectorMutationController),
      removeDependency: inspectorMutationController.removeDependency.bind(inspectorMutationController),
      createStep: inspectorMutationController.createStep.bind(inspectorMutationController),
      linkStep: inspectorMutationController.linkStep.bind(inspectorMutationController),
      markPlanNotRequired: inspectorMutationController.markPlanNotRequired.bind(inspectorMutationController),
      addLabel: inspectorMutationController.addLabel.bind(inspectorMutationController),
      removeLabel: inspectorMutationController.removeLabel.bind(inspectorMutationController),
      applySuggestedLabel: inspectorMutationController.applySuggestedLabel.bind(inspectorMutationController),
      addComment: inspectorMutationController.addComment.bind(inspectorMutationController),
      uploadAttachment: inspectorMutationController.uploadAttachment.bind(inspectorMutationController),
      downloadAttachment: inspectorMutationController.downloadAttachment.bind(inspectorMutationController),
      deleteAttachment: inspectorMutationController.deleteAttachment.bind(inspectorMutationController),
      suggestLabels: inspectorMutationController.suggestLabels.bind(inspectorMutationController),
      retry: inspectorMutationController.retry.bind(inspectorMutationController),
    }
  }, [inspectorMutationController])
  const inspectorMutationSnapshot = useMemo(
    () => inspectorUiSnapshot(inspectorMutationController?.snapshot ?? null, taskId),
    [inspectorMutationController?.snapshot, taskId],
  )

  const navigate = useCallback((target: string, options?: { readonly replace?: boolean }) => {
    if (onNavigate) void onNavigate(target, options)
  }, [onNavigate])
  const updateMapUrlState = useCallback((next: TaskMapUrlState, options?: { readonly replace?: boolean }) => {
    const query = new URLSearchParams(serializeTaskMapUrlState(next))
    navigate(routeTarget(route.boardSlug, "map", query, runtime.webBasePath), options)
  }, [navigate, route.boardSlug, runtime.webBasePath])
  const updateListQuery = (next: TaskListQueryState) => {
    const nextParams = new URLSearchParams(serializeTaskListQuery(next))
    if (taskId) nextParams.set("task", taskId)
    navigate(routeTarget(route.boardSlug, "list", nextParams, runtime.webBasePath))
  }
  const selectTask = (nextTaskId: string) => {
    if (view === "map") {
      updateMapUrlState({ ...mapUrlState, taskId: nextTaskId })
      return
    }
    const nextParams = new URLSearchParams(params)
    nextParams.set("task", nextTaskId)
    navigate(routeTarget(route.boardSlug, view, nextParams, runtime.webBasePath))
  }
  const closeInspector = () => {
    if (view === "map") {
      updateMapUrlState({ ...mapUrlState, taskId: null })
      return
    }
    const nextParams = new URLSearchParams(params)
    nextParams.delete("task")
    navigate(routeTarget(route.boardSlug, view, nextParams, runtime.webBasePath))
  }

  const rememberTaskOpener = useCallback((event: MouseEvent<HTMLElement>) => {
    if (typeof Element === "undefined" || !(event.target instanceof Element)) return
    const opener = event.target.closest<HTMLElement>("[data-task-opener]")
    const encodedTaskId = opener?.getAttribute("data-task-opener")
    if (!opener || !encodedTaskId) return
    let openerTaskId: string
    try {
      openerTaskId = decodeURIComponent(encodedTaskId)
    } catch {
      return
    }
    openerRef.current = { taskId: openerTaskId, element: opener as ExplorerFocusElement }
  }, [])

  const restoreFocus = useCallback(() => {
    if (typeof document === "undefined") return
    const snapshot = openerRef.current
    openerRef.current = null
    restoreExplorerFocus(snapshot, {
      querySelector: (selector) => document.querySelector(selector) as ExplorerFocusElement | null,
    })
  }, [])

  useEffect(() => {
    const previousTaskId = previousTaskIdRef.current
    if (previousTaskId !== null && taskId === null) restoreFocus()
    previousTaskIdRef.current = taskId
  }, [restoreFocus, taskId])
  const updateEventKindFilter = (nextKind: string) => {
    const nextParams = new URLSearchParams(params)
    const normalizedKind = normalizeEventKindFilter(nextKind)
    if (normalizedKind) nextParams.set("kind", normalizedKind)
    else nextParams.delete("kind")
    navigate(routeTarget(route.boardSlug, "events", nextParams, runtime.webBasePath))
  }

  const loadInspectorRuns = useCallback((signal: AbortSignal) => taskId
    ? loadTaskInspectorRuns(runtime, route.boardSlug, taskId, { signal }).then((runs) => runs.map((run) => inspectorRunView(run)))
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), [route.boardSlug, runtime, taskId])
  const loadInspectorEvents = useCallback((signal: AbortSignal) => taskId
    ? loadTaskInspectorEvents(runtime, route.boardSlug, taskId, { signal }).then((events) => events.map((event) => inspectorEventView(event)))
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), [route.boardSlug, runtime, taskId])
  const loadInspectorNeighborhood = useCallback((signal: AbortSignal) => taskId
    ? loadTaskInspectorNeighborhood(runtime, route.boardSlug, taskId, { signal }).then((neighborhood) => ({
      centerTaskId: neighborhood.center_task_id,
      nodes: neighborhood.nodes.map((node) => ({ id: node.task.id, ref: node.task.ref, title: node.task.title, role: node.role })),
      edges: neighborhood.edges.map((edge) => ({ id: edge.id, sourceTaskId: edge.source_task_id, targetTaskId: edge.target_task_id, kind: edge.kind })),
    }))
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), [route.boardSlug, runtime, taskId])
  const resolveInspectorTaskSelector = useCallback((selector: string): string | null => {
    const value = selector.trim()
    const model = inspectorRead.data
    if (!model) return null
    const candidates = [
      model.task,
      ...model.dependencies.parents,
      ...model.dependencies.children,
      ...model.steps.steps.flatMap((step) => step.linked_task ? [step.linked_task] : []),
    ]
    return candidates.find((candidate) => candidate.id === value || candidate.ref === value)?.id ?? null
  }, [inspectorRead.data])

  const clearedTaskIdRef = useRef<string | null>(null)
  useEffect(() => {
    const error = inspectorRead.error
    if (!shouldClearMapTaskFromInspector(view, taskId, error)) {
      if (!taskId) clearedTaskIdRef.current = null
      return
    }
    if (clearedTaskIdRef.current === taskId) return
    clearedTaskIdRef.current = taskId
    updateMapUrlState({ ...mapUrlState, taskId: null }, { replace: true })
  }, [inspectorRead.error, mapUrlState, taskId, updateMapUrlState, view])

  return (
    <section className={styles.explorer} data-testid="explorer-page" onClickCapture={rememberTaskOpener}>
      <header className={styles.explorerHeader}>
        <div>
          <p className={styles.eyebrow}>{copy.eyebrow}</p>
          <h1 tabIndex={-1} data-explorer-focus-fallback>{route.boardSlug}</h1>
        </div>
        {taskId ? <button type="button" className={styles.closeInspector} onClick={closeInspector}>{copy.closeInspector}</button> : null}
      </header>
      {view !== "board" && syncStatus && syncStatus !== "live" ? <div className={styles.boundary} data-testid="explorer-sync-banner" role="status" aria-live="polite"><strong>{syncStatusLabel(syncStatus, copy)}</strong><span> {locale === "en" ? "The last usable snapshot remains visible." : "仍显示最近一次可用快照。"}</span></div> : null}
      <ExplorerTabs route={route} basePath={runtime.webBasePath} taskId={taskId} onNavigate={onNavigate} copy={copy} />
      <div className={showInspector ? styles.contentWithInspector : styles.content}>
        <main className={styles.primaryContent}>
          {view === "board" ? (
            boardRead.loading && !boardRead.data ? <div className={styles.boundary} data-testid="board-loading" role="status"><h2>{copy.boardLoading}</h2></div>
              : boardRead.data ? <BoardView state={{ kind: "ready", model: toBoardViewModel(boardRead.data) }} messages={boardMessagesForLocale(locale)} syncStatus={boardRead.error instanceof ExplorerReadError && boardRead.error.kind === "offline" ? "offline" : syncStatus ?? (boardRead.error ? "stale" : undefined)} onRetry={boardRead.retry} onSelectTask={selectTask} headingLevel={2} taskMutations={taskMutations} />
              : boardRead.error ? <div className={styles.boundary} data-testid={boardRead.error instanceof ExplorerReadError && boardRead.error.kind === "offline" ? "board-offline" : "board-error"} role={boardRead.error instanceof ExplorerReadError && boardRead.error.kind === "offline" ? "status" : "alert"}><h2>{boardRead.error instanceof ExplorerReadError && boardRead.error.kind === "offline" ? copy.boardOffline : copy.boardError}</h2><p>{boardRead.error.message}</p><button type="button" onClick={boardRead.retry}>{copy.retry}</button></div> : null
          ) : null}
          {view === "list" ? (
            <>
              <TaskListView
                state={{ query: listQuery, meta: listRead.data?.meta ?? { offset: (listQuery.page - 1) * listQuery.limit, limit: listQuery.limit, total: 0 } }}
                rows={listRead.data?.tasks.map(taskListRow) ?? []}
                loading={listRead.loading}
                error={listRead.error instanceof Error ? listRead.error : null}
                onQueryChange={updateListQuery}
                onSelectTask={selectTask}
                onRetry={listRead.retry}
                onCreate={listMutationController?.openCreate}
                isMutationPending={listMutationController?.isMutationPending}
                locale={locale}
              />
              {listMutationController && listMutationController.dialog === null ? <MutationNotice controller={listMutationController} copy={boardMessagesForLocale(locale)} /> : null}
              {listMutationController ? <MutationDialog controller={listMutationController} copy={boardMessagesForLocale(locale)} /> : null}
            </>
          ) : null}
          {view === "map" ? (
            <TaskMapChunkBoundary locale={locale}>
              <Suspense fallback={<div className={styles.boundary} data-testid="task-map-route-loading" role="status">{locale === "en" ? "Loading task map page…" : "正在加载关系图页面…"}</div>}>
                <LazyTaskMapView
                  runtime={runtime}
                  board={route.boardSlug}
                  boardIdentity={mapIdentityRead.data}
                  identityLoading={mapIdentityRead.loading}
                  identityError={mapIdentityRead.error}
                  onRetryIdentity={mapIdentityRead.retry}
                  invalidationRevision={boardRevision}
                  online={online !== false}
                  taskId={taskId}
                  onSelectTask={selectTask}
                  urlState={mapUrlState}
                  onUrlStateChange={updateMapUrlState}
                />
              </Suspense>
            </TaskMapChunkBoundary>
          ) : null}
          {view === "runs" ? <TaskRunsView runtime={runtime} taskId={taskId} invalidationRevision={runsRevision} online={online !== false} /> : null}
          {view === "events" ? (
            <EventsView
              runtime={runtime}
              boardSelector={route.boardSlug}
              taskId={taskId}
              kindFilter={kindFilter}
              online={online}
              invalidationRevision={boardRevision}
              eventsRefreshRevision={eventsRefreshRevision}
              batch={eventsBatch}
              onKindFilterChange={updateEventKindFilter}
              onSelectTask={selectTask}
            />
          ) : null}
        </main>
        {showInspector ? (
          inspectorModel ? (
            <aside className={styles.inspectorStack}>
              <TaskInspector
                key={inspectorIdentity}
                identity={inspectorIdentity}
                refreshRevision={inspectorRevision}
                model={inspectorModel}
                refreshError={inspectorRead.error instanceof Error ? inspectorRead.error.message : null}
                refreshOffline={inspectorRead.error instanceof ExplorerReadError && inspectorRead.error.kind === "offline"}
                online={online !== false}
                onRetry={inspectorRead.retry}
                onSelectTask={selectTask}
                locale={locale}
                onLoadRuns={loadInspectorRuns}
                onLoadEvents={loadInspectorEvents}
                onLoadNeighborhood={loadInspectorNeighborhood}
                mutationHandlers={inspectorMutationHandlers}
                mutationSnapshot={inspectorMutationSnapshot}
                claimToken={taskId ? taskMutations?.claimTokens?.get(taskId) ?? null : null}
              />
              {inspectorMutationHandlers && inspectorMutationSnapshot && inspectorRelations && taskId && inspectorRead.data ? (
                <>
                  <TaskInspectorRelationsPanel
                    taskId={taskId}
                    comments={inspectorRelations.comments}
                    dependencies={inspectorRelations.dependencies}
                    steps={inspectorRelations.steps}
                    handlers={inspectorMutationHandlers}
                    snapshot={inspectorMutationSnapshot}
                    onSelectTask={selectTask}
                    resolveTaskSelector={resolveInspectorTaskSelector}
                    locale={locale}
                  />
                  <TaskInspectorAssetsPanel
                    taskId={taskId}
                    labels={inspectorRead.data.task.labels as readonly InspectorAssetLabel[]}
                    attachments={(attachmentsRead.data ?? []) as readonly InspectorAssetAttachment[]}
                    suggestionResult={null}
                    suggestionRequested={false}
                    suggestionLoading={inspectorMutationSnapshot.pending.has(inspectorMutationKey("suggestLabels", taskId))}
                    handlers={inspectorMutationHandlers}
                    snapshot={inspectorMutationSnapshot}
                    attachmentLoading={attachmentsRead.loading}
                    attachmentError={attachmentsRead.error instanceof Error ? attachmentsRead.error.message : null}
                    locale={locale}
                  />
                </>
              ) : null}
            </aside>
          ) : <InspectorBoundary loading={inspectorRead.loading} error={inspectorRead.error instanceof Error ? inspectorRead.error : null} onRetry={inspectorRead.retry} copy={copy} />
        ) : null}
      </div>
    </section>
  )
}
