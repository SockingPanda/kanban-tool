import { PriorityBadge, TaskIdentityLine, TaskStatusBadge } from "@/components/ui/composites"
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { useI18n } from "@/i18n"
import type { Task } from "@/lib/api"

export function TaskSummaryHeader({
  task,
  detailLoading,
}: {
  task: Task
  detailLoading: boolean
}) {
  const { t } = useI18n()
  return (
    <div className="border-b border-border p-4 pr-12">
      <div className="flex items-start justify-between gap-3">
        <SheetHeader className="min-w-0">
          <TaskIdentityLine id={task.id} ref={task.ref} seq={task.seq} />
          <SheetTitle className="mt-1 break-words">{task.title}</SheetTitle>
          <SheetDescription className="sr-only">
            {t("Task workbench with description, dependencies, execution plan, primary action, discussion, runs, events, and metadata.")}
          </SheetDescription>
        </SheetHeader>
        <div className="flex shrink-0 flex-col items-end gap-1">
          <TaskStatusBadge status={task.status} />
          <PriorityBadge priority={task.priority} />
          {detailLoading ? <span className="text-xs text-muted-foreground">{t("refreshing")}</span> : null}
        </div>
      </div>
    </div>
  )
}
