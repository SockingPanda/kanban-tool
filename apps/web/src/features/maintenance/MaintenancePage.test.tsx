import { readFileSync } from "node:fs"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import type { WebRuntimeConfig } from "../../lib/runtime"
import { PreferencesProvider } from "../../lib/preferences-provider"
import { MaintenancePage } from "./MaintenancePage"
import { maintenanceOwnerForAction } from "./maintenance-intents"

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "local",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "build-test",
} satisfies WebRuntimeConfig

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
}

const serviceNowMs = 1_700_000_000_000
const stats = {
  board_id: "b_fixture",
  generated_at: serviceNowMs,
  status_counts: [{ status: "running" as const, count: 1 }],
  stale_claims: [{
    task_id: "t_stale",
    seq: 2,
    title: "Stale title",
    claim_owner: "worker-1",
    claim_expires_at: serviceNowMs,
    last_heartbeat_at: serviceNowMs,
    current_run_id: "run-1",
    retry_count: 0,
    max_retries: 3,
  }],
  blocked_reasons: [{ reason: "waiting", count: 1 }],
  unplanned_active_tasks: 0,
  active_parents_with_incomplete_required_steps: 0,
}

const statusWithLifecycle = {
  ...status,
  stores: [{
    store_name: "projection-store",
    active_generation: null,
    active_fingerprint: null,
    previous_generation: null,
    building_generation: null,
    lifecycle_status: "ready-from-service",
    fence_epoch: 0,
    last_event_id: 0,
    dirty: false,
    pending: 0,
    running: 0,
    failed: 0,
    last_error: null,
    phase: "ready",
    degraded: false,
    errors: [],
    updated_at: serviceNowMs,
  }],
}

describe("MaintenancePage", () => {
  test("freezes the normalized owner in a confirmation intent", () => {
    const owner = maintenanceOwnerForAction(" requested-owner ", "actor-preference", runtime.actor)
    expect(owner).toBe("requested-owner")
    const intent = { kind: "run" as const, owner }
    expect(intent.owner).toBe("requested-owner")
    // Later input changes cannot mutate the already-created confirmation payload.
    expect(maintenanceOwnerForAction("changed-owner", "actor-preference", runtime.actor)).toBe("changed-owner")
    expect(intent.owner).toBe("requested-owner")
  })

  test("renders status, unsupported legacy capability, and loading-safe boundaries", () => {
    const loading = renderToStaticMarkup(
      <PreferencesProvider><MaintenancePage runtime={runtime} boardSlug="default" /></PreferencesProvider>,
    )
    expect(loading).toContain('data-testid="maintenance-loading"')
    expect(loading).toContain('data-frame="content"')
    expect(loading).toContain('aria-busy="true"')
    expect(loading).toMatch(/<header[^>]*aria-busy="true"[^>]*>[\s\S]*data-testid="maintenance-refresh"/)
    expect([...loading.matchAll(/<button\b[^>]*>/g)].every(([tag]) => !tag.includes("aria-busy"))).toBe(true)
    expect(loading).not.toContain("style=")
    expect(loading).toContain("维护")

    const ready = renderToStaticMarkup(
      <PreferencesProvider>
        <MaintenancePage runtime={runtime} boardSlug="default" initial={{ status }} />
      </PreferencesProvider>,
    )
    expect(ready).toContain('data-testid="maintenance-status"')
    expect(ready).toContain("db_fixture")
    expect(ready).toContain('data-testid="maintenance-legacy-import-unsupported"')
    expect(ready).not.toContain("style=")
    expect(ready).not.toContain('data-testid="maintenance-import-v30-submit"')
  })

  test("keeps the closed confirmation dialog hydration-safe without inline styles", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <MaintenancePage runtime={runtime} boardSlug="default" initial={{ status }} />
      </PreferencesProvider>,
    )
    expect(markup).toContain('data-testid="maintenance-confirm-dialog"')
    expect(markup).toContain('data-open="false"')
    expect(markup).toContain('role="alertdialog"')
    expect(markup).toContain('data-testid="maintenance-confirm-cancel"')
    expect(markup).not.toContain("style=")
  })

  test("keeps confirmation and mutation source on local dialog semantics", () => {
    const source = readFileSync(new URL("./MaintenancePage.tsx", import.meta.url), "utf8")
    expect(source).not.toContain("AlertDialog")
    expect(source).not.toMatch(/\bisLoading\s*=/)
    expect(source).toContain('import { Dialog } from "@/ui/astryx/overlays"')
    expect(source).toContain("initialFocusRef={confirmCancelRef}")
    expect(source).toContain("returnFocusRef={confirmOpenerRef}")
    expect(source).toContain("closeOnBackdrop={false}")
    expect(source).toContain("aria-busy")
  })

  test("renders actual server path and checksum in result fixtures", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <MaintenancePage
          runtime={runtime}
          boardSlug="default"
          initial={{
            status,
            results: {
              backup: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" },
              export: { out_path: "/server/export.jsonl", checksum_sha256: "sha256:export", bytes: 14, record_count: 3, source_fingerprint: "sha256:source" },
            },
          }}
        />
      </PreferencesProvider>,
    )
    expect(markup).toContain("/server/backup.sqlite")
    expect(markup).toContain("sha256:backup")
    expect(markup).toContain("/server/export.jsonl")
    expect(markup).toContain("sha256:export")
  })

  test("formats service millisecond timestamps instead of rendering raw epoch numbers", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <MaintenancePage runtime={runtime} boardSlug="default" initial={{ status, stats }} />
      </PreferencesProvider>,
    )
    const expected = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" }).format(new Date(serviceNowMs))
    expect(markup).toContain(expected)
    expect(markup).not.toContain(String(serviceNowMs))
  })

  test("marks service lifecycle literals as non-translatable", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <MaintenancePage runtime={runtime} boardSlug="default" initial={{ status: statusWithLifecycle }} />
      </PreferencesProvider>,
    )
    expect(markup).toContain('translate="no">ready-from-service</span>')
  })
})
