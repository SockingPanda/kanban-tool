import { useState, type ReactNode } from "react"

import type { Meta, StoryObj } from "@storybook/react-vite"

import type { BoardTaskViewModel } from "../../features/board/types"
import type { TaskInspectorViewModel } from "../../features/explorer/TaskInspector"
import {
  BoardColumns,
  DisplayMenu,
  FilterBar,
  SidePeekFrame,
  TaskCard,
  TaskStateBoundary,
  TaskTable,
  ViewSwitcher,
  type BoardColumnProps,
  type TasksDensity,
  type TasksLocale,
  type TasksView,
} from "../../ui/tasks"
import storyStyles from "./tasks-stories.module.css"

const meta = {
  title: "Tasks/Patterns",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "Tasks interaction grammar 的 demo-only Storybook fixtures。只渲染 canonical board/task/inspector 字段，不连接 API、SSE 或 mutation。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

type StoryContext = { readonly globals: Record<string, unknown> }

function localeFor(value: unknown): TasksLocale {
  return value === "en" ? "en" : "zh"
}

function themeFor(value: unknown): "light" | "dark" {
  return value === "dark" ? "dark" : "light"
}

const STORY_COPY = {
  zh: {
    filterAll: "全部任务",
    statusRunning: "状态：运行中",
    planHasSteps: "计划：有步骤",
    footer: "仅 Storybook fixture；tasks.status 仍是唯一事实。",
    listTitle: "列表投影（Storybook）",
    listDescription: "列表只展示 canonical task 字段，不连接 API。",
    mapTitle: "关系图不可用",
    mapDescription: "Storybook 隔离层不模拟关系图；生产 Map 使用 canonical typed read model。",
    ref: "Ref",
    status: "状态",
    steps: "步骤",
    displayColumns: { assignee: "执行者", priority: "优先级" },
    sheetTitle: "窄屏详情 sheet",
  },
  en: {
    filterAll: "All tasks",
    statusRunning: "Status: Running",
    planHasSteps: "Plan: Has steps",
    footer: "Storybook fixture only; tasks.status remains the sole fact.",
    listTitle: "List projection (Storybook)",
    listDescription: "The list shows canonical task fields only; it does not connect to the API.",
    mapTitle: "Map unavailable",
    mapDescription: "Storybook keeps the relation graph isolated; production Map uses the canonical typed read model.",
    ref: "Ref",
    status: "Status",
    steps: "Steps",
    displayColumns: { assignee: "Assignee", priority: "Priority" },
    sheetTitle: "Narrow details sheet",
  },
} as const

function storyCopy(locale: TasksLocale) {
  return STORY_COPY[locale]
}

function displayColumns(locale: TasksLocale) {
  const copy = storyCopy(locale)
  return [{ id: "assignee", label: copy.displayColumns.assignee }, { id: "priority", label: copy.displayColumns.priority }] as const
}

const storyStatusLabels = {
  zh: { todo: "待办", ready: "就绪", running: "运行中", review: "待审核" },
  en: { todo: "To do", ready: "Ready", running: "Running", review: "Review" },
} as const

function storyStatusLabel(status: "todo" | "ready" | "running" | "review", locale: TasksLocale): string {
  return storyStatusLabels[locale][status]
}

/**
 * Demo-only fixture：字段与值均来自 canonical board/task read model 的允许范围。
 * Storybook 不把这些值当作生产数据，也不构造 owner、avatar、progress、risk 或 timeline date。
 */
function taskFixture(overrides: Partial<BoardTaskViewModel> = {}): BoardTaskViewModel {
  return {
    id: "t_demo",
    seq: 503,
    ref: "#503",
    title: "Plane-only Web UI",
    description: null,
    status: "ready",
    position: 0,
    scheduledAt: null,
    dueAt: null,
    lastHeartbeatAt: null,
    statusReason: null,
    labels: [],
    lockVersion: 1,
    priority: 1,
    assignee: null,
    readiness: {
      dependencyBlocked: false,
      unfinishedParentCount: 0,
      executionPlanState: "planned",
      requiredStepCount: 5,
      completedRequiredStepCount: 0,
      optionalStepCount: 0,
    },
    ...overrides,
  }
}

const tasks: readonly BoardTaskViewModel[] = [
  taskFixture({ id: "t_505", seq: 505, ref: "#505", title: "Design system foundations", status: "todo", position: 0, statusReason: "Waiting on dependency", readiness: { ...taskFixture().readiness, dependencyBlocked: true, executionPlanState: "unplanned", requiredStepCount: 0 } }),
  taskFixture({ id: "t_506", seq: 506, ref: "#506", title: "Projects shell and overview", status: "todo", position: 1, statusReason: "Waiting on dependency", readiness: { ...taskFixture().readiness, dependencyBlocked: true, executionPlanState: "unplanned", requiredStepCount: 0 } }),
  taskFixture({ id: "t_507", seq: 507, ref: "#507", title: "Tasks multi-view workspace", status: "todo", position: 2, statusReason: "Waiting on dependency", readiness: { ...taskFixture().readiness, dependencyBlocked: true, executionPlanState: "unplanned", requiredStepCount: 0 } }),
  taskFixture({ id: "t_508", seq: 508, ref: "#508", title: "Web/Desktop acceptance", status: "todo", position: 3, statusReason: "Waiting on dependency", readiness: { ...taskFixture().readiness, dependencyBlocked: true, executionPlanState: "unplanned", requiredStepCount: 0 } }),
  taskFixture({ id: "t_503", seq: 503, ref: "#503", title: "Plane-only Web UI", status: "ready", position: 0, readiness: { ...taskFixture().readiness, requiredStepCount: 5 } }),
  taskFixture({ id: "t_504", seq: 504, ref: "#504", title: "Storybook component lab", status: "running", position: 0, readiness: { ...taskFixture().readiness, requiredStepCount: 1 } }),
]

const columns: readonly BoardColumnProps[] = [
  { id: "todo", status: "todo", title: "Todo", tasks: tasks.filter((task) => task.status === "todo") },
  { id: "ready", status: "ready", title: "Ready", tasks: tasks.filter((task) => task.status === "ready") },
  { id: "running", status: "running", title: "Running", tasks: tasks.filter((task) => task.status === "running") },
  { id: "review", status: "review", title: "Review", tasks: [] },
  { id: "done", status: "done", title: "Done", tasks: [] },
  { id: "blocked", status: "blocked", title: "Blocked", tasks: [] },
]

const columnLabels = {
  zh: { todo: "待办", ready: "就绪", running: "运行中", review: "待审核", done: "已完成", blocked: "已阻塞" },
  en: { todo: "To do", ready: "Ready", running: "Running", review: "Review", done: "Done", blocked: "Blocked" },
} as const

function columnsFor(locale: TasksLocale): readonly BoardColumnProps[] {
  const labels = columnLabels[locale]
  return columns.map((column) => ({ ...column, title: labels[column.id as keyof typeof labels] ?? column.title }))
}

const selectedTask = tasks.find((task) => task.id === "t_504") ?? tasks[0]

function ListProjection({ locale }: { readonly locale: TasksLocale }) {
  const copy = storyCopy(locale)
  return (
    <section className={storyStyles.projectionNotice} aria-labelledby="tasks-list-projection-title">
      <div>
        <h2 id="tasks-list-projection-title">{copy.listTitle}</h2>
        <p>{copy.listDescription}</p>
      </div>
      <ul className={storyStyles.projectionList} aria-label={copy.listTitle}>
        {tasks.map((task) => (
          <li key={task.id} className={storyStyles.projectionListItem}>
            <span className={storyStyles.projectionListMeta} translate="no">{task.ref}</span>
            <span className={storyStyles.projectionListTitle}>{task.title}</span>
            <span className={storyStyles.projectionListMeta}>{storyStatusLabel(task.status as "todo" | "ready" | "running" | "review", locale)}</span>
          </li>
        ))}
      </ul>
    </section>
  )
}

function MapProjectionNotice({ locale }: { readonly locale: TasksLocale }) {
  const copy = storyCopy(locale)
  return (
    <section className={storyStyles.projectionNotice} aria-labelledby="tasks-map-projection-title">
      <h2 id="tasks-map-projection-title">{copy.mapTitle}</h2>
      <p>{copy.mapDescription}</p>
    </section>
  )
}

function TasksProjection({ view, displayVariant, locale, density, selectedId, onSelectTask }: {
  readonly view: TasksView
  readonly displayVariant: "grouped" | "table"
  readonly locale: TasksLocale
  readonly density?: TasksDensity
  readonly selectedId?: string | null
  readonly onSelectTask?: (task: BoardTaskViewModel) => void
}) {
  if (view === "board") return <BoardColumns columns={columnsFor(locale)} density={density} locale={locale} selectedTaskId={selectedId} onSelectTask={onSelectTask} />
  if (view === "list" && displayVariant === "table") return <TaskTable tasks={tasks} density={density} locale={locale} selectedTaskId={selectedId} onSelectTask={onSelectTask} />
  if (view === "list") return <ListProjection locale={locale} />
  return <MapProjectionNotice locale={locale} />
}

/** Demo-only inspector fixture; values use only canonical inspector fields. */
const inspector: TaskInspectorViewModel = {
  task: {
    id: "t_504",
    ref: "#504",
    title: "Storybook component lab",
    status: "running",
    lockVersion: 1,
    scheduledAt: null,
    dueAt: null,
    priority: 1,
    description: null,
    statusReason: null,
    assignee: null,
    executionPlanState: "planned",
    dependencyBlocked: false,
    unfinishedParentCount: 0,
    requiredStepCount: 1,
    completedRequiredStepCount: 0,
    optionalStepCount: 0,
    metadata: null,
    resultSummary: null,
    result: null,
    claimOwner: null,
    claimExpiresAt: null,
    lastHeartbeatAt: null,
    currentRunId: "r_01KZRMHQA8",
    retryCount: 0,
    maxRetries: null,
    createdAt: 0,
    updatedAt: 0,
  },
  steps: [{ id: "step_504", title: "Build a static Storybook", status: "todo", required: true, body: null }],
  parents: [],
  children: [],
  comments: [],
  runs: [],
  events: [],
  runtime: { actor: "storybook", apiBaseUrl: "/", serverVersion: "demo-only", protocolVersion: "demo-only", webBuildId: "demo-only" },
}

function Surface({ children, narrow = false, locale = "zh", theme = "light" }: { readonly children: ReactNode; readonly narrow?: boolean; readonly locale?: TasksLocale; readonly theme?: "light" | "dark" }) {
  return (
    <main className={narrow ? storyStyles.surfaceNarrow : storyStyles.surface} data-locale={locale} data-theme={theme}>
      <div className={narrow ? storyStyles.surfaceContentNarrow : storyStyles.surfaceContent}>{children}</div>
    </main>
  )
}

function WorkspaceBoardStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const copy = storyCopy(locale)
  const [view, setView] = useState<TasksView>("board")
  const [density, setDensity] = useState<TasksDensity>("dense")
  const [displayVariant, setDisplayVariant] = useState<"grouped" | "table">("grouped")
  const [selectedId, setSelectedId] = useState<string | null>("t_504")
  const [query, setQuery] = useState("")
  const [activeFilterIds, setActiveFilterIds] = useState(["all"])
  const filters = activeFilterIds.map((id) => ({ id, label: copy.filterAll, removable: false }))
  return (
    <Surface locale={locale} theme={theme}>
      <header className={storyStyles.surfaceHeader}>
        <div className={storyStyles.surfaceHeaderText}><p className={storyStyles.surfaceEyebrow}>kanban-tool / Tasks</p><h1 className={storyStyles.surfaceTitle}>{locale === "en" ? "Tasks workspace" : "任务工作区"}</h1></div>
        <div className={storyStyles.controlsRight}><ViewSwitcher activeView={view} displayVariant={displayVariant} includeTableDisplay onViewChange={setView} onDisplayChange={setDisplayVariant} locale={locale} /><DisplayMenu options={{ density }} onDensityChange={setDensity} columns={displayColumns(locale)} locale={locale} /></div>
      </header>
      <FilterBar search={query} onSearchChange={setQuery} filters={filters} locale={locale} onRemoveFilter={(id) => setActiveFilterIds((current) => current.filter((filterId) => filterId !== id))} onClearFilters={() => setActiveFilterIds([])} />
      <div className={storyStyles.workspace}>
        <TasksProjection view={view} displayVariant={displayVariant} locale={locale} density={density} selectedId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} />
      </div>
      <p role="status" className={storyStyles.demoNote}>{copy.footer}</p>
    </Surface>
  )
}

function SidePeekSelectedStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const [open, setOpen] = useState(true)
  return <Surface locale={locale} theme={theme}><div className={open ? storyStyles.workspaceWithPeek : storyStyles.workspace}><BoardColumns columns={columnsFor(locale)} locale={locale} selectedTaskId={open ? "t_504" : null} onSelectTask={() => setOpen(true)} />{open ? <SidePeekFrame mode="side-peek" model={inspector} locale={locale} onClose={() => setOpen(false)} onOpenDetails={() => setOpen(false)} /> : null}</div></Surface>
}

function SidePeekNarrowSheetStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const [open, setOpen] = useState(true)
  const copy = storyCopy(locale)
  return (
    <Surface narrow locale={locale} theme={theme}>
      <div className={storyStyles.workspace}>
        <BoardColumns columns={columnsFor(locale)} locale={locale} selectedTaskId={open ? "t_504" : null} onSelectTask={() => setOpen(true)} />
        {open ? <SidePeekFrame mode="sheet" model={inspector} locale={locale} onClose={() => setOpen(false)} onOpenDetails={() => setOpen(false)} /> : <p role="status" className={storyStyles.demoNote}>{copy.sheetTitle}</p>}
      </div>
    </Surface>
  )
}

function TableProjectionStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const [view, setView] = useState<TasksView>("list")
  const [displayVariant, setDisplayVariant] = useState<"grouped" | "table">("table")
  const [selectedId, setSelectedId] = useState("t_504")
  return <Surface locale={locale} theme={theme}><ViewSwitcher activeView={view} displayVariant={displayVariant} includeTableDisplay onViewChange={setView} onDisplayChange={setDisplayVariant} locale={locale} /><TasksProjection view={view} displayVariant={displayVariant} locale={locale} selectedId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} /></Surface>
}

function BoundaryActionStory({ state, locale }: { readonly state: "empty" | "offline" | "stale" | "error"; readonly locale: TasksLocale }) {
  const [attempts, setAttempts] = useState(0)
  return <TaskStateBoundary locale={locale} state={state} actionLabel={locale === "en" ? "Retry" : "重试"} onAction={() => setAttempts((current) => current + 1)}>{attempts > 0 ? <p role="status">{locale === "en" ? `Attempt ${attempts} recorded.` : `已记录第 ${attempts} 次尝试。`}</p> : null}</TaskStateBoundary>
}

export const WorkspaceBoard: Story = {
  render: (_args, context: StoryContext) => <WorkspaceBoardStory locale={localeFor(context.globals.locale)} theme={themeFor(context.globals.theme)} />,
}

export const TableProjection: Story = {
  render: (_args, context: StoryContext) => {
    const locale = localeFor(context.globals.locale)
    return <TableProjectionStory locale={locale} theme={themeFor(context.globals.theme)} />
  },
}

export const EnglishLabels: Story = {
  render: (_args, context: StoryContext) => {
    const locale = localeFor(context.globals.locale)
    const copy = storyCopy(locale)
    return <Surface locale={locale} theme={themeFor(context.globals.theme)}><div className={storyStyles.surfaceHeader}><div className={storyStyles.surfaceHeaderText}><p className={storyStyles.surfaceEyebrow}>kanban-tool / Tasks</p><h1 className={storyStyles.surfaceTitle}>{locale === "en" ? "Locale interaction grammar" : "本地化交互语法"}</h1></div><ViewSwitcher activeView="board" label={locale === "en" ? "Task views" : "任务视图"} locale={locale} /></div><FilterBar defaultSearch="storybook" locale={locale} filters={[{ id: "status", label: copy.statusRunning }]} /><TaskCard task={selectedTask} locale={locale} selected /><TaskTable tasks={[selectedTask]} locale={locale} /><SidePeekFrame model={inspector} locale={locale} /></Surface>
  },
}

export const SidePeekSelected: Story = {
  render: (_args, context: StoryContext) => <SidePeekSelectedStory locale={localeFor(context.globals.locale)} theme={themeFor(context.globals.theme)} />,
}

export const SidePeekNarrowSheet: Story = {
  render: (_args, context: StoryContext) => <SidePeekNarrowSheetStory locale={localeFor(context.globals.locale)} theme={themeFor(context.globals.theme)} />,
}

export const FilterBarActive: Story = {
  render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); const copy = storyCopy(locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><FilterBar defaultSearch="storybook" locale={locale} filters={[{ id: "status", label: copy.statusRunning }, { id: "plan", label: copy.planHasSteps }]} /></Surface> },
}

export const DisplayMenuOpen: Story = {
  render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><div className={storyStyles.controlsRight}><DisplayMenu defaultOpen locale={locale} options={{ density: "comfortable", visibleColumns: { assignee: true, priority: false } }} columns={displayColumns(locale)} /></div></Surface> },
}

export const TimelineUnsupported: Story = {
  render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><ViewSwitcher activeView="timeline" includeUnsupportedTimeline locale={locale} /><TaskStateBoundary locale={locale} state="empty" title={locale === "en" ? "Timeline unavailable" : "时间线不可用"} detail={locale === "en" ? "No canonical timeline read model; this unsupported state is Storybook-only." : "当前没有 canonical timeline read model；Storybook 仅展示 unsupported 状态。"} /></Surface> },
}

export const Loading: Story = { render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><TaskStateBoundary locale={locale} state="loading" /></Surface> } }
export const Empty: Story = { render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><BoundaryActionStory locale={locale} state="empty" /></Surface> } }
export const Offline: Story = { render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><BoundaryActionStory locale={locale} state="offline" /></Surface> } }
export const Stale: Story = { render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><BoundaryActionStory locale={locale} state="stale" /></Surface> } }
export const Recovering: Story = { render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><TaskStateBoundary locale={locale} state="recovering" /></Surface> } }
export const ErrorBoundary: Story = { render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><BoundaryActionStory locale={locale} state="error" /></Surface> } }

export const LongTextNarrow: Story = {
  parameters: { viewport: { defaultViewport: "mobile1" } },
  render: (_args, context: StoryContext) => {
    const locale = localeFor(context.globals.locale)
    const longTask = taskFixture({ id: "t_long", ref: "plane-only-observability-control-surface-with-a-very-long-project-slug#509", title: "A task title that remains readable when the board is narrow and the canonical description contains a long unbroken ref", status: "review", readiness: { ...taskFixture().readiness, requiredStepCount: 2, completedRequiredStepCount: 1 } })
    return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><TaskCard task={longTask} locale={locale} density="comfortable" selected /><TaskTable tasks={[longTask]} locale={locale} density="comfortable" /></Surface>
  },
}
