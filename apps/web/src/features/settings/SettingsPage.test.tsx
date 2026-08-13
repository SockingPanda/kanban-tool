import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import type { HealthReport } from "../../lib/api/health-read-model"
import { PreferencesProvider } from "../../lib/preferences-provider"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { assertCanonicalBoardSlug } from "../../lib/board-slug"
import { SettingsPage } from "./SettingsPage"
import { apiOriginForRuntime, diagnosticsText } from "./settings-diagnostics"

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "local",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "build-test",
} satisfies WebRuntimeConfig

const health = {
  ok: true,
  db: "turso",
  version: "3.0.0",
  db_path: "/tmp/kanban.db",
  db_fingerprint: "sha256:test",
} satisfies HealthReport

describe("SettingsPage", () => {
  test("renders appearance, language, identity, read-only connection and diagnostics controls", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <SettingsPage runtime={runtime} boardSlug={assertCanonicalBoardSlug("default")} initialHealth={health} />
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-testid="settings-page"')
    expect(markup).toContain('data-testid="appearance-theme"')
    expect(markup).toContain('value="system"')
    expect(markup).toContain('data-testid="appearance-density"')
    expect(markup).toContain('value="comfortable"')
    expect(markup).toContain('data-testid="settings-locale"')
    expect(markup).toContain('data-testid="identity-actor"')
    expect(markup).toContain('data-testid="connection-api-origin"')
    expect(markup).toContain('data-testid="connection-default-board"')
    expect(markup).toContain('data-testid="connection-protocol-version"')
    expect(markup).not.toContain('data-testid="connection-sidebar-state"')
    expect(markup).toContain('data-testid="diagnostics-copy"')
    expect(markup).toContain('data-testid="diagnostics-health-link"')
    expect(markup).toContain("sha256:test")
    expect(markup).not.toContain("KANBAN TOOL / 项目")
    expect(markup).not.toContain('class="eyebrow"')
  })

  test("renders a no-board connection boundary without inventing a health route", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <SettingsPage runtime={{ ...runtime, defaultBoard: "" }} />
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-testid="settings-no-board"')
    expect(markup).toContain('data-testid="diagnostics-health-link"')
    expect(markup).toContain("disabled")
  })

  test("does not treat a runtime selector as a canonical board slug", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <SettingsPage runtime={{ ...runtime, defaultBoard: "selector:active" }} />
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-testid="settings-no-board"')
    expect(markup).toContain('data-testid="diagnostics-health-link"')
    expect(markup).toContain("disabled")
  })

  test("keeps diagnostics payload independent from server-controlled database paths", () => {
    expect(apiOriginForRuntime(runtime, "https://kanban.test/app/settings")).toBe("https://kanban.test")
    expect(diagnosticsText(runtime, null, "https://kanban.test/app/settings")).toContain("serverVersion=3.0.0")
    expect(diagnosticsText(runtime, health, "https://kanban.test/app/settings")).not.toContain("/tmp/kanban.db")
  })

  test("accepts an injected clipboard and reconnect seam", () => {
    const clipboard = vi.fn(async () => undefined)
    const reconnect = vi.fn(() => true)
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <SettingsPage runtime={runtime} boardSlug={assertCanonicalBoardSlug("default")} clipboardWrite={clipboard} onReconnect={reconnect} />
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-testid="connection-reconnect"')
    expect(markup).toContain('data-testid="diagnostics-copy"')
  })
})
