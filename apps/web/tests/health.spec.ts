import { readFileSync } from "node:fs"

import { expect, test } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"

const healthFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-health-response.valid.json", import.meta.url), "utf8"),
) as { data: { ok: boolean; db: string; version: string; db_path: string; db_fingerprint: string } }

test.describe("Health operator workflow", () => {
  test.beforeEach(async ({ page }) => {
    await installRuntimeFixture(page)
  })

  test("loads typed health metrics and runtime identity", async ({ page }) => {
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(healthFixture),
      })
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-page")).toBeVisible()
    await expect(page.getByTestId("health-metric-ok")).toContainText("true")
    await expect(page.getByTestId("health-metric-db-fingerprint")).toContainText(healthFixture.data.db_fingerprint)
    await expect(page.getByTestId("health-runtime")).toContainText("local")
    await expect(page.getByTestId("nav-health")).toHaveAttribute("aria-current", "page")
  })

  test("keeps an actionable local error when health request fails", async ({ page }) => {
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { code: "unavailable", message: "service unavailable" } }) })
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-error")).toBeVisible()
    await expect(page.getByTestId("health-error")).toContainText("HTTP 503")
    await expect(page.getByTestId("health-refresh")).toBeEnabled()
  })
})
