import { renderToStaticMarkup } from "react-dom/server"
import { afterEach, describe, expect, test } from "vitest"

import { ProductShell } from "./ProductShell"
import type { BoardListSurface } from "./ProductShell"
import { PreferencesProvider } from "./lib/preferences-provider"
import { PreferencesContext } from "./lib/preferences-context"
import { assertCanonicalBoardSlug } from "./lib/board-slug"
import { parseAppRoute } from "./lib/router"
import type { WebRuntimeConfig } from "./lib/runtime"
import { BoardListReadError, type BoardListItem } from "./lib/api/board-list-read-model"
import { asCanonicalBoardId } from "./lib/sync/contracts"
import { isSuccessfulProjectNavigation, parseRecentProjectSlugs, projectPickerGroups } from "./lib/project-picker"

const runtime: WebRuntimeConfig = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "test",
}

const originalNavigator = globalThis.navigator

afterEach(() => {
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: originalNavigator })
})

function setOffline(): void {
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: { onLine: false } })
}

function render(route: ReturnType<typeof parseAppRoute>) {
  return renderToStaticMarkup(
    <PreferencesProvider>
      <ProductShell runtime={runtime} route={route} />
    </PreferencesProvider>,
  )
}

function renderWithLiveChild(route: ReturnType<typeof parseAppRoute>) {
  return renderToStaticMarkup(
    <PreferencesProvider>
      <ProductShell runtime={runtime} route={route}>
        <div data-testid="live-board-child">live session</div>
      </ProductShell>
    </PreferencesProvider>,
  )
}

function renderWithBoardList(route: ReturnType<typeof parseAppRoute>, surface: BoardListSurface = boardListSurface()) {
  return renderToStaticMarkup(
    <PreferencesProvider>
      <ProductShell runtime={runtime} route={route} boardList={surface}>
        <div data-testid="live-board-child">live session</div>
      </ProductShell>
    </PreferencesProvider>,
  )
}

const boardListItems: readonly BoardListItem[] = [
  Object.freeze({ id: asCanonicalBoardId("b_default"), slug: assertCanonicalBoardSlug("default"), name: "Default Board", description: null, archivedAt: null }),
  Object.freeze({ id: asCanonicalBoardId("b_other"), slug: assertCanonicalBoardSlug("other"), name: "Other Board", description: null, archivedAt: null }),
]

function boardListSurface(overrides: Partial<BoardListSurface> = {}): BoardListSurface {
  return {
    status: "ready",
    items: boardListItems,
    error: null,
    isRefreshing: false,
    onRetry: () => undefined,
    ...overrides,
  }
}

describe("ProductShell route offline boundary", () => {
  test("keeps /app/ as the Projects collection and never mounts the live child", () => {
    const markup = renderWithBoardList(parseAppRoute("http://kanban.test/app/"))

    expect(markup).toContain('data-testid="projects-collection"')
    expect(markup).not.toContain('data-testid="live-board-child"')
    expect(markup).not.toContain('data-testid="board-live-session"')
  })

  test("renders an archived overview from the same board-list snapshot", () => {
    const archived = Object.freeze({
      id: asCanonicalBoardId("b_archived"),
      slug: assertCanonicalBoardSlug("archived"),
      name: "Archived Board",
      description: "Retained identity",
      archivedAt: 1,
    })
    const archivedSurface = boardListSurface({ items: [...boardListItems, archived] })
    const markup = renderWithBoardList(
      parseAppRoute("http://kanban.test/app/boards/archived/overview"),
      archivedSurface,
    )
    const tasksMarkup = renderWithBoardList(
      parseAppRoute("http://kanban.test/app/boards/archived/board"),
      archivedSurface,
    )
    const offlineTasksMarkup = renderWithBoardList(
      parseAppRoute("http://kanban.test/app/boards/archived/board"),
      { ...archivedSurface, status: "offline" },
    )

    expect(markup).toContain('data-testid="project-overview"')
    expect(markup).toContain('data-archived="true"')
    expect(markup).not.toContain('data-testid="board-live-session"')
    expect(markup).not.toContain('data-testid="project-overview-open-tasks"')
    expect(markup).not.toContain('data-testid="project-tree-tasks"')
    expect(markup).not.toContain('data-testid="resource-header-action-open-tasks"')
    expect(markup).not.toContain('href="/app/boards/archived/runs"')
    expect(markup).not.toContain('href="/app/boards/archived/maintenance"')
    expect(tasksMarkup).toContain('data-testid="shell-project-archived"')
    expect(tasksMarkup).not.toContain("路由边界")
    expect(tasksMarkup).toContain('href="/app/boards/archived/overview"')
    expect(tasksMarkup).not.toContain('data-testid="explorer-page"')
    expect(tasksMarkup).not.toContain('data-testid="board-live-session"')
    expect(offlineTasksMarkup).toContain('data-testid="shell-project-archived"')
    expect(offlineTasksMarkup).not.toContain("路由边界")
    expect(offlineTasksMarkup).not.toContain('data-testid="explorer-page"')
    expect(offlineTasksMarkup).not.toContain('data-testid="board-live-session"')
  })

  test("turns a ready snapshot miss into typed not-found instead of fetching a project", () => {
    const markup = renderWithBoardList(parseAppRoute("http://kanban.test/app/boards/missing/overview"))

    expect(markup).toContain('data-testid="shell-project-not-found"')
    expect(markup).not.toContain("路由边界")
    expect(markup).not.toContain('data-testid="project-overview"')
    expect(markup).not.toContain('data-testid="board-live-session"')
  })

  test("keeps feature-owned routes behind the ready project snapshot boundary", () => {
    const markup = renderWithBoardList(parseAppRoute("http://kanban.test/app/boards/missing/signals"))

    expect(markup).toContain('data-testid="shell-project-not-found"')
    expect(markup).not.toContain('data-testid="live-board-child"')
    expect(markup).not.toContain('data-testid="signals-screen"')
  })

  test("does not keep an overview in loading when the list is offline without a snapshot", () => {
    const markup = renderWithBoardList(
      parseAppRoute("http://kanban.test/app/boards/missing/overview"),
      boardListSurface({ status: "offline", items: [] }),
    )

    expect(markup).toContain('data-testid="project-overview-unavailable"')
    expect(markup).toContain('data-status="offline"')
    expect(markup).not.toContain('data-testid="project-overview-loading"')
  })

  test("keeps an overview identity snapshot visible while the list is stale", () => {
    const markup = renderWithBoardList(
      parseAppRoute("http://kanban.test/app/boards/default/overview"),
      boardListSurface({ status: "stale" }),
    )

    expect(markup).toContain('data-testid="project-overview"')
    expect(markup).toContain('data-testid="project-overview-status"')
    expect(markup).toContain('data-status="stale"')
    expect(markup).not.toContain('data-testid="project-overview-loading"')
  })

  test("parses only versioned, bounded canonical recent project slugs", () => {
    const recent = parseRecentProjectSlugs(JSON.stringify({ version: 1, slugs: ["other", "default", "other", "b_invalid!", "third", "fourth", "fifth", "sixth"] }))
    expect(recent).toEqual([assertCanonicalBoardSlug("other"), assertCanonicalBoardSlug("default"), assertCanonicalBoardSlug("third"), assertCanonicalBoardSlug("fourth"), assertCanonicalBoardSlug("fifth")])
    expect(parseRecentProjectSlugs(JSON.stringify({ version: 2, slugs: ["other"] }))).toEqual([])
    expect(parseRecentProjectSlugs("not-json")).toEqual([])
  })

  test("keeps current and recent projects out of the all-projects group", () => {
    const groups = projectPickerGroups(
      boardListItems,
      assertCanonicalBoardSlug("default"),
      [assertCanonicalBoardSlug("other")],
      "",
      "zh",
    )
    expect(groups.current.map((item) => item.slug)).toEqual(["default"])
    expect(groups.recent.map((item) => item.slug)).toEqual(["other"])
    expect(groups.all).toEqual([])

    const filtered = projectPickerGroups(
      boardListItems,
      assertCanonicalBoardSlug("default"),
      [assertCanonicalBoardSlug("other")],
      "default",
      "zh",
    )
    expect(filtered.current.map((item) => item.slug)).toEqual(["default"])
    expect(filtered.recent).toEqual([])
    expect(filtered.all).toEqual([])
  })

  test("remembers a project only after an explicit non-current board route", () => {
    const targetSlug = assertCanonicalBoardSlug("other")
    const reachedTarget = parseAppRoute("http://kanban.test/app/boards/other/board")
    const currentRoute = parseAppRoute("http://kanban.test/app/boards/default/board")

    expect(isSuccessfulProjectNavigation(reachedTarget, targetSlug, assertCanonicalBoardSlug("default"))).toBe(true)
    expect(isSuccessfulProjectNavigation(undefined, targetSlug, assertCanonicalBoardSlug("default"))).toBe(false)
    expect(isSuccessfulProjectNavigation(currentRoute, targetSlug, assertCanonicalBoardSlug("default"))).toBe(false)
    expect(isSuccessfulProjectNavigation(reachedTarget, targetSlug, targetSlug)).toBe(false)
    expect(isSuccessfulProjectNavigation({ kind: "board", boardSlug: targetSlug }, targetSlug)).toBe(false)
    expect(isSuccessfulProjectNavigation({ kind: "not-found", pathname: "/app/missing" }, targetSlug)).toBe(false)
    expect(isSuccessfulProjectNavigation(new Error("navigation failed"), targetSlug)).toBe(false)
  })

  test("keeps Events mounted so its own offline snapshot can render", () => {
    setOffline()
    const markup = render(parseAppRoute("http://kanban.test/app/boards/default/events"))

    expect(markup).toContain('data-testid="explorer-page"')
    expect(markup).toContain('data-testid="events-offline"')
    expect(markup).not.toContain('data-testid="shell-offline"')
  })

  test("keeps the Explorer route mounted for the Board view offline", () => {
    setOffline()
    const markup = render(parseAppRoute("http://kanban.test/app/boards/default"))

    expect(markup).toContain('data-testid="explorer-page"')
    expect(markup).not.toContain('data-testid="shell-offline"')
  })

  test("keeps the live child hidden while the default board exposes Explorer tabs", () => {
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: { onLine: true } })
    const markup = renderWithLiveChild(parseAppRoute("http://kanban.test/app/boards/default"))

    expect(markup).toContain('data-testid="explorer-page"')
    expect(markup).toContain('aria-label="任务视图"')
    expect(markup).toContain('href="/app/boards/default/list"')
    expect(markup).toContain('href="/app/boards/default/map"')
    expect(markup).toContain('href="/app/boards/default/runs"')
    expect(markup).toContain('href="/app/boards/default/events"')
    expect(markup).toContain('href="/app/boards/default/health"')
    expect(markup).toContain('href="/app/boards/default/maintenance"')
    expect(markup.match(/href="\/app\/boards\/default\/runs"/g)).toHaveLength(1)
    expect(markup).toContain('data-testid="board-live-session"')
    expect(markup).toContain('data-testid="live-board-child"')
  })

  test("returns taskless Runs to the current board Tasks route", () => {
    const markup = renderWithLiveChild(parseAppRoute("http://kanban.test/app/boards/default/runs"))

    expect(markup).toContain('data-testid="runs-no-task"')
    expect(markup).toContain("返回任务")
    expect(markup).toContain('href="/app/boards/default/board"')
  })

  test("keeps the feature child visible while BoardLive stays mounted without Explorer overlap", () => {
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: { onLine: true } })
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <ProductShell runtime={runtime} route={parseAppRoute("http://kanban.test/app/boards/default/signals")}>
          <div data-testid="board-live-session-owner">live session owner</div>
          <div data-testid="signals-feature-owner">signals feature owner</div>
        </ProductShell>
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-testid="board-live-session-owner"')
    expect(markup).toContain('data-testid="signals-feature-owner"')
    expect(markup).not.toContain('data-testid="board-live-session"')
    expect(markup).not.toContain('data-testid="explorer-page"')
  })

  test("keeps operator pages mounted with an existing hidden BoardLive child while offline", () => {
    setOffline()
    const healthMarkup = renderWithLiveChild(parseAppRoute("http://kanban.test/app/boards/default/health"))
    expect(healthMarkup).toContain('data-testid="health-page"')
    expect(healthMarkup).toContain('data-testid="board-live-session"')
    expect(healthMarkup).not.toContain('data-testid="shell-offline"')

    const maintenanceMarkup = renderWithLiveChild(parseAppRoute("http://kanban.test/app/boards/default/maintenance"))
    expect(maintenanceMarkup).toContain('data-testid="maintenance-page"')
    expect(maintenanceMarkup).toContain('data-testid="board-live-session"')
    expect(maintenanceMarkup).not.toContain('data-testid="shell-offline"')
  })

  test("does not invent a BoardLive child for direct operator deep links", () => {
    const healthMarkup = render(parseAppRoute("http://kanban.test/app/boards/default/health"))
    const maintenanceMarkup = render(parseAppRoute("http://kanban.test/app/boards/default/maintenance"))

    expect(healthMarkup).toContain('data-testid="health-page"')
    expect(healthMarkup).not.toContain('data-testid="board-live-session"')
    expect(maintenanceMarkup).toContain('data-testid="maintenance-page"')
    expect(maintenanceMarkup).not.toContain('data-testid="board-live-session"')
  })

  test("uses the Plane-only rail, project sidebar and current project surfaces", () => {
    const markup = renderToStaticMarkup(
      <PreferencesContext.Provider value={{
        theme: "light",
        locale: "zh",
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
        <ProductShell
          runtime={runtime}
          canonicalBoardSlug={assertCanonicalBoardSlug("default")}
          route={parseAppRoute("http://kanban.test/app/boards/default/board")}
          boardList={boardListSurface()}
        />
      </PreferencesContext.Provider>,
    )
    const positions = ["product-rail-projects", "projects-sidebar-projects", "project-tree-overview", "project-tree-tasks", "product-rail-settings"].map((testId) => markup.indexOf(`data-testid="${testId}"`))
    expect(positions.every((position) => position >= 0)).toBe(true)
    expect(markup).toContain('data-testid="projects-sidebar"')
    expect(markup).toContain('data-shell-viewport="desktop"')
  })

  test("keeps operator routes as real diagnostics deep links", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <ProductShell runtime={runtime} canonicalBoardSlug={assertCanonicalBoardSlug("default")} route={parseAppRoute("http://kanban.test/app/boards/default/maintenance")} />
      </PreferencesProvider>,
    )
    expect(markup).toContain('data-testid="maintenance-page"')
    expect(markup).toContain('href="/app/boards/default/maintenance"')
    expect(markup).toContain('data-route-surface="maintenance"')
    expect(markup).toContain('data-more-active="true"')
  })

  test("uses descriptor project navigation for diagnostics without marking Tasks active", () => {
    const markup = renderWithBoardList(parseAppRoute("http://kanban.test/app/boards/default/signals"))

    expect(markup).toContain('data-route-surface="signals"')
    expect(markup).not.toContain('aria-current="page" href="/app/boards/default/board" data-testid="project-tree-tasks"')
  })

  test("renders a canonical board switcher without inventing a second selection state", () => {
    const markup = renderToStaticMarkup(
      <PreferencesContext.Provider value={{
        theme: "light",
        locale: "zh",
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
        <ProductShell
          runtime={runtime}
          canonicalBoardSlug={assertCanonicalBoardSlug("other")}
          route={parseAppRoute("http://kanban.test/app/boards/other/board")}
          boardList={boardListSurface()}
        />
      </PreferencesContext.Provider>,
    )

    expect(markup).toContain('data-testid="project-picker"')
    expect(markup).toContain('data-testid="project-picker-options"')
    expect(markup).toContain('data-project-picker-input="true"')
    expect(markup).toContain('data-testid="projects-sidebar"')
    expect(markup).toContain('data-testid="product-rail-settings"')
  })

  test("keeps list failures local and actionable", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <ProductShell
          runtime={runtime}
          canonicalBoardSlug={assertCanonicalBoardSlug("default")}
          route={parseAppRoute("http://kanban.test/app/boards/default/board")}
          boardList={boardListSurface({ status: "error", error: new BoardListReadError("http", "hidden detail") })}
        />
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-testid="project-picker-error"')
    expect(markup).toContain('>重新加载</button>')
    expect(markup).toContain('data-testid="explorer-page"')
  })
})
