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

const tableRows: TaskListRow[] = [
  ...rows,
  {
    id: "t_2",
    ref: "default#2",
    title: "A task title that remains a single local table line on a narrow viewport",
    status: "blocked",
    priority: 1,
    assignee: null,
    executionPlanState: "unplanned",
    dependencyBlocked: true,
    requiredStepCount: 3,
    completedRequiredStepCount: 0,
    optionalStepCount: 1,
    updatedAt: 11,
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
    expect(markup).toContain('role="toolbar"')
    expect(markup).toMatch(
      /(?:id="task-list-controls"[^>]*tabindex="-1"|tabindex="-1"[^>]*id="task-list-controls")/,
    )
    expect(markup).not.toContain(" style=")
    expect(markup).toContain('aria-label="任务列表筛选"')
    expect(markup).toContain('data-testid="task-row"')
    expect(markup).toContain('data-testid="task-attention-lens"')
    expect(markup).toContain("就绪 · 1")
    expect(markup).toContain('aria-multiselectable="true"')
    expect(markup).toContain(">P2<")
    expect(markup).toContain(">有步骤<")
    expect(markup).not.toContain("<fieldset")
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

  test("renders List and Table as distinct projections over the same rows", () => {
    const list = renderToStaticMarkup(<TaskListView state={state} rows={rows} loading={false} displayVariant="grouped" onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)
    const table = renderToStaticMarkup(<TaskListView state={state} rows={rows} loading={false} displayVariant="table" onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)

    expect(list).toContain('data-display-variant="list"')
    expect(list).toContain('data-testid="task-row"')
    expect(list).toContain("运行中")
    expect(list).not.toContain("<table")
    expect(table).toContain('data-display-variant="table"')
    expect(table).toContain("<table")
    expect(table).toContain("First task")
  })

  test("keeps grouped List semantics legal and exposes labeled facts at both densities", () => {
    const dense = renderToStaticMarkup(
      <TaskListView state={state} rows={rows} loading={false} displayVariant="grouped" density="dense" showHeading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />,
    )
    const comfortable = renderToStaticMarkup(
      <TaskListView state={state} rows={rows} loading={false} displayVariant="table" density="comfortable" onQueryChange={vi.fn()} onSelectTask={vi.fn()} />,
    )

    expect(dense).toContain('data-density="dense"')
    expect(dense).toContain("<ul")
    expect(dense).toContain("<li")
    expect(dense).not.toContain('role="listitem"')
    expect(dense).toContain('class="')
    expect(dense).not.toContain("<dl")
    expect(dense).toContain('aria-label="优先级 P2"')
    expect(dense).toContain("优先级")
    expect(dense).toContain('class="')
    expect(comfortable).toContain('data-density="comfortable"')
    expect(comfortable).toContain("<table")
  })

  test("keeps table row identity, status, order, and windowed row semantics while sizing columns locally", () => {
    const markup = renderToStaticMarkup(
      <TaskListView state={state} rows={tableRows} loading={false} displayVariant="table" density="dense" onQueryChange={vi.fn()} onSelectTask={vi.fn()} />,
    )

    const rowIds = [...markup.matchAll(/<tr[^>]*data-testid="task-row"[^>]*data-task-id="([^"]+)"/g)].map((match) => match[1])
    expect(rowIds).toEqual(["t_1", "t_2"])
    expect(markup).toContain('aria-rowcount="26"')
    expect(markup).toContain('aria-rowindex="26"')
    expect(markup).toContain('data-column-key="ref"')
    expect(markup).toContain('data-column-key="title"')
    expect(markup).toContain('data-column-key="status"')
    expect(markup).toContain("w-32")
    expect(markup).toContain("w-64")
    expect(markup).toContain('data-density="compact"')
    expect(markup).toContain('data-text-overflow="truncate"')
    expect(markup).toContain("truncate")
    expect(markup).toContain("已阻塞")
    expect(markup).toContain("default#1")
    expect(markup).toContain("default#2")
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

  test("distinguishes an out-of-range unfiltered page from an empty board", () => {
    const outOfRangeState: TaskListViewState = {
      query: { status: [], priority: [], plan: [], search: "", sort: "updated_at", page: 3, limit: 25, includeArchived: false },
      meta: { offset: 50, limit: 25, total: 26 },
    }
    const markup = renderToStaticMarkup(<TaskListView state={outOfRangeState} rows={[]} loading={false} onQueryChange={vi.fn()} onSelectTask={vi.fn()} />)

    expect(markup).toContain('data-empty-kind="page"')
    expect(markup).toContain('data-testid="task-list-page-empty"')
    expect(markup).not.toContain('data-testid="task-list-board-empty"')
    expect(markup).toContain("当前页没有任务")
    expect(markup).toContain("0 / 26")
  })
})
