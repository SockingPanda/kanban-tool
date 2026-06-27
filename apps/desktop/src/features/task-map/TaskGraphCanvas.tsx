import { useId, useMemo } from "react"

import { cn } from "@/lib/utils"

import { TaskGraphNodeCard } from "./TaskGraphNodeCard"
import { graphEdgeClass } from "./task-map-colors"
import { clampTaskGraphScale } from "./task-graph-scale"
import { layoutTaskGraph } from "./task-graph-layout"
import type { TaskGraph, TaskGraphMode } from "./task-graph-types"

export function TaskGraphCanvas({ graph, selectedTaskId, onSelectTask, mode = "detail", scale = 1, className }: {
  graph: TaskGraph
  selectedTaskId?: string | null
  onSelectTask?: (taskId: string) => void
  mode?: TaskGraphMode
  scale?: number
  className?: string
}) {
  const markerId = `task-graph-arrow-${useId().replace(/:/g, "")}`
  const layout = useMemo(() => layoutTaskGraph(graph, { mode, selectedTaskId }), [graph, mode, selectedTaskId])
  const safeScale = clampTaskGraphScale(scale)
  return (
    <div
      className={cn("relative overflow-auto rounded-md border border-border bg-muted/20", className)}
      aria-label={`Task graph with ${layout.nodes.length} nodes and ${layout.edges.length} edges`}
    >
      <div className="relative" style={{ width: layout.width * safeScale, height: layout.height * safeScale }}>
        <div className="absolute left-0 top-0 origin-top-left" style={{ width: layout.width, height: layout.height, transform: `scale(${safeScale})` }}>
          <svg className="absolute inset-0 pointer-events-none" width={layout.width} height={layout.height} aria-hidden="true">
            <defs>
              <marker id={markerId} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
                <path d="M 0 0 L 10 5 L 0 10 z" className="fill-current" />
              </marker>
            </defs>
            {layout.edges.map((edge) => (
              <path
                key={edge.id}
                d={edge.path}
                className={cn("fill-none stroke-2", graphEdgeClass(edge.kind, edge.blocking))}
                markerEnd={`url(#${markerId})`}
              />
            ))}
          </svg>
          {layout.nodes.map((node) => (
            <TaskGraphNodeCard
              key={node.id}
              node={node}
              selected={node.id === selectedTaskId || node.role === "center"}
              onSelectTask={onSelectTask}
            />
          ))}
        </div>
      </div>
    </div>
  )
}
