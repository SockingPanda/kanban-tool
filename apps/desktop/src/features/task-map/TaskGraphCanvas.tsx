import { cn } from "@/lib/utils"

import { TaskGraphNodeCard } from "./TaskGraphNodeCard"
import { graphEdgeClass } from "./task-map-colors"
import { layoutTaskGraph } from "./task-graph-layout"
import type { TaskGraph, TaskGraphMode } from "./task-graph-types"

export function TaskGraphCanvas({ graph, selectedTaskId, onSelectTask, mode = "detail", className }: {
  graph: TaskGraph
  selectedTaskId?: string | null
  onSelectTask?: (taskId: string) => void
  mode?: TaskGraphMode
  className?: string
}) {
  const layout = layoutTaskGraph(graph, { mode, selectedTaskId })
  return (
    <div className={cn("relative overflow-auto rounded-md border border-border bg-muted/20", className)}>
      <div className="relative" style={{ width: layout.width, height: layout.height }}>
        <svg className="absolute inset-0 pointer-events-none" width={layout.width} height={layout.height} aria-hidden="true">
          <defs>
            <marker id="task-graph-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" className="fill-current" />
            </marker>
          </defs>
          {layout.edges.map((edge) => (
            <path
              key={edge.id}
              d={edge.path}
              className={cn("fill-none stroke-2", graphEdgeClass(edge.kind, edge.blocking))}
              markerEnd="url(#task-graph-arrow)"
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
  )
}
