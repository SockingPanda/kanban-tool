import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { TaskListView, type TaskListRow, type TaskListViewState } from "./TaskListView"

const rows: TaskListRow[] = [
  {
    id: "t_1",
    ref: "default#1",
    title: "First task",
    status: "ready",
    priority: 2,
    assignee: "worker",
    executionPlanState: "planned",
    dependencyBlocked: false,
    requiredStepCount: 2,
    completedRequiredStepCount: 1,
    optionalStepCount: 0,
    updatedAt: 10,
  },
]

const state: TaskListViewState = {
  query: {
    status: ["ready"],
    priority: [2],
    plan: ["has_steps"],
    search: "First",
    sort: "-updated_at",
    page: 2,
    limit: 25,
    includeArchived: false,
  },
  meta: { offset: 25, limit: 25, total: 26 },
}

describe("TaskListView", () => {
  test("renders URL-controlled filters, sorting, pagination and task row", () => {
    const markup = renderToStaticMarkup(
      <TaskListView state={state} rows={rows} loading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />,
    )

    expect(markup).toContain('data-testid="task-list"')
    expect(markup).toContain('data-testid="list-search"')
    expect(markup).toContain('data-testid="task-row"')
    expect(markup).toContain("<caption")
    expect(markup).toContain('scope="col"')
    expect(markup).toContain("First task")
    expect(markup).toContain("Page 2")
    expect(markup).toContain("26")
    expect(markup).toContain('value="-updated_at"')
  })

  test("renders empty and loading states without inventing rows", () => {
    const loading = renderToStaticMarkup(<TaskListView state={state} rows={[]} loading onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)
    expect(loading).toContain('data-testid="task-list-loading"')
    const empty = renderToStaticMarkup(<TaskListView state={state} rows={[]} loading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)
    expect(empty).toContain('data-testid="task-list-empty"')
    expect(empty).not.toContain('data-testid="task-row"')
  })
})
