import { Component, lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react"

import { BoardView } from "../board/BoardView"
import type { BoardViewModel } from "../board/types"
import {
  ExplorerReadError,
  loadTaskInspector,
  loadExplorerBoardIdentity,
  loadTaskListPage,
  parseTaskListQuery,
  serializeTaskListQuery,
  type TaskInspectorReadModel,
  type TaskListQueryState,
} from "../../lib/api/explorer-read-model"
import type { CanonicalBoardSlug } from "../../lib/board-slug"
import type { BoardReadModel } from "../../lib/api/board-read-model"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { routePath, type AppNavigationTarget, type AppRoute, type BoardRouteView } from "../../lib/router"
import { usePreferences } from "../../lib/use-preferences"
import { TaskInspector, type InspectorDependency, type TaskInspectorViewModel } from "./TaskInspector"
import { TaskListView, type TaskListRow } from "./TaskListView"
import { TaskRunsView } from "./TaskRunsView"
import { shouldClearMapTaskFromInspector } from "./ExplorerPage.logic"
import { parseTaskMapUrlState, serializeTaskMapUrlState, type TaskMapUrlState } from "./TaskMapView.logic"
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
}

type AsyncState<T> = {
  readonly data: T | null
  readonly error: ExplorerReadError | Error | null
  readonly loading: boolean
}

function useAsyncRead<T>(
  enabled: boolean,
  key: string,
  load: (signal: AbortSignal) => Promise<T>,
): AsyncState<T> & { readonly retry: () => void } {
  const loadRef = useRef(load)
  loadRef.current = load
  const [generation, setGeneration] = useState(0)
  const [state, setState] = useState<AsyncState<T>>({ data: null, error: null, loading: false })

  useEffect(() => {
    if (!enabled) {
      setState({ data: null, error: null, loading: false })
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({ data: current.data, error: null, loading: true }))
    void loadRef.current(controller.signal).then(
      (data) => {
        if (active) setState({ data, error: null, loading: false })
      },
      (error: unknown) => {
        if (active && !(error instanceof Error && error.name === "AbortError")) {
          setState({ data: null, error: error instanceof Error ? error : new Error(String(error)), loading: false })
        }
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [enabled, generation, key])

  return { ...state, retry: () => setGeneration((current) => current + 1) }
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

function boardViewModel(model: BoardReadModel): BoardViewModel {
  const tasksByStatus: Record<string, BoardViewModel["tasksByStatus"][string]> = {}
  for (const [status, tasks] of Object.entries(model.tasksByStatus)) {
    tasksByStatus[status] = (tasks ?? []).map((task) => ({
      id: task.id,
      ref: task.ref,
      title: task.title,
      status: task.status,
      position: task.position,
      priority: boardPriority(task.priority),
      assignee: task.assignee,
      readiness: {
        dependencyBlocked: task.dependency_blocked,
        unfinishedParentCount: task.unfinished_parent_count,
        executionPlanState: task.execution_plan_state,
        requiredStepCount: task.required_step_count,
        completedRequiredStepCount: task.completed_required_step_count,
        optionalStepCount: task.optional_step_count,
      },
    }))
  }
  return {
    board: { id: model.identity.canonicalBoardId, slug: model.identity.slug, name: model.identity.name },
    columns: model.columns.map((column) => ({ id: column.id, status: column.status, title: column.title, position: column.position, hidden: column.hidden })),
    tasksByStatus,
  }
}

function dependencyView(task: NonNullable<TaskInspectorReadModel["dependencies"]["parents"]>[number]): InspectorDependency {
  return { id: task.id, ref: task.ref, title: task.title, status: task.status }
}

function inspectorViewModel(model: TaskInspectorReadModel): TaskInspectorViewModel {
  const task = model.task
  return {
    task: {
      id: task.id,
      ref: task.ref,
      title: task.title,
      status: task.status,
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
    runs: model.runs.map((run) => ({ id: run.id, status: run.status, workerProfile: run.worker_profile, claimOwner: run.claim_owner, startedAt: run.started_at, finishedAt: run.finished_at, exitCode: run.exit_code, error: run.error, hasLog: run.has_log })),
    events: model.events.map((event) => ({ id: event.id, kind: event.kind, actor: event.actor, createdAt: event.created_at })),
    runtime: model.runtime,
  }
}

function InspectorBoundary({ loading, error, onRetry }: { readonly loading: boolean; readonly error: Error | null; readonly onRetry: () => void }) {
  if (loading) return <aside className={styles.inspectorBoundary} data-testid="task-inspector-loading" role="status">正在加载 Task Inspector…</aside>
  if (error) return <aside className={styles.inspectorBoundary} data-testid="task-inspector-error" role="alert"><strong>Task Inspector 加载失败</strong><p>{error.message}</p><button type="button" onClick={onRetry}>重试</button></aside>
  return null
}

function ExplorerTabs({ route, basePath, taskId, onNavigate }: { readonly route: Extract<AppRoute, { kind: "board" }>; readonly basePath: string; readonly taskId: string | null; readonly onNavigate?: ExplorerPageProps["onNavigate"] }) {
  const params = queryParams(route)
  const views: readonly [BoardRouteView, string][] = [["board", "Board"], ["list", "List"], ["map", "Map"], ["runs", "Runs"], ["events", "Events"]]
  return (
    <nav className={styles.tabs} aria-label="Board explorer views">
      {views.map(([view, label]) => {
        const next = new URLSearchParams(params)
        if (taskId) next.set("task", taskId)
        const href = routeTarget(route.boardSlug, view, next, basePath)
        return <a key={view} href={href} aria-current={(route.view ?? "board") === view ? "page" : undefined} onClick={(event) => { if (!onNavigate) return; event.preventDefault(); void onNavigate(href) }}>{label}</a>
      })}
    </nav>
  )
}

export function ExplorerPage({ runtime, route, onNavigate }: ExplorerPageProps) {
  const { locale } = usePreferences()
  const view = route.view ?? "board"
  const params = queryParams(route)
  const rawTaskId = params.get("task")?.trim() || null
  const mapUrlState = useMemo(() => parseTaskMapUrlState(route.query ?? ""), [route.query])
  const taskId = view === "map" ? mapUrlState.taskId : rawTaskId
  const showInspector = Boolean(taskId) && view !== "runs"
  const listQuery = useMemo(() => parseTaskListQuery(new URLSearchParams(route.query ?? "")), [route.query])
  const listKey = `${route.boardSlug}|${serializeTaskListQuery(listQuery)}`
  const boardRead = useAsyncRead(view === "board", route.boardSlug, (signal) => import("../../lib/api/board-read-model").then(({ loadBoardReadModel }) => loadBoardReadModel(runtime, route.boardSlug, { signal })))
  const listRead = useAsyncRead(view === "list", listKey, (signal) => loadTaskListPage(runtime, route.boardSlug, listQuery, { signal }))
  const mapIdentityRead = useAsyncRead(view === "map", route.boardSlug, (signal) => loadExplorerBoardIdentity(runtime, route.boardSlug, { signal }))
  const inspectorKey = `${route.boardSlug}|${taskId ?? ""}`
  const inspectorRead = useAsyncRead(Boolean(taskId) && view !== "runs", inspectorKey, (signal) => taskId ? loadTaskInspector(runtime, route.boardSlug, taskId, { signal }) : Promise.reject(new Error("Task Inspector 尚未选择任务")))

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
    <section className={styles.explorer} data-testid="explorer-page">
      <header className={styles.explorerHeader}>
        <div>
          <p className={styles.eyebrow}>ASTRYX EXPLORER</p>
          <h1>{route.boardSlug}</h1>
        </div>
        {taskId ? <button type="button" className={styles.closeInspector} onClick={closeInspector}>关闭 Inspector</button> : null}
      </header>
      <ExplorerTabs route={route} basePath={runtime.webBasePath} taskId={taskId} onNavigate={onNavigate} />
      <div className={showInspector ? styles.contentWithInspector : styles.content}>
        <main className={styles.primaryContent}>
          {view === "board" ? (
            boardRead.loading && !boardRead.data ? <div className={styles.boundary} data-testid="board-loading" role="status">正在加载看板…</div>
              : boardRead.error ? <div className={styles.boundary} data-testid="board-error" role="alert"><p>{boardRead.error.message}</p><button type="button" onClick={boardRead.retry}>重试</button></div>
                : boardRead.data ? <BoardView state={{ kind: "ready", model: boardViewModel(boardRead.data) }} onRetry={boardRead.retry} onSelectTask={selectTask} /> : null
          ) : null}
          {view === "list" ? (
            <TaskListView
              state={{ query: listQuery, meta: listRead.data?.meta ?? { offset: (listQuery.page - 1) * listQuery.limit, limit: listQuery.limit, total: 0 } }}
              rows={listRead.data?.tasks.map(taskListRow) ?? []}
              loading={listRead.loading}
              error={listRead.error instanceof Error ? listRead.error : null}
              onQueryChange={updateListQuery}
              onSelectTask={selectTask}
              onRetry={listRead.retry}
            />
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
                  taskId={taskId}
                  onSelectTask={selectTask}
                  urlState={mapUrlState}
                  onUrlStateChange={updateMapUrlState}
                />
              </Suspense>
            </TaskMapChunkBoundary>
          ) : null}
          {view === "runs" ? <TaskRunsView runtime={runtime} taskId={taskId} /> : null}
          {view === "events" ? <EventsPlaceholder taskId={taskId} inspector={inspectorRead.data} /> : null}
        </main>
        {showInspector ? (
          inspectorRead.data ? <TaskInspector model={inspectorViewModel(inspectorRead.data)} onSelectTask={selectTask} /> : <InspectorBoundary loading={inspectorRead.loading} error={inspectorRead.error instanceof Error ? inspectorRead.error : null} onRetry={inspectorRead.retry} />
        ) : null}
      </div>
    </section>
  )
}

function EventsPlaceholder({ taskId, inspector }: { readonly taskId: string | null; readonly inspector: TaskInspectorReadModel | null }) {
  return <section className={styles.boundary} data-testid="events-view"><h2>Events</h2><p>{taskId ? inspector ? `${inspector.events.length} 条任务事件` : "正在加载事件…" : "选择任务后查看事件。"}</p></section>
}
