import { renderToStaticMarkup } from "react-dom/server"
import type { ReactElement } from "react"
import { describe, expect, test, vi } from "vitest"

import type { BoardTaskViewModel } from "../../features/board/types"
import type { TaskInspectorViewModel } from "../../features/explorer/TaskInspector"
import {
  BoardColumn,
  DisplayMenu,
  FilterBar,
  SidePeekFrame,
  TaskCard,
  TaskStateBoundary,
  TaskTable,
  ViewSwitcher,
} from "./TasksPatterns"

const task: BoardTaskViewModel = {
  id: "t_1",
  seq: 1,
  ref: "#1",
  title: "A canonical task",
  description: null,
  status: "running",
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
    requiredStepCount: 1,
    completedRequiredStepCount: 0,
    optionalStepCount: 0,
  },
}

const inspector: TaskInspectorViewModel = {
  task: {
    id: task.id,
    ref: task.ref,
    title: task.title,
    status: task.status,
    lockVersion: task.lockVersion,
    scheduledAt: null,
    dueAt: null,
    priority: task.priority,
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
    currentRunId: "r_demo",
    retryCount: 0,
    maxRetries: null,
    createdAt: 0,
    updatedAt: 0,
  },
  steps: [{ id: "s_1", title: "Build a static Storybook", status: "todo", required: true, body: null }],
  parents: [],
  children: [],
  comments: [],
  runs: [],
  events: [],
  runtime: { actor: "storybook", apiBaseUrl: "/", serverVersion: "demo", protocolVersion: "demo", webBuildId: "demo" },
}

describe("Tasks interaction grammar", () => {
  test("keeps Timeline out of the product switcher unless an unsupported story opts in", () => {
    const product = renderToStaticMarkup(<ViewSwitcher activeView="board" />)
    expect(product).toContain("看板")
    expect(product).not.toContain("时间线")

    const unsupported = renderToStaticMarkup(<ViewSwitcher activeView="timeline" includeUnsupportedTimeline />)
    expect(unsupported).toContain("时间线（不可用）")
    expect(unsupported).toContain('disabled=""')
  })

  test("renders active filters, canonical card facts, and table semantics", () => {
    const filter = renderToStaticMarkup(<FilterBar filters={[{ id: "status", label: "Status: Running" }]} onRemoveFilter={vi.fn()} />)
    expect(filter).toContain("Status: Running")
    expect(filter).toContain("当前筛选")

    const card = renderToStaticMarkup(<TaskCard task={task} selected onSelect={vi.fn()} />)
    expect(card).toContain("#1")
    expect(card).toContain("A canonical task")
    expect(card).toContain("运行中")
    expect(card).toContain("0/1")
    expect(card).toContain('data-task-opener="t_1"')

    const table = renderToStaticMarkup(<TaskTable tasks={[task]} onSelectTask={vi.fn()} visibleColumns={{ assignee: false }} />)
    expect(table).toContain("任务表格")
    expect(table).toContain('scope="col"')
    expect(table).toContain("A canonical task")
    expect(table).not.toContain("执行者")

    const emptyTable = renderToStaticMarkup(<TaskTable tasks={[]} locale="en" />)
    expect(emptyTable).toContain("No tasks")
  })

  test("supports empty columns and each recovery boundary state", () => {
    const column = renderToStaticMarkup(<BoardColumn id="review" status="review" title="Review" tasks={[]} />)
    expect(column).toContain("Review")
    expect(column).toContain("暂无任务")

    for (const state of ["loading", "empty", "offline", "stale", "recovering", "error"] as const) {
      const markup = renderToStaticMarkup(<TaskStateBoundary state={state} />)
      expect(markup).toContain(`data-state="${state}"`)
    }
  })

  test("keeps selected side-peek read model scoped to task, required step, and run", () => {
    const markup = renderToStaticMarkup(<SidePeekFrame model={inspector} mode="sheet" onClose={vi.fn()} onRestoreFocus={vi.fn()} onOpenDetails={vi.fn()} />)
    expect(markup).toContain("#1")
    expect(markup).toContain("Build a static Storybook")
    expect(markup).toContain("r_demo")
    expect(markup).toContain('role="dialog"')
    expect(markup).toContain('aria-modal="true"')
    expect(markup).not.toContain("avatar")
    expect(markup).not.toContain("progress")
  })

  test("keeps display controls keyboard-addressable", () => {
    const markup = renderToStaticMarkup(<DisplayMenu options={{ density: "dense" }} columns={[{ id: "priority", label: "优先级" }]} />)
    expect(markup).toContain("密度")
    expect(markup).toContain('type="checkbox"')
    expect(markup).toContain("优先级")
    expect(markup).toContain('disabled=""')

    const staticCard = renderToStaticMarkup(<TaskCard task={task} />)
    expect(staticCard).not.toContain('data-task-opener="t_1"')
    expect(staticCard).toContain("运行中")
  })

  test("marks a filter search disabled when its projection has no query handler", () => {
    const markup = renderToStaticMarkup(<FilterBar defaultSearch="local" disabled />)
    expect(markup).toContain('disabled=""')
  })

  test("preserves a stable search hook for shared product chrome", () => {
    const markup = renderToStaticMarkup(<FilterBar defaultSearch="local" searchTestId="list-search" />)
    expect(markup).toContain('data-testid="list-search"')
  })

  test("uses one atomic selection callback for the Table projection", () => {
    const onSelectionChange = vi.fn()
    const root = ViewSwitcher({ activeView: "list", includeTableDisplay: true, displayVariant: "grouped", onSelectionChange }) as ReactElement<{ readonly children: readonly ReactElement[] }>
    const buttons = root.props.children
    const tableWrapper = buttons[2] as ReactElement<{ readonly children: ReactElement<{ readonly onClick?: () => void }> }>
    const tableControl = tableWrapper.props.children
    tableControl.props.onClick?.()
    expect(onSelectionChange).toHaveBeenCalledTimes(1)
    expect(onSelectionChange).toHaveBeenCalledWith("list", "table")
  })

  test("keeps href navigation when no local selection handler exists", () => {
    const hrefForView = vi.fn((view: string, display: string) => `/${view}?display=${display}`)
    const root = ViewSwitcher({ activeView: "board", hrefForView }) as ReactElement<{ readonly children: readonly ReactElement[] }>
    const boardWrapper = root.props.children[0] as ReactElement<{ readonly children: ReactElement<{ readonly href?: string; readonly onClick?: unknown }> }>
    const boardLink = boardWrapper.props.children
    expect(boardLink.type).toBe("a")
    expect(boardLink.props.href).toBe("/board?display=grouped")
    expect(boardLink.props.onClick).toBeUndefined()
    expect(hrefForView).toHaveBeenCalled()
  })
})
