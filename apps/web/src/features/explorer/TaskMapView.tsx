import { useEffect, useMemo, useRef, useState } from "react"

import {
  ExplorerReadError,
  loadTaskMap,
  type ExplorerTaskMapReadModel,
} from "../../lib/api/explorer-read-model"
import type { WebRuntimeConfig } from "../../lib/runtime"
import {
  filterTaskMap,
  MAX_MAP_ZOOM,
  MIN_MAP_ZOOM,
  resolveSelectedNode,
  stepMapZoom,
  type BoardMapFilter,
} from "./TaskMapView.logic"
import styles from "./TaskMapView.module.css"

export type TaskMapReadState = {
  readonly data: ExplorerTaskMapReadModel | null
  readonly loading: boolean
  readonly error: ExplorerReadError | Error | null
}

export interface TaskMapViewProps {
  readonly runtime: WebRuntimeConfig
  readonly board: string
  readonly taskId: string | null
  readonly onSelectTask: (taskId: string) => void
}

export interface TaskMapPresentationProps {
  readonly board: string
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

const MAP_LIMIT_NODES = 240

const filterOptions: readonly { readonly value: BoardMapFilter; readonly label: string }[] = [
  { value: "all", label: "全部活动" },
  { value: "blocked", label: "阻塞" },
  { value: "ready", label: "可执行" },
  { value: "running", label: "运行中" },
  { value: "unplanned", label: "未规划" },
  { value: "incomplete-steps", label: "未完成步骤" },
]

function errorReason(error: Error | null): string | null {
  if (!error || !("reason" in error)) return null
  const reason = error.reason
  return typeof reason === "string" ? reason : null
}

function useTaskMapRead(
  runtime: WebRuntimeConfig,
  board: string,
  includeDoneContext: boolean,
  hideIsolated: boolean,
): TaskMapReadState & { readonly retry: () => void } {
  const loadRef = useRef<((signal: AbortSignal) => Promise<ExplorerTaskMapReadModel>) | null>(null)
  loadRef.current = (signal) => loadTaskMap(runtime, board, {
    activeOnly: true,
    contextDepth: 1,
    includeDoneContext,
    includeArchivedContext: false,
    hideIsolated,
    limitNodes: MAP_LIMIT_NODES,
    signal,
  })
  const [generation, setGeneration] = useState(0)
  const [state, setState] = useState<TaskMapReadState>({ data: null, loading: false, error: null })
  const key = `${board}|${includeDoneContext ? "done" : "active"}|${hideIsolated ? "connected" : "all"}`

  useEffect(() => {
    const controller = new AbortController()
    let active = true
    setState((current) => ({ data: current.data, loading: true, error: null }))
    void loadRef.current?.(controller.signal).then(
      (data) => {
        if (active) setState({ data, loading: false, error: null })
      },
      (error: unknown) => {
        if (active && !(error instanceof Error && error.name === "AbortError")) {
          setState((current) => ({
            data: current.data,
            loading: false,
            error: error instanceof Error ? error : new Error(String(error)),
          }))
        }
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [generation, key])

  return { ...state, retry: () => setGeneration((current) => current + 1) }
}

function TaskMapToolbar({
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
    <div className={styles.toolbar} aria-label="关系图筛选与缩放">
      <div className={styles.filterGroup} role="group" aria-label="关系图筛选">
        {filterOptions.map((option) => (
          <button
            key={option.value}
            type="button"
            className={styles.filterButton}
            aria-pressed={filter === option.value}
            onClick={() => onFilterChange(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
      <button type="button" className={styles.toggleButton} aria-pressed={hideIsolated} onClick={onHideIsolatedChange}>
        {hideIsolated ? "显示孤立节点" : "隐藏孤立节点"}
      </button>
      <button type="button" className={styles.toggleButton} aria-pressed={showDoneContext} onClick={onShowDoneContextChange}>
        {showDoneContext ? "隐藏完成上下文" : "显示完成上下文"}
      </button>
      <div className={styles.zoomGroup} role="group" aria-label="关系图缩放">
        <button type="button" aria-label="缩小关系图" onClick={() => onZoomChange(-1)} disabled={zoom <= MIN_MAP_ZOOM}>−</button>
        <output data-testid="task-map-zoom" aria-live="polite">{Math.round(zoom * 100)}%</output>
        <button type="button" aria-label="放大关系图" onClick={() => onZoomChange(1)} disabled={zoom >= MAX_MAP_ZOOM}>＋</button>
        <button type="button" aria-label="重置关系图缩放" onClick={() => onZoomChange(0)}>重置</button>
      </div>
      {onRetry ? <button type="button" className={styles.refreshButton} onClick={onRetry} disabled={loading}>{loading ? "正在刷新…" : "刷新"}</button> : null}
    </div>
  )
}

function TaskMapError({ error, onRetry }: { readonly error: Error; readonly onRetry?: () => void }) {
  const notFound = errorReason(error) === "board-not-found"
  return (
    <section className={styles.state} data-testid={notFound ? "task-map-not-found" : "task-map-error"} role="alert">
      <h2>{notFound ? "看板不存在" : "关系图加载失败"}</h2>
      <p>{error.message}</p>
      {onRetry ? <button type="button" onClick={onRetry}>重试</button> : null}
    </section>
  )
}

function TaskMapInspector({
  node,
  hiddenSelection,
  onSelectTask,
}: {
  readonly node: MapNode | null
  readonly hiddenSelection: boolean
  readonly onSelectTask: (taskId: string) => void
}) {
  if (!node) {
    return <aside className={styles.inspector} data-testid="task-map-inspector"><h2>当前选择</h2><p>选择一个关系图节点查看任务。</p></aside>
  }
  const task = node.task
  return (
    <aside className={styles.inspector} data-testid="task-map-inspector" aria-label="关系图节点 Inspector">
      <h2>当前选择</h2>
      {hiddenSelection ? <p className={styles.hiddenSelection} role="status">当前节点被筛选隐藏。</p> : null}
      <p className={styles.nodeRef} translate="no">{task.ref}</p>
      <h3>{task.title}</h3>
      <dl className={styles.facts}>
        <div><dt>状态</dt><dd translate="no">{task.status}</dd></div>
        <div><dt>优先级</dt><dd>P{task.priority}</dd></div>
        <div><dt>计划</dt><dd translate="no">{task.execution_plan_state}</dd></div>
        <div><dt>必需步骤</dt><dd>{task.completed_required_step_count} / {task.required_step_count}</dd></div>
        <div><dt>节点角色</dt><dd translate="no">{node.role}</dd></div>
      </dl>
      <button type="button" className={styles.openButton} onClick={() => onSelectTask(task.id)}>打开 Inspector</button>
    </aside>
  )
}

export function TaskMapPresentation({
  board,
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
          <p className={styles.eyebrow}>TASK MAP</p>
          <h1 id="task-map-heading">任务关系图</h1>
          <p className={styles.muted} translate="no">{board}{mapMeta ? ` · ${mapMeta.node_count} nodes · ${mapMeta.edge_count} edges` : ""}</p>
        </div>
      </header>

      <TaskMapToolbar
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

      {state.error && sourceGraph ? <div className={styles.inlineError} role="alert" data-testid="task-map-refresh-error"><strong>关系图刷新失败</strong><span>{state.error.message}</span></div> : null}
      {mapMeta?.truncated ? <div className={styles.truncated} role="alert" data-testid="task-map-truncated"><strong>关系图已截断</strong><span>当前节点上限为 {mapMeta.limit_nodes}。</span></div> : null}

      {state.error && !sourceGraph ? <TaskMapError error={state.error} onRetry={onRetry} /> : null}
      {!state.error && state.loading && !sourceGraph ? <section className={styles.state} data-testid="task-map-loading" role="status"><h2>正在加载关系图…</h2><p>正在读取当前看板的任务关系。</p></section> : null}
      {!state.error && !state.loading && sourceGraph && (!visibleGraph || visibleGraph.nodes.length === 0) ? <section className={styles.state} data-testid="task-map-empty" role="status"><h2>暂无关系图节点</h2><p>{sourceGraph.nodes.length === 0 ? "当前看板还没有可展示的任务关系。" : "没有任务匹配当前筛选。"}</p></section> : null}

      {visibleGraph && visibleGraph.nodes.length > 0 ? (
        <div className={styles.layout}>
          <section className={styles.graphPanel} aria-labelledby="task-map-graph-heading">
            <h2 id="task-map-graph-heading" className={styles.visuallyHidden}>任务关系节点</h2>
            <div className={styles.graphScroll} data-testid="task-map-graph" role="region" aria-label="任务关系图，可横向滚动" tabIndex={0}>
              <div className={styles.graphCanvas} style={{ transform: `scale(${zoom})`, transformOrigin: "top left" }}>
                <div className={styles.nodesGrid}>
                  {visibleGraph.nodes.map((node) => {
                    const selected = selectedNode?.task.id === node.task.id
                    return (
                      <article className={selected ? `${styles.node} ${styles.selectedNode}` : styles.node} key={node.task.id} data-testid="task-map-node" data-task-id={node.task.id}>
                        <button type="button" className={styles.nodeButton} aria-pressed={selected} aria-label={`检查任务 ${node.task.ref} ${node.task.title}`} onClick={() => inspect(node.task.id)}>
                          <span className={styles.nodeTopline}><span translate="no">{node.task.ref}</span><span>{node.context_only ? "上下文" : node.role}</span></span>
                          <strong>{node.task.title}</strong>
                          <span className={styles.nodeFacts} translate="no">{node.task.status} · P{node.task.priority}</span>
                        </button>
                      </article>
                    )
                  })}
                </div>
                <div className={styles.edgeList} aria-label="关系边">
                  <h3>关系边</h3>
                  {visibleGraph.edges.length === 0 ? <p>当前筛选没有关系边。</p> : (
                    <ul>
                      {visibleGraph.edges.map((edge) => <li key={edge.id} data-testid="task-map-edge" data-edge-id={edge.id}><span className={styles.edgeId} translate="no">{edge.id}</span><span translate="no">{edge.source_task_id}</span><span aria-hidden="true">→</span><span translate="no">{edge.target_task_id}</span><span>{edge.kind}{edge.required ? " · required" : ""}</span></li>)}
                    </ul>
                  )}
                </div>
              </div>
            </div>
          </section>
          <TaskMapInspector node={selectedNode} hiddenSelection={hiddenSelection} onSelectTask={onSelectTask} />
        </div>
      ) : null}
    </section>
  )
}

export function TaskMapView({ runtime, board, taskId, onSelectTask }: TaskMapViewProps) {
  const [filter, setFilter] = useState<BoardMapFilter>("all")
  const [showDoneContext, setShowDoneContext] = useState(false)
  const [hideIsolated, setHideIsolated] = useState(false)
  const [zoom, setZoom] = useState(1)
  const [inspectedTaskId, setInspectedTaskId] = useState<string | null>(taskId)
  const mapRead = useTaskMapRead(runtime, board, showDoneContext, hideIsolated)

  useEffect(() => {
    if (taskId) setInspectedTaskId(taskId)
  }, [taskId])

  const handleZoom = (direction: -1 | 0 | 1) => setZoom((current) => direction === 0 ? 1 : stepMapZoom(current, direction))
  const state = useMemo<TaskMapReadState>(() => ({ data: mapRead.data, loading: mapRead.loading, error: mapRead.error }), [mapRead.data, mapRead.error, mapRead.loading])

  return (
    <TaskMapPresentation
      board={board}
      taskId={taskId}
      state={state}
      onSelectTask={onSelectTask}
      selectedTaskId={inspectedTaskId}
      onInspectTask={setInspectedTaskId}
      filter={filter}
      hideIsolated={hideIsolated}
      showDoneContext={showDoneContext}
      zoom={zoom}
      onFilterChange={setFilter}
      onHideIsolatedChange={() => setHideIsolated((current) => !current)}
      onShowDoneContextChange={() => setShowDoneContext((current) => !current)}
      onZoomChange={handleZoom}
      onRetry={mapRead.retry}
    />
  )
}
