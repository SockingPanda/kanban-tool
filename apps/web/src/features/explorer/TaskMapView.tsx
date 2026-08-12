import { useEffect, useMemo, useRef, useState } from "react"

import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { Text } from "@astryxdesign/core/Text"

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
  Grid,
  PageFrame,
  SafeHStack,
  SafeMetadataList,
  SafeMetadataListItem,
  SafeSection,
  SafeVStack,
} from "@/ui/astryx"
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
  readonly online?: boolean
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
  readonly offline: string
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
  readonly layout: string
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
    offline: "当前离线，无法加载关系图。",
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
    graphRegion: "关系图",
    layout: "关系图与任务检查器",
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
    offline: "You are offline; the task map cannot be loaded.",
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
    graphRegion: "Task map",
    layout: "Task map and inspector",
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
  online: boolean,
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
    if (!online) {
      setState((current) => ({
        data: current.identityToken === identityToken ? current.data : null,
        loading: false,
        error: new ExplorerReadError("offline", "当前离线，无法加载关系图。"),
        requestKey,
        identityToken,
      }))
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
  }, [boardIdentity, generation, identityToken, invalidationRevision, key, online, requestKey])

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
    <SafeHStack className="min-w-0 flex-wrap gap-2 border border-border bg-surface p-2" role="toolbar" aria-label={copy.toolbar}>
      <SafeHStack className="min-w-0 max-w-full gap-1 overflow-x-auto" role="group" aria-label={copy.filter}>
        {(Object.keys(copy.filterOptions) as BoardMapFilter[]).map((value) => (
          <Button
            key={value}
            label={copy.filterOptions[value]}
            variant={filter === value ? "primary" : "secondary"}
            size="sm"
            aria-pressed={filter === value}
            onClick={() => onFilterChange(value)}
          />
        ))}
      </SafeHStack>
      <Button label={hideIsolated ? copy.showIsolated : copy.hideIsolated} variant="secondary" size="sm" aria-pressed={hideIsolated} onClick={onHideIsolatedChange} />
      <Button label={showDoneContext ? copy.hideDone : copy.showDone} variant="secondary" size="sm" aria-pressed={showDoneContext} onClick={onShowDoneContextChange} />
      <SafeHStack className="ms-auto gap-1" role="group" aria-label={copy.zoom}>
        <Button label={copy.zoomOut} variant="ghost" size="sm" isDisabled={zoom <= MIN_MAP_ZOOM} onClick={() => onZoomChange(-1)} />
        <output className="text-sm tabular-nums text-primary" data-testid="task-map-zoom" aria-live="polite">{Math.round(zoom * 100)}%</output>
        <Button label={copy.zoomIn} variant="ghost" size="sm" isDisabled={zoom >= MAX_MAP_ZOOM} onClick={() => onZoomChange(1)} />
        <Button label={copy.zoomReset} variant="ghost" size="sm" onClick={() => onZoomChange(0)} />
      </SafeHStack>
      {onRetry ? <Button label={loading ? copy.refreshing : copy.refresh} variant="secondary" size="sm" onClick={onRetry} isDisabled={loading} /> : null}
    </SafeHStack>
  )
}

function TaskMapError({ copy, error, onRetry }: { readonly copy: MapCopy; readonly error: Error; readonly onRetry?: () => void }) {
  const notFound = errorReason(error) === "board-not-found"
  const offline = error instanceof ExplorerReadError && error.kind === "offline"
  return (
    <SafeSection variant="transparent" padding={0} data-testid={offline ? "task-map-offline" : notFound ? "task-map-not-found" : "task-map-error"}>
      <Banner
        status={offline ? "warning" : "error"}
        role={offline ? "status" : "alert"}
        title={offline ? copy.offline : notFound ? copy.notFound : copy.error}
        description={!offline ? error.message : undefined}
        endContent={onRetry ? <Button label={copy.retry} variant="secondary" size="sm" onClick={onRetry} /> : undefined}
      />
    </SafeSection>
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
    return (
      <SafeVStack as="aside" className="min-w-0 gap-3 border border-border bg-surface p-4" data-testid="task-map-inspector">
        <Heading level={2}>{copy.selected}</Heading>
        <Text type="supporting">{copy.selectDescription}</Text>
      </SafeVStack>
    )
  }
  const task = node.task
  return (
    <SafeVStack as="aside" className="min-w-0 gap-3 border border-border bg-surface p-4" data-testid="task-map-inspector" aria-label={copy.selected}>
      <Heading level={2}>{copy.selected}</Heading>
      {hiddenSelection ? <Banner status="warning" role="status" title={copy.hiddenSelection} /> : null}
      <Text type="code"><span translate="no">{task.ref}</span></Text>
      <Heading level={3}>{task.title}</Heading>
      <SafeMetadataList>
        <SafeMetadataListItem label={copy.status}><Text type="code"><span translate="no">{task.status}</span></Text></SafeMetadataListItem>
        <SafeMetadataListItem label={copy.priority}><Text type="code"><span translate="no">P{task.priority}</span></Text></SafeMetadataListItem>
        <SafeMetadataListItem label={copy.plan}><Text type="code"><span translate="no">{task.execution_plan_state}</span></Text></SafeMetadataListItem>
        <SafeMetadataListItem label={copy.requiredSteps}><Text type="code" hasTabularNumbers>{task.completed_required_step_count} / {task.required_step_count}</Text></SafeMetadataListItem>
        <SafeMetadataListItem label={copy.nodeRole}><Text type="code"><span translate="no">{node.role}</span></Text></SafeMetadataListItem>
      </SafeMetadataList>
      <Button label={copy.openInspector} variant="secondary" size="sm" data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)} />
    </SafeVStack>
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
  const offline = state.error instanceof ExplorerReadError && state.error.kind === "offline"

  const header = (
    <SafeVStack className="min-w-0 gap-1 border-b border-border pb-3">
      <Heading level={2} id="task-map-heading">{copy.title}</Heading>
      <Text type="code" color="secondary"><span translate="no">{board}</span>{mapMeta ? <> · {mapMeta.node_count} {copy.nodes} · {mapMeta.edge_count} {copy.edges}</> : null}</Text>
    </SafeVStack>
  )
  const toolbar = (
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
  )
  const frameHeader = (
    <SafeVStack className="min-w-0 gap-3">
      {header}
      {toolbar}
    </SafeVStack>
  )

  return (
    <PageFrame
      frame="workspace"
      data-testid="task-map"
      aria-labelledby="task-map-heading"
      header={frameHeader}
      bodyLabel={copy.graphHeading}
    >
      <SafeVStack className="min-w-0 gap-4">
        {state.error && sourceGraph ? (
          <Banner
            status={offline ? "warning" : "error"}
            role={offline ? "status" : "alert"}
            title={offline ? copy.offline : copy.refreshError}
            description={!offline ? state.error.message : undefined}
            data-testid={offline ? "task-map-offline" : "task-map-refresh-error"}
          />
        ) : null}
        {mapMeta?.truncated ? (
          <Banner status="warning" role="alert" title={copy.truncated} description={`${copy.limit} ${mapMeta.limit_nodes}${locale === "zh" ? "。" : "."}`} data-testid="task-map-truncated" />
        ) : null}

        {state.error && !sourceGraph ? <TaskMapError copy={copy} error={state.error} onRetry={onRetry} /> : null}
        {!state.error && state.loading && !sourceGraph ? (
          <SafeSection variant="transparent" padding={0} data-testid="task-map-loading">
            <Banner status="info" role="status" title={copy.loading} description={copy.loadingDescription} />
          </SafeSection>
        ) : null}
        {!state.error && !state.loading && sourceGraph && (!visibleGraph || visibleGraph.nodes.length === 0) ? (
          <SafeSection variant="transparent" padding={0} data-testid="task-map-empty">
            <Banner status="info" role="status" title={copy.empty} description={sourceGraph.nodes.length === 0 ? copy.emptyDescription : copy.filteredEmpty} />
          </SafeSection>
        ) : null}

        {visibleGraph && visibleGraph.nodes.length > 0 ? (
          <Grid label={copy.layout} columns="responsive-split" gap={4} className="min-w-0" data-testid="task-map-layout">
            <SafeVStack className="min-w-0 gap-2" aria-labelledby="task-map-graph-heading">
              <Heading level={3} id="task-map-graph-heading" className="sr-only">{copy.graphHeading}</Heading>
              <SafeVStack as="div" className="min-w-0 max-h-96 overflow-auto overscroll-contain border border-border bg-body" data-testid="task-map-graph" role="region" aria-label={copy.graphRegion} tabIndex={0}>
                <SafeVStack as="div" className={styles.graphCanvas}>
                  <SafeVStack as="div" className={zoomClassName(zoom)} data-zoom={clampMapZoom(zoom)}>
                    <SafeVStack as="div" className="min-w-0 gap-4 p-4">
                      <Grid label={copy.nodes} columns="auto-md" gap={3} className="min-w-0" data-testid="task-map-nodes">
                        {visibleGraph.nodes.map((node) => {
                          const selected = selectedNode?.task.id === node.task.id
                          return (
                            <article className={selected ? "min-w-0 border border-accent bg-surface ring-2 ring-accent/25" : "min-w-0 border border-border bg-surface"} key={node.task.id} data-testid="task-map-node" data-task-id={node.task.id}>
                              <Button
                                label={`${copy.inspectTask} ${node.task.ref} ${node.task.title}`}
                                variant="ghost"
                                size="sm"
                                className="h-auto w-full justify-start text-start"
                                data-task-opener={taskOpenerKey(node.task.id)}
                                aria-pressed={selected}
                                onClick={() => inspect(node.task.id)}
                              >
                                <SafeVStack as="div" className="min-w-0 gap-1 text-start">
                                  <SafeHStack as="div" justify="between" className="min-w-0 gap-2">
                                    <Text as="span" type="code"><span translate="no">{node.task.ref}</span></Text>
                                    <Text as="span" type="supporting">{node.context_only ? copy.context : <span translate="no">{node.role}</span>}</Text>
                                  </SafeHStack>
                                  <Text as="span" type="label">{node.task.title}</Text>
                                  <Text as="span" type="code" color="secondary"><span translate="no">{node.task.status} · P{node.task.priority}</span></Text>
                                </SafeVStack>
                              </Button>
                            </article>
                          )
                        })}
                      </Grid>
                      <SafeSection variant="transparent" padding={0} aria-labelledby="task-map-edges-heading">
                        <Heading level={3} id="task-map-edges-heading">{copy.edgesHeading}</Heading>
                        {visibleGraph.edges.length === 0 ? <Text as="p" type="supporting">{copy.noEdges}</Text> : (
                          <List density="compact" hasDividers>
                            {visibleGraph.edges.map((edge) => (
                              <ListItem
                                key={edge.id}
                                data-testid="task-map-edge"
                                data-edge-id={edge.id}
                                label={<Text type="code"><span translate="no">{edge.id}</span></Text>}
                                endContent={<Text type="supporting"><span translate="no">{edge.source_task_id} → {edge.target_task_id} · {edge.kind}</span>{edge.required ? ` · ${copy.required}` : ""}</Text>}
                              />
                            ))}
                          </List>
                        )}
                      </SafeSection>
                    </SafeVStack>
                  </SafeVStack>
                </SafeVStack>
              </SafeVStack>
            </SafeVStack>
            <TaskMapInspector copy={copy} node={selectedNode} hiddenSelection={hiddenSelection} onSelectTask={onSelectTask} />
          </Grid>
        ) : null}
      </SafeVStack>
    </PageFrame>
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
  online = typeof navigator === "undefined" || navigator.onLine,
  taskId,
  onSelectTask,
  urlState = defaultTaskMapUrlState,
  onUrlStateChange,
}: TaskMapViewProps) {
  const { locale } = usePreferences()
  const routeIdentity = boardIdentity && boardIdentity.slug === board ? boardIdentity : null
  const mapRead = useTaskMapRead(runtime, board, routeIdentity, urlState.showDoneContext, urlState.hideIsolated, invalidationRevision, online)
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
