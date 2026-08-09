import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import type { WebRuntimeConfig } from "../../lib/runtime"
import { PreferencesProvider } from "../../lib/preferences-provider"
import { MaintenancePage } from "./MaintenancePage"

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

describe("MaintenancePage", () => {
  test("renders status, unsupported legacy capability, and loading-safe boundaries", () => {
    const loading = renderToStaticMarkup(
      <PreferencesProvider><MaintenancePage runtime={runtime} boardSlug="default" /></PreferencesProvider>,
    )
    expect(loading).toContain('data-testid="maintenance-loading"')
    expect(loading).toContain("维护")

    const ready = renderToStaticMarkup(
      <PreferencesProvider>
        <MaintenancePage runtime={runtime} boardSlug="default" initial={{ status }} />
      </PreferencesProvider>,
    )
    expect(ready).toContain('data-testid="maintenance-status"')
    expect(ready).toContain("db_fixture")
    expect(ready).toContain('data-testid="maintenance-legacy-import-unsupported"')
    expect(ready).not.toContain('data-testid="maintenance-import-v30-submit"')
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
})
