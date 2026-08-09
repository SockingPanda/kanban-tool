import { expect, test } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"

const healthFixture = {
  data: {
    ok: true,
    db: "turso",
    version: "3.0.0",
    db_path: "/tmp/kanban.db",
    db_fingerprint: "sha256:settings",
  },
}

test.describe("Astryx Settings", () => {
  test.beforeEach(async ({ page }) => {
    await installRuntimeFixture(page)
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(healthFixture) })
    })
  })

  test("persists appearance, language and actor preferences in the Web namespace", async ({ page }) => {
    let streamRequests = 0
    await page.route("**/api/v1/stream/events**", async (route) => {
      streamRequests += 1
      await route.abort()
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
    await page.getByTestId("appearance-theme").selectOption("system")
    await page.getByTestId("appearance-density").selectOption("compact")
    await page.getByTestId("settings-locale").selectOption("en")
    await page.getByTestId("identity-actor").fill("browser-reviewer")
    await page.getByTestId("identity-actor-save").click()
    await page.getByTestId("identity-actor-reset").click()

    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light")
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact")
    await expect(page.locator("html")).toHaveAttribute("lang", "en")
    await expect(page.getByTestId("identity-actor-saved")).toBeVisible()
    await expect(page.evaluate(() => localStorage.getItem("kb:web:actor"))).resolves.toBe("")
    expect(streamRequests).toBe(0)
    await expect
      .poll(() => page.evaluate(() => Object.keys(localStorage).sort()))
      .toEqual(["kb:web:actor", "kb:web:density", "kb:web:locale", "kb:web:sidebar", "kb:web:theme"])
  })

  test("rejects unsafe actor input without persisting it", async ({ page }) => {
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByTestId("identity-actor").fill("   ")
    await page.getByTestId("identity-actor").blur()

    await expect(page.getByTestId("identity-actor-error")).toBeVisible()
    await expect(page.getByTestId("identity-actor-save")).toBeDisabled()
    await expect(page.evaluate(() => localStorage.getItem("kb:web:actor"))).resolves.not.toBe("   ")
  })

  test("copies validated diagnostics safely and opens the canonical health route", async ({ page }) => {
    await page.goto("/app/boards/default/board", { waitUntil: "networkidle" })
    await page.getByTestId("nav-settings").click()
    await page.getByTestId("settings-page").waitFor()
    await page.evaluate(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: async () => undefined },
      })
    })
    await page.getByTestId("diagnostics-copy").click()
    await expect(page.getByTestId("diagnostics-feedback")).toContainText("已复制")
    await page.getByTestId("diagnostics-health-link").click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/health$/)
  })

  test("keeps clipboard failure and no-board state actionable", async ({ page }) => {
    await page.evaluate(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: () => { throw new Error("denied") } },
      })
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByTestId("diagnostics-copy").click()
    await expect(page.getByTestId("diagnostics-feedback")).toContainText("复制")

    await page.route("**/app/runtime.json", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          apiBaseUrl: "",
          webBasePath: "/app/",
          actor: "local",
          defaultBoard: "",
          serverVersion: "3.0.0",
          protocolVersion: "v1",
          webBuildId: "dev",
        }),
      })
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await expect(page.getByTestId("settings-no-board")).toBeVisible()
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
  })

  test("retains the canonical board session when moving Board to Settings", async ({ page }) => {
    let streamRequests = 0
    const streamPending = new Promise<void>(() => undefined)
    await page.route("**/api/v1/boards?include_archived=false", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }],
        }),
      })
    })
    await page.route("**/api/v1/boards/default/columns", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: [{ id: "col_todo", board_id: "b_default", status: "todo", title: "Todo", position: 1, hidden: false, wip_limit: null, created_at: 1, updated_at: 1 }],
        }),
      })
    })
    await page.route("**/api/v1/boards/default/tasks/by-status**", async (route) => {
      const status = new URL(route.request().url()).searchParams.get("status") ?? "todo"
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: { statuses: [{ status, tasks: [], page: { limit: 1000, offset: 0, total: 0 } }] },
          meta: { limit: 1000, offset: 0 },
        }),
      })
    })
    await page.route("**/api/v1/stream/events**", async () => {
      streamRequests += 1
      await streamPending
    })

    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
    await expect.poll(() => streamRequests).toBe(1)

    await page.getByTestId("nav-settings").click()
    await expect(page).toHaveURL(/\/app\/settings$/)
    await expect(page.getByTestId("connection-reconnect")).toBeEnabled()
    await page.getByTestId("connection-reconnect").click()
    await expect(page.getByTestId("connection-feedback")).toContainText("仍在连接")
    await expect.poll(() => streamRequests).toBe(1)

    await page.getByTestId("nav-board").click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/board$/)
    await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
    expect(streamRequests).toBe(1)
  })
})
