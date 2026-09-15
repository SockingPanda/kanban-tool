import { useAsyncRead } from "../../application/query/use-async-read";

import { useWorkspaceOperations } from "../../application/workspace/use-workspace-operations";



import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, type MouseEvent, type KeyboardEvent } from "react";





import { boardMessagesForLocale, type BoardViewModel } from "../../domain/tasks/board"

import { projectTask } from "../../application/tasks/board-model";
import { useBoardTaskMutationController } from "../../application/tasks/task-mutation-controller"



import type { BoardSyncStatus } from "../../domain/tasks/board"

import type { BoardTaskCanonicalReloadHandler, BoardTaskMutationSurface } from "../../application/tasks/task-mutation-state"

import { serializeTaskListQuery, type TaskInspectorReadModel, type TaskListQueryState, type ExplorerTaskListPage } from "../../application/data/explorer-read-model";

import { parseTaskListQuery } from "../../application/data/explorer-read-model";

import type { CanonicalBoardSlug } from "../../domain/board-slug"

import type { Locale } from "../../platform/preferences/preferences"

import type { WebRuntimeConfig } from "../../lib/runtime"

import { routePath, type AppNavigationTarget, type AppRoute, type BoardRouteView } from "../../application/navigation/router"

import { usePreferences } from "../../platform/preferences/use-preferences"

import { restoreExplorerFocus, type ExplorerFocusElement, type ExplorerFocusSnapshot } from "../../platform/focus/explorer-focus"

import type { InspectorDependency, TaskInspectorViewModel } from "./inspector-model"





import { inspectorMutationKey, type TaskInspectorMutationHandlers, type TaskInspectorMutationSnapshot, type TaskInspectorMutationSurface } from "../../application/tasks/task-inspector-mutation-state"

import { useTaskInspectorMutationController } from "../../application/tasks/task-inspector-mutation-controller"





import {
  shouldClearMapTaskFromInspector,
  inspectorRelationsView,
} from "./ExplorerPage.logic"

import { parseTaskMapUrlState, serializeTaskMapUrlState, type TaskMapUrlState } from "./TaskMapView.logic"



import type { BoardEventsBatch } from "../../application/data/explorer-read-model";



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
  readonly onVisibleCanonicalReloadChange?: (reload: BoardTaskCanonicalReloadHandler | undefined, releasedReload?: BoardTaskCanonicalReloadHandler) => void
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
    eyebrow: "TASK WORKSPACE",
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
    map: "依赖图",
    runs: "运行记录",
    events: "项目动态",
    syncConnecting: "正在连接实时同步…",
    syncRecovering: "正在恢复同步",
    syncStale: "同步暂时中断",
    syncCircuitOpen: "同步暂时不可用",
    syncOffline: "当前离线",
  },
  en: {
    eyebrow: "TASK WORKSPACE",
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
    comments: model.comments.map((comment) => ({ id: comment.id, author: comment.author, kind: comment.kind, body: comment.body, createdAt: comment.created_at, metadata: comment.metadata })),
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

const inspectorWriteOperations = [
  "saveTask",
  "transition",
  "addDependency",
  "removeDependency",
  "mutateStep",
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

const collectionViews = new Set<BoardRouteView>(['list', 'board', 'map'])

function taskPageMutationModel(page: ExplorerTaskListPage | null): BoardViewModel | null {
  if (!page) return null
  const tasksByStatus: Record<string, ReturnType<typeof projectTask>[]> = {}
  for (const task of page.tasks) (tasksByStatus[task.status] ??= []).push(projectTask(task, task.status))
  return { board: { id: page.board.id, slug: page.board.slug, name: page.board.name }, columns: [], tasksByStatus }
}

export function useTaskWorkspace({ runtime, route, onNavigate, online, invalidationRevision = 0, boardRevision = invalidationRevision, inspectorRevision = invalidationRevision, runsRevision = invalidationRevision, eventsRefreshRevision = invalidationRevision, eventsBatch, syncStatus, taskMutations, onVisibleCanonicalReloadChange }: ExplorerPageProps) {
  const { loadTaskInspectorAttachments, loadTaskInspector, loadTaskInspectorEvents, loadTaskInspectorNeighborhood, loadTaskInspectorRuns, loadExplorerBoardIdentity, loadTaskListPage, createAttachmentDownloadClient } = useWorkspaceOperations();
  const { locale } = usePreferences()
  const copy = explorerCopies[locale]
  const view: BoardRouteView = route.view ?? "board"
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
  const collectionVisible = collectionViews.has(view)
  const listRead = useAsyncRead(collectionVisible, listKey, (signal) => loadTaskListPage(runtime, route.boardSlug, listQuery, { signal }), boardRevision, online !== false)
  const listMutationModel = useMemo(() => taskPageMutationModel(listRead.data), [listRead.data])
  const listMutationController = useBoardTaskMutationController(listMutationModel, taskMutations, [], boardMessagesForLocale(locale))
  const mapIdentityRead = useAsyncRead(view === "map", route.boardSlug, (signal) => loadExplorerBoardIdentity(runtime, route.boardSlug, { signal }), boardRevision, online !== false)
  const inspectorKey = `${route.boardSlug}|${taskId ?? ""}`
  const inspectorIdentity = `${runtime.apiBaseUrl}\u0000${runtime.webBuildId}\u0000${inspectorKey}`
  const inspectorRead = useAsyncRead(Boolean(taskId) && view !== "runs", inspectorKey, (signal) => taskId ? loadTaskInspector(runtime, route.boardSlug, taskId, { signal, includeNeighborhood: false, includeRuns: false, includeEvents: false, includeAttachments: false }) : Promise.reject(new Error("Task Inspector 尚未选择任务")), inspectorRevision, online !== false)
  const attachmentsRead = useAsyncRead(showInspector, `${inspectorKey}\u0000attachments`, (signal) => taskId
    ? loadTaskInspectorAttachments(runtime, route.boardSlug, taskId, { signal })
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), inspectorRevision, online !== false)
  const reloadInspector = inspectorRead.reload
  const reloadAttachments = attachmentsRead.reload
  const reloadList = listRead.reload
  const reloadVisibleInspector = useCallback(async () => {
    const [page, ...details] = await Promise.allSettled([
      collectionVisible ? reloadList() : Promise.resolve(null),
      ...(showInspector ? [reloadInspector(), reloadAttachments()] : []),
    ])
    if (page.status === "rejected") throw page.reason
    for (const detail of details) if (detail.status === "rejected") throw detail.reason
    return taskPageMutationModel(page.value)
  }, [collectionVisible, reloadList, reloadAttachments, reloadInspector, showInspector])
  useLayoutEffect(() => {
    if (!collectionVisible && !showInspector) {
      onVisibleCanonicalReloadChange?.(undefined)
      return
    }
    onVisibleCanonicalReloadChange?.(reloadVisibleInspector)
    return () => onVisibleCanonicalReloadChange?.(undefined, reloadVisibleInspector)
  }, [collectionVisible, onVisibleCanonicalReloadChange, reloadVisibleInspector, showInspector])
  const inspectorModel = useMemo(() => inspectorRead.data ? inspectorViewModel(inspectorRead.data) : null, [inspectorRead.data])
  const inspectorRelations = useMemo(() => inspectorRead.data ? inspectorRelationsView(inspectorRead.data) : null, [inspectorRead.data])
  const attachmentDownload = useMemo(() => {
    try {
      return createAttachmentDownloadClient(runtime)
    } catch {
      return undefined
    }
  }, [createAttachmentDownloadClient, runtime])
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
      },
    }
  }, [attachmentDownload, inspectorIdentity, inspectorRead, taskId, taskMutations])
  const inspectorMutationController = useTaskInspectorMutationController(inspectorMutationSurface)
  const inspectorMutationHandlers = useMemo<TaskInspectorMutationHandlers | undefined>(() => {
    if (inspectorMutationController === null) return undefined
    return {
      saveTask: inspectorMutationController.saveTask.bind(inspectorMutationController),
      transition: inspectorMutationController.transition.bind(inspectorMutationController),
      addDependency: inspectorMutationController.addDependency.bind(inspectorMutationController),
      removeDependency: inspectorMutationController.removeDependency.bind(inspectorMutationController),
      mutateStep: inspectorMutationController.mutateStep.bind(inspectorMutationController),
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
  const relationOwner = inspectorMutationHandlers !== undefined
    && inspectorMutationSnapshot !== undefined
    && inspectorRelations !== null
    && taskId !== null
    && inspectorRead.data !== null
    ? { taskId, data: inspectorRead.data, relations: inspectorRelations, handlers: inspectorMutationHandlers, snapshot: inspectorMutationSnapshot }
    : null
  const relationPanelVisible = relationOwner !== null

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
    navigate(routeTarget(route.boardSlug, view === "board" ? "board" : "list", nextParams, runtime.webBasePath))
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

  const rememberTaskOpener = useCallback((event: MouseEvent<HTMLElement> | KeyboardEvent<HTMLElement>) => {
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
    if (!snapshot) return
    openerRef.current = null
    restoreExplorerFocus(snapshot, {
      querySelector: (selector) => document.querySelector(selector) as ExplorerFocusElement | null,
    })
  }, [])

  useEffect(() => {
    const previousTaskId = previousTaskIdRef.current
    // Events 关闭 task 筛选会重新挂载事件列表，由该查询完成后恢复 opener。
    if (previousTaskId !== null && taskId === null && view !== "events") restoreFocus()
    previousTaskIdRef.current = taskId
  }, [restoreFocus, taskId, view])
  const updateEventKindFilter = (nextKind: string) => {
    const nextParams = new URLSearchParams(params)
    const normalizedKind = normalizeEventKindFilter(nextKind)
    if (normalizedKind) nextParams.set("kind", normalizedKind)
    else nextParams.delete("kind")
    navigate(routeTarget(route.boardSlug, "events", nextParams, runtime.webBasePath))
  }

  const loadInspectorRuns = useCallback((signal: AbortSignal) => taskId
    ? loadTaskInspectorRuns(runtime, route.boardSlug, taskId, { signal }).then((runs) => runs.map((run) => inspectorRunView(run)))
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), [loadTaskInspectorRuns, route.boardSlug, runtime, taskId])
  const loadInspectorEvents = useCallback((signal: AbortSignal) => taskId
    ? loadTaskInspectorEvents(runtime, route.boardSlug, taskId, { signal }).then((events) => events.map((event) => inspectorEventView(event)))
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), [loadTaskInspectorEvents, route.boardSlug, runtime, taskId])
  const loadInspectorNeighborhood = useCallback((signal: AbortSignal) => taskId
    ? loadTaskInspectorNeighborhood(runtime, route.boardSlug, taskId, { signal }).then((neighborhood) => ({
      centerTaskId: neighborhood.center_task_id,
      nodes: neighborhood.nodes.map((node) => ({ id: node.task.id, ref: node.task.ref, title: node.task.title, role: node.role })),
      edges: neighborhood.edges.map((edge) => ({ id: edge.id, sourceTaskId: edge.source_task_id, targetTaskId: edge.target_task_id, kind: edge.kind })),
    }))
    : Promise.reject(new Error("Task Inspector 尚未选择任务")), [loadTaskInspectorNeighborhood, route.boardSlug, runtime, taskId])
  const resolveInspectorTaskSelector = useCallback((selector: string): string | null => {
    const value = selector.trim()
    const boardResolved = taskMutations?.resolveTaskSelector?.(value)
    if (boardResolved !== undefined && boardResolved !== null) return boardResolved
    const model = inspectorRead.data
    if (!model) return null
    const candidates = [
      model.task,
      ...model.dependencies.parents,
      ...model.dependencies.children,
      ...model.steps.steps.flatMap((step) => step.linked_task ? [step.linked_task] : []),
    ]
    return candidates.find((candidate) => candidate.id === value || candidate.ref === value)?.id ?? null
  }, [inspectorRead.data, taskMutations])

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

  return {
    rememberTaskOpener,
    restoreFocus,
    route,
    copy,
    taskId,
    closeInspector,
    view,
    syncStatus,
    locale,
    runtime,
    onNavigate,
    selectTask,
    taskMutations,
    listQuery,
    listRead,
    updateListQuery,
    listMutationController,
    listMutationModel,
    mapIdentityRead,
    boardRevision,
    online,
    mapUrlState,
    updateMapUrlState,
    runsRevision,
    kindFilter,
    eventsRefreshRevision,
    eventsBatch,
    updateEventKindFilter,
    showInspector,
    inspectorModel,
    inspectorIdentity,
    inspectorRevision,
    inspectorRead,
    loadInspectorRuns,
    loadInspectorEvents,
    loadInspectorNeighborhood,
    inspectorMutationHandlers,
    inspectorMutationSnapshot,
    relationOwner,
    resolveInspectorTaskSelector,
    relationPanelVisible,
    attachmentsRead
  };
}

export type TaskWorkspaceState = ReturnType<typeof useTaskWorkspace>;
