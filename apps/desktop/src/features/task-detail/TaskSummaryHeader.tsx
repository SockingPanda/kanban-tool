import { Pencil } from "lucide-react"

import { Button } from "@/components/ui/button"
import { PriorityBadge, TaskIdentityLine, TaskStatusBadge } from "@/components/ui/composites"
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import type { Task } from "@/lib/api"

export function TaskSummaryHeader({
  task,
  editing,
  editEnabled,
  detailLoading,
  onEdit,
}: {
  task: Task
  editing: boolean
  editEnabled: boolean
  detailLoading: boolean
  onEdit: () => void
}) {
  return (
    <div className="border-b border-border p-4 pr-12">
      <div className="flex items-start justify-between gap-3">
        <SheetHeader className="min-w-0">
          <TaskIdentityLine id={task.id} ref={task.ref} seq={task.seq} />
          <SheetTitle className="mt-1 break-words">{editing ? "Edit task" : task.title}</SheetTitle>
          <SheetDescription className="sr-only">
            Task workbench with one-hop map, description, execution plan, primary action, discussion, runs, events, and metadata.
          </SheetDescription>
        </SheetHeader>
        <div className="flex shrink-0 flex-col items-end gap-1">
          <TaskStatusBadge status={task.status} />
          <PriorityBadge priority={task.priority} />
          {detailLoading ? <span className="text-xs text-muted-foreground">refreshing</span> : null}
        </div>
      </div>
      {!editing ? (
        <Button className="mt-3" variant="outline" size="sm" disabled={!editEnabled} onClick={onEdit}>
          <Pencil className="h-3.5 w-3.5" />
          Edit
        </Button>
      ) : null}
    </div>
  )
}
