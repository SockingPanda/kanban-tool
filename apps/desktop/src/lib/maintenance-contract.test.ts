import { afterEach, describe, expect, it, vi } from "vitest"

import { KanbanApi } from "./api"

const config = {
  apiBaseUrl: "http://127.0.0.1:8721",
  actor: "desktop-test",
  board: "default",
}

const store = {
  store_name: "tasks",
  active_generation: "gen-current",
  active_fingerprint: "fp-current",
  previous_generation: null,
  building_generation: null,
  lifecycle_status: "ready",
  fence_epoch: 7,
  last_event_id: 42,
  dirty: false,
  pending: 0,
  running: 0,
  failed: 0,
  last_error: null,
  phase: "ready",
  degraded: false,
  errors: [],
  updated_at: 1700000000,
}

afterEach(() => vi.unstubAllGlobals())

describe("Desktop host maintenance API", () => {
  it("calls the typed host-admin routes and parses their reports", async () => {
    const responses = [
      { data: { out_path: "/tmp/backup.db", checksum_sha256: "sha-backup", bytes: 12, source_fingerprint: "source" } },
      { data: { out_path: "/tmp/export.jsonl", checksum_sha256: "sha-export", bytes: 14, record_count: 3, source_fingerprint: "source" } },
      { data: { in_path: "/tmp/export.jsonl", source_fingerprint: "source", imported_records: 3, skipped_records: 1, rebuild_jobs_enqueued: 2, journal_id: "journal-1" } },
      { data: { journal_id: "journal-2", phase: "completed", source_path: "/tmp/legacy.sqlite", source_fingerprint: "source", schema_fingerprint: "schema", resumed: false, attachment_count: 1, table_counts: [{ table: "tasks", source_rows: 3, target_rows: 3 }] } },
      { data: { ok: true, before_bytes: 100, after_bytes: 80, source_fingerprint: "source" } },
      { data: { database_instance_id: "db-1", protocol_version: 2, owner: { owner: "desktop-test", mode: "continuous", lease_expires_at: 1700000020, fence_epoch: 7, build_identity: "build-1", last_heartbeat_at: 1700000000, active: true }, stores: [store] } },
      { data: { database_instance_id: "db-1", protocol_version: 2, owner: "desktop-test", mode: "oneshot", action: "rebuild", processed: 1, phase: "degraded", degraded: true, errors: ["vector provider unavailable"], stores: [store] } },
    ]
    const fetchMock = vi.fn(async () => new Response(JSON.stringify(responses.shift()), { status: 200, headers: { "content-type": "application/json" } }))
    vi.stubGlobal("fetch", fetchMock)
    const api = new KanbanApi(config, { locale: "zh-CN" })

    await expect(api.backup("/tmp/backup.db")).resolves.toMatchObject({ out_path: "/tmp/backup.db" })
    await expect(api.exportData("/tmp/export.jsonl")).resolves.toMatchObject({ record_count: 3 })
    await expect(api.importData("/tmp/export.jsonl", true)).resolves.toMatchObject({ imported_records: 3 })
    await expect(api.importLegacySqliteV30("/tmp/legacy.sqlite", "/tmp/attachments")).resolves.toMatchObject({ phase: "completed" })
    await expect(api.vacuum()).resolves.toMatchObject({ after_bytes: 80 })
    await expect(api.maintenanceStatus()).resolves.toMatchObject({ owner: { owner: "desktop-test" }, stores: [{ active_generation: "gen-current" }] })
    await expect(api.maintenanceRebuild("desktop-test")).resolves.toMatchObject({ action: "rebuild" })

    const calls = fetchMock.mock.calls as unknown as [RequestInfo | URL, RequestInit][]
    expect(calls.map(([url]) => new URL(String(url)).pathname)).toEqual([
      "/api/v1/maintenance/backup",
      "/api/v1/maintenance/export",
      "/api/v1/maintenance/import",
      "/api/v1/maintenance/import-v30",
      "/api/v1/maintenance/vacuum",
      "/api/v1/maintenance/status",
      "/api/v1/maintenance/rebuild",
    ])
    expect(JSON.parse(String(calls[2]?.[1]?.body))).toEqual({ path: "/tmp/export.jsonl", replace: true })
    expect(JSON.parse(String(calls[3]?.[1]?.body))).toEqual({ path: "/tmp/legacy.sqlite", canonical_attachment_root: "/tmp/attachments" })
  })

  it("rejects malformed maintenance status reports instead of accepting unknown fields", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ data: { database_instance_id: "db-1", protocol_version: 2, owner: {}, stores: [] } }), { status: 200 })))

    await expect(new KanbanApi(config).maintenanceStatus()).rejects.toMatchObject({ code: "invalid_response" })
  })
})
