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
import { parseRecentProjectSlugs, projectPickerGroups } from "./lib/project-picker"

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
    expect(markup).toContain('aria-label="看板浏览视图"')
    expect(markup).toContain('href="/app/boards/default/list"')
    expect(markup).toContain('href="/app/boards/default/map"')
    expect(markup).toContain('href="/app/boards/default/runs"')
    expect(markup).toContain('href="/app/boards/default/events"')
    expect(markup).toContain('data-testid="board-live-session"')
    expect(markup).toContain('data-testid="live-board-child"')
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

  test("orders collapsed navigation as Board, Signals, Ontology, Health, Maintenance, Settings", () => {
    const markup = renderToStaticMarkup(
      <PreferencesContext.Provider value={{
        theme: "light",
        locale: "zh",
        sidebarExpanded: false,
        density: "comfortable",
        actor: "",
        setTheme: () => undefined,
        setLocale: () => undefined,
        setDensity: () => undefined,
        setActor: () => undefined,
        setSidebarExpanded: () => undefined,
        toggleSidebar: () => undefined,
      }}>
        <ProductShell
          runtime={runtime}
          canonicalBoardSlug={assertCanonicalBoardSlug("default")}
          route={parseAppRoute("http://kanban.test/app/boards/default/board")}
          boardList={boardListSurface()}
        />
      </PreferencesContext.Provider>,
    )
    const positions = ["nav-board", "nav-signals", "nav-ontology", "nav-health", "nav-maintenance", "nav-settings"].map((testId) => markup.indexOf(`data-testid="${testId}"`))
    expect(positions.every((position) => position >= 0)).toBe(true)
    expect(positions).toEqual([...positions].sort((left, right) => left - right))
    expect(markup).toContain('data-testid="board-switcher-collapsed-trigger"')
  })

  test("marks operator navigation as the current page", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <ProductShell runtime={runtime} canonicalBoardSlug={assertCanonicalBoardSlug("default")} route={parseAppRoute("http://kanban.test/app/boards/default/maintenance")} />
      </PreferencesProvider>,
    )
    const nav = markup.slice(markup.indexOf('data-testid="nav-maintenance"') - 240, markup.indexOf('data-testid="nav-maintenance"') + 80)
    expect(nav).toContain('aria-current="page"')
  })

  test("renders a canonical board switcher without inventing a second selection state", () => {
    const markup = renderToStaticMarkup(
      <PreferencesContext.Provider value={{
        theme: "light",
        locale: "zh",
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
        <ProductShell
          runtime={runtime}
          canonicalBoardSlug={assertCanonicalBoardSlug("other")}
          route={parseAppRoute("http://kanban.test/app/boards/other/board")}
          boardList={boardListSurface()}
        />
      </PreferencesContext.Provider>,
    )

    expect(markup).toContain('data-testid="board-switcher"')
    expect(markup).toContain('data-testid="board-switcher-search"')
    expect(markup).toContain('data-testid="board-switcher-option-default"')
    expect(markup).toContain('data-testid="board-switcher-option-other"')
    expect(markup).toContain('<optgroup label="当前项目">')
    expect(markup).toContain('<optgroup label="所有项目">')
    expect(markup).toContain('data-testid="compact-app-nav"')
    expect(markup).toContain('data-testid="compact-nav-settings"')
    expect(markup).toContain('data-testid="compact-nav-maintenance"')
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

    expect(markup).toContain('data-testid="board-switcher-error"')
    expect(markup).toContain('data-testid="board-switcher-retry"')
    expect(markup).toContain('data-testid="explorer-page"')
  })
})
