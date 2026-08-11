import { expect, test } from "@playwright/test"

import { installExplorerFixture } from "./explorer-fixture"
import { installPlaneAcceptanceFixture } from "./plane-acceptance-fixture"
import { expectNoPageOverflow, expectTaskUrl } from "./plane-acceptance-support"

test.describe("Plane-only Tasks workspace acceptance", () => {
  test("switches Board, List, Table, and Map without dropping legal q and task URL state", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board?q=agent&task=t_default_ready", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/board")

    const viewSwitcher = page.getByRole("group", { name: "任务视图" })
    await viewSwitcher.getByRole("button", { name: "列表", exact: true }).click()
    await expect(page.getByTestId("task-list")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/list")

    await viewSwitcher.getByRole("button", { name: "表格", exact: true }).click()
    await expect(page.getByTestId("task-list").locator('[data-display-variant="table"]')).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/list", "table")

    await viewSwitcher.getByRole("button", { name: "关系图", exact: true }).click()
    await expect(page.getByTestId("task-map")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/map")

    await viewSwitcher.getByRole("button", { name: "看板", exact: true }).click()
    await expect(page.getByTestId("board-view")).toBeVisible()
    expectTaskUrl(page, "/app/boards/default/board")
  })

  test("keeps the 430px shell usable with a drawer, inspector sheet, and no page overflow", async ({ page }) => {
    await installExplorerFixture(page)
    await page.setViewportSize({ width: 430, height: 900 })
    await page.goto("/app/boards/default/board?task=t_ready", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("resource-header-menu")).toBeVisible()
    await expect(page.getByTestId("projects-sidebar")).toHaveAttribute("data-open", "false")
    await page.getByTestId("resource-header-menu").click()
    const drawer = page.getByTestId("projects-sidebar")
    await expect(drawer).toHaveAttribute("role", "dialog")
    await expect(drawer).toHaveAttribute("aria-modal", "true")
    await expect(drawer).toHaveAttribute("data-open", "true")
    await page.getByTestId("resource-header-menu").click()
    await expect(drawer).toHaveAttribute("data-open", "false")

    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await expect(page.getByTestId("task-inspector")).toHaveAttribute("data-mode", "sheet")
    await expectNoPageOverflow(page)
  })

  test("keeps product rail and task view controls keyboard reachable", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board?q=agent&task=t_default_ready", { waitUntil: "domcontentloaded" })

    const projects = page.getByTestId("product-rail-projects")
    const settings = page.getByTestId("product-rail-settings")
    await projects.focus()
    await expect(projects).toBeFocused()
    await projects.press("Enter")
    await expect(page).toHaveURL(/\/app\/boards\/default\/board\?q=agent&task=t_default_ready$/)

    await settings.focus()
    await expect(settings).toBeFocused()
    await settings.press("Enter")
    await expect(page).toHaveURL(/\/app\/settings$/)

    await page.goto("/app/boards/default/board?q=agent&task=t_default_ready", { waitUntil: "domcontentloaded" })
    const viewSwitcher = page.getByRole("group", { name: "任务视图" })
    const list = viewSwitcher.getByRole("button", { name: "列表", exact: true })
    await list.focus()
    await expect(list).toBeFocused()
    await list.press("Enter")
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?q=agent&task=t_default_ready$/)
  })
})
