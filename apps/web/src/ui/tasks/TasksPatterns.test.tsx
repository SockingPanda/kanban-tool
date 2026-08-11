import { renderToStaticMarkup } from "react-dom/server"
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
    expect(product).toContain("Board")
    expect(product).not.toContain("Timeline")

    const unsupported = renderToStaticMarkup(<ViewSwitcher activeView="timeline" includeUnsupportedTimeline />)
    expect(unsupported).toContain("Timeline（不可用）")
    expect(unsupported).toContain('disabled=""')
  })

  test("renders active filters, canonical card facts, and table semantics", () => {
    const filter = renderToStaticMarkup(<FilterBar filters={[{ id: "status", label: "Status: Running" }]} onRemoveFilter={vi.fn()} />)
    expect(filter).toContain("Status: Running")
    expect(filter).toContain("Active filters")

    const card = renderToStaticMarkup(<TaskCard task={task} selected onSelect={vi.fn()} />)
    expect(card).toContain("#1")
    expect(card).toContain("A canonical task")
    expect(card).toContain("运行中")
    expect(card).toContain("0/1")

    const table = renderToStaticMarkup(<TaskTable tasks={[task]} onSelectTask={vi.fn()} />)
    expect(table).toContain("任务表格")
    expect(table).toContain('scope="col"')
    expect(table).toContain("A canonical task")
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
    const markup = renderToStaticMarkup(<SidePeekFrame model={inspector} onClose={vi.fn()} onOpenDetails={vi.fn()} />)
    expect(markup).toContain("#1")
    expect(markup).toContain("Build a static Storybook")
    expect(markup).toContain("r_demo")
    expect(markup).not.toContain("avatar")
    expect(markup).not.toContain("progress")
  })

  test("keeps display controls keyboard-addressable", () => {
    const markup = renderToStaticMarkup(<DisplayMenu options={{ density: "dense" }} columns={[{ id: "priority", label: "优先级" }]} />)
    expect(markup).toContain("Density")
    expect(markup).toContain('type="checkbox"')
    expect(markup).toContain("优先级")
  })
})
