import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { assertCanonicalBoardSlug } from "../../lib/board-slug"
import { asCanonicalBoardId } from "../../lib/sync/contracts"
import { ProjectPicker } from "./ProjectPicker"
import { ProjectTree } from "./ProjectTree"
import { ProjectsSidebar } from "./ProjectsSidebar"
import { ProductRail } from "./ProductRail"
import { ResourceHeader } from "./ResourceHeader"
import type { NavigationProject, ProjectPickerStatus } from "./types"

const project: NavigationProject = Object.freeze({
  id: asCanonicalBoardId("b_navigation"),
  slug: assertCanonicalBoardSlug("navigation"),
  name: "Navigation",
  description: "Navigation fixture",
  archivedAt: null,
})

type PickerOverrides = {
  readonly status?: ProjectPickerStatus
  readonly onRetry?: () => void
  readonly isRefreshing?: boolean
  readonly hasSnapshot?: boolean
}

function pickerMarkup(overrides: PickerOverrides = {}): string {
  return renderToStaticMarkup(<ProjectPicker projects={[project]} {...overrides} />)
}

describe("navigation accessibility contracts", () => {
  test("gives the narrow sidebar a real dialog seam and makes a closed drawer inert", () => {
    const open = renderToStaticMarkup(
      <ProjectsSidebar projects={[project]} open drawerId="navigation-drawer" onClose={vi.fn()} />,
    )
    const closed = renderToStaticMarkup(
      <ProjectsSidebar projects={[project]} open={false} drawerId="navigation-drawer" onClose={vi.fn()} />,
    )

    expect(open).toContain('role="dialog"')
    expect(open).toContain('aria-modal="true"')
    expect(open).toContain('id="navigation-drawer"')
    expect(open).toContain('data-testid="projects-sidebar-backdrop"')
    expect(closed).toContain('aria-hidden="true"')
    expect(closed).toContain('inert=""')
    expect(closed).toContain('data-open="false"')
  })

  test("keeps a cached snapshot available across offline and recovering states", () => {
    const offline = pickerMarkup({ status: "offline", onRetry: undefined })
    const recovering = pickerMarkup({ status: "recovering", isRefreshing: true, onRetry: vi.fn() })

    expect(offline).toContain('data-testid="project-picker-options"')
    expect(offline).toContain("Navigation")
    expect(offline).not.toContain('disabled=""')
    expect(offline).not.toContain("Retry")
    expect(recovering).toContain('data-testid="project-picker-options"')
    expect(recovering).toContain("Retry")
  })

  test("only disables first-load search and avoids dangling combobox IDREFs", () => {
    const loading = renderToStaticMarkup(<ProjectPicker projects={[]} status="loading" />)
    const empty = renderToStaticMarkup(<ProjectPicker projects={[]} status="ready" />)

    expect(loading).toContain('disabled=""')
    expect(loading).not.toContain("aria-controls=")
    expect(empty).not.toContain("aria-controls=")
    expect(empty).not.toContain("aria-activedescendant=")
  })

  test("uses canonical archive semantics and a semantic nested list", () => {
    const archivedProject: NavigationProject = { ...project, archivedAt: 1 }
    const tree = renderToStaticMarkup(
      <ProjectTree project={archivedProject} activeSurface="overview" labels={{ archived: "已归档" }} />,
    )

    expect(tree).toContain("已归档")
    expect(tree).not.toContain("archived")
    expect(tree).not.toContain('role="tree"')
    expect(tree).not.toContain('role="treeitem"')
    expect(tree).toContain('aria-current="page"')
    expect(tree).not.toContain('data-testid="project-tree-tasks"')
  })

  test("keeps product and project destinations as native fallback links", () => {
    const rail = renderToStaticMarkup(
      <ProductRail hrefs={{ projects: "/app/", settings: "/app/settings" }} onNavigate={vi.fn()} />,
    )
    const sidebar = renderToStaticMarkup(
      <ProjectsSidebar
        projects={[project]}
        activeProjectSlug={project.slug}
        activeSurface="overview"
        activeSection="project"
        projectsHref="/app/"
        basePath="/app/"
        onSectionSelect={vi.fn()}
        onProjectSelect={vi.fn()}
        onSurfaceSelect={vi.fn()}
      />,
    )

    expect(rail).toContain('href="/app/"')
    expect(rail).toContain('href="/app/settings"')
    expect(sidebar).toContain('data-testid="projects-sidebar-projects"')
    expect(sidebar).toContain('href="/app/"')
    expect(sidebar).toContain('href="/app/boards/navigation/overview"')
    expect(sidebar).toContain('href="/app/boards/navigation/board"')
  })

  test("exposes a desktop sidebar resize separator without inline style state", () => {
    const markup = renderToStaticMarkup(
      <ProjectsSidebar
        projects={[project]}
        sidebarWidthStep={62}
        onSidebarWidthStepChange={vi.fn()}
        onSidebarWidthReset={vi.fn()}
      />,
    )

    expect(markup).toContain('role="separator"')
    expect(markup).toContain('aria-valuemin="56"')
    expect(markup).toContain('aria-valuemax="80"')
    expect(markup).toContain('aria-valuenow="62"')
    expect(markup).toContain('data-testid="sidebar-resize-handle"')
    expect(markup).not.toContain('style=')
  })

  test("keeps the picker query API controlled when both props are supplied", () => {
    const onQueryChange = vi.fn()
    const markup = renderToStaticMarkup(
      <ProjectPicker projects={[project]} query="nav" onQueryChange={onQueryChange} />,
    )

    expect(markup).toContain('value="nav"')
    expect(markup).toContain('role="combobox"')
  })

  test("connects the menu trigger to the drawer and does not navigate disabled More links", () => {
    const header = renderToStaticMarkup(
      <ResourceHeader
        breadcrumbs={[{ label: "Projects" }]}
        onMenuToggle={vi.fn()}
        menuControlsId="navigation-drawer"
        menuOpen={false}
      />,
    )
    const more = renderToStaticMarkup(
      <ResourceHeader
        breadcrumbs={[{ label: "Projects" }]}
        moreItems={[{ id: "disabled", label: "Disabled", href: "/should-not-navigate", disabled: true }]}
      />,
    )

    expect(header).toContain('aria-controls="navigation-drawer"')
    expect(header).toContain("Open project navigation")
    expect(more).not.toContain('href="/should-not-navigate"')
    expect(more).toContain('disabled=""')
  })

  test("keeps callback-less header actions deferred and removes the placeholder Home", () => {
    const sidebar = renderToStaticMarkup(<ProjectsSidebar projects={[project]} />)
    const header = renderToStaticMarkup(
      <ResourceHeader
        breadcrumbs={[{ label: "Projects" }]}
        actions={[{ id: "diagnostics", label: "Diagnostics" }]}
        moreItems={[{ id: "future", label: "Future" }]}
      />,
    )

    expect(sidebar).not.toContain("Home (planned)")
    expect(sidebar).not.toContain('data-testid="projects-sidebar-home"')
    expect(header).toContain('data-testid="resource-header-action-diagnostics"')
    expect((header.match(/disabled=""/g) ?? []).length).toBe(2)
  })
})
