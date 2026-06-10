import type { FormEvent } from "react"
import {
  Activity,
  Command,
  Database,
  Inbox,
  Loader2,
  RefreshCcw,
  Search,
  Settings,
  ShieldAlert,
  SlidersHorizontal,
  SquareKanban,
  TerminalSquare,
  Plus,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { BoardView } from "@/features/board/BoardView"
import { filterStatuses } from "@/features/board/board-config"
import { TaskDetail } from "@/features/task-detail/TaskDetail"
import type { DetailState } from "@/features/task-detail/detail-state"
import type { TaskEditDraft } from "@/features/task-detail/task-draft"
import type {
  BoardColumn,
  KanbanApi,
  Run,
  RuntimeConfig,
  SearchMeta,
  Task,
  TaskStatus,
} from "@/lib/api"
import { cn } from "@/lib/utils"

export function AppShell({
  config,
  api,
  columns,
  groupedTasks,
  selectedTask,
  selectedId,
  detail,
  activeRun,
  search,
  searchMeta,
  statusFilter,
  showArchived,
  newTitle,
  newDescription,
  blockReason,
  dependencyInput,
  commentBody,
  editDraft,
  claimToken,
  busy,
  error,
  lastRefreshAt,
  queueCounts,
  onSearchChange,
  onStatusFilterChange,
  onShowArchivedChange,
  onRefreshTasks,
  onCreateTask,
  onNewTitleChange,
  onNewDescriptionChange,
  onSelectTask,
  onBlockReasonChange,
  onDependencyInputChange,
  onCommentBodyChange,
  onEditDraftChange,
  onAction,
  onAddDependency,
  onSaveTask,
  onAddComment,
}: {
  config: RuntimeConfig | null
  api: KanbanApi | null
  columns: BoardColumn[]
  groupedTasks: Map<TaskStatus, Task[]>
  selectedTask: Task | null
  selectedId: string | null
  detail: DetailState
  activeRun?: Run
  search: string
  searchMeta: SearchMeta | null
  statusFilter: TaskStatus | "all"
  showArchived: boolean
  newTitle: string
  newDescription: string
  blockReason: string
  dependencyInput: string
  commentBody: string
  editDraft: TaskEditDraft | null
  claimToken: string | null
  busy: boolean
  error: string | null
  lastRefreshAt: number | null
  queueCounts: { ready: number; running: number; blocked: number }
  onSearchChange: (value: string) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onShowArchivedChange: (value: boolean) => void
  onRefreshTasks: () => void
  onCreateTask: (event: FormEvent) => void
  onNewTitleChange: (value: string) => void
  onNewDescriptionChange: (value: string) => void
  onSelectTask: (taskId: string) => void
  onBlockReasonChange: (value: string) => void
  onDependencyInputChange: (value: string) => void
  onCommentBodyChange: (value: string) => void
  onEditDraftChange: (value: TaskEditDraft) => void
  onAction: (action: () => Promise<unknown>) => Promise<void>
  onAddDependency: () => Promise<void>
  onSaveTask: () => Promise<void>
  onAddComment: () => Promise<void>
}) {
  return (
    <div className="flex h-screen bg-[#f7f7f5] text-neutral-950">
      <aside className="flex w-52 shrink-0 flex-col border-r border-neutral-200 bg-[#fbfbfa]">
        <div className="flex h-14 items-center gap-2 px-4">
          <div className="flex h-7 w-7 items-center justify-center rounded-md bg-neutral-950 text-sm font-semibold text-white">
            kb
          </div>
          <div>
            <div className="text-sm font-semibold">Kanban Tool</div>
            <div className="text-xs text-neutral-500">local queue</div>
          </div>
        </div>
        <nav className="space-y-1 px-2 py-3">
          <NavItem icon={SquareKanban} label="Board" active />
          <NavItem icon={Inbox} label="Ready Queue" />
          <NavItem icon={ShieldAlert} label="Blocked" />
          <NavItem icon={TerminalSquare} label="Runs" />
          <NavItem icon={Activity} label="Events" />
          <NavItem icon={Settings} label="Settings" />
        </nav>
        <div className="mt-auto space-y-3 border-t border-neutral-200 p-3 text-xs text-neutral-500">
          <div className="flex items-center gap-2">
            <Database className="h-3.5 w-3.5" />
            <span className="truncate">{config?.dbPath ?? "loading db"}</span>
          </div>
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2">
              <span className="h-2 w-2 rounded-full bg-emerald-500" />
              API
            </span>
            <span>{config ? apiEndpointLabel(config.apiBaseUrl) : "-"}</span>
          </div>
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center gap-3 border-b border-neutral-200 bg-white px-4">
          <div className="relative w-80">
            <Search className="pointer-events-none absolute left-2.5 top-2 h-4 w-4 text-neutral-400" />
            <Input
              className="pl-8"
              placeholder="Search tasks"
              value={search}
              onChange={(event) => onSearchChange(event.target.value)}
            />
          </div>
          {search.trim() && searchMeta ? <SearchBackendBadge meta={searchMeta} /> : null}
          <Button variant="secondary" size="icon" onClick={onRefreshTasks}>
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCcw className="h-4 w-4" />}
          </Button>
          <div className="flex rounded-md border border-neutral-200 bg-neutral-50 p-0.5 text-sm">
            {["Board", "List", "Events"].map((view, index) => (
              <button
                key={view}
                className={cn(
                  "rounded px-3 py-1 text-neutral-500",
                  index === 0 && "bg-white text-neutral-950 shadow-sm",
                )}
              >
                {view}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-2 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs">
            <SlidersHorizontal className="h-3.5 w-3.5 text-neutral-500" />
            <select
              className="bg-transparent outline-none"
              value={statusFilter}
              onChange={(event) => onStatusFilterChange(event.target.value as TaskStatus | "all")}
            >
              <option value="all">all active</option>
              {filterStatuses.map((status) => (
                <option key={status} value={status}>{status}</option>
              ))}
            </select>
          </div>
          <label className="flex items-center gap-2 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs text-neutral-600">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(event) => onShowArchivedChange(event.target.checked)}
            />
            Archived
          </label>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant="secondary">actor {config?.actor ?? "-"}</Badge>
            <Badge variant="ready">dispatcher observed</Badge>
            <Button variant="ghost" size="icon">
              <Command className="h-4 w-4" />
            </Button>
          </div>
        </header>

        {error ? (
          <div className="border-b border-red-200 bg-red-50 px-4 py-2 text-sm text-red-700">{error}</div>
        ) : null}

        <div className="flex min-h-0 flex-1">
          <section className="flex min-w-0 flex-1 flex-col">
            <form onSubmit={onCreateTask} className="grid grid-cols-[1fr_1.4fr_auto] gap-2 border-b border-neutral-200 bg-white px-4 py-3">
              <Input value={newTitle} onChange={(event) => onNewTitleChange(event.target.value)} placeholder="New task title" />
              <Input
                value={newDescription}
                onChange={(event) => onNewDescriptionChange(event.target.value)}
                placeholder="Optional spec or description"
              />
              <Button type="submit" disabled={!newTitle.trim() || busy}>
                <Plus className="h-4 w-4" />
                New task
              </Button>
            </form>

            <BoardView
              columns={columns}
              groupedTasks={groupedTasks}
              selectedId={selectedTask?.id ?? selectedId ?? undefined}
              dependencies={detail.dependencies}
              onSelectTask={onSelectTask}
            />
          </section>

          <TaskDetail
            api={api}
            task={selectedTask}
            detail={detail}
            activeRun={activeRun}
            blockReason={blockReason}
            setBlockReason={onBlockReasonChange}
            dependencyInput={dependencyInput}
            setDependencyInput={onDependencyInputChange}
            claimToken={claimToken}
            commentBody={commentBody}
            setCommentBody={onCommentBodyChange}
            editDraft={editDraft}
            setEditDraft={onEditDraftChange}
            busy={busy}
            onAction={onAction}
            onAddDependency={onAddDependency}
            onSaveTask={onSaveTask}
            onAddComment={onAddComment}
          />
        </div>

        <footer className="flex h-8 items-center justify-between border-t border-neutral-200 bg-white px-4 text-xs text-neutral-500">
          <span>Last refresh {lastRefreshAt ? new Date(lastRefreshAt).toLocaleTimeString() : "-"}</span>
          <span>
            ready {queueCounts.ready} / running {queueCounts.running} / blocked {queueCounts.blocked}
          </span>
        </footer>
      </main>
    </div>
  )
}

function SearchBackendBadge({ meta }: { meta: SearchMeta }) {
  return (
    <Badge variant={meta.stale ? "review" : "secondary"}>
      search {meta.backend}
      {meta.stale ? " stale/degraded" : ""}
      {meta.index_lag_events && meta.index_lag_events > 0 ? ` +${meta.index_lag_events}` : ""}
    </Badge>
  )
}

function NavItem({ icon: Icon, label, active = false }: { icon: React.ElementType; label: string; active?: boolean }) {
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-neutral-600",
        active && "bg-neutral-100 text-neutral-950",
      )}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  )
}

function apiEndpointLabel(apiBaseUrl: string) {
  if (!apiBaseUrl) return "same-origin"
  return new URL(apiBaseUrl).port
}
