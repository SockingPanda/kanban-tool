import { renderToStaticMarkup } from "react-dom/server"
import type { ReactNode } from "react"
import { describe, expect, test, vi } from "vitest"

import { ProjectOverview } from "./ProjectOverview"
import { ProjectsCollection } from "./ProjectsCollection"
import { assertCanonicalBoardSlug } from "../../lib/board-slug"
import { asCanonicalBoardId } from "../../lib/sync/contracts"
import { PreferencesContext } from "../../lib/preferences-context"
import type { Locale } from "../../lib/preferences"

const active = Object.freeze({
  id: asCanonicalBoardId("b_active"),
  slug: assertCanonicalBoardSlug("active"),
  name: "Active project",
  description: "Canonical description",
  archivedAt: null,
})

const archived = Object.freeze({
  id: asCanonicalBoardId("b_archived"),
  slug: assertCanonicalBoardSlug("archived"),
  name: "Archived project",
  description: null,
  archivedAt: 1,
})

function renderWithLocale(node: ReactNode, locale: Locale = "en") {
  return renderToStaticMarkup(
    <PreferencesContext.Provider value={{
      theme: "light",
      locale,
      sidebarWidthStep: 62,
      density: "comfortable",
      actor: "",
      setTheme: () => undefined,
      setLocale: () => undefined,
      setDensity: () => undefined,
      setActor: () => undefined,
      setSidebarWidthStep: () => undefined,
      resetSidebarWidth: () => undefined,
    }}>
      {node}
    </PreferencesContext.Provider>,
  )
}

describe("production Projects surfaces", () => {
  test("filters archived projects from the default collection without losing search", () => {
    const markup = renderWithLocale(<ProjectsCollection projects={[active, archived]} onOpenProject={vi.fn()} />)

    expect(markup).toContain("Active project")
    expect(markup).not.toContain("Archived project")
    expect(markup).toContain('data-frame="content"')
    expect(markup).toContain('data-testid="projects-search"')
    expect(markup).toContain('data-testid="projects-collection-list"')
    const projectLink = markup.match(/<a\b[^>]*data-testid="projects-collection-project-active"[^>]*>/)?.[0]
    expect(projectLink).toContain('href="/app/boards/active/overview"')
    expect(markup).not.toMatch(/<span[^>]*><div/)
    expect(markup).not.toMatch(/\sstyle=/)
  })

  test("keeps a cached snapshot visible while offline", () => {
    const markup = renderWithLocale(<ProjectsCollection projects={[active]} status="offline" onRetry={vi.fn()} onOpenProject={vi.fn()} />)

    expect(markup).toContain('data-status="offline"')
    expect(markup).toContain('data-has-snapshot="true"')
    expect(markup).toContain("Active project")
    expect(markup).toContain("Reload")
  })

  test("does not claim a cached snapshot when offline or error has no items", () => {
    const offline = renderWithLocale(<ProjectsCollection projects={[]} status="offline" onRetry={vi.fn()} />)
    const error = renderWithLocale(<ProjectsCollection projects={[]} status="error" onRetry={vi.fn()} />)

    expect(offline).toContain('data-has-snapshot="false"')
    expect(offline).toContain("no project snapshot is available yet")
    expect(offline).not.toContain("Showing the cached project snapshot")
    expect(error).toContain('data-has-snapshot="false"')
    expect(error).toContain("no snapshot is available yet")
    expect(error).not.toContain("Showing the cached project snapshot")
  })

  test("disables search without a loading snapshot and keeps retry semantics explicit", () => {
    const loading = renderWithLocale(<ProjectsCollection projects={[]} status="loading" onRetry={vi.fn()} />)
    const refreshing = renderWithLocale(<ProjectsCollection projects={[active]} status="ready" isRefreshing onRetry={vi.fn()} />)

    expect(loading).toContain('data-status="loading"')
    expect(loading).toContain('data-has-snapshot="false"')
    expect(loading).toContain('id="projects-search"')
    expect(loading).toContain("disabled")
    expect(loading).toContain('data-testid="projects-collection-loading"')
    expect(refreshing).toContain("Refreshing projects")
    expect(refreshing).not.toContain('data-testid="projects-collection-ready"')

    const recovering = renderWithLocale(<ProjectsCollection projects={[active]} status="recovering" onRetry={vi.fn()} />)
    expect(recovering).not.toContain("Retry")
  })

  test("renders only board identity, description and archive state on overview", () => {
    const markup = renderWithLocale(<ProjectOverview project={archived} onOpenTasks={vi.fn()} />)

    expect(markup).toContain('data-testid="project-overview"')
    expect(markup).toContain('data-archived="true"')
    expect(markup).toContain("Archived")
    expect(markup).toContain("archived")
    expect(markup).not.toMatch(/metric|owner|cover|activity|progress|risk/i)
    expect(markup).not.toContain("BoardLive")
    expect(markup).not.toContain("SSE")
    expect(markup).not.toContain('href="/app/boards/archived/board"')
    expect(markup).not.toContain('data-testid="project-overview-open-tasks"')
    expect(markup).not.toContain("此项目没有提供")
    expect(markup).toContain('data-testid="project-overview-identity"')
    expect(markup).toContain('data-frame="content"')
    expect(markup).toContain('translate="no"')
    expect(markup).not.toMatch(/\sstyle=/)
  })

  test("keeps an identity snapshot visible while the list is stale", () => {
    const markup = renderWithLocale(<ProjectOverview project={active} status="stale" onRetry={vi.fn()} />)

    expect(markup).toContain('data-testid="project-overview-status"')
    expect(markup).toContain('data-status="stale"')
    expect(markup).toContain("Showing the last successful canonical snapshot.")
  })

  test("keeps the overview task selector on the real anchor and suppresses retry while recovering", () => {
    const ready = renderWithLocale(<ProjectOverview project={active} onOpenTasks={vi.fn()} />)
    const recovering = renderWithLocale(<ProjectOverview project={active} status="recovering" onRetry={vi.fn()} />)
    const taskLink = ready.match(/<a\b[^>]*data-testid="project-overview-open-tasks"[^>]*>/)?.[0]

    expect(taskLink).toContain('href="/app/boards/active/board"')
    expect(recovering).not.toContain("Retry")
  })
})
