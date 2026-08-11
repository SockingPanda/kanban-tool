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

function Surface({ children, narrow = false }: { readonly children: ReactNode; readonly narrow?: boolean }) {
  return (
    <main className={narrow ? storyStyles.surfaceNarrow : storyStyles.surface}>
      <div className={narrow ? storyStyles.surfaceContentNarrow : storyStyles.surfaceContent}>{children}</div>
    </main>
  )
}

function WorkspaceBoardStory() {
  const [view, setView] = useState<TasksView>("board")
  const [density, setDensity] = useState<TasksDensity>("dense")
  const [selectedId, setSelectedId] = useState<string | null>("t_504")
  const [query, setQuery] = useState("")
  const [filters, setFilters] = useState([{ id: "all", label: "All tasks", removable: false }])
  return (
    <Surface>
      <header className={storyStyles.surfaceHeader}>
        <div className={storyStyles.surfaceHeaderText}><p className={storyStyles.surfaceEyebrow}>kanban-tool / Tasks</p><h1 className={storyStyles.surfaceTitle}>Tasks workspace</h1></div>
        <div className={storyStyles.controlsRight}><ViewSwitcher activeView={view} onViewChange={setView} /><DisplayMenu options={{ density }} onDensityChange={setDensity} columns={displayColumns} /></div>
      </header>
      <FilterBar search={query} onSearchChange={setQuery} filters={filters} onRemoveFilter={(id) => setFilters((current) => current.filter((filter) => filter.id !== id))} onClearFilters={() => setFilters([])} />
      <BoardColumns columns={columns} density={density} selectedTaskId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} />
      <p role="status" className={storyStyles.demoNote}>Demo-only fixture · tasks.status remains the sole fact.</p>
    </Surface>
  )
}

function SidePeekSelectedStory() {
  const [open, setOpen] = useState(true)
  return <Surface><div className={open ? storyStyles.workspaceWithPeek : storyStyles.workspace}><BoardColumns columns={columns} selectedTaskId={open ? "t_504" : null} onSelectTask={() => setOpen(true)} />{open ? <SidePeekFrame model={inspector} onClose={() => setOpen(false)} onOpenDetails={() => undefined} /> : null}</div></Surface>
}

export const WorkspaceBoard: Story = {
  render: () => <WorkspaceBoardStory />,
}

export const TableProjection: Story = {
  render: () => <Surface><ViewSwitcher activeView="table" /><TaskTable tasks={tasks} selectedTaskId="t_504" onSelectTask={() => undefined} /></Surface>,
}

export const EnglishLabels: Story = {
  render: () => <Surface><div className={storyStyles.surfaceHeader}><div className={storyStyles.surfaceHeaderText}><p className={storyStyles.surfaceEyebrow}>kanban-tool / Tasks</p><h1 className={storyStyles.surfaceTitle}>English interaction grammar</h1></div><ViewSwitcher activeView="board" label="Task views" /></div><FilterBar search="storybook" filterButtonLabel="Filters" clearButtonLabel="Clear filters" filters={[{ id: "status", label: "Status: Running" }]} /><TaskCard task={selectedTask} locale="en" selected onSelect={() => undefined} /><TaskTable tasks={[selectedTask]} locale="en" onSelectTask={() => undefined} /><SidePeekFrame model={inspector} locale="en" statusLabel="Status" requiredStepLabel="Required step" runLabel="Run" closeLabel="Close task details" detailsLabel="Open full details" onClose={() => undefined} onOpenDetails={() => undefined} /></Surface>,
}

export const SidePeekSelected: Story = {
  render: () => <SidePeekSelectedStory />,
}

export const FilterBarActive: Story = {
  render: () => <Surface narrow><FilterBar search="storybook" filters={[{ id: "status", label: "Status: Running" }, { id: "plan", label: "Plan: Has steps" }]} onSearchChange={() => undefined} onRemoveFilter={() => undefined} onClearFilters={() => undefined} /></Surface>,
}

export const DisplayMenuOpen: Story = {
  render: () => <Surface narrow><div className={storyStyles.controlsRight}><DisplayMenu defaultOpen options={{ density: "comfortable", visibleColumns: { assignee: true, priority: false } }} columns={[{ id: "assignee", label: "执行者" }, { id: "priority", label: "优先级" }]} /></div></Surface>,
}

export const TimelineUnsupported: Story = {
  render: () => <Surface narrow><ViewSwitcher activeView="timeline" includeUnsupportedTimeline /><TaskStateBoundary state="empty" title="Timeline 不可用" detail="当前没有 canonical timeline read model；Storybook 仅展示 unsupported 状态。" /></Surface>,
}

export const Loading: Story = { render: () => <Surface narrow><TaskStateBoundary state="loading" /></Surface> }
export const Empty: Story = { render: () => <Surface narrow><TaskStateBoundary state="empty" actionLabel="创建任务" onAction={() => undefined} /></Surface> }
export const Offline: Story = { render: () => <Surface narrow><TaskStateBoundary state="offline" actionLabel="重试" onAction={() => undefined} /></Surface> }
export const Stale: Story = { render: () => <Surface narrow><TaskStateBoundary state="stale" actionLabel="刷新" onAction={() => undefined} /></Surface> }
export const Recovering: Story = { render: () => <Surface narrow><TaskStateBoundary state="recovering" /></Surface> }
export const ErrorBoundary: Story = { render: () => <Surface narrow><TaskStateBoundary state="error" detail="服务端返回了不可读的任务响应。" actionLabel="重新加载" onAction={() => undefined} /></Surface> }

export const LongTextNarrow: Story = {
  parameters: { viewport: { defaultViewport: "mobile1" } },
  render: () => {
    const longTask = taskFixture({ id: "t_long", ref: "#509", title: "A task title that remains readable when the board is narrow and the canonical description contains a long unbroken ref", status: "review", readiness: { ...taskFixture().readiness, requiredStepCount: 2, completedRequiredStepCount: 1 } })
    return <Surface narrow><TaskCard task={longTask} density="comfortable" selected onSelect={() => undefined} /><TaskTable tasks={[longTask]} density="comfortable" onSelectTask={() => undefined} /></Surface>
  },
}
