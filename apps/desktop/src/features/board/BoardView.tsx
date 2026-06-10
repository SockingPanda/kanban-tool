import type { BoardColumn as ApiBoardColumn, Dependencies, Task, TaskStatus } from "@/lib/api"
import { cn, formatRelativeTime } from "@/lib/utils"

import { columnHints, statusAccent } from "./board-config"

export function BoardView({
  columns,
  groupedTasks,
  selectedId,
  dependencies,
  onSelectTask,
}: {
  columns: ApiBoardColumn[]
  groupedTasks: Map<TaskStatus, Task[]>
  selectedId?: string
  dependencies: Dependencies
  onSelectTask: (taskId: string) => void
}) {
  return (
    <div
      className="grid min-h-0 flex-1 gap-px overflow-hidden bg-neutral-200"
      style={{ gridTemplateColumns: `repeat(${Math.max(1, columns.length)}, minmax(160px, 1fr))` }}
    >
      {columns.map((column) => (
        <BoardColumn
          key={column.id}
          column={column}
          tasks={groupedTasks.get(column.status) ?? []}
          selectedId={selectedId}
          dependencies={dependencies}
          onSelect={onSelectTask}
        />
      ))}
    </div>
  )
}

function BoardColumn({
  column,
  tasks,
  selectedId,
  dependencies,
  onSelect,
}: {
  column: ApiBoardColumn
  tasks: Task[]
  selectedId?: string
  dependencies: Dependencies
  onSelect: (taskId: string) => void
}) {
  return (
    <div className="flex min-w-0 flex-col bg-[#f7f7f5]">
      <div className="border-b border-neutral-200 bg-white px-3 py-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className={cn("h-2 w-2 rounded-full", statusAccent[column.status])} />
            <span className="text-sm font-semibold">{column.title}</span>
          </div>
          <span className="text-xs text-neutral-500">{tasks.length}</span>
        </div>
        <div className="mt-0.5 text-xs text-neutral-500">{columnHints[column.status]}</div>
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-2">
        {tasks.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            selected={task.id === selectedId}
            dependencyCount={
              task.id === selectedId
                ? dependencies.parents.length + dependencies.children.length
                : undefined
            }
            onSelect={() => onSelect(task.id)}
          />
        ))}
      </div>
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
  return (
    <button
      className={cn(
        "w-full rounded-md border bg-white p-2 text-left transition-colors hover:border-neutral-300",
        selected ? "border-neutral-900 shadow-sm" : "border-neutral-200",
      )}
      onClick={onSelect}
    >
      <div className="flex items-start gap-2">
        <span className={cn("mt-1.5 h-2 w-2 rounded-full", statusAccent[task.status])} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">#{task.seq} {task.title}</div>
          <div className="mt-1 flex flex-wrap gap-1 text-xs text-neutral-500">
            <span>P{task.priority}</span>
            {task.due_at ? <span>due {formatRelativeTime(task.due_at)}</span> : null}
            {task.scheduled_at ? <span>scheduled {formatRelativeTime(task.scheduled_at)}</span> : null}
            {task.status === "running" ? <span>heartbeat {formatRelativeTime(task.last_heartbeat_at)}</span> : null}
            {typeof dependencyCount === "number" ? <span>{dependencyCount} deps</span> : null}
          </div>
          {task.status_reason ? <div className="mt-1 line-clamp-2 text-xs text-red-700">{task.status_reason}</div> : null}
        </div>
      </div>
    </button>
  )
}
