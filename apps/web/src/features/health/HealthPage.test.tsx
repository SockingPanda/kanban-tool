import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import type { WebRuntimeConfig } from "../../lib/runtime"
import type { HealthReport } from "../../lib/api/health-read-model"
import { PreferencesProvider } from "../../lib/preferences-provider"
import { HealthPage } from "./HealthPage"
import { healthMetricTone } from "./health-metrics"

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
  db: "ok",
  version: "3.0.0",
  db_path: "/tmp/kanban.db",
  db_fingerprint: "sha256:test",
} satisfies HealthReport

describe("HealthPage", () => {
  test("renders every diagnostic metric and runtime identity", () => {
    const markup = renderToStaticMarkup(<PreferencesProvider><HealthPage runtime={runtime} initialReport={health} /></PreferencesProvider>)

    expect(markup).toContain('data-testid="health-page"')
    expect(markup).toContain('data-testid="health-metric-ok"')
    expect(markup).toContain("true")
    expect(markup).toContain("sha256:test")
    expect(markup).toContain(runtime.actor)
    expect(markup).toContain(runtime.webBuildId)
  })

  test("renders a retryable loading boundary without inventing health data", () => {
    const markup = renderToStaticMarkup(<PreferencesProvider><HealthPage runtime={runtime} /></PreferencesProvider>)

    expect(markup).toContain('data-testid="health-loading"')
    expect(markup).not.toContain('data-testid="health-metric-ok"')
  })

  test("treats the production turso database identity as healthy when the report is healthy", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <HealthPage runtime={runtime} initialReport={{ ...health, db: "turso" }} />
      </PreferencesProvider>,
    )

    expect(healthMetricTone(true)).toBe("ready")
    expect(markup).toContain("turso")
  })

  test("uses the localized not-reported fallback for empty identity fields", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <HealthPage runtime={runtime} initialReport={{ ...health, version: "", db_path: "", db_fingerprint: "" }} />
      </PreferencesProvider>,
    )

    expect(markup).toContain("未报告")
  })
})
