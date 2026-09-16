import { renderToStaticMarkup } from "../../../tests/support/render"
import { afterEach, describe, expect, test } from "vitest"

import { ProductShell } from "./app-shell"
import { PreferencesProvider } from "../../platform/preferences/preferences-provider"
import { PreferencesContext } from "../../platform/preferences/preferences-context"
import { assertCanonicalBoardSlug } from "../../domain/board-slug"
import { parseAppRoute } from "../../application/navigation/router"
import type { WebRuntimeConfig } from "../../lib/runtime"

const runtime: WebRuntimeConfig = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v2",
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

describe("ProductShell route offline boundary", () => {
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
    expect(markup).toContain('aria-label="视图选项"')
    expect(markup).toContain('href="/app/boards/default/list"')
    expect(markup).toContain("依赖图")
    expect(markup).toContain('data-testid="nav-runs"')
    expect(markup).toContain('data-testid="nav-events"')
    expect(markup).toContain('data-testid="board-live-session"')
    expect(markup).toContain('data-testid="live-board-child"')
  })

  test.each(["signals", "ontology"])("redirects a removed %s route without mounting its feature", (view) => {
    const markup = renderWithLiveChild(parseAppRoute("http://kanban.test/app/boards/default/" + view));
    expect(markup).toContain('data-testid="explorer-page"');
    expect(markup).toContain('该功能已移除');
    expect(markup).not.toContain('data-testid="nav-signals"');
    expect(markup).not.toContain('data-testid="nav-ontology"');
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

  test("orders collapsed navigation by task views and operator pages", () => {
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
        <ProductShell runtime={runtime} canonicalBoardSlug={assertCanonicalBoardSlug("default")} route={parseAppRoute("http://kanban.test/app/boards/default/board")} />
      </PreferencesContext.Provider>,
    )
    const positions = ["nav-board", "nav-events", "nav-runs", "nav-settings"].map((testId) => markup.indexOf(`data-testid="${testId}"`))
    expect(positions.every((position) => position >= 0)).toBe(true)
    expect(positions).toEqual([...positions].sort((left, right) => left - right))
  })

  test("marks operator navigation as the current page", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <ProductShell runtime={runtime} canonicalBoardSlug={assertCanonicalBoardSlug("default")} route={parseAppRoute("http://kanban.test/app/boards/default/maintenance")} />
      </PreferencesProvider>,
    )
    const nav = markup.slice(markup.indexOf('data-testid="nav-settings"') - 240, markup.indexOf('data-testid="nav-settings"') + 80)
    expect(nav).toContain('aria-current="page"')
  })
})
