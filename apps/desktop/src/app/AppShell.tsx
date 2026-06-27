import { lazy, memo, Suspense, useEffect, useState, type ElementType, type FormEvent, type ReactNode, type TransitionEvent } from "react"
import {
  Activity,
  ChevronDown,
  Database,
  DatabaseBackup,
  HeartPulse,
  Inbox,
  Loader2,
  Map,
  Network,
  RefreshCcw,
  Search,
  Settings,
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
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Input } from "@/components/ui/input"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "@/components/ui/sidebar"
import { Sheet, SheetContent } from "@/components/ui/sheet"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { shouldOpenTaskDetailSheet } from "@/app/task-selection"
import { BoardView } from "@/features/board/BoardView"
import { EventsView } from "@/features/events/EventsView"
import { HealthView } from "@/features/health/HealthView"
import { ListView } from "@/features/list/ListView"
import type { OperatorView } from "@/features/navigation/view-types"
import { primaryViews, sidebarViews } from "@/features/navigation/view-types"
import { RunsView } from "@/features/runs/RunsView"
import { SettingsView } from "@/features/settings/SettingsView"
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
import type { RunActionOptions } from "@/app/useTaskMutations"
import type {
  BoardColumn,
  Board,
  KanbanApi,
  LabelSuggestionResult,
  Run,
  RuntimeConfig,
  SearchTasksMeta,
  Task,
  TaskPlanFilter,
  TaskStatus,
  PageMeta,
} from "@/lib/api"
import { cn } from "@/lib/utils"

const viewMetadata: Record<OperatorView, { label: string; icon: ElementType }> = {
  board: { label: "Board", icon: SquareKanban },
  list: { label: "List", icon: Inbox },
  map: { label: "Map", icon: Map },
  events: { label: "Events", icon: Activity },
  runs: { label: "Runs", icon: TerminalSquare },
  ontology: { label: "Review", icon: Network },
  maintenance: { label: "Maintenance", icon: DatabaseBackup },
  health: { label: "Health", icon: HeartPulse },
  settings: { label: "Settings", icon: Settings },
}

const themeModeOptions: MenuSelectOption<ThemeMode>[] = [
  { value: "system", label: "system" },
  { value: "light", label: "light" },
  { value: "dark", label: "dark" },
]

const BoardTaskMapView = lazy(() => import("@/features/task-map/BoardTaskMapView").then((module) => ({ default: module.BoardTaskMapView })))
const MaintenanceView = lazy(() => import("@/features/maintenance/MaintenanceView").then((module) => ({ default: module.MaintenanceView })))
const OntologyReviewWorkbench = lazy(() =>
  import("@/features/ontology/OntologyReviewWorkbench").then((module) => ({ default: module.OntologyReviewWorkbench })),
)
const TaskDetail = lazy(() => import("@/features/task-detail/TaskDetail").then((module) => ({ default: module.TaskDetail })))

export type AppShellRuntimeProps = {
  config: RuntimeConfig | null
  api: KanbanApi | null
  boards: Board[]
  boardsLoading: boolean
  boardsError: string | null
  themeMode: ThemeMode
  pendingAction: string | null
  error: string | null
  lastRefreshAt: number | null
  queueCounts: { ready: number; running: number; blocked: number }
}

export type AppShellNavigationProps = {
  view: OperatorView
  sidebarOpen: boolean
}

export type AppShellTaskCollectionProps = {
  columns: BoardColumn[]
  tasks: Task[]
  groupedTasks: Map<TaskStatus, Task[]>
  search: string
  debouncedSearch: string
  searchMeta: SearchTasksMeta | null
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  planFilters: TaskPlanFilter[]
  listSort: ListSortState
  showArchived: boolean
  page: PageMeta
  hasNextPage: boolean
  hasPreviousPage: boolean
  canGoLastPage: boolean
  rowsPerPage: number
  tasksRefreshing: boolean
}

export type AppShellTaskDetailProps = {
  selectedTask: Task | null
  selectedId: string | null
  dependencySnapshot: SelectedDependencySnapshot
  detail: DetailState
  labelSuggestions: LabelSuggestionResult | null
  labelSuggestionsRequested: boolean
  labelSuggestionsLoading: boolean
  labelSuggestionsError: string | null
  activeRun?: Run
  blockReason: string
  dependencyInput: string
  commentBody: string
  editDraft: TaskEditDraft | null
  draftDirty: boolean
  claimToken: string | null
  detailLoading: boolean
  taskCommentsExpanded: boolean
  taskDependenciesExpanded: boolean
  taskEventsExpanded: boolean
  taskGraphExpanded: boolean
  taskRunsExpanded: boolean
  taskStepsExpanded: boolean
}

export type AppShellTaskCreationProps = {
  open: boolean
  title: string
  description: string
  firstStepTitle: string
}

export type AppShellCommandProps = {
  addComment: () => Promise<void>
  addDependency: () => Promise<void>
  cancelTaskEdit: () => void
  changeBoard: (board: string) => void
  changeThemeMode: (value: ThemeMode) => void
  closeTaskDetail: () => void
  createTask: () => Promise<boolean>
  cycleThemeMode: () => void
  dropTask: (taskId: string, targetStatus: TaskStatus) => void
  firstPage: () => void
  lastPage: () => void
  nextPage: () => void
  previousPage: () => void
  refreshTasks: () => void
  removeDependency: (parentTaskId: string) => Promise<void>
  requestLabelSuggestions: () => void
  resetListFilters: () => void
  runAction: (action: () => Promise<unknown>, options?: RunActionOptions | string) => Promise<unknown>
  saveTask: () => Promise<boolean>
  selectTask: (taskId: string) => void
  setBlockReason: (value: string) => void
  setCommentBody: (value: string) => void
  setDependencyInput: (value: string) => void
  setEditDraft: (value: TaskEditDraft) => void
  setListSort: (value: ListSortState) => void
  setPlanFilters: (value: TaskPlanFilter[]) => void
  setPriorityFilters: (value: number[]) => void
  setRowsPerPage: (value: number) => void
  setSearch: (value: string) => void
  setShowArchived: (value: boolean) => void
  setSidebarOpen: (value: boolean) => void
  setStatusFilter: (value: TaskStatus | "all") => void
  setTaskCommentsExpanded: (value: boolean) => void
  setTaskCreationDescription: (value: string) => void
  setTaskCreationFirstStepTitle: (value: string) => void
  setTaskCreationOpen: (value: boolean) => void
  setTaskCreationTitle: (value: string) => void
  setTaskDependenciesExpanded: (value: boolean) => void
  setTaskEventsExpanded: (value: boolean) => void
  setTaskGraphExpanded: (value: boolean) => void
  setTaskRunsExpanded: (value: boolean) => void
  setTaskStepsExpanded: (value: boolean) => void
  setView: (value: OperatorView) => void
}

export type AppShellProps = {
  runtime: AppShellRuntimeProps
  navigation: AppShellNavigationProps
  taskCollection: AppShellTaskCollectionProps
  taskDetail: AppShellTaskDetailProps
  taskCreation: AppShellTaskCreationProps
  commands: AppShellCommandProps
}

export function AppShell({ runtime, navigation, taskCollection, taskDetail, taskCreation, commands }: AppShellProps) {
  const { api, boards, boardsError, boardsLoading, config, error, lastRefreshAt, pendingAction, queueCounts, themeMode } = runtime
  const { sidebarOpen, view } = navigation
  const { description: newDescription, firstStepTitle: newFirstStepTitle, open: taskCreationOpen, title: newTitle } = taskCreation
  const {
    canGoLastPage,
    columns,
    debouncedSearch,
    groupedTasks,
    hasNextPage,
    hasPreviousPage,
    listSort,
    page,
    planFilters,
    priorityFilters,
    rowsPerPage,
    search,
    searchMeta,
    showArchived,
    statusFilter,
    tasks,
    tasksRefreshing,
  } = taskCollection
  const {
    dependencySnapshot,
    detail,
    selectedId,
    selectedTask,
  } = taskDetail
  const showDetailSheet = shouldOpenTaskDetailSheet(view, selectedTask)

  return (
    <SidebarProvider
      open={sidebarOpen}
      onOpenChange={commands.setSidebarOpen}
      className="h-screen w-screen overflow-hidden bg-background text-foreground"
    >
      <MemoShellSidebar
        config={config}
        boards={boards}
        boardsLoading={boardsLoading}
        boardsError={boardsError}
        switchingBoard={pendingAction === "board"}
        view={view}
        open={sidebarOpen}
        onBoardChange={commands.changeBoard}
        onViewChange={commands.setView}
      />

      <SidebarInset className="flex flex-col overflow-hidden bg-background">
        <MemoShellHeader
          config={config}
          view={view}
          canCreateTask={Boolean(api)}
          themeMode={themeMode}
          sidebarOpen={sidebarOpen}
          search={search}
          debouncedSearch={debouncedSearch}
          searchMeta={searchMeta}
          showArchived={showArchived}
          newTitle={newTitle}
          newDescription={newDescription}
          newFirstStepTitle={newFirstStepTitle}
          tasksRefreshing={tasksRefreshing}
          pendingAction={pendingAction}
          taskCreationOpen={taskCreationOpen}
          onSearchChange={commands.setSearch}
          onViewChange={commands.setView}
          onThemeModeChange={commands.changeThemeMode}
          onCycleThemeMode={commands.cycleThemeMode}
          onSidebarOpenChange={commands.setSidebarOpen}
          onShowArchivedChange={commands.setShowArchived}
          onRefreshTasks={commands.refreshTasks}
          onCreateTask={commands.createTask}
          onNewTitleChange={commands.setTaskCreationTitle}
          onNewDescriptionChange={commands.setTaskCreationDescription}
          onNewFirstStepTitleChange={commands.setTaskCreationFirstStepTitle}
          onTaskCreationOpenChange={commands.setTaskCreationOpen}
        />

        {error ? (
          <div
            role="alert"
            aria-live="assertive"
            className="border-b border-[var(--status-blocked-ring)] bg-[var(--status-blocked-bg)] px-4 py-2 text-sm text-[var(--status-blocked-fg)]"
          >
            {error}
          </div>
        ) : null}

        <div className="flex min-h-0 min-w-0 flex-1">
          <section className="flex min-w-0 flex-1 flex-col">
            <MemoMainView
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
              onSelectTask={commands.selectTask}
              onDropTask={commands.dropTask}
              page={page}
              hasNextPage={hasNextPage}
              hasPreviousPage={hasPreviousPage}
              canGoLastPage={canGoLastPage}
              rowsPerPage={rowsPerPage}
              statusFilter={statusFilter}
              priorityFilters={priorityFilters}
              planFilters={planFilters}
              listSort={listSort}
              tasksRefreshing={tasksRefreshing}
              onStatusFilterChange={commands.setStatusFilter}
              onPriorityFiltersChange={commands.setPriorityFilters}
              onPlanFiltersChange={commands.setPlanFilters}
              onListSortChange={commands.setListSort}
              onResetListFilters={commands.resetListFilters}
              onFirstPage={commands.firstPage}
              onPreviousPage={commands.previousPage}
              onNextPage={commands.nextPage}
              onLastPage={commands.lastPage}
              onRowsPerPageChange={commands.setRowsPerPage}
            />
          </section>

          <MemoTaskDetailSheet
            api={api}
            open={showDetailSheet}
            pendingAction={pendingAction}
            taskDetail={taskDetail}
            commands={commands}
          />
        </div>

        <MemoStatusBar lastRefreshAt={lastRefreshAt} queueCounts={queueCounts} />
      </SidebarInset>
    </SidebarProvider>
  )
}

const MemoShellSidebar = memo(ShellSidebar)
const MemoShellHeader = memo(ShellHeader)
const MemoMainView = memo(MainView)
const MemoTaskDetailSheet = memo(TaskDetailSheet)
const MemoStatusBar = memo(StatusBar)

function TaskDetailSheet({
  api,
  commands,
  open,
  pendingAction,
  taskDetail,
}: {
  api: KanbanApi | null
  commands: AppShellCommandProps
  open: boolean
  pendingAction: string | null
  taskDetail: AppShellTaskDetailProps
}) {
  const {
    activeRun,
    blockReason,
    claimToken,
    commentBody,
    dependencyInput,
    detail,
    detailLoading,
    draftDirty,
    editDraft,
    labelSuggestions,
    labelSuggestionsError,
    labelSuggestionsLoading,
    labelSuggestionsRequested,
    selectedTask,
    taskCommentsExpanded,
    taskDependenciesExpanded,
    taskEventsExpanded,
    taskGraphExpanded,
    taskRunsExpanded,
    taskStepsExpanded,
  } = taskDetail

  return (
    <Sheet
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) commands.closeTaskDetail()
      }}
    >
      <SheetContent side="right" className="w-[min(1100px,calc(100vw-24px))] p-0">
        <Suspense fallback={<LazyViewFallback label="Loading task detail" />}>
          <TaskDetail
            api={api}
            task={selectedTask}
            detail={detail}
            labelSuggestions={labelSuggestions}
            labelSuggestionsRequested={labelSuggestionsRequested}
            labelSuggestionsLoading={labelSuggestionsLoading}
            labelSuggestionsError={labelSuggestionsError}
            activeRun={activeRun}
            blockReason={blockReason}
            setBlockReason={commands.setBlockReason}
            dependencyInput={dependencyInput}
            setDependencyInput={commands.setDependencyInput}
            claimToken={claimToken}
            commentBody={commentBody}
            setCommentBody={commands.setCommentBody}
            editDraft={editDraft}
            draftDirty={draftDirty}
            setEditDraft={commands.setEditDraft}
            detailLoading={detailLoading}
            commentsExpanded={taskCommentsExpanded}
            dependenciesExpanded={taskDependenciesExpanded}
            eventsExpanded={taskEventsExpanded}
            graphExpanded={taskGraphExpanded}
            runsExpanded={taskRunsExpanded}
            stepsExpanded={taskStepsExpanded}
            pendingAction={pendingAction}
            onAction={commands.runAction}
            onAddDependency={commands.addDependency}
            onRemoveDependency={commands.removeDependency}
            onRequestLabelSuggestions={commands.requestLabelSuggestions}
            onSelectTask={commands.selectTask}
            onSaveTask={commands.saveTask}
            onCancelEdit={commands.cancelTaskEdit}
            onAddComment={commands.addComment}
            onCommentsExpandedChange={commands.setTaskCommentsExpanded}
            onDependenciesExpandedChange={commands.setTaskDependenciesExpanded}
            onEventsExpandedChange={commands.setTaskEventsExpanded}
            onGraphExpandedChange={commands.setTaskGraphExpanded}
            onRunsExpandedChange={commands.setTaskRunsExpanded}
            onStepsExpandedChange={commands.setTaskStepsExpanded}
          />
        </Suspense>
      </SheetContent>
    </Sheet>
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
    <Sidebar
      className={cn(
        open ? "w-60 max-sm:w-14" : "w-14",
      )}
      onTransitionEnd={handleTransitionEnd}
    >
      <SidebarHeader className={cn(!contentOpen && "justify-center px-2")}>
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-semibold text-primary-foreground">
          kb
        </div>
        <div className={cn("min-w-0 max-sm:hidden", !contentOpen && "hidden")}>
          <div className="text-sm font-semibold">Kanban Tool</div>
          <div className="text-xs text-muted-foreground">local queue</div>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarNavGroup label="Task Explorer" open={contentOpen}>
          {sidebarViews.filter((item) => ["board", "list", "map", "runs", "events", "ontology"].includes(item)).map((item) => (
            <SidebarNavItem
              key={item}
              icon={viewIcon(item)}
              label={viewLabel(item)}
              active={view === item}
              open={contentOpen}
              onClick={() => onViewChange(item)}
            />
          ))}
        </SidebarNavGroup>
        <SidebarNavGroup label="System" open={contentOpen}>
          {sidebarViews.filter((item) => ["maintenance", "health", "settings"].includes(item)).map((item) => (
            <SidebarNavItem
              key={item}
              icon={viewIcon(item)}
              label={viewLabel(item)}
              active={view === item}
              open={contentOpen}
              onClick={() => onViewChange(item)}
            />
          ))}
        </SidebarNavGroup>
      </SidebarContent>
      <SidebarFooter className={cn(!contentOpen && "hidden")}>
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
      </SidebarFooter>
    </Sidebar>
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
  canCreateTask,
  themeMode,
  sidebarOpen,
  search,
  debouncedSearch,
  searchMeta,
  showArchived,
  newTitle,
  newDescription,
  newFirstStepTitle,
  tasksRefreshing,
  pendingAction,
  taskCreationOpen,
  onSearchChange,
  onViewChange,
  onThemeModeChange,
  onCycleThemeMode,
  onSidebarOpenChange,
  onShowArchivedChange,
  onRefreshTasks,
  onCreateTask,
  onNewTitleChange,
  onNewDescriptionChange,
  onNewFirstStepTitleChange,
  onTaskCreationOpenChange,
}: {
  config: RuntimeConfig | null
  view: OperatorView
  canCreateTask: boolean
  themeMode: ThemeMode
  sidebarOpen: boolean
  search: string
  debouncedSearch: string
  searchMeta: SearchTasksMeta | null
  showArchived: boolean
  newTitle: string
  newDescription: string
  newFirstStepTitle: string
  tasksRefreshing: boolean
  pendingAction: string | null
  taskCreationOpen: boolean
  onSearchChange: (value: string) => void
  onViewChange: (value: OperatorView) => void
  onThemeModeChange: (value: ThemeMode) => void
  onCycleThemeMode: () => void
  onSidebarOpenChange: (value: boolean) => void
  onShowArchivedChange: (value: boolean) => void
  onRefreshTasks: () => void
  onCreateTask: () => Promise<boolean>
  onNewTitleChange: (value: string) => void
  onNewDescriptionChange: (value: string) => void
  onNewFirstStepTitleChange: (value: string) => void
  onTaskCreationOpenChange: (value: boolean) => void
}) {
  const ThemeIcon = themeMode === "dark" ? Moon : themeMode === "light" ? Sun : Monitor
  const showAddTask = shouldShowTaskExplorerToolbar(view)
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
          aria-label="Search tasks"
          name="task-search"
          autoComplete="off"
          placeholder="Search tasks"
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
        />
      </div>
      {debouncedSearch.trim() && searchMeta ? <SearchBackendBadge meta={searchMeta} /> : null}
      <Button
        variant="secondary"
        size="icon"
        aria-label={tasksRefreshing ? "Refreshing tasks" : "Refresh tasks"}
        title={tasksRefreshing ? "Refreshing tasks" : "Refresh tasks"}
        onClick={onRefreshTasks}
        disabled={tasksRefreshing}
      >
        {tasksRefreshing ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCcw className="h-4 w-4" />}
      </Button>
      <Tabs value={primaryViews.includes(view) ? view : ""} onValueChange={(value) => onViewChange(value as OperatorView)}>
        <TabsList>
          {primaryViews.map((item) => (
            <TabsTrigger key={item} value={item}>
              {viewLabel(item)}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
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
        {showAddTask ? (
          <AddTaskDialog
            canCreateTask={canCreateTask}
            open={taskCreationOpen}
            newTitle={newTitle}
            newDescription={newDescription}
            newFirstStepTitle={newFirstStepTitle}
            pendingAction={pendingAction}
            onCreateTask={onCreateTask}
            onOpenChange={onTaskCreationOpenChange}
            onNewTitleChange={onNewTitleChange}
            onNewDescriptionChange={onNewDescriptionChange}
            onNewFirstStepTitleChange={onNewFirstStepTitleChange}
          />
        ) : null}
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
      </div>
    </header>
  )
}

function AddTaskDialog({
  canCreateTask,
  open,
  newTitle,
  newDescription,
  newFirstStepTitle,
  pendingAction,
  onCreateTask,
  onOpenChange,
  onNewTitleChange,
  onNewDescriptionChange,
  onNewFirstStepTitleChange,
}: {
  canCreateTask: boolean
  open: boolean
  newTitle: string
  newDescription: string
  newFirstStepTitle: string
  pendingAction: string | null
  onCreateTask: () => Promise<boolean>
  onOpenChange: (value: boolean) => void
  onNewTitleChange: (value: string) => void
  onNewDescriptionChange: (value: string) => void
  onNewFirstStepTitleChange: (value: string) => void
}) {
  const creating = pendingAction === "create"

  async function submitTask(event: FormEvent) {
    event.preventDefault()
    if (!canCreateTask || !newTitle.trim() || creating) return
    const created = await onCreateTask()
    if (created) onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <Button type="button" aria-label="Add task" onClick={() => onOpenChange(true)}>
        <Plus className="h-4 w-4" />
        Add task
      </Button>
      <DialogContent>
        <form onSubmit={submitTask} className="space-y-4">
          <DialogHeader>
            <DialogTitle>Add task</DialogTitle>
            <DialogDescription>Create a task on the active board.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <Input
              aria-label="New task title"
              name="new-task-title"
              autoComplete="off"
              value={newTitle}
              onChange={(event) => onNewTitleChange(event.target.value)}
              placeholder="Title"
            />
            <Textarea
              aria-label="New task description"
              name="new-task-description"
              autoComplete="off"
              value={newDescription}
              onChange={(event) => onNewDescriptionChange(event.target.value)}
              placeholder="Optional spec or description"
            />
            <Input
              aria-label="First step title"
              name="new-first-step-title"
              autoComplete="off"
              value={newFirstStepTitle}
              onChange={(event) => onNewFirstStepTitleChange(event.target.value)}
              placeholder="Optional first required step"
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={!canCreateTask || !newTitle.trim() || creating}>
              {creating ? "Creating…" : "Create task"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
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
  statusFilter,
  priorityFilters,
  planFilters,
  listSort,
  tasksRefreshing,
  onStatusFilterChange,
  onPriorityFiltersChange,
  onPlanFiltersChange,
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
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  planFilters: TaskPlanFilter[]
  listSort: ListSortState
  tasksRefreshing: boolean
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onPriorityFiltersChange: (value: number[]) => void
  onPlanFiltersChange: (value: TaskPlanFilter[]) => void
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
  if (view === "map") {
    return (
      <Suspense fallback={<LazyViewFallback label="Loading task map" />}>
        <BoardTaskMapView api={api} selectedTaskId={selectedId} onSelectTask={onSelectTask} />
      </Suspense>
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
        statusFilter={statusFilter}
        priorityFilters={priorityFilters}
        planFilters={planFilters}
        listSort={listSort}
        tasksRefreshing={tasksRefreshing}
        onStatusFilterChange={onStatusFilterChange}
        onPriorityFiltersChange={onPriorityFiltersChange}
        onPlanFiltersChange={onPlanFiltersChange}
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
  if (view === "ontology") {
    return (
      <Suspense fallback={<LazyViewFallback label="Loading review workbench" />}>
        <OntologyReviewWorkbench api={api} />
      </Suspense>
    )
  }
  if (view === "maintenance") {
    return (
      <Suspense fallback={<LazyViewFallback label="Loading maintenance" />}>
        <MaintenanceView api={api} />
      </Suspense>
    )
  }
  if (view === "health") return <HealthView api={api} config={config} />
  return <SettingsView config={config} />
}

function LazyViewFallback({ label }: { label: string }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
      <div className="text-sm text-muted-foreground">{label}</div>
      <Skeleton className="h-10 w-72" />
      <Skeleton className="h-48 w-full" />
      <Skeleton className="h-32 w-2/3" />
    </div>
  )
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

function SidebarNavItem({
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
    <SidebarMenuItem>
      <SidebarMenuButton
        title={!open ? label : undefined}
        aria-label={label}
        className={cn(
          !open && "justify-center",
        )}
        active={active}
        onClick={onClick}
      >
        <Icon className="h-4 w-4 shrink-0" />
        <span className={cn("max-sm:sr-only", !open && "sr-only")}>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}

function SidebarNavGroup({ label, open, children }: { label: string; open: boolean; children: ReactNode }) {
  return (
    <SidebarGroup>
      <SidebarGroupLabel className={cn(!open && "sr-only")}>{label}</SidebarGroupLabel>
      <SidebarMenu>{children}</SidebarMenu>
    </SidebarGroup>
  )
}

function viewLabel(view: OperatorView): string {
  return viewMetadata[view].label
}

function viewIcon(view: OperatorView): ElementType {
  return viewMetadata[view].icon
}
