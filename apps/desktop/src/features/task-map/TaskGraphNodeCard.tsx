import { GitBranch, GitMerge, ListChecks } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

import { graphNodeRoleClass } from "./task-map-colors"
import type { TaskGraphLayoutNode } from "./task-graph-types"

export function TaskGraphNodeCard({ node, selected, onSelectTask, className }: {
  node: TaskGraphLayoutNode
  selected: boolean
  onSelectTask?: (taskId: string) => void
  className?: string
}) {
  const Icon = node.role === "step_child" || node.role === "step_parent" ? ListChecks : node.role === "dependency_child" ? GitMerge : GitBranch
  return (
    <button
      type="button"
      aria-label={`Open task ${node.ref} ${node.title}`}
      aria-pressed={selected}
      className={cn(
        "flex h-[72px] w-44 flex-col rounded-md border p-2 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        graphNodeRoleClass(node.role, selected, node.contextOnly),
        className,
      )}
      onClick={() => onSelectTask?.(node.id)}
    >
      <div className="flex min-w-0 items-center justify-between gap-2">
        <span className="truncate text-xs font-medium text-muted-foreground">{node.ref}</span>
        <Badge variant="secondary" className="shrink-0 px-1.5 py-0 text-[10px]">{node.status}</Badge>
      </div>
      <div className="mt-1 flex min-w-0 items-start gap-1.5">
        <Icon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span className="line-clamp-2 min-w-0 text-xs font-medium leading-4">{node.title}</span>
      </div>
      {node.stepCounts ? (
        <span className="mt-auto text-[10px] text-muted-foreground">
          {node.stepCounts.incomplete}/{node.stepCounts.total} open
        </span>
      ) : null}
    </button>
  )
}
