import { DragDropProvider, useDraggable, useDroppable } from "@dnd-kit/react"
import { useVirtualizer } from "@tanstack/react-virtual"
import { useMemo, useRef } from "react"

import { Badge } from "@/components/ui/badge"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { BoardColumn as ApiBoardColumn, Task, TaskStatus } from "@/lib/api"
import { cn, formatRelativeTime } from "@/lib/utils"

import {
  priorityBadgeClass,
  priorityBadgeLabel,
  dependencyBlockedTodoClass,
  selectedDependencyCountForTask,
  type SelectedDependencySnapshot,
} from "./board-card-state"
import { columnHints, statusAccent } from "./board-config"
import { boardGridStyle, boardScrollerClassName } from "./board-layout"

export function BoardView({
  columns,
  groupedTasks,
  selectedId,
  dependencySnapshot,
  onSelectTask,
  onDropTask,
}: {
  columns: ApiBoardColumn[]
  groupedTasks: Map<TaskStatus, Task[]>
  selectedId?: string
  dependencySnapshot: SelectedDependencySnapshot
  onSelectTask: (taskId: string) => void
  onDropTask: (taskId: string, targetStatus: TaskStatus) => void
}) {
  const taskIds = useMemo(() => {
    const ids = new Set<string>()
    for (const tasks of groupedTasks.values()) {
      for (const task of tasks) ids.add(task.id)
    }
    return ids
  }, [groupedTasks])

  return (
    <DragDropProvider
      onDragEnd={(event) => {
        if (event.canceled) return
        const sourceId = event.operation.source?.id
        const targetStatus = event.operation.target?.data?.status
        if (typeof sourceId !== "string" || !taskIds.has(sourceId) || !isTaskStatus(targetStatus)) return
        onDropTask(sourceId, targetStatus)
      }}
    >
      <div className={boardScrollerClassName}>
        <div className="grid h-full min-h-0 gap-px" style={boardGridStyle(columns.length)}>
          {columns.map((column) => (
            <BoardColumn
              key={column.id}
              column={column}
              tasks={groupedTasks.get(column.status) ?? []}
              selectedId={selectedId}
              dependencySnapshot={dependencySnapshot}
              onSelect={onSelectTask}
            />
          ))}
        </div>
      </div>
    </DragDropProvider>
  )
}

function BoardColumn({
  column,
  tasks,
  selectedId,
  dependencySnapshot,
  onSelect,
}: {
  column: ApiBoardColumn
  tasks: Task[]
  selectedId?: string
  dependencySnapshot: SelectedDependencySnapshot
  onSelect: (taskId: string) => void
}) {
  const parentRef = useRef<HTMLDivElement | null>(null)
  const { ref, isDropTarget } = useDroppable({
    id: `column:${column.status}`,
    data: { type: "column", status: column.status },
  })
  const rowVirtualizer = useVirtualizer({
    count: tasks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 96,
    getItemKey: (index) => tasks[index]?.id ?? index,
    overscan: 6,
    useFlushSync: false,
  })

  return (
    <div
      ref={ref}
      className={cn(
        "flex min-h-0 min-w-0 flex-col overflow-hidden bg-muted/60",
        isDropTarget && "outline outline-2 outline-offset-[-2px] outline-ring",
      )}
    >
      <div className="border-b border-border bg-card px-3 py-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className={cn("h-2 w-2 rounded-full", statusAccent[column.status])} />
            <span className="text-sm font-semibold">{column.title}</span>
          </div>
          <span className="text-xs text-muted-foreground">{tasks.length}</span>
        </div>
        <div className="mt-0.5 text-xs text-muted-foreground">{columnHints[column.status]}</div>
      </div>
      <ScrollArea className="flex-1" viewportRef={parentRef} viewportClassName="p-2 pb-8 scroll-pb-8">
        <div className="relative w-full" style={{ height: `${rowVirtualizer.getTotalSize()}px` }}>
          {rowVirtualizer.getVirtualItems().map((virtualRow) => {
            const task = tasks[virtualRow.index]
            if (!task) return null
            return (
              <div
                key={task.id}
                ref={rowVirtualizer.measureElement}
                data-index={virtualRow.index}
                className="absolute left-0 top-0 w-full pb-2"
                style={{ transform: `translateY(${virtualRow.start}px)` }}
              >
                <TaskCard
                  task={task}
                  selected={task.id === selectedId}
                  dependencyCount={selectedDependencyCountForTask(task.id, dependencySnapshot)}
                  onSelect={() => onSelect(task.id)}
                />
              </div>
            )
          })}
        </div>
      </ScrollArea>
    </div>
  )
}

function TaskCard({
  task,
  selected,
  dependencyCount,
  onSelect,
}: {
  task: Task
  selected: boolean
  dependencyCount?: number
  onSelect: () => void
}) {
  const { ref, isDragging } = useDraggable({
    id: task.id,
    data: { type: "task", taskId: task.id },
  })

  return (
    <button
      ref={ref}
      className={cn(
        "w-full rounded-md border bg-card p-2 text-left text-card-foreground transition-colors hover:border-ring",
        selected ? "border-ring shadow-sm" : "border-border",
        dependencyBlockedTodoClass(task),
        isDragging && "opacity-60",
      )}
      onClick={onSelect}
    >
      <div className="flex items-start gap-2">
        <span className={cn("mt-1.5 h-2 w-2 rounded-full", statusAccent[task.status])} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">#{task.seq} {task.title}</div>
          <div className="mt-1 flex flex-wrap gap-1 text-xs text-muted-foreground">
            <Badge variant="secondary" className={cn("px-1.5 py-0 text-[11px] leading-5", priorityBadgeClass(task.priority))}>
              {priorityBadgeLabel(task.priority)}
            </Badge>
            {task.due_at ? <span>due {formatRelativeTime(task.due_at)}</span> : null}
            {task.scheduled_at ? <span>scheduled {formatRelativeTime(task.scheduled_at)}</span> : null}
            {task.status === "running" ? <span>heartbeat {formatRelativeTime(task.last_heartbeat_at)}</span> : null}
            {typeof dependencyCount === "number" ? <span>{dependencyCount} deps</span> : null}
          </div>
          {task.status_reason ? <div className="mt-1 line-clamp-2 text-xs text-destructive">{task.status_reason}</div> : null}
        </div>
      </div>
    </button>
  )
}

function isTaskStatus(value: unknown): value is TaskStatus {
  return (
    value === "triage" ||
    value === "todo" ||
    value === "scheduled" ||
    value === "ready" ||
    value === "running" ||
    value === "blocked" ||
    value === "review" ||
    value === "done" ||
    value === "archived"
  )
}
