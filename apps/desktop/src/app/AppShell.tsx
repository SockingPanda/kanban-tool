import { useEffect, useState, type ElementType, type FormEvent, type ReactNode, type TransitionEvent } from "react"
import {
  Activity,
  ChevronDown,
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
  Monitor,
  Moon,
  PanelLeft,
  Sun,
} from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Input } from "@/components/ui/input"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { Sheet, SheetContent } from "@/components/ui/sheet"
import { Skeleton } from "@/components/ui/skeleton"
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
import {
  isSidebarWidthTransition,
  nextSidebarContentOpen,
  SIDEBAR_WIDTH_TRANSITION_MS,
} from "@/app/sidebar-animation"
import type { ThemeMode } from "@/app/theme"
import type { DetailState } from "@/features/task-detail/detail-state"
import type { TaskEditDraft } from "@/features/task-detail/task-draft"
import type { SelectedDependencySnapshot } from "@/features/board/board-card-state"
import { apiEndpointLabel, shouldShowTaskExplorerToolbar } from "@/app/shell-rules"
import type { ListSortState } from "@/features/list/table-state"
import type {
  BoardColumn,
  Board,
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

const statusFilterOptions: MenuSelectOption<TaskStatus | "all">[] = [
  { value: "all", label: "all active" },
  ...filterStatuses.map((status) => ({ value: status, label: status })),
]

const themeModeOptions: MenuSelectOption<ThemeMode>[] = [
  { value: "system", label: "system" },
  { value: "light", label: "light" },
  { value: "dark", label: "dark" },
]

export function AppShell({
  config,
  api,
  boards,
  boardsLoading,
  boardsError,
  view,
  themeMode,
  sidebarOpen,
  columns,
  tasks,
  groupedTasks,
  selectedTask,
  selectedId,
  dependencySnapshot,
  detail,
  activeRun,
  search,
  debouncedSearch,
  searchMeta,
  statusFilter,
  priorityFilters,
  listSort,
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
  onBoardChange,
  onViewChange,
  onThemeModeChange,
  onCycleThemeMode,
  onSidebarOpenChange,
  onStatusFilterChange,
  onPriorityFiltersChange,
  onListSortChange,
  onResetListFilters,
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
  boards: Board[]
  boardsLoading: boolean
  boardsError: string | null
  view: OperatorView
  themeMode: ThemeMode
  sidebarOpen: boolean
  columns: BoardColumn[]
  tasks: Task[]
  groupedTasks: Map<TaskStatus, Task[]>
  selectedTask: Task | null
  selectedId: string | null
  dependencySnapshot: SelectedDependencySnapshot
  detail: DetailState
  activeRun?: Run
  search: string
  debouncedSearch: string
  searchMeta: SearchTasksMeta | null
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  listSort: ListSortState
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
  onBoardChange: (board: string) => void
  onViewChange: (value: OperatorView) => void
  onThemeModeChange: (value: ThemeMode) => void
  onCycleThemeMode: () => void
  onSidebarOpenChange: (value: boolean) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onPriorityFiltersChange: (value: number[]) => void
  onListSortChange: (value: ListSortState) => void
  onResetListFilters: () => void
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
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      <ShellSidebar
        config={config}
        boards={boards}
        boardsLoading={boardsLoading}
        boardsError={boardsError}
        switchingBoard={pendingAction === "board"}
        view={view}
        open={sidebarOpen}
        onBoardChange={onBoardChange}
        onViewChange={onViewChange}
      />

      <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-background">
        <ShellHeader
          config={config}
          view={view}
          themeMode={themeMode}
          sidebarOpen={sidebarOpen}
          search={search}
          debouncedSearch={debouncedSearch}
          searchMeta={searchMeta}
          statusFilter={statusFilter}
          showArchived={showArchived}
          tasksRefreshing={tasksRefreshing}
          onSearchChange={onSearchChange}
          onViewChange={onViewChange}
          onThemeModeChange={onThemeModeChange}
          onCycleThemeMode={onCycleThemeMode}
          onSidebarOpenChange={onSidebarOpenChange}
          onStatusFilterChange={onStatusFilterChange}
          onShowArchivedChange={onShowArchivedChange}
          onRefreshTasks={onRefreshTasks}
        />

        {error ? (
          <div className="border-b border-[var(--status-blocked-ring)] bg-[var(--status-blocked-bg)] px-4 py-2 text-sm text-[var(--status-blocked-fg)]">{error}</div>
        ) : null}

        <div className="flex min-h-0 min-w-0 flex-1">
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
              dependencySnapshot={dependencySnapshot}
              detail={detail}
              onSelectTask={onSelectTask}
              onDropTask={onDropTask}
              page={page}
              hasNextPage={hasNextPage}
              hasPreviousPage={hasPreviousPage}
              canGoLastPage={canGoLastPage}
              rowsPerPage={rowsPerPage}
              search={search}
              statusFilter={statusFilter}
              priorityFilters={priorityFilters}
              listSort={listSort}
              tasksRefreshing={tasksRefreshing}
              onSearchChange={onSearchChange}
              onStatusFilterChange={onStatusFilterChange}
              onPriorityFiltersChange={onPriorityFiltersChange}
              onListSortChange={onListSortChange}
              onResetListFilters={onResetListFilters}
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
                onSelectTask={onSelectTask}
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
  boards,
  boardsLoading,
  boardsError,
  switchingBoard,
  view,
  open,
  onBoardChange,
  onViewChange,
}: {
  config: RuntimeConfig | null
  boards: Board[]
  boardsLoading: boolean
  boardsError: string | null
  switchingBoard: boolean
  view: OperatorView
  open: boolean
  onBoardChange: (value: string) => void
  onViewChange: (value: OperatorView) => void
}) {
  const [contentOpen, setContentOpen] = useState(open)

  useEffect(() => {
    setContentOpen((current) =>
      nextSidebarContentOpen(current, { type: "width-transition-start", sidebarOpen: open }),
    )
    const timeoutId = globalThis.setTimeout(() => {
      setContentOpen((current) =>
        nextSidebarContentOpen(current, { type: "width-transition-finish", sidebarOpen: open }),
      )
    }, SIDEBAR_WIDTH_TRANSITION_MS)

    return () => globalThis.clearTimeout(timeoutId)
  }, [open])

  function handleTransitionEnd(event: TransitionEvent<HTMLElement>) {
    if (event.currentTarget !== event.target || !isSidebarWidthTransition(event.propertyName)) return
    setContentOpen((current) =>
      nextSidebarContentOpen(current, { type: "width-transition-finish", sidebarOpen: open }),
    )
  }

  return (
    <aside
      className={cn(
        "flex shrink-0 flex-col overflow-hidden border-r border-border bg-sidebar transition-[width] duration-200",
        open ? "w-60 max-sm:w-14" : "w-14",
      )}
      onTransitionEnd={handleTransitionEnd}
    >
      <div className={cn("flex h-14 items-center gap-2 px-3 max-sm:justify-center max-sm:px-2", !contentOpen && "justify-center px-2")}>
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-semibold text-primary-foreground">
          kb
        </div>
        <div className={cn("min-w-0 max-sm:hidden", !contentOpen && "hidden")}>
          <div className="text-sm font-semibold">Kanban Tool</div>
          <div className="text-xs text-muted-foreground">local queue</div>
        </div>
      </div>
      <nav className="space-y-4 px-2 py-3">
        <NavGroup label="Task Explorer" open={contentOpen}>
          {sidebarViews.filter((item) => ["board", "list", "runs", "events"].includes(item)).map((item) => (
            <NavItem
              key={item}
              icon={viewIcon(item)}
              label={viewLabel(item)}
              active={view === item}
              open={contentOpen}
              onClick={() => onViewChange(item)}
            />
          ))}
        </NavGroup>
        <NavGroup label="System" open={contentOpen}>
          {sidebarViews.filter((item) => ["maintenance", "health", "settings"].includes(item)).map((item) => (
            <NavItem
              key={item}
              icon={viewIcon(item)}
              label={viewLabel(item)}
              active={view === item}
              open={contentOpen}
              onClick={() => onViewChange(item)}
            />
          ))}
        </NavGroup>
      </nav>
      <div className={cn("mt-auto space-y-3 border-t border-border p-3 text-xs text-muted-foreground max-sm:hidden", !contentOpen && "hidden")}>
        <BoardSwitcher
          config={config}
          boards={boards}
          loading={boardsLoading}
          error={boardsError}
          switching={switchingBoard}
          onBoardChange={onBoardChange}
        />
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

function BoardSwitcher({
  config,
  boards,
  loading,
  error,
  switching,
  onBoardChange,
}: {
  config: RuntimeConfig | null
  boards: Board[]
  loading: boolean
  error: string | null
  switching: boolean
  onBoardChange: (value: string) => void
}) {
  const activeBoard = boards.find((board) => board.slug === config?.board || board.id === config?.board)
  const activeLabel = activeBoard?.name ?? config?.board ?? "loading board"

  if (!config || loading) {
    return (
      <div className="space-y-2">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-4 w-2/3" />
      </div>
    )
  }

  return (
    <div className="space-y-2">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="outline"
            className="h-auto w-full justify-start gap-2 px-2 py-2 text-left"
            disabled={switching || boards.length === 0}
          >
            <SquareKanban className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-xs font-medium text-foreground">{activeLabel}</span>
              <span className="block truncate text-[11px] text-muted-foreground">board {config.board}</span>
            </span>
            <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="right" align="end" className="w-64">
          <DropdownMenuRadioGroup value={config.board} onValueChange={onBoardChange}>
            {boards.map((board) => (
              <DropdownMenuRadioItem key={board.id} value={board.slug} className="items-start gap-2">
                <span className="min-w-0 flex-1">
                  <span className="block truncate">{board.name}</span>
                  <span className="block truncate text-xs text-muted-foreground">{board.slug}</span>
                </span>
                {board.slug === config.board ? <Badge variant="secondary">active</Badge> : null}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      {error ? (
        <Alert className="px-2 py-1 text-xs">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}
    </div>
  )
}

function ShellHeader({
  config,
  view,
  themeMode,
  sidebarOpen,
  search,
  debouncedSearch,
  searchMeta,
  statusFilter,
  showArchived,
  tasksRefreshing,
  onSearchChange,
  onViewChange,
  onThemeModeChange,
  onCycleThemeMode,
  onSidebarOpenChange,
  onStatusFilterChange,
  onShowArchivedChange,
  onRefreshTasks,
}: {
  config: RuntimeConfig | null
  view: OperatorView
  themeMode: ThemeMode
  sidebarOpen: boolean
  search: string
  debouncedSearch: string
  searchMeta: SearchTasksMeta | null
  statusFilter: TaskStatus | "all"
  showArchived: boolean
  tasksRefreshing: boolean
  onSearchChange: (value: string) => void
  onViewChange: (value: OperatorView) => void
  onThemeModeChange: (value: ThemeMode) => void
  onCycleThemeMode: () => void
  onSidebarOpenChange: (value: boolean) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onShowArchivedChange: (value: boolean) => void
  onRefreshTasks: () => void
}) {
  const ThemeIcon = themeMode === "dark" ? Moon : themeMode === "light" ? Sun : Monitor
  return (
    <header className="flex min-h-14 flex-wrap items-center gap-2 border-b border-border bg-card px-3 py-2 sm:flex-nowrap sm:gap-3 sm:px-4">
      <Button
        variant="ghost"
        size="icon"
        aria-label={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
        title={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
        onClick={() => onSidebarOpenChange(!sidebarOpen)}
      >
        <PanelLeft className="h-4 w-4" />
      </Button>
      <div className="relative min-w-0 flex-1 basis-64 sm:w-80 sm:flex-none">
        <Search className="pointer-events-none absolute left-2.5 top-2 h-4 w-4 text-muted-foreground" />
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
      <div className="flex rounded-md border border-border bg-muted p-0.5 text-sm">
        {primaryViews.map((item) => (
          <Button
            key={item}
            type="button"
            variant="ghost"
            size="sm"
            className={cn(
              "h-7 px-3 text-muted-foreground hover:bg-background",
              view === item && "bg-background text-foreground shadow-sm",
            )}
            onClick={() => onViewChange(item)}
          >
            {viewLabel(item)}
          </Button>
        ))}
      </div>
      <div className="flex items-center gap-1">
        <SlidersHorizontal className="h-3.5 w-3.5 text-muted-foreground" />
        <MenuSelect
          ariaLabel="Status filter"
          prefix="Status"
          options={statusFilterOptions}
          value={statusFilter}
          onValueChange={onStatusFilterChange}
          triggerClassName="min-w-32"
        />
      </div>
      <Button
        type="button"
        variant={showArchived ? "secondary" : "outline"}
        size="sm"
        aria-pressed={showArchived}
        aria-label={showArchived ? "Archived tasks included" : "Archived tasks hidden"}
        onClick={() => onShowArchivedChange(!showArchived)}
      >
        Archived
      </Button>
      <div className="ml-auto flex items-center gap-2">
        <Badge variant="secondary">actor {config?.actor ?? "-"}</Badge>
        <Badge variant="ready">local dispatcher</Badge>
        <MenuSelect
          ariaLabel="Theme mode"
          prefix="Theme"
          options={themeModeOptions}
          value={themeMode}
          onValueChange={onThemeModeChange}
          triggerClassName="min-w-28"
          align="end"
        />
        <Button variant="ghost" size="icon" aria-label="Cycle theme mode" title={`Theme: ${themeMode}`} onClick={onCycleThemeMode}>
          <ThemeIcon className="h-4 w-4" />
        </Button>
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
      <form onSubmit={onCreateTask} className="grid grid-cols-[1fr_1.4fr_auto] gap-2 border-b border-border bg-card px-4 py-3">
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

      <div className="flex h-8 items-center justify-between border-b border-border bg-card px-4 text-xs text-muted-foreground">
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
  dependencySnapshot,
  detail,
  onSelectTask,
  onDropTask,
  page,
  hasNextPage,
  hasPreviousPage,
  canGoLastPage,
  rowsPerPage,
  search,
  statusFilter,
  priorityFilters,
  listSort,
  tasksRefreshing,
  onSearchChange,
  onStatusFilterChange,
  onPriorityFiltersChange,
  onListSortChange,
  onResetListFilters,
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
  dependencySnapshot: SelectedDependencySnapshot
  detail: DetailState
  onSelectTask: (taskId: string) => void
  onDropTask: (taskId: string, targetStatus: TaskStatus) => void
  page: PageMeta
  hasNextPage: boolean
  hasPreviousPage: boolean
  canGoLastPage: boolean
  rowsPerPage: number
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  listSort: ListSortState
  tasksRefreshing: boolean
  onSearchChange: (value: string) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onPriorityFiltersChange: (value: number[]) => void
  onListSortChange: (value: ListSortState) => void
  onResetListFilters: () => void
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
        dependencySnapshot={dependencySnapshot}
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
        search={search}
        statusFilter={statusFilter}
        priorityFilters={priorityFilters}
        listSort={listSort}
        tasksRefreshing={tasksRefreshing}
        onSearchChange={onSearchChange}
        onStatusFilterChange={onStatusFilterChange}
        onPriorityFiltersChange={onPriorityFiltersChange}
        onListSortChange={onListSortChange}
        onResetListFilters={onResetListFilters}
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
    <footer className="flex h-8 items-center justify-between border-t border-border bg-card px-4 text-xs text-muted-foreground">
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
  open = true,
  onClick,
}: {
  icon: ElementType
  label: string
  active?: boolean
  open?: boolean
  onClick: () => void
}) {
  return (
    <button
      title={!open ? label : undefined}
      aria-label={label}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
        !open && "justify-center",
        "max-sm:justify-center",
        active && "bg-sidebar-accent text-sidebar-accent-foreground",
      )}
      onClick={onClick}
    >
      <Icon className="h-4 w-4 shrink-0" />
      <span className={cn("max-sm:sr-only", !open && "sr-only")}>{label}</span>
    </button>
  )
}

function NavGroup({ label, open, children }: { label: string; open: boolean; children: ReactNode }) {
  return (
    <div>
      <div className={cn("mb-1 px-2 text-[11px] font-medium uppercase tracking-normal text-muted-foreground max-sm:sr-only", !open && "sr-only")}>{label}</div>
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
