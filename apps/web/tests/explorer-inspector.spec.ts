import { expect, test, type Page } from "@playwright/test"

import { installExplorerFixture, type ExplorerFixture, type ExplorerFixtureOptions } from "./explorer-fixture"

const boardPath = "/app/boards/default"
const taskId = "t_ready"

async function open(page: Page, path: string): Promise<void> {
  await page.goto(`${boardPath}/${path}`, { waitUntil: "domcontentloaded" })
}

async function install(page: Page, options: ExplorerFixtureOptions = {}): Promise<ExplorerFixture> {
  return installExplorerFixture(page, options)
}

test.describe("Astryx Explorer browser acceptance", () => {
  test("opens and closes Task Inspector from Board, List, Map and Events with focus return", async ({ page }) => {
    await install(page)

    await open(page, "board")
    const boardOpener = page.getByRole("button", { name: "Ready task" })
    await expect(boardOpener).toBeVisible()
    await boardOpener.scrollIntoViewIfNeeded()
    await boardOpener.focus()
    await expect(boardOpener).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(page).toHaveURL(new RegExp(`/board\\?task=${taskId}$`))
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.screenshot({ path: test.info().outputPath("task-inspector-board.png"), fullPage: true })
    await page.getByRole("button", { name: "关闭任务检查器" }).click()
    await expect(page).toHaveURL(/\/board$/)
    await expect(boardOpener).toBeFocused()

    await open(page, "list")
    const listOpener = page.getByRole("button", { name: "Ready task" })
    await expect(listOpener).toBeVisible()
    await listOpener.click()
    await expect(page).toHaveURL(new RegExp(`/list\\?task=${taskId}$`))
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.getByRole("button", { name: "关闭任务检查器" }).click()
    await expect(page).toHaveURL(/\/list$/)
    await expect(listOpener).toBeFocused()

    await open(page, "map")
    const mapOpener = page.getByRole("button", { name: /检查任务 default#1 Ready task/ })
    await expect(mapOpener).toBeVisible()
    await mapOpener.click()
    await expect(page).toHaveURL(new RegExp(`/map\\?.*task=${taskId}`))
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.getByRole("button", { name: "关闭任务检查器" }).click()
    await expect(page).toHaveURL(/\/map$/)
    await expect(mapOpener).toBeFocused()

    await open(page, "events")
    const eventOpener = page.getByRole("button", { name: taskId })
    await expect(eventOpener).toBeVisible()
    await eventOpener.click()
    await expect(page).toHaveURL(new RegExp(`/events\\?task=${taskId}$`))
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.getByRole("button", { name: "关闭任务检查器" }).click()
    await expect(page).toHaveURL(/\/events$/)
    await expect(eventOpener).toBeFocused()

    await page.screenshot({ path: test.info().outputPath("explorer-views-without-inspector.png"), fullPage: true })
  })

  test("restores deep links and preserves URL state after a reload", async ({ page }) => {
    await install(page)
    await open(page, "list?status=ready&q=Ready&page=1")
    await expect(page.getByTestId("task-row")).toHaveCount(2)
    await expect(page).toHaveURL(/status=ready&q=Ready&page=1$/)

    await page.reload({ waitUntil: "domcontentloaded" })
    await expect(page).toHaveURL(/\/list\?status=ready&q=Ready&page=1$/)
    await expect(page.getByTestId("task-list")).toBeVisible()
    await page.getByRole("button", { name: "Ready task" }).click()
    await expect(page).toHaveURL(new RegExp(`/list\\?status=ready&q=Ready&page=1&task=${taskId}$`))
    await page.reload({ waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await expect(page).toHaveURL(new RegExp(`/list\\?status=ready&q=Ready&page=1&task=${taskId}$`))

    await open(page, `map?filter=ready&zoom=1.2&task=${taskId}`)
    await expect(page.getByTestId("task-map")).toBeVisible()
    await expect(page.getByTestId("task-map-zoom")).toHaveText("120%")
    await page.reload({ waitUntil: "domcontentloaded" })
    await expect(page).toHaveURL(new RegExp(`/map\\?filter=ready&zoom=1.2&task=${taskId}$`))
    await expect(page.getByTestId("task-map")).toBeVisible()
  })

  test("creates and selects a task from the List header", async ({ page }) => {
    await install(page)
    await open(page, "list")
    await expect(page.getByTestId("task-list")).toBeVisible()
    await page.getByTestId("task-create").click()
    await page.getByTestId("task-title-input").fill("Created from list")
    await page.getByTestId("task-mutation-dialog").getByRole("button", { name: "创建", exact: true }).click()
    await expect(page.getByTestId("task-mutation-dialog")).toHaveCount(0)
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?task=t_[^&]+$/, { timeout: 15_000 })
  })

  test("keeps one persistent SSE connection while Events refreshes from a business event", async ({ page }) => {
    const fixture = await install(page)
    await open(page, "events")
    await expect(page.getByTestId("events-ready")).toBeVisible()
    await expect(page.getByTestId("event-row")).toHaveCount(1)
    const initialConnections = await fixture.getSseConnectionCount()
    expect(initialConnections).toBe(1)
    const eventRequests = () => fixture.apiRequests.filter((request) => request.startsWith("/api/v1/events?"))
    const initialEventRequests = [...eventRequests()]
    expect(initialEventRequests.length).toBeGreaterThan(0)
    const initialAfterZeroRequests = initialEventRequests.filter((request) => new URLSearchParams(request.split("?", 2)[1]).get("after") === "0").length

    await fixture.emitHeartbeat()
    await expect(page.getByTestId("events-ready")).toBeVisible()
    await fixture.emitTaskUpdated()
    await expect(page.getByTestId("event-row")).toHaveCount(2)
    await expect(page.getByText("2 条事件")).toBeVisible()
    expect(await fixture.getSseConnectionCount()).toBe(initialConnections)
    expect(eventRequests()).toHaveLength(initialEventRequests.length)
    expect(eventRequests().filter((request) => new URLSearchParams(request.split("?", 2)[1]).get("after") === "0")).toHaveLength(initialAfterZeroRequests)
    await page.screenshot({ path: test.info().outputPath("events-sse-refresh.png"), fullPage: true })
  })

  test("renders the Runs route from a deep link and keeps the selected task in the URL", async ({ page }) => {
    await install(page)
    await open(page, `runs?task=${taskId}`)
    await expect(page).toHaveURL(new RegExp(`/runs\\?task=${taskId}$`))
    await expect(page.getByTestId("runs-ready")).toBeVisible()
    await expect(page.getByTestId("run-row")).toHaveCount(2)
    await expect(page.getByTestId("runs-log")).toContainText("playwright fixture log")
    await expect(page.getByTestId("task-inspector")).toHaveCount(0)
    await page.screenshot({ path: test.info().outputPath("runs-deep-link.png"), fullPage: true })

    const emptyRunsPage = await page.context().newPage()
    await install(emptyRunsPage, { emptyRuns: true })
    await open(emptyRunsPage, `runs?task=${taskId}`)
    await expect(emptyRunsPage.getByTestId("runs-empty")).toBeVisible()
    await emptyRunsPage.close()
  })

  test("renders loading, empty, error and offline boundaries without invented API errors", async ({ page }) => {
    test.setTimeout(60_000)
    const loadingFixture = await install(page, { delayList: true })
    await open(page, "list")
    await expect(page.getByTestId("task-list-loading")).toBeVisible()
    loadingFixture.releaseList()
    await expect(page.getByTestId("task-list")).toBeVisible()

    await open(page, "events")
    await expect(page.getByTestId("events-ready")).toBeVisible()
    await page.evaluate(() => window.dispatchEvent(new Event("offline")))
    await expect(page.getByTestId("events-stale-notice")).toBeVisible()
    await expect(page.getByTestId("events-stale-notice")).toContainText("当前离线")

    const emptyEventsPage = await page.context().newPage()
    await install(emptyEventsPage, { emptyEvents: true })
    await open(emptyEventsPage, "events")
    await expect(emptyEventsPage.getByTestId("events-empty")).toBeVisible()
    await emptyEventsPage.close()

    const emptyMapPage = await page.context().newPage()
    await install(emptyMapPage, { emptyMap: true })
    await open(emptyMapPage, "map")
    await expect(emptyMapPage.getByTestId("task-map-empty")).toBeVisible()
    await emptyMapPage.close()

    const mapErrorPage = await page.context().newPage()
    await install(mapErrorPage, { failMap: true })
    await open(mapErrorPage, "map")
    await expect(mapErrorPage.getByTestId("task-map-error")).toBeVisible()
    await mapErrorPage.close()

    const errorPage = await page.context().newPage()
    await install(errorPage, { failList: true })
    await open(errorPage, "list")
    await expect(errorPage.getByTestId("task-list-error")).toBeVisible()
    await errorPage.close()

    const inspectorErrorPage = await page.context().newPage()
    await install(inspectorErrorPage, { failInspector: true })
    await open(inspectorErrorPage, `list?task=${taskId}`)
    await expect(inspectorErrorPage.getByTestId("task-inspector-error")).toBeVisible()
    await inspectorErrorPage.close()
  })

  test("keeps Explorer controls keyboard reachable", async ({ page }) => {
    await install(page)
    await open(page, "list")
    const search = page.getByTestId("list-search")
    await search.focus()
    await expect(search).toBeFocused()
    await page.keyboard.press("Tab")
    await expect(page.getByRole("button", { name: "筛选", exact: true })).toBeFocused()
    await page.keyboard.press("Tab")
    await expect(page.getByTestId("task-create")).toBeFocused()

    for (const lens of ["ready", "running", "blocked", "review"]) {
      await page.keyboard.press("Tab")
      await expect(page.getByTestId(`attention-lens-${lens}`)).toBeFocused()
    }

    const readyLens = page.getByTestId("attention-lens-ready")
    await readyLens.focus()
    await readyLens.press("Enter")
    await expect(readyLens).toHaveAttribute("aria-pressed", "true")
    await expect(page.getByTestId("attention-active-filter")).toBeVisible()
    await readyLens.focus()
    await expect(readyLens).toBeFocused()
    for (const lens of ["running", "blocked", "review"]) {
      await page.keyboard.press("Tab")
      await expect(page.getByTestId(`attention-lens-${lens}`)).toBeFocused()
    }
    await page.keyboard.press("Tab")
    await expect(page.getByTestId("attention-clear")).toBeFocused()
    await page.keyboard.press("Tab")
    await expect(page.getByTestId("list-status-filter")).toBeFocused()

    await open(page, "map")
    const zoomIn = page.getByRole("button", { name: "放大关系图" })
    await zoomIn.focus()
    await expect(zoomIn).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(page.getByTestId("task-map-zoom")).toHaveText("115%")

    await open(page, "events")
    const eventFilter = page.getByRole("searchbox", { name: "事件类型筛选" })
    await eventFilter.focus()
    await expect(eventFilter).toBeFocused()
    await page.keyboard.type("task.updated")
    await page.keyboard.press("Enter")
    await expect(page).toHaveURL(/kind=task.updated$/)
    await expect(page.getByTestId("events-filter-empty")).toBeVisible()
  })
})
