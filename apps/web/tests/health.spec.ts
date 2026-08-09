import { readFileSync } from "node:fs"

import { expect, test } from "@playwright/test"

import type { ApiErrorResponseContract } from "../src/lib/api/generated/contracts/api-error-response"
import { installRuntimeFixture } from "./runtime-fixture"

const healthFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-health-response.valid.json", import.meta.url), "utf8"),
) as { data: { ok: boolean; db: string; version: string; db_path: string; db_fingerprint: string } }
const malformedHealthFixture = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-health-response.invalid.json", import.meta.url), "utf8"),
) as unknown
const serverUnavailableFixture = {
  error: { code: "server_unavailable", message: "kanban serve unavailable" },
} satisfies ApiErrorResponseContract

test.describe("Health operator workflow", () => {
  test.beforeEach(async ({ page }) => {
    await installRuntimeFixture(page)
  })

  test("loads typed health metrics and runtime identity", async ({ page }) => {
    const boardRequests: string[] = []
    page.on("request", (request) => {
      const pathname = new URL(request.url()).pathname
      if (pathname === "/api/v1/boards" || pathname.startsWith("/api/v1/boards/")) boardRequests.push(pathname)
    })
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
    expect(boardRequests).toEqual([])
  })

  test("keeps an actionable local error when health request fails", async ({ page }) => {
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify(serverUnavailableFixture) })
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-error")).toBeVisible()
    await expect(page.getByTestId("health-error")).toContainText("HTTP 503")
    await expect(page.getByTestId("health-refresh")).toBeEnabled()
  })

  test("uses a safe fallback for a malformed health response", async ({ page }) => {
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(malformedHealthFixture) })
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-error")).toBeVisible()
    await expect(page.getByTestId("health-error-detail")).toContainText("无法识别")
    await expect(page.getByTestId("health-error")).not.toContainText("/tmp/kanban.db")
  })

  test("keeps the last report visible when refresh becomes stale", async ({ page }) => {
    let requestCount = 0
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      requestCount += 1
      if (requestCount === 1) {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(healthFixture) })
      } else {
        await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify(serverUnavailableFixture) })
      }
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })
    await expect(page.getByTestId("health-metric-db-fingerprint")).toContainText(healthFixture.data.db_fingerprint)

    await page.getByTestId("health-refresh").click()

    await expect(page.getByTestId("health-stale")).toBeVisible()
    await expect(page.getByTestId("health-metric-db-fingerprint")).toContainText(healthFixture.data.db_fingerprint)
    await expect(page.getByTestId("health-stale")).toContainText("503")
  })

  test("loads settings health diagnostics without a board query", async ({ page }) => {
    const boardRequests: string[] = []
    page.on("request", (request) => {
      const pathname = new URL(request.url()).pathname
      if (pathname === "/api/v1/boards" || pathname.startsWith("/api/v1/boards/")) boardRequests.push(pathname)
    })
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(healthFixture),
      })
    })

    await page.goto("/app/settings", { waitUntil: "networkidle" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await expect(page.getByTestId("settings-health")).toContainText(healthFixture.data.db_fingerprint)
    expect(boardRequests).toEqual([])
  })

  test("offers a safe retry and next step when settings health is unavailable", async ({ page }) => {
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify(serverUnavailableFixture) })
    })

    await page.goto("/app/settings", { waitUntil: "networkidle" })

    await expect(page.getByTestId("settings-health-error")).toBeVisible()
    await expect(page.getByTestId("settings-health-error")).toContainText("kanban serve")
    await expect(page.getByTestId("settings-health-retry")).toBeEnabled()
  })
})
