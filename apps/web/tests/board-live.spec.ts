import { expect, test, type Page } from "@playwright/test"

import { installBoardFixture } from "./runtime-fixture"

function trackBrowserErrors(page: Page) {
  const consoleErrors: string[] = []
  const pageErrors: string[] = []
  const requestFailures: string[] = []
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text())
  })
  page.on("pageerror", (error) => pageErrors.push(error.message))
  page.on("requestfailed", (request) => requestFailures.push(`${request.url()} ${request.failure()?.errorText ?? "unknown"}`))
  return { consoleErrors, pageErrors, requestFailures }
}

test.describe("BoardLive browser pipeline", () => {
  test("redirects canonical /app/ home, renders all statuses, and keeps the skip link keyboard reachable", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    const errors = trackBrowserErrors(page)
    await page.goto("/app/", { waitUntil: "domcontentloaded" })

    await expect(page).toHaveURL(/\/app\/boards\/default\/board$/)
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.getByTestId("board-column")).toHaveCount(8)
    await expect(page.getByTestId("board-task")).toHaveCount(8)

    const skipLink = page.getByRole("link", { name: "跳转到看板列" })
    await skipLink.focus()
    await skipLink.press("Enter")
    await expect(page.locator("#astryx-board-columns")).toBeFocused()

    await page.screenshot({ path: test.info().outputPath("board-live-home-redirect.png"), fullPage: true })
    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards?"))).toBe(true)
    expect(errors.consoleErrors).toEqual([])
    expect(errors.pageErrors).toEqual([])
    expect(errors.requestFailures).toEqual([])
  })

  test("renders the server no-board empty state without inventing columns", async ({ page }) => {
    const fixture = await installBoardFixture(page, { emptyBoards: true })
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("board-empty")).toBeVisible()
    await expect(page.getByRole("heading", { name: "暂无看板" })).toBeVisible()
    expect(fixture.apiRequests.some((request) => request.includes("/columns"))).toBe(false)
    expect(fixture.apiRequests.some((request) => request.includes("/tasks/by-status"))).toBe(false)
  })

  test("does not issue board API requests on Settings", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    await page.goto("/app/settings", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    expect(fixture.apiRequests.filter((request) => request.includes("/api/v1/boards")).length).toBe(0)
  })

  test("uses the real Fetch SSE pipeline for heartbeat and task.updated refresh", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    const errors = trackBrowserErrors(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-task").filter({ hasText: "Ready task" })).toBeVisible()

    await fixture.emitHeartbeat()
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "live")

    fixture.setReadyTaskTitle("Updated ready task")
    await fixture.emitTaskUpdated()
    await expect(page.getByTestId("board-task").filter({ hasText: "Updated ready task" })).toBeVisible()

    await page.screenshot({ path: test.info().outputPath("board-live-updated.png"), fullPage: true })
    expect(errors.consoleErrors).toEqual([])
    expect(errors.pageErrors).toEqual([])
    expect(errors.requestFailures).toEqual([])
  })

  test("retains the ready board while offline and recovers the live stream", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    const errors = trackBrowserErrors(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-task").filter({ hasText: "Ready task" })).toBeVisible()
    await fixture.emitHeartbeat()
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "live")
    const initialConnectionCount = await fixture.getSseConnectionCount()

    await page.evaluate(() => window.dispatchEvent(new Event("offline")))
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "stale")
    await expect(page.getByTestId("board-task").filter({ hasText: "Ready task" })).toBeVisible()

    await page.evaluate(() => window.dispatchEvent(new Event("online")))
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "recovering")
    await fixture.closeSse()
    await fixture.waitForSseConnection(initialConnectionCount)
    await fixture.cancelSseConnection(initialConnectionCount)
    await fixture.emitHeartbeat()
    fixture.setReadyTaskTitle("Recovered ready task")
    await fixture.emitTaskUpdated()
    await expect(page.getByTestId("board-task").filter({ hasText: "Recovered ready task" })).toBeVisible()
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", /^(recovering|live)$/)

    expect(errors.consoleErrors).toEqual([])
    expect(errors.pageErrors).toEqual([])
    expect(errors.requestFailures).toEqual([])
  })
})
