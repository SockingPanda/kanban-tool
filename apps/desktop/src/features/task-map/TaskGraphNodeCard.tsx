import { GitBranch, GitMerge, ListChecks } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

import { graphNodeStatusClass, graphNodeStepProgressClass } from "./task-map-colors"
import type { TaskGraphLayoutNode } from "./task-graph-types"

export function TaskGraphNodeCard({ node, selected, onSelectTask, onOpenTask, className }: {
  node: TaskGraphLayoutNode
  selected: boolean
  onSelectTask?: (taskId: string) => void
  onOpenTask?: (taskId: string) => void
  className?: string
}) {
  const Icon = node.role === "step_child" || node.role === "step_parent" ? ListChecks : node.role === "dependency_child" ? GitMerge : GitBranch
  const stepProgressPercent = taskGraphNodeStepProgressPercent(node.stepCounts)
  return (
    <button
      type="button"
      aria-label={`Open task ${node.ref} ${node.title}`}
      aria-pressed={selected}
      className={cn(
        "relative flex h-[72px] w-44 flex-col overflow-hidden rounded-md border p-2 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        graphNodeStatusClass(node.status, selected, node.contextOnly, node.dependencyBlocked),
        className,
      )}
      onClick={() => onSelectTask?.(node.id)}
      onDoubleClick={() => onOpenTask?.(node.id)}
    >
      {stepProgressPercent > 0 ? (
        <span
          aria-hidden={true}
          data-testid="task-graph-node-step-progress"
          className={cn("pointer-events-none absolute inset-y-0 left-0", graphNodeStepProgressClass())}
          style={{ width: `${stepProgressPercent}%` }}
        />
      ) : null}
      <span className="relative z-10 flex min-h-0 flex-1 flex-col">
        <span className="flex min-w-0 items-center justify-between gap-2">
          <span className="truncate text-xs font-medium text-muted-foreground">{node.ref}</span>
          <Badge variant="secondary" className="shrink-0 px-1.5 py-0 text-[10px]">{node.status}</Badge>
        </span>
        <span className="mt-1 flex min-w-0 items-start gap-1.5">
          <Icon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span className="line-clamp-2 min-w-0 text-xs font-medium leading-4">{node.title}</span>
        </span>
        {node.stepCounts ? (
          <span className="mt-auto text-[10px] text-muted-foreground">
            {node.stepCounts.completed}/{node.stepCounts.total} step
          </span>
        ) : null}
      </span>
    </button>
  )
}

function taskGraphNodeStepProgressPercent(stepCounts: TaskGraphLayoutNode["stepCounts"]) {
  if (!stepCounts || stepCounts.total <= 0) return 0
  return Math.min(100, Math.max(0, Math.round((stepCounts.completed / stepCounts.total) * 100)))
}
