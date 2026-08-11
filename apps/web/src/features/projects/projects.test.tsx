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
      sidebarExpanded: true,
      density: "comfortable",
      actor: "",
      setTheme: () => undefined,
      setLocale: () => undefined,
      setDensity: () => undefined,
      setActor: () => undefined,
      setSidebarExpanded: () => undefined,
      toggleSidebar: () => undefined,
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
    expect(markup).toContain('data-testid="projects-collection-list"')
    expect(markup).toContain('href="/app/boards/active/overview"')
  })

  test("keeps a cached snapshot visible while offline", () => {
    const markup = renderWithLocale(<ProjectsCollection projects={[active]} status="offline" onRetry={vi.fn()} onOpenProject={vi.fn()} />)

    expect(markup).toContain('data-status="offline"')
    expect(markup).toContain("Active project")
    expect(markup).toContain("Reload")
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
    expect(markup).toContain('href="/app/boards/archived/board"')
    expect(markup).not.toContain("此项目没有提供")
  })

  test("keeps an identity snapshot visible while the list is stale", () => {
    const markup = renderWithLocale(<ProjectOverview project={active} status="stale" onRetry={vi.fn()} />)

    expect(markup).toContain('data-testid="project-overview-status"')
    expect(markup).toContain('data-status="stale"')
    expect(markup).toContain("Showing the last successful canonical snapshot.")
  })
})
