import { renderToStaticMarkup } from "react-dom/server"
import { afterEach, describe, expect, test } from "vitest"

import { ProductShell } from "./ProductShell"
import { PreferencesProvider } from "./lib/preferences-provider"
import { PreferencesContext } from "./lib/preferences-context"
import { assertCanonicalBoardSlug } from "./lib/board-slug"
import { parseAppRoute } from "./lib/router"
import type { WebRuntimeConfig } from "./lib/runtime"

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

  test("orders collapsed navigation as Board, Signals, Ontology, Settings", () => {
    const markup = renderToStaticMarkup(
      <PreferencesContext.Provider value={{
        theme: "light",
        locale: "zh",
        sidebarExpanded: false,
        setTheme: () => undefined,
        setLocale: () => undefined,
        setSidebarExpanded: () => undefined,
        toggleSidebar: () => undefined,
      }}>
        <ProductShell runtime={runtime} canonicalBoardSlug={assertCanonicalBoardSlug("default")} route={parseAppRoute("http://kanban.test/app/boards/default/board")} />
      </PreferencesContext.Provider>,
    )
    const positions = ["nav-board", "nav-signals", "nav-ontology", "nav-settings"].map((testId) => markup.indexOf(`data-testid="${testId}"`))
    expect(positions.every((position) => position >= 0)).toBe(true)
    expect(positions).toEqual([...positions].sort((left, right) => left - right))
  })
})
