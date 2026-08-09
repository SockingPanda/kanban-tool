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

  test.beforeEach(async ({ page }) => {
    statusRequests = 0
    await installRuntimeFixture(page)
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/status", async (route) => {
      statusRequests += 1
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(statusFixture) })
    })
    await page.route("http://127.0.0.1:4173/api/v1/stats?board=default", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(statsFixture) })
    })
    await page.route("http://127.0.0.1:4173/api/v1/search/status?board=default", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(searchFixture) })
    })
    await page.route("http://127.0.0.1:4173/api/v1/maintenance/doctor", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(doctorFixture) })
    })
  })

  test("shows server maintenance status and honest unsupported legacy import", async ({ page }) => {
    await page.goto("/app/boards/default/maintenance", { waitUntil: "networkidle" })

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
    await page.goto("/app/boards/default/maintenance", { waitUntil: "networkidle" })

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
    await expect.poll(() => page.evaluate(() => Number(document.body.dataset.maintenanceHealthRefreshCount ?? "0"))).toBe(1)
    await page.screenshot({ path: "test-results/maintenance-backup.png", fullPage: true })
  })
})
