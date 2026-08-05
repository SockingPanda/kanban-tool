import { AlertTriangle, EyeOff, ListFilter, Loader2, Minus, Network, Plus, RefreshCcw, RotateCcw } from "lucide-react"
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { MetricStrip, PageToolbar, PriorityBadge, SectionCard, TaskIdentityLine, TaskStatusBadge } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { useI18n } from "@/i18n"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import type { BoardTaskMap, KanbanApi, Task, TaskGraphNode as ApiTaskGraphNode } from "@/lib/api"
import { presentApiError } from "@/lib/api/error-presentation"
import { cn } from "@/lib/utils"

import { apiTaskGraphToCanvasGraph } from "./task-graph-adapter"
import { useBoardTaskMap } from "./useBoardTaskMap"

type BoardMapFilter = "all" | "blocked" | "ready" | "running" | "unplanned" | "incomplete-steps"

const MIN_MAP_ZOOM = 0.65
const MAX_MAP_ZOOM = 1.5
const MAP_ZOOM_STEP = 0.15
const TaskGraphCanvas = lazy(() => import("./TaskGraphCanvas").then((module) => ({ default: module.TaskGraphCanvas })))

export function BoardTaskMapView({
  api,
  selectedTaskId,
  onSelectTask,
}: {
  api: KanbanApi | null
  selectedTaskId: string | null
  onSelectTask: (taskId: string) => void
}) {
  const { t } = useI18n()
  const [filter, setFilter] = useState<BoardMapFilter>("all")
  const [showDoneContext, setShowDoneContext] = useState(false)
  const [hideIsolated, setHideIsolated] = useState(false)
  const [zoom, setZoom] = useState(1)
  const [inspectedTaskId, setInspectedTaskId] = useState<string | null>(selectedTaskId)
  const mapQuery = useBoardTaskMap(api, { includeDoneContext: showDoneContext, hideIsolated })
  const sourceGraph = mapQuery.data ?? null
  const visibleGraph = useMemo(
    () => apiTaskGraphToCanvasGraph(filterBoardMap(sourceGraph, filter, hideIsolated)),
    [filter, hideIsolated, sourceGraph],
  )
  const selectedNode = useMemo(() => resolveBoardMapSelectedNode(sourceGraph, inspectedTaskId, selectedTaskId), [
    inspectedTaskId,
    selectedTaskId,
    sourceGraph,
  ])
  const relationCountByTaskId = useMemo(() => buildRelationCountIndex(sourceGraph), [sourceGraph])
  const inspectTask = useCallback((taskId: string) => setInspectedTaskId(taskId), [])
  const hiddenSelection = Boolean(selectedNode && !visibleGraph.nodes.some((node) => node.id === selectedNode.task.id))
  const zoomLabel = `${Math.round(zoom * 100)}%`
  const filterOptions = useMemo<{ value: BoardMapFilter; label: string }[]>(
    () => [
      { value: "all", label: t("All active") },
      { value: "blocked", label: t("Blocked") },
      { value: "ready", label: t("Ready now") },
      { value: "running", label: t("Running") },
      { value: "unplanned", label: t("Unplanned steps") },
      { value: "incomplete-steps", label: t("Incomplete steps") },
    ],
    [t],
  )

  useEffect(() => {
    if (selectedTaskId) setInspectedTaskId(selectedTaskId)
  }, [selectedTaskId])

  if (!api) {
    return (
      <div className="p-4">
        <Empty>
          <EmptyDescription>{t("API client is not ready.")}</EmptyDescription>
        </Empty>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex min-h-0 flex-1 flex-col gap-3 p-3 lg:flex-row lg:p-4">
        <section className="flex min-w-0 flex-1 flex-col gap-3">
          <PageToolbar className="rounded-md border border-border bg-card">
            <div className="flex min-w-0 flex-wrap items-center gap-1.5">
              {filterOptions.map((option) => (
                <Button
                  key={option.value}
                  type="button"
                  variant={filter === option.value ? "secondary" : "outline"}
                  size="sm"
                  aria-pressed={filter === option.value}
                  onClick={() => setFilter(option.value)}
                >
                  {option.label}
                </Button>
              ))}
            </div>
            <Separator orientation="vertical" className="hidden h-6 sm:block" />
            <Button
              type="button"
              variant={hideIsolated ? "secondary" : "outline"}
              size="sm"
              aria-pressed={hideIsolated}
              onClick={() => setHideIsolated((current) => !current)}
            >
              <EyeOff className="h-4 w-4" />
              {t("Hide isolated")}
            </Button>
            <Button
              type="button"
              variant={showDoneContext ? "secondary" : "outline"}
              size="sm"
              aria-pressed={showDoneContext}
              onClick={() => setShowDoneContext((current) => !current)}
            >
              {t("Show done context")}
            </Button>
            <Separator orientation="vertical" className="hidden h-6 sm:block" />
            <div className="flex items-center gap-1 rounded-md border border-border bg-background p-0.5">
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t("Zoom out task map")}
                title={t("Zoom out")}
                disabled={zoom <= MIN_MAP_ZOOM}
                onClick={() => setZoom((current) => stepMapZoom(current, -1))}
              >
                <Minus className="h-4 w-4" />
              </Button>
              <span className="w-12 text-center text-xs tabular-nums text-muted-foreground">{zoomLabel}</span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t("Zoom in task map")}
                title={t("Zoom in")}
                disabled={zoom >= MAX_MAP_ZOOM}
                onClick={() => setZoom((current) => stepMapZoom(current, 1))}
              >
                <Plus className="h-4 w-4" />
              </Button>
              <Button type="button" variant="ghost" size="icon" aria-label={t("Reset task map zoom")} title={t("Reset zoom")} onClick={() => setZoom(1)}>
                <RotateCcw className="h-4 w-4" />
              </Button>
            </div>
            <Button type="button" variant="outline" size="sm" disabled={mapQuery.isFetching} onClick={() => void mapQuery.refetch()}>
              {mapQuery.isFetching ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCcw className="h-4 w-4" />}
              {t("Refresh")}
            </Button>
          </PageToolbar>

          {mapQuery.error ? (
            <Alert className="border-destructive/50 bg-destructive/5">
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>{t("Map failed")}</AlertTitle>
              <AlertDescription>{presentApiError(mapQuery.error, t)}</AlertDescription>
            </Alert>
          ) : null}
          {sourceGraph?.meta.truncated ? (
            <Alert>
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>{t("Graph truncated")}</AlertTitle>
              <AlertDescription>
                {t("Showing {nodes} nodes and {edges} edges within the current node cap.", {
                  nodes: sourceGraph.meta.node_count,
                  edges: sourceGraph.meta.edge_count,
                })}
              </AlertDescription>
            </Alert>
          ) : null}

          <div className="min-h-0 flex-1">
            {mapQuery.isLoading ? (
              <Card className="flex h-full min-h-[420px] items-center justify-center p-6 text-sm text-muted-foreground">
                <Loader2 className="mr-2 h-4 w-4 animate-spin" /> {t("Loading task map")}
              </Card>
            ) : visibleGraph.nodes.length ? (
              <Suspense fallback={<TaskMapSkeleton label={t("Loading graph renderer")} className="h-full min-h-[520px]" />}>
                <TaskGraphCanvas
                  graph={visibleGraph}
                  selectedTaskId={selectedNode?.task.id ?? inspectedTaskId ?? selectedTaskId}
                  onSelectTask={inspectTask}
                  onOpenTask={onSelectTask}
                  mode="board-map"
                  scale={zoom}
                  className="h-full min-h-[520px]"
                />
              </Suspense>
            ) : (
              <Empty className="h-full min-h-[420px] rounded-md border border-border bg-muted/20">
                <EmptyDescription>{t("No tasks match the current map filter.")}</EmptyDescription>
              </Empty>
            )}
          </div>
        </section>

        <MapInspector
          node={selectedNode}
          relationCountByTaskId={relationCountByTaskId}
          hiddenSelection={hiddenSelection}
          onOpenTask={onSelectTask}
        />
      </div>
    </div>
  )
}

function resolveBoardMapSelectedNode(graph: BoardTaskMap | null, inspectedTaskId: string | null, selectedTaskId: string | null) {
  if (!graph) return null
  return (
    graph.nodes.find((node) => node.task.id === inspectedTaskId) ??
    graph.nodes.find((node) => node.task.id === selectedTaskId) ??
    graph.nodes.find((node) => !node.context_only) ??
    null
  )
}

function filterBoardMap(
  graph: BoardTaskMap | null,
  filter: BoardMapFilter,
  hideIsolated: boolean,
): Pick<BoardTaskMap, "nodes" | "edges"> | null {
  if (!graph) return null
  const matchingIds = new Set(graph.nodes.filter((node) => nodeMatchesFilter(node, filter)).map((node) => node.task.id))
  const edges = graph.edges.filter((edge) => matchingIds.has(edge.source_task_id) && matchingIds.has(edge.target_task_id))
  if (!hideIsolated) {
    return { nodes: graph.nodes.filter((node) => matchingIds.has(node.task.id)), edges }
  }
  const connected = new Set<string>()
  for (const edge of edges) {
    connected.add(edge.source_task_id)
    connected.add(edge.target_task_id)
  }
  return { nodes: graph.nodes.filter((node) => matchingIds.has(node.task.id) && connected.has(node.task.id)), edges }
}

function nodeMatchesFilter(node: ApiTaskGraphNode, filter: BoardMapFilter) {
  if (filter === "all") return !node.task.archived_at
  if (filter === "blocked") return (node.task.status === "blocked" || node.task.dependency_blocked) && !node.context_only
  if (filter === "ready") return node.task.status === "ready" && !node.context_only
  if (filter === "running") return node.task.status === "running" && !node.context_only
  if (filter === "unplanned") return node.task.execution_plan_state === "unplanned" && !node.context_only
  return incompleteRequiredSteps(node.task) > 0 && !node.context_only
}

function stepMapZoom(current: number, direction: -1 | 1) {
  return clampMapZoom(Number((current + direction * MAP_ZOOM_STEP).toFixed(2)))
}

function clampMapZoom(value: number) {
  if (!Number.isFinite(value)) return 1
  return Math.min(MAX_MAP_ZOOM, Math.max(MIN_MAP_ZOOM, value))
}

function MapInspector({
  node,
  relationCountByTaskId,
  hiddenSelection,
  onOpenTask,
}: {
  node: ApiTaskGraphNode | null
  relationCountByTaskId: Map<string, RelationCounts>
  hiddenSelection: boolean
  onOpenTask: (taskId: string) => void
}) {
  const { t } = useI18n()
  const task = node?.task ?? null
  const counts = (task ? relationCountByTaskId.get(task.id) : null) ?? emptyRelationCounts()
  return (
    <aside className="min-w-0 shrink-0 lg:w-80">
      <SectionCard title={t("Inspector")} icon={Network}>
        {!task ? (
          <Empty className="items-start p-0 text-left">
            <EmptyDescription>{t("Select a map node to inspect it.")}</EmptyDescription>
          </Empty>
        ) : (
          <div className="space-y-4">
            {hiddenSelection ? (
              <Badge variant="secondary">
                <ListFilter className="h-3.5 w-3.5" /> {t("hidden by filter")}
              </Badge>
            ) : null}
            <div className="space-y-2">
              <TaskIdentityLine id={task.id} ref={task.ref} seq={task.seq} title={task.title} />
              <div className="flex flex-wrap gap-1.5">
                <TaskStatusBadge status={task.status} />
                <PriorityBadge priority={task.priority} />
                {node?.context_only ? <Badge variant="secondary">{t("context")}</Badge> : null}
              </div>
            </div>
            <Separator />
            <MetricStrip
              className="grid-cols-2 text-sm"
              items={[
                { id: "plan", label: t("Plan"), value: task.execution_plan_state },
                { id: "required-open", label: t("Required open"), value: String(incompleteRequiredSteps(task)) },
                { id: "parents", label: t("Parents"), value: String(counts.parents) },
                { id: "children", label: t("Children"), value: String(counts.children) },
                { id: "steps", label: t("Steps"), value: String(counts.steps) },
                { id: "blocked-by", label: t("Blocked by"), value: String(task.unfinished_parent_count) },
              ]}
            />
            <Button type="button" className="w-full" onClick={() => onOpenTask(task.id)}>
              {t("Open detail")}
            </Button>
          </div>
        )}
      </SectionCard>
    </aside>
  )
}

type RelationCounts = { parents: number; children: number; steps: number }

function buildRelationCountIndex(graph: BoardTaskMap | null) {
  const countsByTaskId = new Map<string, RelationCounts>()
  if (!graph) return countsByTaskId
  for (const edge of graph.edges) {
    if (edge.kind === "step") {
      relationCountsFor(countsByTaskId, edge.source_task_id).steps += 1
    } else {
      relationCountsFor(countsByTaskId, edge.target_task_id).parents += 1
      relationCountsFor(countsByTaskId, edge.source_task_id).children += 1
    }
  }
  return countsByTaskId
}

function relationCountsFor(countsByTaskId: Map<string, RelationCounts>, taskId: string) {
  const existing = countsByTaskId.get(taskId)
  if (existing) return existing
  const counts = emptyRelationCounts()
  countsByTaskId.set(taskId, counts)
  return counts
}

function emptyRelationCounts(): RelationCounts {
  return { parents: 0, children: 0, steps: 0 }
}

function TaskMapSkeleton({ label, className }: { label: string; className?: string }) {
  return (
    <Card className={cn("space-y-3 p-4", className)}>
      <div className="text-sm text-muted-foreground">{label}</div>
      <Skeleton className="h-24 w-3/4" />
      <Skeleton className="ml-auto h-24 w-2/3" />
      <Skeleton className="h-24 w-1/2" />
    </Card>
  )
}

function incompleteRequiredSteps(task: Task) {
  return Math.max(0, task.required_step_count - task.completed_required_step_count)
}

export const __test = { buildRelationCountIndex, clampMapZoom, filterBoardMap, resolveBoardMapSelectedNode, stepMapZoom }
