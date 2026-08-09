import { describe, expect, test, vi } from "vitest"

import type { HttpTransportResponse } from "./http-transport"
import {
  createMaintenanceApi,
  type MaintenanceApiTransport,
} from "./maintenance-api"

function response(payload: unknown): HttpTransportResponse {
  return { payload, bytes: JSON.stringify(payload).length }
}

const status = {
  database_instance_id: "db_fixture",
  protocol_version: 2,
  owner: {
    owner: null,
    mode: null,
    lease_expires_at: null,
    fence_epoch: 0,
    build_identity: null,
    last_heartbeat_at: null,
    active: false,
  },
  stores: [],
} as const

describe("maintenance API seam", () => {
  test("reads maintenance status, doctor, stats and search through generated contracts", async () => {
    const get = vi.fn(async (path: string) => {
      if (path === "/api/v1/maintenance/status") return response({ data: status })
      if (path === "/api/v1/maintenance/doctor") {
        return response({
          data: {
            active_parents_with_incomplete_required_steps: 0,
            archived_dependency_edges: 0,
            consistency_errors: 0,
            consistency_issues: [],
            consistency_warnings: 0,
            dependency_cycles: 0,
            derived_dirty_stores: 0,
            derived_error_stores: 0,
            derived_stores: [],
            executable_dependency_violations: 0,
            executable_schedule_violations: 0,
            executable_spec_violations: 0,
            expired_running_tasks: 0,
            integrity_check: "ok",
            migration_version: 1,
            missing_run_logs: 0,
            ok: true,
            ontology_ledger_errors: 0,
            ontology_ledger_issues: [],
            ontology_ledger_warnings: 0,
            orphan_running_runs: 0,
            outbox_failed: 0,
            outbox_pending: 0,
            outbox_running: 0,
            running_tasks_without_active_run: 0,
            suspicious_run_log_paths: 0,
            unplanned_active_tasks: 0,
            user_version: 1,
          },
        })
      }
      if (path === "/api/v1/stats?board=default") {
        return response({
          data: {
            board_id: "b_default",
            generated_at: 1700000000,
            status_counts: [],
            stale_claims: [],
            blocked_reasons: [],
            unplanned_active_tasks: 0,
            active_parents_with_incomplete_required_steps: 0,
          },
        })
      }
      expect(path).toBe("/api/v1/search/status?board=default")
      return response({
        data: {
          backend: "fts",
          derived_index: true,
          stale: false,
          database_instance_id: "db_fixture",
          protocol_version: 2,
          generation: "gen-1",
          resolved_board_id: "b_default",
          fallback_reason: null,
          index_version: "v1",
          last_event_id: 9,
          index_lag_events: 0,
          message: "ready",
        },
      })
    })
    const api = createMaintenanceApi({ transport: { get } })

    await expect(api.status()).resolves.toEqual(status)
    await expect(api.doctor()).resolves.toMatchObject({ ok: true, integrity_check: "ok" })
    await expect(api.stats("default")).resolves.toMatchObject({ board_id: "b_default" })
    await expect(api.searchStatus("default")).resolves.toMatchObject({ backend: "fts", stale: false })
    expect(get).toHaveBeenCalledTimes(4)
  })

  test("rejects invalid maintenance status payloads at the generated contract seam", async () => {
    const api = createMaintenanceApi({
      transport: {
        get: vi.fn(async () => response({ data: { ...status, owner: {} } })),
      },
    })

    await expect(api.status()).rejects.toMatchObject({
      kind: "invalid_contract",
      contractId: "api.maintenance-status.response",
    })
  })

  test("uses the active POST routes and preserves server result evidence", async () => {
    const request = vi.fn<NonNullable<MaintenanceApiTransport["request"]>>(async ({ path }) => {
      const payload = path.endsWith("/backup")
        ? { data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }
        : path.endsWith("/export")
          ? { data: { out_path: "/server/export.jsonl", checksum_sha256: "sha256:export", bytes: 14, record_count: 3, source_fingerprint: "sha256:source" } }
          : path.endsWith("/import")
            ? { data: { in_path: "/server/export.jsonl", source_fingerprint: "sha256:source", imported_records: 3, skipped_records: 1, rebuild_jobs_enqueued: 2, journal_id: "journal-1", phase: "completed", restart_required: false, staged_database_path: null, target_fingerprint_before: null, staged_fingerprint: null, publish_preconditions: [] } }
            : path.endsWith("/vacuum")
              ? { data: { ok: true, before_bytes: 100, after_bytes: 80, source_fingerprint: "sha256:source" } }
              : { data: { busy: 0, log_frames: 4, checkpointed_frames: 4 } }
      if (path.endsWith("/run") || path.endsWith("/rebuild") || path.endsWith("/cleanup")) {
        return response({ data: { database_instance_id: "db_fixture", protocol_version: 2, owner: "actor", mode: "once", action: path.split("/").at(-1), processed: 0, phase: "completed", degraded: false, errors: [], stores: [] } })
      }
      return response(payload)
    })
    const api = createMaintenanceApi({ transport: { get: vi.fn(), request } })

    await expect(api.backup("/requested/backup.sqlite")).resolves.toMatchObject({
      out_path: "/server/backup.sqlite",
      checksum_sha256: "sha256:backup",
    })
    await expect(api.exportData("/requested/export.jsonl")).resolves.toMatchObject({ record_count: 3 })
    await expect(api.importData("/requested/export.jsonl", true)).resolves.toMatchObject({ restart_required: false })
    await expect(api.vacuum()).resolves.toMatchObject({ after_bytes: 80 })
    await expect(api.checkpoint()).resolves.toMatchObject({ checkpointed_frames: 4 })
    await expect(api.maintenanceRun(" ", "run")).resolves.toMatchObject({ action: "run" })
    await expect(api.maintenanceRebuild("actor")).resolves.toMatchObject({ action: "rebuild" })
    await expect(api.maintenanceCleanup("actor")).resolves.toMatchObject({ action: "cleanup" })

    expect(request.mock.calls.map(([call]) => [call.method, call.path])).toEqual([
      ["POST", "/api/v1/maintenance/backup"],
      ["POST", "/api/v1/maintenance/export"],
      ["POST", "/api/v1/maintenance/import"],
      ["POST", "/api/v1/maintenance/vacuum"],
      ["POST", "/api/v1/maintenance/checkpoint"],
      ["POST", "/api/v1/maintenance/run"],
      ["POST", "/api/v1/maintenance/rebuild"],
      ["POST", "/api/v1/maintenance/cleanup"],
    ])
    expect(request.mock.calls[2]?.[0].body).toEqual({ path: "/requested/export.jsonl", replace: true })
    expect(request.mock.calls[5]?.[0].body).toEqual({ owner: null, action: "run" })
    expect(request.mock.calls[6]?.[0].body).toEqual({ owner: "actor", action: null })
    expect(request.mock.calls[7]?.[0].body).toEqual({ owner: "actor", action: null })
  })
})
