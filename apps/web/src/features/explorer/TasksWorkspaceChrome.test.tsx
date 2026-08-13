import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { TasksWorkspaceChrome } from "./TasksWorkspaceChrome"

describe("TasksWorkspaceChrome", () => {
  test("keeps the shared Task views label, list search hook, and diagnostic hrefs", () => {
    const markup = renderToStaticMarkup(
      <TasksWorkspaceChrome
        locale="zh"
        scope="default"
        hrefForView={(view, display) => `/app/boards/default/${view}?display=${display}`}
        activeView="list"
        displayVariant="grouped"
        density="comfortable"
        listQuery={{ status: [], priority: [], plan: [], search: "", sort: "updated_at", page: 1, limit: 100, includeArchived: false }}
        onViewChange={vi.fn()}
        onSearchChange={vi.fn()}
        onDensityChange={vi.fn()}
        visibleColumns={{}}
        onVisibleColumnsChange={vi.fn()}
        diagnostics={[{ id: "runs", label: "运行记录", href: "/app/boards/default/runs" }]}
        hasInspector={false}
      />,
    )

    expect(markup).toContain('role="group" aria-label="任务视图"')
    expect(markup).toContain('href="/app/boards/default/map?display=grouped"')
    expect(markup).not.toContain("data-collapsed=")
    expect(markup).toContain('data-testid="list-search"')
    expect(markup).toContain('href="/app/boards/default/runs"')
    expect(markup).not.toContain(" style=")
  })

  test("keeps q visible in the shared toolbar while filters stay view-specific", () => {
    const markup = renderToStaticMarkup(
      <TasksWorkspaceChrome
        locale="zh"
        scope="default"
        activeView="board"
        displayVariant="grouped"
        density="comfortable"
        searchQuery="agent"
        onViewChange={vi.fn()}
        onSearchChange={vi.fn()}
        onOpenFilters={vi.fn()}
        onDensityChange={vi.fn()}
        visibleColumns={{}}
        onVisibleColumnsChange={vi.fn()}
        diagnostics={[]}
        hasInspector={false}
      />,
    )

    expect(markup).toContain('data-testid="tasks-workspace-toolbar"')
    expect(markup).toContain('role="toolbar"')
    expect(markup).toContain('aria-label="任务搜索"')
    expect(markup).toContain('value="agent"')
    expect(markup).toContain('data-view-filter="board"')
    expect(markup).toContain('data-task-state="none"')
    expect(markup).not.toContain('data-frame="content"')
    expect(markup).toContain('data-search-support="url-only"')
  })

  test("marks list filters available without creating a second page frame", () => {
    const markup = renderToStaticMarkup(
      <TasksWorkspaceChrome
        locale="zh"
        scope="default"
        activeView="list"
        displayVariant="grouped"
        density="comfortable"
        searchQuery=""
        listQuery={{ status: ["ready"], priority: [], plan: [], search: "", sort: "updated_at", page: 1, limit: 100, includeArchived: false }}
        onViewChange={vi.fn()}
        onSearchChange={vi.fn()}
        onOpenFilters={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
        onDensityChange={vi.fn()}
        visibleColumns={{}}
        onVisibleColumnsChange={vi.fn()}
        diagnostics={[]}
        hasInspector={false}
      />,
    )

    expect(markup).toContain('data-view-filter="list"')
    expect(markup).toContain("状态: ready")
    expect(markup).not.toContain('data-frame="content"')
  })
})
