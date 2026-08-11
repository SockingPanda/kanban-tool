import { expect, test, type Page, type Request } from "@playwright/test"

import { installBoardFixture } from "./runtime-fixture"

type RequestFailure = {
  readonly method: string
  readonly url: string
  readonly errorText: string | null
}

function trackBrowserErrors(page: Page) {
  const consoleErrors: string[] = []
  const pageErrors: string[] = []
  const requestFailures: RequestFailure[] = []
  const offlineRequestFailures: RequestFailure[] = []
  const pendingApiRequests = new Set<Request>()
  const offlineRequests = new Set<Request>()
  let offlineWindowActive = false
  const isTrackedApiRequest = (request: Request): boolean => {
    const url = new URL(request.url())
    return url.pathname.startsWith("/api/v1/") && url.pathname !== "/api/v1/stream/events"
  }
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text())
  })
  page.on("pageerror", (error) => pageErrors.push(error.message))
  page.on("request", (request) => {
    if (!isTrackedApiRequest(request)) return
    pendingApiRequests.add(request)
    if (offlineWindowActive) offlineRequests.add(request)
  })
  page.on("requestfinished", (request) => {
    pendingApiRequests.delete(request)
    offlineRequests.delete(request)
  })
  page.on("requestfailed", (request) => {
    pendingApiRequests.delete(request)
    const failure = {
      method: request.method(),
      url: request.url(),
      errorText: request.failure()?.errorText ?? null,
    }
    if (offlineRequests.delete(request)) offlineRequestFailures.push(failure)
    else requestFailures.push(failure)
  })
  return {
    consoleErrors,
    pageErrors,
    requestFailures,
    pendingApiRequestCount: () => pendingApiRequests.size,
    beginOfflineWindow: () => {
      offlineWindowActive = true
      for (const request of pendingApiRequests) offlineRequests.add(request)
    },
    endOfflineWindow: () => {
      offlineWindowActive = false
      return offlineRequestFailures.splice(0, offlineRequestFailures.length)
    },
  }
}

function assertExpectedOfflineBoardCancellations(failures: readonly RequestFailure[], origin: string): void {
  for (const failure of failures) {
    expect(failure.method).toBe("GET")
    const url = new URL(failure.url)
    expect(url.origin).toBe(origin)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks/by-status")
    expect(["net::ERR_ABORTED", "NS_BINDING_ABORTED"]).toContain(failure.errorText)
  }
}

test.describe("BoardLive browser pipeline", () => {
  test("renders the Projects collection at canonical /app/ without inventing a board session", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    const errors = trackBrowserErrors(page)
    await page.goto("/app/", { waitUntil: "domcontentloaded" })

    await expect(page).toHaveURL(/\/app\/$/)
    await expect(page.getByTestId("projects-collection")).toBeVisible()
    await expect(page.getByTestId("projects-collection-project-default")).toBeVisible()
    await expect(page.getByTestId("board-view")).toHaveCount(0)

    await page.screenshot({ path: test.info().outputPath("board-live-projects-collection.png"), fullPage: true })
    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards?"))).toBe(true)
    expect(fixture.apiRequests.some((request) => request.includes("/columns"))).toBe(false)
    expect(fixture.apiRequests.some((request) => request.includes("/tasks/by-status"))).toBe(false)
    expect(errors.consoleErrors).toEqual([])
    expect(errors.pageErrors).toEqual([])
    expect(errors.requestFailures).toEqual([])
  })

  test("renders the server no-board empty state without inventing columns", async ({ page }) => {
    const fixture = await installBoardFixture(page, { emptyBoards: true })
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("shell-project-not-found")).toBeVisible()
    await expect(page.getByRole("heading", { name: "页面不存在" })).toBeVisible()
    expect(fixture.apiRequests.some((request) => request.includes("/columns"))).toBe(false)
    expect(fixture.apiRequests.some((request) => request.includes("/tasks/by-status"))).toBe(false)
  })

  test("only reads the global board list on a direct Settings route", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    await page.goto("/app/settings", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await expect(page.getByTestId("project-picker")).toBeVisible()
    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards?include_archived=true"))).toBe(true)
    expect(fixture.apiRequests.some((request) => request.includes("/columns"))).toBe(false)
    expect(fixture.apiRequests.some((request) => request.includes("/tasks/by-status"))).toBe(false)
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

  test("refreshes Projects and releases the active session when board.archived arrives", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    const initialBoardListReads = fixture.apiRequests.filter((request) => request.startsWith("/api/v1/boards?")).length

    await fixture.emitBoardArchived()

    await expect.poll(() => fixture.apiRequests.filter((request) => request.startsWith("/api/v1/boards?")).length).toBeGreaterThan(initialBoardListReads)
    await expect(page.getByTestId("shell-project-archived")).toBeVisible()
    await expect(page.getByTestId("board-view")).toHaveCount(0)
    await expect(page.getByTestId("board-live-session")).toHaveCount(0)
    await expect(page.getByTestId("project-tree-tasks")).toHaveCount(0)
  })

  test("retains the ready board while offline and recovers the live stream", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    const errors = trackBrowserErrors(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-task").filter({ hasText: "Ready task" })).toBeVisible()
    await fixture.emitHeartbeat()
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "live")
    const initialConnectionCount = await fixture.getSseConnectionCount()

    errors.beginOfflineWindow()
    await page.evaluate(() => window.dispatchEvent(new Event("offline")))
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "offline")
    await expect(page.getByTestId("board-task").filter({ hasText: "Ready task" })).toBeVisible()
    await expect.poll(() => errors.pendingApiRequestCount()).toBe(0)

    await page.evaluate(() => window.dispatchEvent(new Event("online")))
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "recovering")
    await fixture.closeSse()
    await fixture.waitForSseConnection(initialConnectionCount)
    await fixture.cancelSseConnection(initialConnectionCount)
    await fixture.emitHeartbeat()
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "live")
    await expect.poll(() => errors.pendingApiRequestCount()).toBe(0)
    fixture.setReadyTaskTitle("Recovered ready task")
    await fixture.emitTaskUpdated()
    await expect(page.getByTestId("board-task").filter({ hasText: "Recovered ready task" })).toBeVisible()
    await expect(page.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", /^(recovering|live)$/)
    await expect.poll(() => errors.pendingApiRequestCount()).toBe(0)
    const offlineFailures = errors.endOfflineWindow()
    assertExpectedOfflineBoardCancellations(offlineFailures, new URL(page.url()).origin)

    expect(errors.consoleErrors).toEqual([])
    expect(errors.pageErrors).toEqual([])
    expect(errors.requestFailures).toEqual([])
  })
})
