import { describe, expect, it } from "vitest"

import { buildHealthRuntimeModel } from "./HealthView"
import type { HealthStatus } from "@/lib/api"

describe("Health runtime model", () => {
  it("includes database path and fingerprint with fallbacks for older health responses", () => {
    const model = buildHealthRuntimeModel({ ok: true, db: "ok", version: "1.1.2" })

    expect(model.metrics).toEqual([
      { id: "ok", label: "ok", value: "true", tone: "ready" },
      { id: "db", label: "db", value: "ok", tone: "ready" },
      { id: "version", label: "version", value: "1.1.2", tone: "secondary" },
      { id: "db-path", label: "db_path", value: "not reported", tone: "secondary" },
      { id: "db-fingerprint", label: "db_fingerprint", value: "not reported", tone: "secondary" },
    ])
  })

  it("uses health database identity when the backend reports it", () => {
    const health = {
      ok: true,
      db: "ok",
      version: "1.1.2",
      db_path: "/tmp/current/kb.db",
      db_fingerprint: "sqlite:131072:1717520000000",
    } satisfies HealthStatus

    const model = buildHealthRuntimeModel(health)

    expect(model.metrics).toContainEqual({ id: "db-path", label: "db_path", value: "/tmp/current/kb.db", tone: "secondary" })
    expect(model.metrics).toContainEqual({
      id: "db-fingerprint",
      label: "db_fingerprint",
      value: "sqlite:131072:1717520000000",
      tone: "secondary",
    })
  })
})
