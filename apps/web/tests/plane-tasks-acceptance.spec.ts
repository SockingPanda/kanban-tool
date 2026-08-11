import { expect, test } from "@playwright/test"

import { installPlaneAcceptanceFixture } from "./plane-acceptance-fixture"
import { expectNoPageOverflow, expectTaskUrl } from "./plane-acceptance-support"

test.describe("Plane-only Tasks workspace acceptance", () => {
  test("switches Board, List, Table, and Map without dropping legal q and task URL state", async ({ page }) => {
    const fixture = await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board?q=agent&task=t_default_ready", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/board")

    const viewSwitcher = page.getByRole("group", { name: "任务视图" })
    await viewSwitcher.getByRole("link", { name: "列表", exact: true }).click()
    await expect(page.getByTestId("task-list")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/list")

    await viewSwitcher.getByRole("link", { name: "表格", exact: true }).click()
    await expect(page.getByTestId("task-list").locator('[data-display-variant="table"]')).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/list", "table")

    await viewSwitcher.getByRole("link", { name: "关系图", exact: true }).click()
    await expect(page.getByTestId("task-map")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/map")

    await viewSwitcher.getByRole("link", { name: "看板", exact: true }).click()
    await expect(page.getByTestId("board-view")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/board")
    expect(fixture.apiRequests.every((request) => request.startsWith("GET "))).toBe(true)
  })

  test("keeps the 430px shell usable with a drawer, inspector sheet, and no page overflow", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.setViewportSize({ width: 430, height: 900 })
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("resource-header-menu")).toBeVisible()
    await expect(page.getByTestId("projects-sidebar")).toHaveAttribute("data-open", "false")
    await page.getByTestId("resource-header-menu").click()
    const drawer = page.getByTestId("projects-sidebar")
    await expect(drawer).toHaveAttribute("role", "dialog")
    await expect(drawer).toHaveAttribute("aria-modal", "true")
    await expect(drawer).toHaveAttribute("data-open", "true")
    await page.getByTestId("resource-header-menu").click()
    await expect(drawer).toHaveAttribute("data-open", "false")

    const taskOpener = page.getByRole("button", { name: /ready task$/i }).first()
    await taskOpener.click()
    const inspectorDialog = page.getByTestId("task-inspector-dialog")
    await expect(inspectorDialog).toHaveAttribute("role", "dialog")
    await expect(inspectorDialog).toHaveAttribute("aria-modal", "true")
    await expect(inspectorDialog).toHaveAttribute("data-mode", "sheet")
    await expect(page.getByTestId("task-inspector-scrim")).toBeVisible()
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await expect(page.getByTestId("task-inspector")).toHaveAttribute("data-mode", "sheet")
    await expect(page.getByTestId("task-inspector-mobile-close")).toBeFocused()
    await page.keyboard.press("Tab")
    await expect.poll(() => page.evaluate(() => document.activeElement?.closest("[data-testid='task-inspector-dialog']") !== null)).toBe(true)
    await page.keyboard.press("Shift+Tab")
    await expect(page.getByTestId("task-inspector-mobile-close")).toBeFocused()
    await expectNoPageOverflow(page)
    await page.keyboard.press("Escape")
    await expect(page).toHaveURL(/\/app\/boards\/default\/board$/)
    await expect(taskOpener).toBeFocused()
  })

  test("keeps product rail and task view controls keyboard reachable", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board?q=agent&task=t_default_ready", { waitUntil: "domcontentloaded" })

    const projects = page.getByTestId("product-rail-projects")
    const settings = page.getByTestId("product-rail-settings")
    await projects.focus()
    await expect(projects).toBeFocused()
    await projects.press("Enter")
    await expect(page).toHaveURL(/\/app\/$/)

    await settings.focus()
    await expect(settings).toBeFocused()
    await settings.press("Enter")
    await expect(page).toHaveURL(/\/app\/settings$/)

    await page.goto("/app/boards/default/board?q=agent&task=t_default_ready", { waitUntil: "domcontentloaded" })
    const viewSwitcher = page.getByRole("group", { name: "任务视图" })
    const list = viewSwitcher.getByRole("link", { name: "列表", exact: true })
    await list.focus()
    await expect(list).toBeFocused()
    await list.press("Enter")
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?q=agent&task=t_default_ready$/)
  })

  test("keeps diagnostics and Settings on the retained canonical board session", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect.poll(() => page.evaluate(() => Reflect.get(window, "__kanbanSseConnectionCount"))).toBe(1)
    await page.evaluate(() => Reflect.set(window, "__planeSpaSentinel", "retained"))

    const resourceHeader = page.getByTestId("resource-header")
    await resourceHeader.locator("summary").click()
    await resourceHeader.locator('a[href="/app/boards/default/health"]').click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/health$/)
    await expect(page.getByTestId("health-page")).toBeVisible()
    await expect(page.getByTestId("health-metrics")).toBeVisible()
    expect(await page.evaluate(() => Reflect.get(window, "__planeSpaSentinel"))).toBe("retained")

    await page.goBack()
    await expect(page.getByTestId("board-view")).toBeVisible()
    await page.getByTestId("product-rail-settings").click()
    await expect(page).toHaveURL(/\/app\/settings$/)
    await expect(page.getByTestId("settings-page")).toBeVisible()
    await expect(page.getByTestId("settings-no-board")).toHaveCount(0)
    await expect(page.getByTestId("diagnostics-health-link")).toHaveAttribute("href", "/app/boards/default/health")
    expect(await page.evaluate(() => Reflect.get(window, "__planeSpaSentinel"))).toBe("retained")
  })
})
