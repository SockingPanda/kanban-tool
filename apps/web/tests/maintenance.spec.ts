import { readFileSync } from "node:fs"

import { expect, test } from "@playwright/test"

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
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/status", async (route) => {
      statusRequests += 1
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(statusFixture) })
    })
    await page.route("http://127.0.0.1:4173/api/v1/stats?board=default", async (route) => {
      statsRequests += 1
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(statsFixture) })
    })
    await page.route("http://127.0.0.1:4173/api/v1/search/status?board=default", async (route) => {
      searchRequests += 1
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(searchFixture) })
    })
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/doctor", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(doctorFixture) })
    })
  })

  test("shows server maintenance status and honest unsupported legacy import", async ({ page }) => {
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("maintenance-page")).toBeVisible()
    await expect(page.getByTestId("maintenance-status")).toContainText("db_fixture")
    await expect(page.getByTestId("maintenance-legacy-import-unsupported")).toContainText("legacy")
    await expect(page.getByTestId("maintenance-doctor-submit")).toBeEnabled()
    await expect(page.getByTestId("nav-maintenance")).toHaveAttribute("aria-current", "page")
  })

  test("confirms backup with keyboard and renders server path plus checksum", async ({ page }) => {
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/backup", async (route) => {
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify({ data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }),
      })
    })
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })

    await page.evaluate(() => {
      document.body.dataset.maintenanceHealthRefreshCount = "0"
      window.addEventListener("kanban:health-refresh", () => {
        document.body.dataset.maintenanceHealthRefreshCount = String(Number(document.body.dataset.maintenanceHealthRefreshCount ?? "0") + 1)
      }, { once: false })
    })
    await page.getByTestId("maintenance-backup-path").fill("/requested/backup.sqlite")
    await page.getByTestId("maintenance-backup-submit").click()
    const dialog = page.getByRole("alertdialog")
    await expect(dialog).toBeVisible()
    await expect(dialog).toContainText("/requested/backup.sqlite")
    await page.keyboard.press("Tab")
    await page.keyboard.press("Enter")

    await expect(page.getByTestId("maintenance-backup-result")).toContainText("/server/backup.sqlite")
    await expect(page.getByTestId("maintenance-backup-result")).toContainText("sha256:backup")
    await expect.poll(() => statusRequests).toBeGreaterThan(1)
    await expect.poll(() => statsRequests).toBeGreaterThan(1)
    await expect.poll(() => searchRequests).toBeGreaterThan(1)
    await expect.poll(() => page.evaluate(() => Number(document.body.dataset.maintenanceHealthRefreshCount ?? "0"))).toBe(1)
    await page.screenshot({ path: "test-results/maintenance-backup.png", fullPage: true })
  })

  test("freezes the confirmed maintenance owner before the request is sent", async ({ page }) => {
    let runBody: unknown = null
    let runHeaders: Record<string, string> | null = null
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/run", async (route) => {
      runBody = route.request().postDataJSON()
      runHeaders = route.request().headers()
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { database_instance_id: "db_fixture", protocol_version: 2, owner: "owner-a", mode: "once", action: "run", processed: 0, phase: "completed", degraded: false, errors: [], stores: [] } }),
      })
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
    expect(runHeaders?.["content-type"]).toBe("application/json")
    expect(runHeaders?.["x-kb-actor"]).toBeUndefined()
  })

  test("fences stale diagnostics when the board changes", async ({ page }) => {
    let releaseAlphaStats!: () => void
    let releaseAlphaSearch!: () => void
    const alphaStats = new Promise<void>((resolve) => { releaseAlphaStats = resolve })
    const alphaSearch = new Promise<void>((resolve) => { releaseAlphaSearch = resolve })
    await page.route("http://127.0.0.1:4173/api/v1/stats?board=alpha", async (route) => {
      await alphaStats
      try {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ...statsFixture, data: { ...statsFixture.data, board_id: "alpha" } }) })
      } catch {
        // The board switch is expected to abort this stale request.
      }
    })
    await page.route("http://127.0.0.1:4173/api/v1/search/status?board=alpha", async (route) => {
      await alphaSearch
      try {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ...searchFixture, data: { ...searchFixture.data, generation: "alpha-generation" } }) })
      } catch {
        // The board switch is expected to abort this stale request.
      }
    })
    await page.route("http://127.0.0.1:4173/api/v1/stats?board=beta", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ...statsFixture, data: { ...statsFixture.data, board_id: "beta" } }) })
    })
    await page.route("**/api/v1/search/status**", async (route) => {
      const board = new URL(route.request().url()).searchParams.get("board")
      if (board === "beta") {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ ...searchFixture, data: { ...searchFixture.data, generation: "beta-generation" } }) })
      } else {
        await route.continue()
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
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/doctor", async (route) => {
      doctorRequests += 1
      if (doctorRequests === 1) {
        await oldDoctor
        try {
          await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(doctorPayload("alpha_store", 11)) })
        } catch {
          // The board switch is expected to abort this stale request.
        }
        return
      }
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(doctorPayload("beta_store", 22)) })
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
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/backup", async (route) => {
      backupRequests += 1
      await backupResponse
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify({ data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }),
      })
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
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/backup", async (route) => {
      backupRequests += 1
      if (backupRequests === 1) {
        await route.fulfill({ status: 201, contentType: "application/json", body: JSON.stringify({ data: { out_path: "/server/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 12, source_fingerprint: "sha256:source" } }) })
      } else {
        await route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: { code: "internal", message: "SECRET_BACKEND_ERROR" } }) })
      }
    })
    await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })
    await page.evaluate(() => {
      document.body.dataset.maintenanceHealthRefreshCount = "0"
      window.addEventListener("kanban:health-refresh", () => {
        document.body.dataset.maintenanceHealthRefreshCount = String(Number(document.body.dataset.maintenanceHealthRefreshCount ?? "0") + 1)
      })
    })
    const submit = page.getByTestId("maintenance-backup-submit")
    await page.getByTestId("maintenance-backup-path").fill("/requested/backup.sqlite")
    await submit.click()
    await page.getByRole("alertdialog").getByRole("button", { name: "继续" }).click()
    await expect(page.getByTestId("maintenance-backup-result")).toBeVisible()
    await expect.poll(() => page.evaluate(() => Number(document.body.dataset.maintenanceHealthRefreshCount ?? "0"))).toBe(1)
    await submit.click()
    await page.getByRole("alertdialog").getByRole("button", { name: "继续" }).click()
    const error = page.getByTestId("maintenance-backup-error")
    await expect(error).toContainText("维护操作失败")
    await expect(error).not.toContainText("SECRET_BACKEND_ERROR")
    await expect(page.getByTestId("maintenance-backup-result")).toHaveCount(0)
    await expect.poll(() => page.evaluate(() => Number(document.body.dataset.maintenanceHealthRefreshCount ?? "0"))).toBe(1)
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
