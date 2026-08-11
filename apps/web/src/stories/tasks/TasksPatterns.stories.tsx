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

const selectedTask = tasks.find((task) => task.id === "t_504") ?? tasks[0]

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

const displayColumns = [{ id: "assignee", label: "执行者" }, { id: "priority", label: "优先级" }] as const

function Surface({ children, narrow = false, locale = "zh", theme = "light" }: { readonly children: ReactNode; readonly narrow?: boolean; readonly locale?: TasksLocale; readonly theme?: "light" | "dark" }) {
  return (
    <main className={narrow ? storyStyles.surfaceNarrow : storyStyles.surface} data-locale={locale} data-theme={theme}>
      <div className={narrow ? storyStyles.surfaceContentNarrow : storyStyles.surfaceContent}>{children}</div>
    </main>
  )
}

function WorkspaceBoardStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const [view, setView] = useState<TasksView>("board")
  const [density, setDensity] = useState<TasksDensity>("dense")
  const [displayVariant, setDisplayVariant] = useState<"grouped" | "table">("grouped")
  const [selectedId, setSelectedId] = useState<string | null>("t_504")
  const [query, setQuery] = useState("")
  const [filters, setFilters] = useState([{ id: "all", label: "All tasks", removable: false }])
  return (
    <Surface locale={locale} theme={theme}>
      <header className={storyStyles.surfaceHeader}>
        <div className={storyStyles.surfaceHeaderText}><p className={storyStyles.surfaceEyebrow}>kanban-tool / Tasks</p><h1 className={storyStyles.surfaceTitle}>{locale === "en" ? "Tasks workspace" : "任务工作区"}</h1></div>
        <div className={storyStyles.controlsRight}><ViewSwitcher activeView={view} displayVariant={displayVariant} includeTableDisplay onViewChange={setView} onDisplayChange={setDisplayVariant} locale={locale} /><DisplayMenu options={{ density }} onDensityChange={setDensity} columns={displayColumns} locale={locale} /></div>
      </header>
      <FilterBar search={query} onSearchChange={setQuery} filters={filters} locale={locale} onRemoveFilter={(id) => setFilters((current) => current.filter((filter) => filter.id !== id))} onClearFilters={() => setFilters([])} />
      {displayVariant === "table" ? <TaskTable tasks={tasks} density={density} locale={locale} selectedTaskId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} /> : <BoardColumns columns={columns} density={density} locale={locale} selectedTaskId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} />}
      <p role="status" className={storyStyles.demoNote}>Demo-only fixture · tasks.status remains the sole fact.</p>
    </Surface>
  )
}

function SidePeekSelectedStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const [open, setOpen] = useState(true)
  return <Surface locale={locale} theme={theme}><div className={open ? storyStyles.workspaceWithPeek : storyStyles.workspace}><BoardColumns columns={columns} locale={locale} selectedTaskId={open ? "t_504" : null} onSelectTask={() => setOpen(true)} />{open ? <SidePeekFrame mode="sheet" model={inspector} locale={locale} onClose={() => setOpen(false)} onOpenDetails={() => setOpen(false)} /> : null}</div></Surface>
}

function TableProjectionStory({ locale, theme }: { readonly locale: TasksLocale; readonly theme: "light" | "dark" }) {
  const [view, setView] = useState<TasksView>("list")
  const [displayVariant, setDisplayVariant] = useState<"grouped" | "table">("table")
  const [selectedId, setSelectedId] = useState("t_504")
  return <Surface locale={locale} theme={theme}><ViewSwitcher activeView={view} displayVariant={displayVariant} includeTableDisplay onViewChange={setView} onDisplayChange={setDisplayVariant} locale={locale} />{displayVariant === "table" ? <TaskTable tasks={tasks} locale={locale} selectedTaskId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} /> : <BoardColumns columns={columns} locale={locale} selectedTaskId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} />}</Surface>
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
    return <Surface locale={locale} theme={themeFor(context.globals.theme)}><div className={storyStyles.surfaceHeader}><div className={storyStyles.surfaceHeaderText}><p className={storyStyles.surfaceEyebrow}>kanban-tool / Tasks</p><h1 className={storyStyles.surfaceTitle}>{locale === "en" ? "Locale interaction grammar" : "本地化交互语法"}</h1></div><ViewSwitcher activeView="board" label={locale === "en" ? "Task views" : "任务视图"} locale={locale} /></div><FilterBar defaultSearch="storybook" locale={locale} filters={[{ id: "status", label: locale === "en" ? "Status: Running" : "状态：运行中" }]} /><TaskCard task={selectedTask} locale={locale} selected /><TaskTable tasks={[selectedTask]} locale={locale} /><SidePeekFrame model={inspector} locale={locale} /></Surface>
  },
}

export const SidePeekSelected: Story = {
  render: (_args, context: StoryContext) => <SidePeekSelectedStory locale={localeFor(context.globals.locale)} theme={themeFor(context.globals.theme)} />,
}

export const FilterBarActive: Story = {
  render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><FilterBar defaultSearch="storybook" locale={locale} filters={[{ id: "status", label: locale === "en" ? "Status: Running" : "状态：运行中" }, { id: "plan", label: locale === "en" ? "Plan: Has steps" : "计划：有步骤" }]} /></Surface> },
}

export const DisplayMenuOpen: Story = {
  render: (_args, context: StoryContext) => { const locale = localeFor(context.globals.locale); return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><div className={storyStyles.controlsRight}><DisplayMenu defaultOpen locale={locale} options={{ density: "comfortable", visibleColumns: { assignee: true, priority: false } }} columns={[{ id: "assignee", label: locale === "en" ? "Assignee" : "执行者" }, { id: "priority", label: locale === "en" ? "Priority" : "优先级" }]} /></div></Surface> },
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
    const longTask = taskFixture({ id: "t_long", ref: "#509", title: "A task title that remains readable when the board is narrow and the canonical description contains a long unbroken ref", status: "review", readiness: { ...taskFixture().readiness, requiredStepCount: 2, completedRequiredStepCount: 1 } })
    return <Surface narrow locale={locale} theme={themeFor(context.globals.theme)}><TaskCard task={longTask} locale={locale} density="comfortable" selected /><TaskTable tasks={[longTask]} locale={locale} density="comfortable" /></Surface>
  },
}
