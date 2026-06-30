import { DragDropProvider, useDraggable, useDroppable } from "@dnd-kit/react"
import { useVirtualizer } from "@tanstack/react-virtual"
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, type ComponentProps } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { PriorityBadge, TaskStatusBadge } from "@/components/ui/composites"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { BoardColumn as ApiBoardColumn, Task, TaskStatus } from "@/lib/api"
import { cn, formatRelativeTime } from "@/lib/utils"
import { useI18n } from "@/i18n"

import {
  dependencyBlockedTodoClass,
  requiredStepProgressLabel,
  selectedDependencyCountForTask,
  selectedUnlockCountForTask,
  taskNeedsExecutionPlan,
  type SelectedDependencySnapshot,
} from "./board-card-state"
import { columnHints, statusAccent } from "./board-config"
import { boardGridStyle, boardScrollerClassName, clampBoardScrollLeft } from "./board-layout"

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
  const scrollerRef = useRef<HTMLDivElement | null>(null)
  const scrollEndTimerRef = useRef<number | null>(null)
  const scrollFrameRef = useRef<number | null>(null)
  const taskIds = useMemo(() => {
    const ids = new Set<string>()
    for (const tasks of groupedTasks.values()) {
      for (const task of tasks) ids.add(task.id)
    }
    return ids
  }, [groupedTasks])
  const selectedStatus = useMemo(() => {
    if (!selectedId) return undefined
    for (const [status, tasks] of groupedTasks.entries()) {
      if (tasks.some((task) => task.id === selectedId)) return status
    }
    return undefined
  }, [groupedTasks, selectedId])

  useLayoutEffect(() => {
    if (scrollerRef.current) clampBoardScrollLeft(scrollerRef.current, columns.length)
  }, [columns.length])

  useEffect(() => {
    return () => {
      if (scrollEndTimerRef.current) window.clearTimeout(scrollEndTimerRef.current)
      if (scrollFrameRef.current) window.cancelAnimationFrame(scrollFrameRef.current)
    }
  }, [])

  const handleBoardScroll = useCallback(() => {
    const scroller = scrollerRef.current
    if (!scroller) return

    if (scrollFrameRef.current === null) {
      scrollFrameRef.current = window.requestAnimationFrame(() => {
        scrollFrameRef.current = null
        const currentScroller = scrollerRef.current
        if (currentScroller) currentScroller.dataset.scrolling = "true"
      })
    }
    if (scrollEndTimerRef.current) window.clearTimeout(scrollEndTimerRef.current)
    scrollEndTimerRef.current = window.setTimeout(() => {
      scroller.removeAttribute("data-scrolling")
      scrollEndTimerRef.current = null
    }, 700)
  }, [])

  const handleDragEnd = useCallback(
    (event: Parameters<NonNullable<ComponentProps<typeof DragDropProvider>["onDragEnd"]>>[0]) => {
      if (event.canceled) return
      const sourceId = event.operation.source?.id
      const targetStatus = event.operation.target?.data?.status
      if (typeof sourceId !== "string" || !taskIds.has(sourceId) || !isTaskStatus(targetStatus)) return
      onDropTask(sourceId, targetStatus)
    },
    [onDropTask, taskIds],
  )

  return (
    <DragDropProvider onDragEnd={handleDragEnd}>
      <div ref={scrollerRef} className={boardScrollerClassName} onScroll={handleBoardScroll}>
        <div className="grid h-full min-h-0 gap-px" style={boardGridStyle(columns.length)}>
          {columns.map((column) => (
            <BoardColumnBridge
              key={column.id}
              column={column}
              tasks={groupedTasks.get(column.status) ?? []}
              selectedId={column.status === selectedStatus ? selectedId : undefined}
              dependencySnapshot={dependencySnapshot}
              onSelectTask={onSelectTask}
            />
          ))}
        </div>
      </div>
    </DragDropProvider>
  )
}

const BoardColumnBridge = memo(function BoardColumnBridge({
  column,
  tasks,
  selectedId,
  dependencySnapshot,
  onSelectTask,
}: {
  column: ApiBoardColumn
  tasks: Task[]
  selectedId?: string
  dependencySnapshot: SelectedDependencySnapshot
  onSelectTask: (taskId: string) => void
}) {
  return <BoardColumn column={column} tasks={tasks} selectedId={selectedId} dependencySnapshot={dependencySnapshot} onSelect={onSelectTask} />
}, areBoardColumnBridgePropsEqual)

const BoardColumn = memo(function BoardColumn({
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
  const { t } = useI18n()
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
            <span className="text-sm font-semibold">{t(column.title)}</span>
          </div>
          <span className="text-xs text-muted-foreground">{tasks.length}</span>
        </div>
        <div className="mt-0.5 text-xs text-muted-foreground">{t(columnHints[column.status])}</div>
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
                  taskId={task.id}
                  selected={task.id === selectedId}
                  dependencyCount={selectedDependencyCountForTask(task.id, dependencySnapshot)}
                  unlockCount={selectedUnlockCountForTask(task.id, dependencySnapshot)}
                  onSelectTask={onSelect}
                />
              </div>
            )
          })}
        </div>
      </ScrollArea>
    </div>
  )
}, areBoardColumnBridgePropsEqual)

function areBoardColumnBridgePropsEqual(
  previous: {
    column: ApiBoardColumn
    tasks: Task[]
    selectedId?: string
    dependencySnapshot: SelectedDependencySnapshot
    onSelectTask?: (taskId: string) => void
    onSelect?: (taskId: string) => void
  },
  next: {
    column: ApiBoardColumn
    tasks: Task[]
    selectedId?: string
    dependencySnapshot: SelectedDependencySnapshot
    onSelectTask?: (taskId: string) => void
    onSelect?: (taskId: string) => void
  },
) {
  return (
    previous.column.id === next.column.id &&
    previous.column.status === next.column.status &&
    previous.column.title === next.column.title &&
    previous.column.position === next.column.position &&
    previous.column.hidden === next.column.hidden &&
    previous.column.wip_limit === next.column.wip_limit &&
    previous.tasks === next.tasks &&
    previous.selectedId === next.selectedId &&
    previous.dependencySnapshot.selectedTaskId === next.dependencySnapshot.selectedTaskId &&
    previous.dependencySnapshot.detailTaskId === next.dependencySnapshot.detailTaskId &&
    previous.dependencySnapshot.dependencies === next.dependencySnapshot.dependencies &&
    previous.dependencySnapshot.loading === next.dependencySnapshot.loading &&
    previous.onSelectTask === next.onSelectTask &&
    previous.onSelect === next.onSelect
  )
}

const TaskCard = memo(function TaskCard({
  task,
  taskId,
  selected,
  dependencyCount,
  unlockCount,
  onSelectTask,
}: {
  task: Task
  taskId: string
  selected: boolean
  dependencyCount?: number
  unlockCount?: number
  onSelectTask: (taskId: string) => void
}) {
  const { t } = useI18n()
  const { ref, isDragging } = useDraggable({
    id: taskId,
    data: { type: "task", taskId },
  })
  const handleSelect = useCallback(() => onSelectTask(taskId), [onSelectTask, taskId])
  const requiredStepProgress = requiredStepProgressLabel(task)

  return (
    <Button
      type="button"
      ref={ref}
      variant="outline"
      className={cn(
        "h-auto min-w-0 w-full shrink overflow-hidden justify-start rounded-md bg-card p-2 text-left text-card-foreground transition-colors hover:border-ring",
        selected ? "border-ring shadow-sm" : "border-border",
        dependencyBlockedTodoClass(task),
        isDragging && "opacity-60",
      )}
      onClick={handleSelect}
    >
      <div className="flex min-w-0 w-full items-start gap-2">
        <span className={cn("mt-1.5 h-2 w-2 shrink-0 rounded-full", statusAccent[task.status])} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">#{task.seq} {task.title}</div>
          <div className="mt-1 flex flex-wrap gap-1 text-xs text-muted-foreground">
            <TaskStatusBadge status={task.status} className="px-1.5 py-0 text-[11px] leading-5" />
            <PriorityBadge priority={task.priority} className="px-1.5 py-0 text-[11px] leading-5" />
            {task.due_at ? <span>{t("due {time}", { time: formatRelativeTime(task.due_at) })}</span> : null}
            {task.scheduled_at ? <span>{t("scheduled {time}", { time: formatRelativeTime(task.scheduled_at) })}</span> : null}
            {task.status === "running" ? <span>{t("heartbeat {time}", { time: formatRelativeTime(task.last_heartbeat_at) })}</span> : null}
            {requiredStepProgress ? <span>{requiredStepProgress}</span> : null}
            {taskNeedsExecutionPlan(task) ? <Badge variant="blocked" className="px-1.5 py-0 text-[11px] leading-5">{t("plan needed")}</Badge> : null}
            {task.dependency_blocked ? <span>{t("blocked by {count}", { count: task.unfinished_parent_count })}</span> : null}
            {typeof unlockCount === "number" && unlockCount > 0 ? <span>{t("unlocks {count}", { count: unlockCount })}</span> : null}
            {typeof dependencyCount === "number" ? <span>{t("{count} deps", { count: dependencyCount })}</span> : null}
          </div>
          {task.labels.length ? (
            <div className="mt-1 flex flex-wrap gap-1">
              {task.labels.map((label) => (
                <Badge key={label.id} variant="secondary" className="max-w-full truncate px-1.5 py-0 text-[11px] leading-5">
                  {label.name}
                </Badge>
              ))}
            </div>
          ) : null}
          {task.status_reason ? <div className="mt-1 line-clamp-2 break-words text-xs text-destructive">{task.status_reason}</div> : null}
        </div>
      </div>
    </Button>
  )
})

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
