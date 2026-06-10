import { useVirtualizer } from "@tanstack/react-virtual"
import { useRef } from "react"

import { Badge } from "@/components/ui/badge"
import type { Task } from "@/lib/api"
import { cn, formatRelativeTime, shortId } from "@/lib/utils"

export function ListView({
  tasks,
  selectedId,
  onSelectTask,
}: {
  tasks: Task[]
  selectedId: string | null
  onSelectTask: (taskId: string) => void
}) {
  const parentRef = useRef<HTMLDivElement | null>(null)
  const rowVirtualizer = useVirtualizer({
    count: tasks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 58,
    getItemKey: (index) => tasks[index]?.id ?? index,
    overscan: 8,
    useFlushSync: false,
  })

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-white">
      <div className="grid grid-cols-[72px_120px_1fr_96px_120px_120px] gap-3 border-b border-neutral-200 px-4 py-2 text-xs font-medium uppercase tracking-normal text-neutral-500">
        <span>Ref</span>
        <span>Status</span>
        <span>Title</span>
        <span>Priority</span>
        <span>Assignee</span>
        <span>Updated</span>
      </div>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        <div className="relative w-full" style={{ height: `${rowVirtualizer.getTotalSize()}px` }}>
          {rowVirtualizer.getVirtualItems().map((virtualRow) => {
            const task = tasks[virtualRow.index]
            if (!task) return null
            return (
              <button
                key={task.id}
                ref={rowVirtualizer.measureElement}
                data-index={virtualRow.index}
                className={cn(
                  "absolute left-0 top-0 grid w-full grid-cols-[72px_120px_1fr_96px_120px_120px] gap-3 border-b border-neutral-100 px-4 py-2 text-left text-sm hover:bg-neutral-50",
                  selectedId === task.id && "bg-neutral-100",
                )}
                style={{ transform: `translateY(${virtualRow.start}px)` }}
                onClick={() => onSelectTask(task.id)}
              >
                <span className="text-xs text-neutral-500">#{task.seq}</span>
                <span><Badge variant={badgeVariant(task.status)}>{task.status}</Badge></span>
                <span className="truncate font-medium">{task.title}</span>
                <span>{task.priority}</span>
                <span className="truncate text-neutral-600">{task.assignee ?? "-"}</span>
                <span className="text-xs text-neutral-500" title={shortId(task.id)}>{formatRelativeTime(task.updated_at)}</span>
              </button>
            )
          })}
        </div>
      </div>
    </div>
  )
}

function badgeVariant(status: Task["status"]) {
  if (status === "ready" || status === "done") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  return "secondary"
}
