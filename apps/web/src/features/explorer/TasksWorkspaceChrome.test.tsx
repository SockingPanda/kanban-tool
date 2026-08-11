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

    expect(markup).toContain('aria-label="任务视图"')
    expect(markup).toContain('data-testid="list-search"')
    expect(markup).toContain('href="/app/boards/default/runs"')
  })
})
