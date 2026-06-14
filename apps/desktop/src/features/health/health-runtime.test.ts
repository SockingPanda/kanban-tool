import { describe, expect, it } from "vitest"

import { buildHealthRuntimeModel } from "./HealthView"
import type { HealthStatus, RuntimeConfig } from "@/lib/api"

const config = {
  apiBaseUrl: "/__kb_api__",
  dbPath: "/tmp/current/kb.db",
  actor: "desktop-test",
  board: "default",
} satisfies RuntimeConfig

describe("Health runtime model", () => {
  it("includes database path and fingerprint with fallbacks for older health responses", () => {
    const model = buildHealthRuntimeModel({ ok: true, db: "ok", version: "1.1.2" }, config)

    expect(model.metrics).toEqual([
      { label: "ok", value: "true", tone: "ready" },
      { label: "db", value: "ok", tone: "ready" },
      { label: "version", value: "1.1.2", tone: "secondary" },
      { label: "db_path", value: "not reported", tone: "secondary" },
      { label: "db_fingerprint", value: "not reported", tone: "secondary" },
    ])
    expect(model.warning).toBeNull()
  })

  it("uses health database identity when the backend reports it", () => {
    const health = {
      ok: true,
      db: "ok",
      version: "1.1.2",
      db_path: "/tmp/current/kb.db",
      db_fingerprint: "sqlite:131072:1717520000000",
    } satisfies HealthStatus

    const model = buildHealthRuntimeModel(health, config)

    expect(model.metrics).toContainEqual({ label: "db_path", value: "/tmp/current/kb.db", tone: "secondary" })
    expect(model.metrics).toContainEqual({
      label: "db_fingerprint",
      value: "sqlite:131072:1717520000000",
      tone: "secondary",
    })
    expect(model.warning).toBeNull()
  })

  it("warns when health path clearly disagrees with runtime config", () => {
    const model = buildHealthRuntimeModel(
      {
        ok: true,
        db: "ok",
        version: "1.1.2",
        db_path: "/tmp/stale/kb.db",
        db_fingerprint: "sqlite:65536:1717529999999",
      },
      config,
    )

    expect(model.warning).toEqual({
      title: "Runtime database mismatch",
      message:
        "Health is responding from /tmp/stale/kb.db, but the desktop runtime is configured for /tmp/current/kb.db. Restart kanban serve, check that this window is using the intended port, and verify VITE_KB_API_BASE_URL / VITE_KB_DEV_PROXY_TARGET.",
    })
  })

  it("does not warn when the configured database is an external API placeholder", () => {
    const model = buildHealthRuntimeModel(
      {
        ok: true,
        db: "ok",
        version: "1.1.2",
        db_path: "/tmp/server/kb.db",
        db_fingerprint: "sqlite:65536:1717529999999",
      },
      { ...config, dbPath: "external API" },
    )

    expect(model.warning).toBeNull()
  })
})
