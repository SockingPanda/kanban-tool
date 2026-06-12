import type { ElementType, FormEvent, ReactNode } from "react"
import {
  Activity,
  Command,
  Database,
  DatabaseBackup,
  HeartPulse,
  Inbox,
  Loader2,
  RefreshCcw,
  Search,
  Settings,
  SlidersHorizontal,
  SquareKanban,
  TerminalSquare,
  Plus,
  Server,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Sheet, SheetContent } from "@/components/ui/sheet"
import { shouldOpenTaskDetailSheet } from "@/app/task-selection"
import { BoardView } from "@/features/board/BoardView"
import { filterStatuses } from "@/features/board/board-config"
import { EventsView } from "@/features/events/EventsView"
import { HealthView } from "@/features/health/HealthView"
import { ListView } from "@/features/list/ListView"
import { MaintenanceView } from "@/features/maintenance/MaintenanceView"
import type { OperatorView } from "@/features/navigation/view-types"
import { primaryViews, sidebarViews } from "@/features/navigation/view-types"
import { RunsView } from "@/features/runs/RunsView"
import { SettingsView } from "@/features/settings/SettingsView"
import { TaskDetail } from "@/features/task-detail/TaskDetail"
import type { DetailState } from "@/features/task-detail/detail-state"
import type { TaskEditDraft } from "@/features/task-detail/task-draft"
import { apiEndpointLabel, shouldShowTaskExplorerToolbar } from "@/app/shell-rules"
import type {
  BoardColumn,
  KanbanApi,
  Run,
  RuntimeConfig,
  SearchTasksMeta,
  Task,
  TaskStatus,
  PageMeta,
} from "@/lib/api"
import { pageRangeLabel } from "@/lib/pagination"
import { cn } from "@/lib/utils"

const viewMetadata: Record<OperatorView, { label: string; icon: ElementType }> = {
  board: { label: "Board", icon: SquareKanban },
  list: { label: "List", icon: Inbox },
  events: { label: "Events", icon: Activity },
  runs: { label: "Runs", icon: TerminalSquare },
  maintenance: { label: "Maintenance", icon: DatabaseBackup },
  health: { label: "Health", icon: HeartPulse },
  settings: { label: "Settings", icon: Settings },
}

export function AppShell({
  config,
  api,
  view,
  columns,
  tasks,
  groupedTasks,
  selectedTask,
  selectedId,
  detail,
  activeRun,
  search,
  debouncedSearch,
  searchMeta,
  statusFilter,
  showArchived,
  page,
  visibleTaskCount,
  hasNextPage,
  hasPreviousPage,
  canGoLastPage,
  rowsPerPage,
  newTitle,
  newDescription,
  blockReason,
  dependencyInput,
  commentBody,
  editDraft,
  draftDirty,
  claimToken,
  tasksLoading,
  tasksRefreshing,
  detailLoading,
  pendingAction,
  error,
  lastRefreshAt,
  queueCounts,
  onSearchChange,
  onViewChange,
  onStatusFilterChange,
  onShowArchivedChange,
  onRefreshTasks,
  onFirstPage,
  onPreviousPage,
  onNextPage,
  onLastPage,
  onRowsPerPageChange,
  onCreateTask,
  onNewTitleChange,
  onNewDescriptionChange,
  onSelectTask,
  onCloseTaskDetail,
  onDropTask,
  onBlockReasonChange,
  onDependencyInputChange,
  onCommentBodyChange,
  onEditDraftChange,
  onAction,
  onAddDependency,
  onRemoveDependency,
  onSaveTask,
  onCancelTaskEdit,
  onAddComment,
}: {
  config: RuntimeConfig | null
  api: KanbanApi | null
  view: OperatorView
  columns: BoardColumn[]
  tasks: Task[]
  groupedTasks: Map<TaskStatus, Task[]>
  selectedTask: Task | null
  selectedId: string | null
  detail: DetailState
  activeRun?: Run
  search: string
  debouncedSearch: string
  searchMeta: SearchTasksMeta | null
  statusFilter: TaskStatus | "all"
  showArchived: boolean
  page: PageMeta
  visibleTaskCount: number
  hasNextPage: boolean
  hasPreviousPage: boolean
  canGoLastPage: boolean
  rowsPerPage: number
  newTitle: string
  newDescription: string
  blockReason: string
  dependencyInput: string
  commentBody: string
  editDraft: TaskEditDraft | null
  draftDirty: boolean
  claimToken: string | null
  tasksLoading: boolean
  tasksRefreshing: boolean
  detailLoading: boolean
  pendingAction: string | null
  error: string | null
  lastRefreshAt: number | null
  queueCounts: { ready: number; running: number; blocked: number }
  onSearchChange: (value: string) => void
  onViewChange: (value: OperatorView) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onShowArchivedChange: (value: boolean) => void
  onRefreshTasks: () => void
  onFirstPage: () => void
  onPreviousPage: () => void
  onNextPage: () => void
  onLastPage: () => void
  onRowsPerPageChange: (value: number) => void
  onCreateTask: (event: FormEvent) => void
  onNewTitleChange: (value: string) => void
  onNewDescriptionChange: (value: string) => void
  onSelectTask: (taskId: string) => void
  onCloseTaskDetail: () => void
  onDropTask: (taskId: string, targetStatus: TaskStatus) => void
  onBlockReasonChange: (value: string) => void
  onDependencyInputChange: (value: string) => void
  onCommentBodyChange: (value: string) => void
  onEditDraftChange: (value: TaskEditDraft) => void
  onAction: (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null }) => Promise<unknown>
  onAddDependency: () => Promise<void>
  onRemoveDependency: (parentTaskId: string) => Promise<void>
  onSaveTask: () => Promise<boolean>
  onCancelTaskEdit: () => void
  onAddComment: () => Promise<void>
}) {
  let taskActivityLabel = ""
  if (tasksLoading) {
    taskActivityLabel = " · loading"
  } else if (tasksRefreshing) {
    taskActivityLabel = " · refreshing"
  }
  const showTaskExplorerToolbar = shouldShowTaskExplorerToolbar(view)
  const showDetailSheet = shouldOpenTaskDetailSheet(view, selectedTask)

  return (
    <div className="flex h-screen bg-[#f7f7f5] text-neutral-950">
      <ShellSidebar config={config} view={view} onViewChange={onViewChange} />

      <main className="flex min-w-0 flex-1 flex-col">
        <ShellHeader
          config={config}
          view={view}
          search={search}
          debouncedSearch={debouncedSearch}
          searchMeta={searchMeta}
          statusFilter={statusFilter}
          showArchived={showArchived}
          tasksRefreshing={tasksRefreshing}
          onSearchChange={onSearchChange}
          onViewChange={onViewChange}
          onStatusFilterChange={onStatusFilterChange}
          onShowArchivedChange={onShowArchivedChange}
          onRefreshTasks={onRefreshTasks}
        />

        {error ? (
          <div className="border-b border-red-200 bg-red-50 px-4 py-2 text-sm text-red-700">{error}</div>
        ) : null}

        <div className="flex min-h-0 flex-1">
          <section className="flex min-w-0 flex-1 flex-col">
            {showTaskExplorerToolbar ? (
              <TaskExplorerToolbar
                view={view}
                page={page}
                visibleTaskCount={visibleTaskCount}
                taskActivityLabel={taskActivityLabel}
                hasNextPage={hasNextPage}
                hasPreviousPage={hasPreviousPage}
                newTitle={newTitle}
                newDescription={newDescription}
                tasksRefreshing={tasksRefreshing}
                pendingAction={pendingAction}
                onCreateTask={onCreateTask}
                onNewTitleChange={onNewTitleChange}
                onNewDescriptionChange={onNewDescriptionChange}
                onPreviousPage={onPreviousPage}
                onNextPage={onNextPage}
              />
            ) : null}

            <MainView
              api={api}
              config={config}
              view={view}
              columns={columns}
              tasks={tasks}
              groupedTasks={groupedTasks}
              selectedTask={selectedTask}
              selectedId={selectedId}
              detail={detail}
              onSelectTask={onSelectTask}
              onDropTask={onDropTask}
              page={page}
              hasNextPage={hasNextPage}
              hasPreviousPage={hasPreviousPage}
              canGoLastPage={canGoLastPage}
              rowsPerPage={rowsPerPage}
              tasksRefreshing={tasksRefreshing}
              onFirstPage={onFirstPage}
              onPreviousPage={onPreviousPage}
              onNextPage={onNextPage}
              onLastPage={onLastPage}
              onRowsPerPageChange={onRowsPerPageChange}
            />
          </section>

          <Sheet
            open={showDetailSheet}
            onOpenChange={(open) => {
              if (!open) onCloseTaskDetail()
            }}
          >
            <SheetContent side="right" className="w-[min(620px,calc(100vw-32px))] p-0">
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
                draftDirty={draftDirty}
                setEditDraft={onEditDraftChange}
                detailLoading={detailLoading}
                pendingAction={pendingAction}
                onAction={onAction}
                onAddDependency={onAddDependency}
                onRemoveDependency={onRemoveDependency}
                onSaveTask={onSaveTask}
                onCancelEdit={onCancelTaskEdit}
                onAddComment={onAddComment}
              />
            </SheetContent>
          </Sheet>
        </div>

        <StatusBar lastRefreshAt={lastRefreshAt} queueCounts={queueCounts} />
      </main>
    </div>
  )
}

function ShellSidebar({
  config,
  view,
  onViewChange,
}: {
  config: RuntimeConfig | null
  view: OperatorView
  onViewChange: (value: OperatorView) => void
}) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-neutral-200 bg-[#fbfbfa]">
      <div className="flex h-14 items-center gap-2 px-4">
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-neutral-950 text-sm font-semibold text-white">
          kb
        </div>
        <div>
          <div className="text-sm font-semibold">Kanban Tool</div>
          <div className="text-xs text-neutral-500">local queue</div>
        </div>
      </div>
      <nav className="space-y-4 px-2 py-3">
        <NavGroup label="Task Explorer">
          {sidebarViews.filter((item) => ["board", "list", "runs", "events"].includes(item)).map((item) => (
            <NavItem
              key={item}
              icon={viewIcon(item)}
              label={viewLabel(item)}
              active={view === item}
              onClick={() => onViewChange(item)}
            />
          ))}
        </NavGroup>
        <NavGroup label="System">
          {sidebarViews.filter((item) => ["maintenance", "health", "settings"].includes(item)).map((item) => (
            <NavItem
              key={item}
              icon={viewIcon(item)}
              label={viewLabel(item)}
              active={view === item}
              onClick={() => onViewChange(item)}
            />
          ))}
        </NavGroup>
      </nav>
      <div className="mt-auto space-y-3 border-t border-neutral-200 p-3 text-xs text-neutral-500">
        <div className="flex items-center gap-2">
          <Database className="h-3.5 w-3.5" />
          <span className="truncate">{config?.dbPath ?? "loading db"}</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="flex items-center gap-2">
            <Server className="h-3.5 w-3.5" />
            API
          </span>
          <span>{config ? apiEndpointLabel(config.apiBaseUrl) : "-"}</span>
        </div>
      </div>
    </aside>
  )
}

function ShellHeader({
  config,
  view,
  search,
  debouncedSearch,
  searchMeta,
  statusFilter,
  showArchived,
  tasksRefreshing,
  onSearchChange,
  onViewChange,
  onStatusFilterChange,
  onShowArchivedChange,
  onRefreshTasks,
}: {
  config: RuntimeConfig | null
  view: OperatorView
  search: string
  debouncedSearch: string
  searchMeta: SearchTasksMeta | null
  statusFilter: TaskStatus | "all"
  showArchived: boolean
  tasksRefreshing: boolean
  onSearchChange: (value: string) => void
  onViewChange: (value: OperatorView) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onShowArchivedChange: (value: boolean) => void
  onRefreshTasks: () => void
}) {
  return (
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
      {debouncedSearch.trim() && searchMeta ? <SearchBackendBadge meta={searchMeta} /> : null}
      <Button variant="secondary" size="icon" onClick={onRefreshTasks} disabled={tasksRefreshing}>
        {tasksRefreshing ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCcw className="h-4 w-4" />}
      </Button>
      <div className="flex rounded-md border border-neutral-200 bg-neutral-50 p-0.5 text-sm">
        {primaryViews.map((item) => (
          <button
            key={item}
            className={cn(
              "rounded px-3 py-1 text-neutral-500",
              view === item && "bg-white text-neutral-950 shadow-sm",
            )}
            onClick={() => onViewChange(item)}
          >
            {viewLabel(item)}
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
        <Badge variant="ready">local dispatcher</Badge>
        <Button variant="ghost" size="icon">
          <Command className="h-4 w-4" />
        </Button>
      </div>
    </header>
  )
}

function TaskExplorerToolbar({
  view,
  page,
  visibleTaskCount,
  taskActivityLabel,
  hasNextPage,
  hasPreviousPage,
  newTitle,
  newDescription,
  tasksRefreshing,
  pendingAction,
  onCreateTask,
  onNewTitleChange,
  onNewDescriptionChange,
  onPreviousPage,
  onNextPage,
}: {
  view: OperatorView
  page: PageMeta
  visibleTaskCount: number
  taskActivityLabel: string
  hasNextPage: boolean
  hasPreviousPage: boolean
  newTitle: string
  newDescription: string
  tasksRefreshing: boolean
  pendingAction: string | null
  onCreateTask: (event: FormEvent) => void
  onNewTitleChange: (value: string) => void
  onNewDescriptionChange: (value: string) => void
  onPreviousPage: () => void
  onNextPage: () => void
}) {
  return (
    <>
      <form onSubmit={onCreateTask} className="grid grid-cols-[1fr_1.4fr_auto] gap-2 border-b border-neutral-200 bg-white px-4 py-3">
        <Input value={newTitle} onChange={(event) => onNewTitleChange(event.target.value)} placeholder="New task title" />
        <Input
          value={newDescription}
          onChange={(event) => onNewDescriptionChange(event.target.value)}
          placeholder="Optional spec or description"
        />
        <Button type="submit" disabled={!newTitle.trim() || pendingAction === "create"}>
          <Plus className="h-4 w-4" />
          New task
        </Button>
      </form>

      <div className="flex h-8 items-center justify-between border-b border-neutral-200 bg-white px-4 text-xs text-neutral-500">
        <span>
          {pageRangeLabel(page, visibleTaskCount)}
          {taskActivityLabel}
        </span>
        {view === "board" ? (
          <span className="flex items-center gap-2">
            <Button variant="ghost" size="sm" disabled={!hasPreviousPage || tasksRefreshing} onClick={onPreviousPage}>
              Previous
            </Button>
            <Button variant="ghost" size="sm" disabled={!hasNextPage || tasksRefreshing} onClick={onNextPage}>
              Next
            </Button>
          </span>
        ) : null}
      </div>
    </>
  )
}

function MainView({
  api,
  config,
  view,
  columns,
  tasks,
  groupedTasks,
  selectedTask,
  selectedId,
  detail,
  onSelectTask,
  onDropTask,
  page,
  hasNextPage,
  hasPreviousPage,
  canGoLastPage,
  rowsPerPage,
  tasksRefreshing,
  onFirstPage,
  onPreviousPage,
  onNextPage,
  onLastPage,
  onRowsPerPageChange,
}: {
  api: KanbanApi | null
  config: RuntimeConfig | null
  view: OperatorView
  columns: BoardColumn[]
  tasks: Task[]
  groupedTasks: Map<TaskStatus, Task[]>
  selectedTask: Task | null
  selectedId: string | null
  detail: DetailState
  onSelectTask: (taskId: string) => void
  onDropTask: (taskId: string, targetStatus: TaskStatus) => void
  page: PageMeta
  hasNextPage: boolean
  hasPreviousPage: boolean
  canGoLastPage: boolean
  rowsPerPage: number
  tasksRefreshing: boolean
  onFirstPage: () => void
  onPreviousPage: () => void
  onNextPage: () => void
  onLastPage: () => void
  onRowsPerPageChange: (value: number) => void
}) {
  if (view === "board") {
    return (
      <BoardView
        columns={columns}
        groupedTasks={groupedTasks}
        selectedId={selectedTask?.id ?? selectedId ?? undefined}
        dependencies={detail.dependencies}
        onSelectTask={onSelectTask}
        onDropTask={onDropTask}
      />
    )
  }
  if (view === "list") {
    return (
      <ListView
        tasks={tasks}
        selectedId={selectedId}
        page={page}
        hasNextPage={hasNextPage}
        hasPreviousPage={hasPreviousPage}
        canGoLastPage={canGoLastPage}
        rowsPerPage={rowsPerPage}
        tasksRefreshing={tasksRefreshing}
        onSelectTask={onSelectTask}
        onFirstPage={onFirstPage}
        onPreviousPage={onPreviousPage}
        onNextPage={onNextPage}
        onLastPage={onLastPage}
        onRowsPerPageChange={onRowsPerPageChange}
      />
    )
  }
  if (view === "events") return <EventsView api={api} />
  if (view === "runs") return <RunsView selectedTask={selectedTask} detail={detail} />
  if (view === "maintenance") return <MaintenanceView api={api} />
  if (view === "health") return <HealthView api={api} config={config} />
  return <SettingsView config={config} />
}

function StatusBar({
  lastRefreshAt,
  queueCounts,
}: {
  lastRefreshAt: number | null
  queueCounts: { ready: number; running: number; blocked: number }
}) {
  return (
    <footer className="flex h-8 items-center justify-between border-t border-neutral-200 bg-white px-4 text-xs text-neutral-500">
      <span>Last refresh {lastRefreshAt ? new Date(lastRefreshAt).toLocaleTimeString() : "-"}</span>
      <span>
        ready {queueCounts.ready} / running {queueCounts.running} / blocked {queueCounts.blocked}
      </span>
    </footer>
  )
}

function SearchBackendBadge({ meta }: { meta: SearchTasksMeta }) {
  return (
    <Badge variant={meta.stale ? "review" : "secondary"}>
      search {meta.backend}
      {meta.stale ? " stale/degraded" : ""}
      {meta.index_lag_events && meta.index_lag_events > 0 ? ` +${meta.index_lag_events}` : ""}
    </Badge>
  )
}

function NavItem({
  icon: Icon,
  label,
  active = false,
  onClick,
}: {
  icon: ElementType
  label: string
  active?: boolean
  onClick: () => void
}) {
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-neutral-600",
        active && "bg-neutral-100 text-neutral-950",
      )}
      onClick={onClick}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  )
}

function NavGroup({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-1 px-2 text-[11px] font-medium uppercase tracking-normal text-neutral-400">{label}</div>
      <div className="space-y-1">{children}</div>
    </div>
  )
}

function viewLabel(view: OperatorView): string {
  return viewMetadata[view].label
}

function viewIcon(view: OperatorView): ElementType {
  return viewMetadata[view].icon
}
