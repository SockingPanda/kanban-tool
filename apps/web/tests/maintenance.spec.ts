import { readFileSync } from "node:fs"

import { expect, test } from "@playwright/test"

import { installRpcFixture, rpcFailure } from "./rpc-fixture"
import { Code } from "@connectrpc/connect"
import { DtoApiErrorCode } from "../src/generated/rpc/kanban/v1/dto_pb"

import { installRuntimeFixture } from "./runtime-fixture"

const statusFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-maintenance-status-response.valid.json", import.meta.url), "utf8"),
)
const statsFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-get-stats-response.valid.json", import.meta.url), "utf8"),
)
const searchFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-search-status-response.valid.json", import.meta.url), "utf8"),
)
const doctorFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-doctor-response.valid.json", import.meta.url), "utf8"),
)

test.describe("Maintenance operator workflow", () => {
  let statusRequests = 0
  let statsRequests = 0
  let searchRequests = 0

  test.beforeEach(async ({ page }) => {
    statusRequests = 0
    statsRequests = 0
    searchRequests = 0
    await installRuntimeFixture(page)
    ;(await installRpcFixture(page)).handle("MaintenanceStatus", async () => {
      statusRequests += 1
      return statusFixture
    })
    ;(await installRpcFixture(page)).handle("GetStats", async (call) => {
      if (call.query.board !== "default") return undefined
      statsRequests += 1
      return statsFixture
    })
    ;(await installRpcFixture(page)).handle("SearchStatus", async (call) => {
      if (call.query.board !== "default") return undefined
      searchRequests += 1
      return searchFixture
    })
    ;(await installRpcFixture(page)).handle("Doctor", async () => {
      return doctorFixture
    })
  })

  test("shows server maintenance status and honest unsupported legacy import", async ({ page }) => {
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("maintenance-page")).toBeVisible()
    await expect(page.getByTestId("maintenance-status")).toContainText("db_fixture")
    await expect(page.getByTestId("maintenance-legacy-import-unsupported")).toContainText("legacy")
    await expect(page.getByTestId("maintenance-doctor-submit")).toBeEnabled()
    await expect(page.getByTestId("nav-settings")).toHaveAttribute("aria-current", "page")
  })

  test("confirms backup with keyboard and renders server path plus checksum", async ({ page }) => {
    let healthRequests = 0
    ;(await installRpcFixture(page)).handle('GetHealth', () => {
      healthRequests += 1
      return { data: { ok: true, db: 'ok', version: '3.0.0', db_path: '/server/kanban.db', db_fingerprint: 'sha256:after-backup' } }
    })
    ;(await installRpcFixture(page)).handle("MaintenanceBackup", async () => {
      return { data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }
    })
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })

    await page.getByTestId("maintenance-backup-path").fill("/requested/backup.sqlite")
    await page.getByTestId("maintenance-backup-submit").click()
    const dialog = page.getByRole("alertdialog")
    await expect(dialog).toBeVisible()
    await expect(dialog).toContainText("/requested/backup.sqlite")
    await dialog.getByRole("button", { name: "取消", exact: true }).focus()
    await page.keyboard.press("Tab")
    await page.keyboard.press("Enter")

    await expect(page.getByTestId("maintenance-backup-result")).toContainText("/server/backup.sqlite")
    await expect(page.getByTestId("maintenance-backup-result")).toContainText("sha256:backup")
    await expect.poll(() => statusRequests).toBeGreaterThan(1)
    await expect.poll(() => statsRequests).toBeGreaterThan(1)
    await expect.poll(() => searchRequests).toBeGreaterThan(1)
    await expect(page.getByTestId("maintenance-backup-submit")).toBeEnabled()
    await page.screenshot({ path: "test-results/maintenance-backup.png", fullPage: true })
    expect(healthRequests).toBe(0)
    await page.goto('/app/boards/default/health', { waitUntil: 'domcontentloaded' })
    await expect(page.getByTestId('health-metric-db-fingerprint')).toContainText('sha256:after-backup')
    expect(healthRequests).toBeGreaterThan(0)
  })

  test("freezes the confirmed maintenance owner before the request is sent", async ({ page }) => {
    let runBody: unknown = null
    let runHeaders: Record<string, string> | null = null
    ;(await installRpcFixture(page)).handle("MaintenanceRun", async (call) => {
      runBody = call.input
      runHeaders = call.headers
      return { data: { database_instance_id: "db_fixture", protocol_version: 2, owner: "owner-a", mode: "once", action: "run", processed: 0, phase: "completed", degraded: false, errors: [], stores: [] } }
    })
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })
    const ownerInput = page.getByTestId("maintenance-owner")
    await ownerInput.fill("owner-a")
    await page.getByTestId("maintenance-run-submit").click()
    const dialog = page.getByRole("alertdialog")
    await expect(dialog).toContainText("owner-a")

    await page.evaluate(() => {
      const input = document.querySelector('[data-testid="maintenance-owner"]') as HTMLInputElement | null
      if (!input) throw new Error("maintenance owner input missing")
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set
      setter?.call(input, "owner-b")
      input.dispatchEvent(new Event("input", { bubbles: true }))
    })
    await dialog.getByRole("button", { name: "继续" }).click()
    await expect.poll(() => runBody).toEqual({ owner: "owner-a", action: "run" })
    expect(runHeaders?.["content-type"]).toBe("application/grpc-web+proto")
    expect(runHeaders?.["x-kb-actor"]).toBeUndefined()
  })

  test("fences stale diagnostics when the board changes", async ({ page }) => {
    let releaseAlphaStats!: () => void
    let releaseAlphaSearch!: () => void
    const alphaStats = new Promise<void>((resolve) => { releaseAlphaStats = resolve })
    const alphaSearch = new Promise<void>((resolve) => { releaseAlphaSearch = resolve })
    ;(await installRpcFixture(page)).handle("GetStats", async (call) => {
      if (call.query.board !== "alpha") return undefined
      await alphaStats
      try {
        return { ...statsFixture, data: { ...statsFixture.data, board_id: "alpha" } }
      } catch {
        // The board switch is expected to abort this stale request.
      }
    })
    ;(await installRpcFixture(page)).handle("SearchStatus", async (call) => {
      if (call.query.board !== "alpha") return undefined
      await alphaSearch
      try {
        return { ...searchFixture, data: { ...searchFixture.data, generation: "alpha-generation" } }
      } catch {
        // The board switch is expected to abort this stale request.
      }
    })
    ;(await installRpcFixture(page)).handle("GetStats", async (call) => {
      if (call.query.board !== "beta") return undefined
      return { ...statsFixture, data: { ...statsFixture.data, board_id: "beta" } }
    })
    ;(await installRpcFixture(page)).handle("SearchStatus", async (call) => {
      const board = call.query.board
      if (board === "beta") {
        return { ...searchFixture, data: { ...searchFixture.data, generation: "beta-generation" } }
      } else {
        return undefined
      }
    })

    await page.goto("/app/boards/alpha/maintenance", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("maintenance-page")).toBeVisible()
    await page.evaluate(() => {
      history.pushState({}, "", "/app/boards/beta/maintenance")
      window.dispatchEvent(new PopStateEvent("popstate"))
    })
    await expect(page.getByTestId("maintenance-stats")).toContainText("beta")
    await expect(page.getByTestId("maintenance-search-status")).toContainText("beta-generation")
    releaseAlphaStats()
    releaseAlphaSearch()
    await expect(page.getByTestId("maintenance-stats")).not.toContainText("alpha")
  })

  test("aborts a deferred doctor request when the board changes", async ({ page }) => {
    let releaseOldDoctor!: () => void
    const oldDoctor = new Promise<void>((resolve) => { releaseOldDoctor = resolve })
    let doctorRequests = 0
    const doctorPayload = (storeName: string, migrationVersion: number) => ({
      ...doctorFixture,
      data: {
        ...doctorFixture.data,
        migration_version: migrationVersion,
        derived_stores: [{ ...doctorFixture.data.derived_stores[0], store_name: storeName }],
      },
    })
    ;(await installRpcFixture(page)).handle("Doctor", async () => {
      doctorRequests += 1
      if (doctorRequests === 1) {
        await oldDoctor
        try {
          return doctorPayload("alpha_store", 11)
        } catch {
          // The board switch is expected to abort this stale request.
        }
        return
      }
      return doctorPayload("beta_store", 22)
    })

    await page.goto("/app/boards/alpha/maintenance", { waitUntil: "domcontentloaded" })
    await page.getByTestId("maintenance-doctor-submit").click()
    await expect.poll(() => doctorRequests).toBe(1)
    await page.evaluate(() => {
      history.pushState({}, "", "/app/boards/beta/maintenance")
      window.dispatchEvent(new PopStateEvent("popstate"))
    })
    await expect(page.getByTestId("maintenance-doctor-submit")).toBeEnabled()
    await page.getByTestId("maintenance-doctor-submit").click()
    await expect(page.getByTestId("maintenance-doctor")).toContainText("beta_store")
    releaseOldDoctor()
    await expect(page.getByTestId("maintenance-doctor")).not.toContainText("alpha_store")
  })

  test("keeps the main action pending after confirmation and suppresses duplicate submits", async ({ page }) => {
    let releaseBackup!: () => void
    const backupResponse = new Promise<void>((resolve) => { releaseBackup = resolve })
    let backupRequests = 0
    ;(await installRpcFixture(page)).handle("MaintenanceBackup", async () => {
      backupRequests += 1
      await backupResponse
      return { data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }
    })
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("maintenance-stats")).toContainText("b_fixture")
    const submit = page.getByTestId("maintenance-backup-submit")
    await page.getByTestId("maintenance-backup-path").fill("/requested/backup.sqlite")
    await submit.click()
    await page.getByRole("alertdialog").getByRole("button", { name: "继续" }).click()
    await expect(submit).toBeDisabled()
    await expect(page.getByTestId("maintenance-backup")).toHaveAttribute("aria-busy", "true")
    expect(backupRequests).toBe(1)
    releaseBackup()
    await expect(page.getByTestId("maintenance-backup-result")).toBeVisible()
    expect(backupRequests).toBe(1)
  })

  test("clears stale evidence and presents a stable safe error on a retry", async ({ page }) => {
    let backupRequests = 0
    ;(await installRpcFixture(page)).handle("MaintenanceBackup", async () => {
      backupRequests += 1
      if (backupRequests === 1) {
        return { data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }
      } else {
        throw rpcFailure(Code.Internal, DtoApiErrorCode.INTERNAL, "SECRET_BACKEND_ERROR")
      }
    })
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })
    const submit = page.getByTestId("maintenance-backup-submit")
    await page.getByTestId("maintenance-backup-path").fill("/requested/backup.sqlite")
    await submit.click()
    await page.getByRole("alertdialog").getByRole("button", { name: "继续" }).click()
    await expect(page.getByTestId("maintenance-backup-result")).toBeVisible()
    await expect(page.getByTestId("maintenance-backup-submit")).toBeEnabled()
    const freshStatus = statusRequests, freshStats = statsRequests, freshSearch = searchRequests
    await submit.click()
    await page.getByRole("alertdialog").getByRole("button", { name: "继续" }).click()
    const error = page.getByTestId("maintenance-backup-error")
    await expect(error).toContainText("维护操作失败")
    await expect(error).not.toContainText("SECRET_BACKEND_ERROR")
    await expect(page.getByTestId("maintenance-backup-result")).toHaveCount(0)
    await expect.poll(() => statusRequests).toBeGreaterThan(freshStatus)
    await expect.poll(() => statsRequests).toBeGreaterThan(freshStats)
    await expect.poll(() => searchRequests).toBeGreaterThan(freshSearch)
    await expect(page.getByTestId("maintenance-backup-submit")).toBeEnabled()
  })

  test("returns focus to the triggering action when confirmation is escaped", async ({ page }) => {
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })
    const submit = page.getByTestId("maintenance-backup-submit")
    await page.getByTestId("maintenance-backup-path").fill("/requested/backup.sqlite")
    await submit.click()
    await expect(page.getByRole("alertdialog")).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(page.getByRole("alertdialog")).toBeHidden()
    await expect(submit).toBeFocused()
  })
})
