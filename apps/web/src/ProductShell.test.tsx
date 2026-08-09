import { renderToStaticMarkup } from "react-dom/server"
import { afterEach, describe, expect, test } from "vitest"

import { ProductShell } from "./ProductShell"
import { PreferencesProvider } from "./lib/preferences-provider"
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

  test("keeps the shell offline boundary for the Board live view", () => {
    setOffline()
    const markup = render(parseAppRoute("http://kanban.test/app/boards/default"))

    expect(markup).toContain('data-testid="shell-offline"')
    expect(markup).not.toContain('data-testid="explorer-page"')
  })

  test("keeps the live child hidden while the default board exposes Explorer tabs", () => {
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: { onLine: true } })
    const markup = renderWithLiveChild(parseAppRoute("http://kanban.test/app/boards/default"))

    expect(markup).toContain('data-testid="explorer-page"')
    expect(markup).toContain('aria-label="看板浏览视图"')
    expect(markup).toContain('data-testid="board-live-session"')
    expect(markup).toContain('data-testid="live-board-child"')
  })
})
