import { AlertTriangle, EyeOff, ListFilter, Loader2, Minus, Network, Plus, RefreshCcw, RotateCcw } from "lucide-react"
import { useMemo, useState } from "react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Separator } from "@/components/ui/separator"
import type { BoardTaskMap, KanbanApi, Task, TaskGraphNode as ApiTaskGraphNode } from "@/lib/api"
import { priorityBadgeClass, priorityLabel } from "@/lib/priority"
import { shortId } from "@/lib/utils"

import { TaskGraphCanvas } from "./TaskGraphCanvas"
import { apiTaskGraphToCanvasGraph } from "./task-graph-adapter"
import { useBoardTaskMap } from "./useBoardTaskMap"

type BoardMapFilter = "all" | "blocked" | "ready" | "running" | "unplanned" | "incomplete-steps"

const MIN_MAP_ZOOM = 0.65
const MAX_MAP_ZOOM = 1.5
const MAP_ZOOM_STEP = 0.15

const filterOptions: { value: BoardMapFilter; label: string }[] = [
  { value: "all", label: "All active" },
  { value: "blocked", label: "Blocked" },
  { value: "ready", label: "Ready now" },
  { value: "running", label: "Running" },
  { value: "unplanned", label: "Unplanned steps" },
  { value: "incomplete-steps", label: "Incomplete steps" },
]

export function BoardTaskMapView({
  api,
  selectedTaskId,
  onSelectTask,
}: {
  api: KanbanApi | null
  selectedTaskId: string | null
  onSelectTask: (taskId: string) => void
}) {
  const [filter, setFilter] = useState<BoardMapFilter>("all")
  const [showDoneContext, setShowDoneContext] = useState(false)
  const [hideIsolated, setHideIsolated] = useState(false)
  const [zoom, setZoom] = useState(1)
  const mapQuery = useBoardTaskMap(api, { includeDoneContext: showDoneContext })
  const sourceGraph = mapQuery.data ?? null
  const visibleGraph = useMemo(
    () => apiTaskGraphToCanvasGraph(filterBoardMap(sourceGraph, filter, hideIsolated)),
    [filter, hideIsolated, sourceGraph],
  )
  const selectedNode = useMemo(
    () =>
      sourceGraph?.nodes.find((node) => node.task.id === selectedTaskId) ??
      sourceGraph?.nodes.find((node) => !node.context_only) ??
      null,
    [selectedTaskId, sourceGraph],
  )
  const hiddenSelection = Boolean(selectedNode && !visibleGraph.nodes.some((node) => node.id === selectedNode.task.id))
  const zoomLabel = `${Math.round(zoom * 100)}%`

  if (!api) {
    return (
      <div className="p-4">
        <Empty>
          <EmptyDescription>API client is not ready.</EmptyDescription>
        </Empty>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex min-h-0 flex-1 flex-col gap-3 p-3 lg:flex-row lg:p-4">
        <section className="flex min-w-0 flex-1 flex-col gap-3">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
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
              Hide isolated
            </Button>
            <Button
              type="button"
              variant={showDoneContext ? "secondary" : "outline"}
              size="sm"
              aria-pressed={showDoneContext}
              onClick={() => setShowDoneContext((current) => !current)}
            >
              Show done context
            </Button>
            <Separator orientation="vertical" className="hidden h-6 sm:block" />
            <div className="flex items-center gap-1 rounded-md border border-border bg-background p-0.5">
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label="Zoom out task map"
                title="Zoom out"
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
                aria-label="Zoom in task map"
                title="Zoom in"
                disabled={zoom >= MAX_MAP_ZOOM}
                onClick={() => setZoom((current) => stepMapZoom(current, 1))}
              >
                <Plus className="h-4 w-4" />
              </Button>
              <Button type="button" variant="ghost" size="icon" aria-label="Reset task map zoom" title="Reset zoom" onClick={() => setZoom(1)}>
                <RotateCcw className="h-4 w-4" />
              </Button>
            </div>
            <Button type="button" variant="outline" size="sm" disabled={mapQuery.isFetching} onClick={() => void mapQuery.refetch()}>
              {mapQuery.isFetching ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCcw className="h-4 w-4" />}
              Refresh
            </Button>
          </div>

          {mapQuery.error ? (
            <Alert className="border-destructive/50 bg-destructive/5">
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>Map failed</AlertTitle>
              <AlertDescription>{errorMessage(mapQuery.error)}</AlertDescription>
            </Alert>
          ) : null}
          {sourceGraph?.meta.truncated ? (
            <Alert>
              <AlertTriangle className="h-4 w-4" />
              <AlertTitle>Graph truncated</AlertTitle>
              <AlertDescription>
                Showing {sourceGraph.meta.node_count} nodes and {sourceGraph.meta.edge_count} edges within the current node cap.
              </AlertDescription>
            </Alert>
          ) : null}

          <div className="min-h-0 flex-1">
            {mapQuery.isLoading ? (
              <Card className="flex h-full min-h-[420px] items-center justify-center p-6 text-sm text-muted-foreground">
                <Loader2 className="mr-2 h-4 w-4 animate-spin" /> Loading task map
              </Card>
            ) : visibleGraph.nodes.length ? (
              <TaskGraphCanvas
                graph={visibleGraph}
                selectedTaskId={selectedNode?.task.id ?? selectedTaskId}
                onSelectTask={onSelectTask}
                mode="board-map"
                scale={zoom}
                className="h-full min-h-[520px]"
              />
            ) : (
              <Empty className="h-full min-h-[420px] rounded-md border border-border bg-muted/20">
                <EmptyDescription>No tasks match the current map filter.</EmptyDescription>
              </Empty>
            )}
          </div>
        </section>

        <MapInspector node={selectedNode} graph={sourceGraph} hiddenSelection={hiddenSelection} onSelectTask={onSelectTask} />
      </div>
    </div>
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
  graph,
  hiddenSelection,
  onSelectTask,
}: {
  node: ApiTaskGraphNode | null
  graph: BoardTaskMap | null
  hiddenSelection: boolean
  onSelectTask: (taskId: string) => void
}) {
  const task = node?.task ?? null
  const counts = task && graph ? relationCounts(graph, task.id) : { parents: 0, children: 0, steps: 0 }
  return (
    <aside className="min-w-0 shrink-0 lg:w-80">
      <Card className="space-y-4 p-3">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Network className="h-4 w-4" />
          Inspector
        </div>
        {!task ? (
          <Empty className="items-start p-0 text-left">
            <EmptyDescription>Select a map node to inspect it.</EmptyDescription>
          </Empty>
        ) : (
          <div className="space-y-4">
            {hiddenSelection ? (
              <Badge variant="secondary">
                <ListFilter className="h-3.5 w-3.5" /> hidden by filter
              </Badge>
            ) : null}
            <div className="space-y-2">
              <div className="text-xs text-muted-foreground">
                {task.ref} · {shortId(task.id)}
              </div>
              <div className="break-words text-sm font-semibold">{task.title}</div>
              <div className="flex flex-wrap gap-1.5">
                <Badge variant={badgeVariant(task.status)}>{task.status}</Badge>
                <Badge variant="secondary" className={priorityBadgeClass(task.priority)}>
                  {priorityLabel(task.priority)}
                </Badge>
                {node?.context_only ? <Badge variant="secondary">context</Badge> : null}
              </div>
            </div>
            <Separator />
            <div className="grid grid-cols-2 gap-2 text-sm">
              <InfoTile label="Plan" value={task.execution_plan_state} />
              <InfoTile label="Required open" value={String(incompleteRequiredSteps(task))} />
              <InfoTile label="Parents" value={String(counts.parents)} />
              <InfoTile label="Children" value={String(counts.children)} />
              <InfoTile label="Steps" value={String(counts.steps)} />
              <InfoTile label="Blocked by" value={String(task.unfinished_parent_count)} />
            </div>
            <Button type="button" className="w-full" onClick={() => onSelectTask(task.id)}>
              Open detail
            </Button>
          </div>
        )}
      </Card>
    </aside>
  )
}

function relationCounts(graph: BoardTaskMap, taskId: string) {
  return graph.edges.reduce(
    (counts, edge) => {
      if (edge.kind === "step" && edge.source_task_id === taskId) counts.steps += 1
      if (edge.kind === "dependency" && edge.target_task_id === taskId) counts.parents += 1
      if (edge.kind === "dependency" && edge.source_task_id === taskId) counts.children += 1
      return counts
    },
    { parents: 0, children: 0, steps: 0 },
  )
}

function incompleteRequiredSteps(task: Task) {
  return Math.max(0, task.required_step_count - task.completed_required_step_count)
}

function InfoTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-muted/30 p-2">
      <div className="text-[10px] uppercase tracking-normal text-muted-foreground">{label}</div>
      <div className="mt-1 truncate text-sm font-medium">{value}</div>
    </div>
  )
}

function badgeVariant(status: string) {
  if (status === "ready") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  if (status === "done") return "secondary"
  return "secondary"
}

function errorMessage(err: unknown) {
  return err instanceof Error ? err.message : String(err)
}

export const __test = { clampMapZoom, filterBoardMap, stepMapZoom }
