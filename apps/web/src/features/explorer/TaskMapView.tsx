import { useEffect, useMemo, useRef, useState } from "react"

import {
  ExplorerReadError,
  loadTaskMap,
  type ExplorerBoardIdentity,
  type ExplorerTaskMapReadModel,
} from "../../lib/api/explorer-read-model"
import type { Locale } from "../../lib/preferences"
import { taskOpenerKey } from "../../lib/explorer-focus"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { usePreferences } from "../../lib/use-preferences"
import {
  defaultTaskMapUrlState,
  filterTaskMap,
  clampMapZoom,
  fenceTaskMapReadModel,
  MAX_MAP_ZOOM,
  MIN_MAP_ZOOM,
  resolveSelectedNode,
  stepMapZoom,
  taskMapIdentityKey,
  type BoardMapFilter,
  type TaskMapUrlState,
} from "./TaskMapView.logic"
import styles from "./TaskMapView.module.css"

export type TaskMapReadState = {
  readonly data: ExplorerTaskMapReadModel | null
  readonly loading: boolean
  readonly error: ExplorerReadError | Error | null
}

export type TaskMapUrlStateChangeOptions = {
  readonly replace?: boolean
}

export interface TaskMapViewProps {
  readonly runtime: WebRuntimeConfig
  readonly board: string
  readonly boardIdentity: ExplorerBoardIdentity | null
  readonly identityLoading?: boolean
  readonly identityError?: ExplorerReadError | Error | null
  readonly onRetryIdentity?: () => void
  readonly invalidationRevision?: number
  readonly taskId: string | null
  readonly onSelectTask: (taskId: string) => void
  readonly urlState?: TaskMapUrlState
  readonly onUrlStateChange?: (state: TaskMapUrlState, options?: TaskMapUrlStateChangeOptions) => void
}

export interface TaskMapPresentationProps {
  readonly board: string
  readonly locale?: Locale
  readonly taskId: string | null
  readonly state: TaskMapReadState
  readonly onSelectTask: (taskId: string) => void
  readonly selectedTaskId?: string | null
  readonly onInspectTask?: (taskId: string) => void
  readonly filter?: BoardMapFilter
  readonly hideIsolated?: boolean
  readonly showDoneContext?: boolean
  readonly zoom?: number
  readonly onFilterChange?: (filter: BoardMapFilter) => void
  readonly onHideIsolatedChange?: () => void
  readonly onShowDoneContextChange?: () => void
  readonly onZoomChange?: (direction: -1 | 0 | 1) => void
  readonly onRetry?: () => void
}

type MapNode = NonNullable<ReturnType<typeof resolveSelectedNode>>

type TaskMapReadInternalState = TaskMapReadState & {
  readonly requestKey: string
  readonly identityToken: string
}

const MAP_LIMIT_NODES = 240

const zoomClassByValue: Readonly<Record<string, string>> = {
  "0.65": styles.zoom65,
  "0.7": styles.zoom70,
  "0.75": styles.zoom75,
  "0.8": styles.zoom80,
  "0.85": styles.zoom85,
  "0.9": styles.zoom90,
  "0.95": styles.zoom95,
  "1": styles.zoom100,
  "1.05": styles.zoom105,
  "1.1": styles.zoom110,
  "1.15": styles.zoom115,
  "1.2": styles.zoom120,
  "1.25": styles.zoom125,
  "1.3": styles.zoom130,
  "1.35": styles.zoom135,
  "1.4": styles.zoom140,
  "1.45": styles.zoom145,
  "1.5": styles.zoom150,
}

function zoomClassName(zoom: number): string {
  return zoomClassByValue[String(clampMapZoom(zoom))] ?? styles.zoom100
}

type MapCopy = {
  readonly kicker: string
  readonly title: string
  readonly nodes: string
  readonly edges: string
  readonly toolbar: string
  readonly filter: string
  readonly filterOptions: Readonly<Record<BoardMapFilter, string>>
  readonly hideIsolated: string
  readonly showIsolated: string
  readonly hideDone: string
  readonly showDone: string
  readonly zoom: string
  readonly zoomOut: string
  readonly zoomIn: string
  readonly zoomReset: string
  readonly refresh: string
  readonly refreshing: string
  readonly notFound: string
  readonly error: string
  readonly retry: string
  readonly loading: string
  readonly loadingDescription: string
  readonly empty: string
  readonly emptyDescription: string
  readonly filteredEmpty: string
  readonly refreshError: string
  readonly truncated: string
  readonly limit: string
  readonly graphHeading: string
  readonly graphRegion: string
  readonly edgesHeading: string
  readonly noEdges: string
  readonly selected: string
  readonly selectDescription: string
  readonly hiddenSelection: string
  readonly status: string
  readonly priority: string
  readonly plan: string
  readonly requiredSteps: string
  readonly nodeRole: string
  readonly context: string
  readonly inspectTask: string
  readonly openInspector: string
  readonly required: string
}

const copies: Record<Locale, MapCopy> = {
  zh: {
    kicker: "任务关系图",
    title: "任务关系图",
    nodes: "节点",
    edges: "边",
    toolbar: "关系图筛选与缩放",
    filter: "关系图筛选",
    filterOptions: { all: "全部活动", blocked: "阻塞", ready: "可执行", running: "运行中", unplanned: "未规划", "incomplete-steps": "未完成步骤" },
    hideIsolated: "隐藏孤立节点",
    showIsolated: "显示孤立节点",
    hideDone: "隐藏完成上下文",
    showDone: "显示完成上下文",
    zoom: "关系图缩放",
    zoomOut: "缩小关系图",
    zoomIn: "放大关系图",
    zoomReset: "重置关系图缩放",
    refresh: "刷新",
    refreshing: "正在刷新…",
    notFound: "看板不存在",
    error: "关系图加载失败",
    retry: "重试",
    loading: "正在加载关系图…",
    loadingDescription: "正在读取当前看板的任务关系。",
    empty: "暂无关系图节点",
    emptyDescription: "当前看板还没有可展示的任务关系。",
    filteredEmpty: "没有任务匹配当前筛选。",
    refreshError: "关系图刷新失败",
    truncated: "关系图已截断",
    limit: "当前节点上限为",
    graphHeading: "任务关系节点",
    graphRegion: "任务关系图，可横向滚动",
    edgesHeading: "关系边",
    noEdges: "当前筛选没有关系边。",
    selected: "当前选择",
    selectDescription: "选择一个关系图节点查看任务。",
    hiddenSelection: "当前节点被筛选隐藏。",
    status: "状态",
    priority: "优先级",
    plan: "计划",
    requiredSteps: "必需步骤",
    nodeRole: "节点角色",
    context: "上下文",
    inspectTask: "检查任务",
    openInspector: "打开任务检查器",
    required: "必需",
  },
  en: {
    kicker: "TASK MAP",
    title: "Task map",
    nodes: "nodes",
    edges: "edges",
    toolbar: "Task map filters and zoom",
    filter: "Task map filter",
    filterOptions: { all: "All active", blocked: "Blocked", ready: "Ready", running: "Running", unplanned: "Unplanned", "incomplete-steps": "Incomplete steps" },
    hideIsolated: "Hide isolated nodes",
    showIsolated: "Show isolated nodes",
    hideDone: "Hide done context",
    showDone: "Show done context",
    zoom: "Task map zoom",
    zoomOut: "Zoom out task map",
    zoomIn: "Zoom in task map",
    zoomReset: "Reset task map zoom",
    refresh: "Refresh",
    refreshing: "Refreshing…",
    notFound: "Board not found",
    error: "Task map failed to load",
    retry: "Retry",
    loading: "Loading task map…",
    loadingDescription: "Reading task relations for this board.",
    empty: "No task map nodes",
    emptyDescription: "This board has no task relations to show.",
    filteredEmpty: "No tasks match the current filter.",
    refreshError: "Task map refresh failed",
    truncated: "Task map truncated",
    limit: "Node limit is",
    graphHeading: "Task map nodes",
    graphRegion: "Task map, horizontally scrollable",
    edgesHeading: "Relations",
    noEdges: "No relations match the current filter.",
    selected: "Current selection",
    selectDescription: "Select a task map node to inspect it.",
    hiddenSelection: "The selected node is hidden by the filter.",
    status: "Status",
    priority: "Priority",
    plan: "Plan",
    requiredSteps: "Required steps",
    nodeRole: "Node role",
    context: "Context",
    inspectTask: "Inspect task",
    openInspector: "Open Inspector",
    required: "required",
  },
}

function errorReason(error: Error | null): string | null {
  if (!error || !("reason" in error)) return null
  const reason = error.reason
  return typeof reason === "string" ? reason : null
}

function useTaskMapRead(
  runtime: WebRuntimeConfig,
  board: string,
  boardIdentity: ExplorerBoardIdentity | null,
  includeDoneContext: boolean,
  hideIsolated: boolean,
  invalidationRevision: number,
): TaskMapReadState & { readonly retry: () => void } {
  const loadRef = useRef<((signal: AbortSignal) => Promise<ExplorerTaskMapReadModel>) | null>(null)
  loadRef.current = boardIdentity
    ? (signal) => loadTaskMap(runtime, board, {
      boardIdentity,
      activeOnly: true,
      contextDepth: 1,
      includeDoneContext,
      includeArchivedContext: false,
      hideIsolated,
      limitNodes: MAP_LIMIT_NODES,
      signal,
    })
    : null
  const [generation, setGeneration] = useState(0)
  const key = `${board}|${boardIdentity?.id ?? "identity-pending"}|${includeDoneContext ? "done" : "active"}|${hideIsolated ? "connected" : "all"}`
  const identityToken = `${board}|${boardIdentity ? taskMapIdentityKey(boardIdentity) : "identity-pending"}`
  const requestKey = `${key}|${generation}|${invalidationRevision}`
  const [state, setState] = useState<TaskMapReadInternalState>(() => ({
    data: null,
    loading: false,
    error: null,
    requestKey,
    identityToken,
  }))

  useEffect(() => {
    if (!boardIdentity) {
      setState({ data: null, loading: false, error: null, requestKey, identityToken })
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({
      data: current.identityToken === identityToken ? current.data : null,
      loading: true,
      error: null,
      requestKey,
      identityToken,
    }))
    void loadRef.current?.(controller.signal).then(
      (data) => {
        if (active) setState({ data, loading: false, error: null, requestKey, identityToken })
      },
      (error: unknown) => {
        if (active && !(error instanceof Error && error.name === "AbortError")) {
          setState((current) => ({
            data: current.identityToken === identityToken ? current.data : null,
            loading: false,
            error: error instanceof Error ? error : new Error(String(error)),
            requestKey,
            identityToken,
          }))
        }
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [boardIdentity, generation, identityToken, invalidationRevision, key, requestKey])

  const sameIdentity = state.identityToken === identityToken
  const currentRequest = sameIdentity && state.requestKey === requestKey
  return {
    data: sameIdentity ? state.data : null,
    loading: sameIdentity ? (currentRequest ? state.loading : true) : true,
    error: currentRequest ? state.error : null,
    retry: () => setGeneration((current) => current + 1),
  }
}

function TaskMapToolbar({
  copy,
  filter,
  hideIsolated,
  showDoneContext,
  zoom,
  loading,
  onFilterChange,
  onHideIsolatedChange,
  onShowDoneContextChange,
  onZoomChange,
  onRetry,
}: {
  readonly copy: MapCopy
  readonly filter: BoardMapFilter
  readonly hideIsolated: boolean
  readonly showDoneContext: boolean
  readonly zoom: number
  readonly loading: boolean
  readonly onFilterChange: (filter: BoardMapFilter) => void
  readonly onHideIsolatedChange: () => void
  readonly onShowDoneContextChange: () => void
  readonly onZoomChange: (direction: -1 | 0 | 1) => void
  readonly onRetry?: () => void
}) {
  return (
    <div className={styles.toolbar} role="toolbar" aria-label={copy.toolbar}>
      <div className={styles.filterGroup} role="group" aria-label={copy.filter}>
        {(Object.keys(copy.filterOptions) as BoardMapFilter[]).map((value) => (
          <button
            key={value}
            type="button"
            className={styles.filterButton}
            aria-pressed={filter === value}
            onClick={() => onFilterChange(value)}
          >
            {copy.filterOptions[value]}
          </button>
        ))}
      </div>
      <button type="button" className={styles.toggleButton} aria-pressed={hideIsolated} onClick={onHideIsolatedChange}>
        {hideIsolated ? copy.showIsolated : copy.hideIsolated}
      </button>
      <button type="button" className={styles.toggleButton} aria-pressed={showDoneContext} onClick={onShowDoneContextChange}>
        {showDoneContext ? copy.hideDone : copy.showDone}
      </button>
      <div className={styles.zoomGroup} role="group" aria-label={copy.zoom}>
        <button type="button" aria-label={copy.zoomOut} onClick={() => onZoomChange(-1)} disabled={zoom <= MIN_MAP_ZOOM}>−</button>
        <output data-testid="task-map-zoom" aria-live="polite">{Math.round(zoom * 100)}%</output>
        <button type="button" aria-label={copy.zoomIn} onClick={() => onZoomChange(1)} disabled={zoom >= MAX_MAP_ZOOM}>＋</button>
        <button type="button" aria-label={copy.zoomReset} onClick={() => onZoomChange(0)}>↺</button>
      </div>
      {onRetry ? <button type="button" className={styles.refreshButton} onClick={onRetry} disabled={loading}>{loading ? copy.refreshing : copy.refresh}</button> : null}
    </div>
  )
}

function TaskMapError({ copy, error, onRetry }: { readonly copy: MapCopy; readonly error: Error; readonly onRetry?: () => void }) {
  const notFound = errorReason(error) === "board-not-found"
  return (
    <section className={styles.state} data-testid={notFound ? "task-map-not-found" : "task-map-error"} role="alert">
      <h2>{notFound ? copy.notFound : copy.error}</h2>
      <p>{error.message}</p>
      {onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}
    </section>
  )
}

function TaskMapInspector({
  copy,
  node,
  hiddenSelection,
  onSelectTask,
}: {
  readonly copy: MapCopy
  readonly node: MapNode | null
  readonly hiddenSelection: boolean
  readonly onSelectTask: (taskId: string) => void
}) {
  if (!node) {
    return <aside className={styles.inspector} data-testid="task-map-inspector"><h2>{copy.selected}</h2><p>{copy.selectDescription}</p></aside>
  }
  const task = node.task
  return (
    <aside className={styles.inspector} data-testid="task-map-inspector" aria-label={copy.selected}>
      <h2>{copy.selected}</h2>
      {hiddenSelection ? <p className={styles.hiddenSelection} role="status">{copy.hiddenSelection}</p> : null}
      <p className={styles.nodeRef} translate="no">{task.ref}</p>
      <h3>{task.title}</h3>
      <dl className={styles.facts}>
        <div><dt>{copy.status}</dt><dd translate="no">{task.status}</dd></div>
        <div><dt>{copy.priority}</dt><dd translate="no">P{task.priority}</dd></div>
        <div><dt>{copy.plan}</dt><dd translate="no">{task.execution_plan_state}</dd></div>
        <div><dt>{copy.requiredSteps}</dt><dd>{task.completed_required_step_count} / {task.required_step_count}</dd></div>
        <div><dt>{copy.nodeRole}</dt><dd translate="no">{node.role}</dd></div>
      </dl>
      <button type="button" className={styles.openButton} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>{copy.openInspector}</button>
    </aside>
  )
}

export function TaskMapPresentation({
  board,
  locale = "zh",
  taskId,
  state,
  onSelectTask,
  selectedTaskId,
  onInspectTask,
  filter = "all",
  hideIsolated = false,
  showDoneContext = false,
  zoom = 1,
  onFilterChange,
  onHideIsolatedChange,
  onShowDoneContextChange,
  onZoomChange,
  onRetry,
}: TaskMapPresentationProps) {
  const copy = copies[locale]
  const sourceGraph = state.data?.map ?? null
  const visibleGraph = sourceGraph ? filterTaskMap(sourceGraph, filter, hideIsolated) : null
  const selectedNode = resolveSelectedNode(sourceGraph, selectedTaskId ?? null, taskId)
  const hiddenSelection = Boolean(selectedNode && visibleGraph && !visibleGraph.nodes.some((node) => node.task.id === selectedNode.task.id))
  const inspect = onInspectTask ?? onSelectTask
  const updateFilter = onFilterChange ?? (() => undefined)
  const updateHideIsolated = onHideIsolatedChange ?? (() => undefined)
  const updateDoneContext = onShowDoneContextChange ?? (() => undefined)
  const updateZoom = onZoomChange ?? (() => undefined)
  const mapMeta = sourceGraph?.meta

  return (
    <section className={styles.map} data-testid="task-map" aria-labelledby="task-map-heading">
      <header className={styles.header}>
        <div>
          <p className={styles.eyebrow} translate="no">{copy.kicker}</p>
          <h2 id="task-map-heading">{copy.title}</h2>
          <p className={styles.muted}><span translate="no">{board}</span>{mapMeta ? <> · {mapMeta.node_count} {copy.nodes} · {mapMeta.edge_count} {copy.edges}</> : null}</p>
        </div>
      </header>

      <TaskMapToolbar
        copy={copy}
        filter={filter}
        hideIsolated={hideIsolated}
        showDoneContext={showDoneContext}
        zoom={zoom}
        loading={state.loading}
        onFilterChange={updateFilter}
        onHideIsolatedChange={updateHideIsolated}
        onShowDoneContextChange={updateDoneContext}
        onZoomChange={updateZoom}
        onRetry={onRetry}
      />

      {state.error && sourceGraph ? <div className={styles.inlineError} role="alert" data-testid="task-map-refresh-error"><strong>{copy.refreshError}</strong><span>{state.error.message}</span></div> : null}
      {mapMeta?.truncated ? <div className={styles.truncated} role="alert" data-testid="task-map-truncated"><strong>{copy.truncated}</strong><span>{copy.limit} {mapMeta.limit_nodes}{locale === "zh" ? "。" : "."}</span></div> : null}

      {state.error && !sourceGraph ? <TaskMapError copy={copy} error={state.error} onRetry={onRetry} /> : null}
      {!state.error && state.loading && !sourceGraph ? <section className={styles.state} data-testid="task-map-loading" role="status"><h2>{copy.loading}</h2><p>{copy.loadingDescription}</p></section> : null}
      {!state.error && !state.loading && sourceGraph && (!visibleGraph || visibleGraph.nodes.length === 0) ? <section className={styles.state} data-testid="task-map-empty" role="status"><h2>{copy.empty}</h2><p>{sourceGraph.nodes.length === 0 ? copy.emptyDescription : copy.filteredEmpty}</p></section> : null}

      {visibleGraph && visibleGraph.nodes.length > 0 ? (
        <div className={styles.layout}>
          <section className={styles.graphPanel} aria-labelledby="task-map-graph-heading">
            <h3 id="task-map-graph-heading" className={styles.visuallyHidden}>{copy.graphHeading}</h3>
            <div className={styles.graphScroll} data-testid="task-map-graph" role="region" aria-label={copy.graphRegion} tabIndex={0}>
              <div className={`${styles.graphCanvas} ${zoomClassName(zoom)}`} data-zoom={clampMapZoom(zoom)}>
                <div className={styles.nodesGrid}>
                  {visibleGraph.nodes.map((node) => {
                    const selected = selectedNode?.task.id === node.task.id
                    return (
                      <article className={selected ? `${styles.node} ${styles.selectedNode}` : styles.node} key={node.task.id} data-testid="task-map-node" data-task-id={node.task.id}>
                        <button type="button" className={styles.nodeButton} data-task-opener={taskOpenerKey(node.task.id)} aria-pressed={selected} aria-label={`${copy.inspectTask} ${node.task.ref} ${node.task.title}`} onClick={() => inspect(node.task.id)}>
                          <span className={styles.nodeTopline}><span translate="no">{node.task.ref}</span><span translate={node.context_only ? undefined : "no"}>{node.context_only ? copy.context : node.role}</span></span>
                          <strong>{node.task.title}</strong>
                          <span className={styles.nodeFacts} translate="no">{node.task.status} · P{node.task.priority}</span>
                        </button>
                      </article>
                    )
                  })}
                </div>
                <div className={styles.edgeList} aria-label={copy.edgesHeading}>
                  <h3>{copy.edgesHeading}</h3>
                  {visibleGraph.edges.length === 0 ? <p>{copy.noEdges}</p> : (
                    <ul>
                      {visibleGraph.edges.map((edge) => <li key={edge.id} data-testid="task-map-edge" data-edge-id={edge.id}><span className={styles.edgeId} translate="no">{edge.id}</span><span translate="no">{edge.source_task_id}</span><span aria-hidden="true">→</span><span translate="no">{edge.target_task_id}</span><span><span translate="no">{edge.kind}</span>{edge.required ? ` · ${copy.required}` : ""}</span></li>)}
                    </ul>
                  )}
                </div>
              </div>
            </div>
          </section>
          <TaskMapInspector copy={copy} node={selectedNode} hiddenSelection={hiddenSelection} onSelectTask={onSelectTask} />
        </div>
      ) : null}
    </section>
  )
}

export function TaskMapView({
  runtime,
  board,
  boardIdentity,
  identityLoading = false,
  identityError = null,
  onRetryIdentity,
  invalidationRevision = 0,
  taskId,
  onSelectTask,
  urlState = defaultTaskMapUrlState,
  onUrlStateChange,
}: TaskMapViewProps) {
  const { locale } = usePreferences()
  const routeIdentity = boardIdentity && boardIdentity.slug === board ? boardIdentity : null
  const mapRead = useTaskMapRead(runtime, board, routeIdentity, urlState.showDoneContext, urlState.hideIsolated, invalidationRevision)
  const fencedData = fenceTaskMapReadModel(board, routeIdentity, mapRead.data)
  const state = useMemo<TaskMapReadState>(() => {
    if (!routeIdentity) return { data: null, loading: identityLoading, error: identityError }
    return { data: fencedData, loading: mapRead.loading || (!fencedData && !mapRead.error), error: identityError ?? mapRead.error }
  }, [fencedData, identityError, identityLoading, mapRead.error, mapRead.loading, routeIdentity])
  const selectedTaskId = urlState.taskId ?? taskId
  const updateUrlState = (next: Partial<TaskMapUrlState>) => onUrlStateChange?.({ ...urlState, ...next })
  const retry = identityError || !routeIdentity ? onRetryIdentity : mapRead.retry

  return (
    <TaskMapPresentation
      board={board}
      locale={locale}
      taskId={selectedTaskId}
      state={state}
      onSelectTask={onSelectTask}
      selectedTaskId={selectedTaskId}
      onInspectTask={onSelectTask}
      filter={urlState.filter}
      hideIsolated={urlState.hideIsolated}
      showDoneContext={urlState.showDoneContext}
      zoom={urlState.zoom}
      onFilterChange={(filter) => updateUrlState({ filter })}
      onHideIsolatedChange={() => updateUrlState({ hideIsolated: !urlState.hideIsolated })}
      onShowDoneContextChange={() => updateUrlState({ showDoneContext: !urlState.showDoneContext })}
      onZoomChange={(direction) => updateUrlState({ zoom: direction === 0 ? 1 : stepMapZoom(urlState.zoom, direction) })}
      onRetry={retry}
    />
  )
}
