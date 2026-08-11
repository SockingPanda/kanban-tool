import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { queryWithAttentionLens } from "../attention/attention-lens"
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
  test("maps an attention lens to the URL-controlled status query and first page", () => {
    const next = queryWithAttentionLens(state.query, "blocked")
    expect(next).toMatchObject({ status: ["blocked"], page: 1, priority: [2], plan: ["has_steps"], search: "First" })
  })

  test("renders URL-controlled filters, sorting, pagination and task row", () => {
    const markup = renderToStaticMarkup(
      <TaskListView state={state} rows={rows} loading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />,
    )

    expect(markup).toContain('data-testid="task-list"')
    expect(markup).toContain('data-testid="list-search"')
    expect(markup).toContain('role="group"')
    expect(markup).toContain('aria-label="任务列表筛选"')
    expect(markup).toContain('data-testid="task-row"')
    expect(markup).toContain('data-testid="task-attention-lens"')
    expect(markup).toContain('data-testid="attention-lens-ready"')
    expect(markup).toContain('data-testid="attention-count-ready"')
    expect(markup).toContain('data-testid="attention-active-filter"')
    expect(markup).toContain('data-testid="attention-clear"')
    expect(markup).toContain('aria-pressed="true"')
    expect(markup).toContain("<caption")
    expect(markup).toContain('scope="col"')
    expect(markup).toContain("First task")
    expect(markup).toContain("第 2 页")
    expect(markup).toContain("26")
    expect(markup).toContain('value="-updated_at"')
    expect(markup).not.toContain("TASK EXPLORER")
    expect(markup).not.toContain("Page")
    expect(markup).not.toContain("包含 archived")
  })

  test("renders empty and loading states without inventing rows", () => {
    const loading = renderToStaticMarkup(<TaskListView state={state} rows={[]} loading onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)
    expect(loading).toContain('data-testid="task-list-loading"')
    const empty = renderToStaticMarkup(<TaskListView state={state} rows={[]} loading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)
    expect(empty).toContain('data-testid="task-list-empty"')
    expect(empty).toContain('data-empty-kind="filter"')
    expect(empty).toContain('data-testid="task-list-filter-empty"')
    expect(empty).not.toContain('data-testid="task-row"')
  })

  test("distinguishes an empty board from a filtered no-result list", () => {
    const emptyBoardState: TaskListViewState = {
      query: { status: [], priority: [], plan: [], search: "", sort: "updated_at", page: 1, limit: 100, includeArchived: false },
      meta: { offset: 0, limit: 100, total: 0 },
    }
    const markup = renderToStaticMarkup(<TaskListView state={emptyBoardState} rows={[]} loading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)
    expect(markup).toContain('data-empty-kind="board"')
    expect(markup).toContain('data-testid="task-list-board-empty"')
    expect(markup).not.toContain('data-testid="task-list-filter-empty"')
  })
})
