import AxeBuilder from "@axe-core/playwright"
import { expect, test } from "@playwright/test"

import { installPlaneAcceptanceFixture } from "./plane-acceptance-fixture"
import { expectNoPageOverflow, expectStrictCsp, expectTaskUrl } from "./plane-acceptance-support"

async function expectTasksSurfaceAxeClean(page: import("@playwright/test").Page): Promise<void> {
  await expect(page.locator("main:visible")).toHaveCount(1)
  const axe = await new AxeBuilder({ page }).analyze()
  expect(axe.violations).toEqual([])
}

test.describe("Plane-only Tasks workspace acceptance", () => {
  test("keeps the shell mode and navigation ownership at exact responsive boundaries", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)

    for (const [width, mode] of [[768, "tablet"], [1023, "tablet"], [1024, "desktop"]] as const) {
      await page.setViewportSize({ width, height: 900 })
      await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
      const shell = page.getByTestId("product-shell")
      await expect(shell).toHaveAttribute("data-shell-viewport", mode)
      if (mode === "tablet") {
        await expect(page.getByTestId("product-rail")).toBeVisible()
        await expect(page.getByTestId("resource-header-menu")).toBeVisible()
      }
    }
  })

  test("keeps the tablet navigation drawer modal, closable, and overflow-safe", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.setViewportSize({ width: 1023, height: 900 })
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

    const menu = page.getByTestId("resource-header-menu")
    const drawer = page.getByTestId("projects-sidebar")
    await menu.click()
    await expect(drawer).toHaveAttribute("role", "dialog")
    await expect(drawer).toHaveAttribute("aria-modal", "true")
    await expect(page.getByTestId("projects-sidebar-backdrop")).toBeVisible()
    await expectNoPageOverflow(page)
    await expectTasksSurfaceAxeClean(page)

    await page.keyboard.press("Escape")
    await expect(drawer).toHaveAttribute("data-open", "false")
    await expect(menu).toBeFocused()

    await menu.click()
    await drawer.getByTestId("project-tree-overview").click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/overview$/)
    await expect(drawer).toHaveAttribute("data-open", "false")
  })

  test("keeps 320px and 430px mobile navigation compact, modal, and overflow-safe", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)

    for (const width of [320, 430]) {
      await page.setViewportSize({ width, height: 900 })
      await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

      const shell = page.getByTestId("product-shell")
      const menu = page.getByTestId("resource-header-menu")
      const drawer = page.getByTestId("projects-sidebar")
      await expect(shell).toHaveAttribute("data-shell-viewport", "mobile")
      await expect(page.getByTestId("product-rail")).toBeHidden()
      await expect(page.getByTestId("resource-header")).toHaveAttribute("data-mobile-topbar", "true")
      await expect(menu).toBeVisible()
      await expectNoPageOverflow(page)

      await menu.click()
      await expect(drawer).toHaveAttribute("role", "dialog")
      await expect(drawer).toHaveAttribute("aria-modal", "true")
      await expect(page.getByTestId("projects-sidebar-backdrop")).toBeVisible()
      await expectTasksSurfaceAxeClean(page)

      await page.keyboard.press("Escape")
      await expect(drawer).toHaveAttribute("data-open", "false")
      await expect(menu).toBeFocused()

      await menu.click()
      await drawer.getByTestId("project-tree-overview").click()
      await expect(page).toHaveURL(/\/app\/boards\/default\/overview$/)
      await expect(drawer).toHaveAttribute("data-open", "false")
    }
  })

  test("keeps desktop sidebar width bounded and keyboard/pointer/reset accessible", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.setViewportSize({ width: 1440, height: 900 })
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

    const shell = page.getByTestId("product-shell")
    const handle = page.getByTestId("sidebar-resize-handle")
    await expect(shell).toHaveAttribute("data-shell-viewport", "desktop")
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "62")
    await expect(handle).toHaveAttribute("role", "separator")

    await handle.focus()
    await page.keyboard.press("ArrowRight")
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "63")
    await page.keyboard.press("Home")
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "56")
    await page.keyboard.press("End")
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "80")
    await page.keyboard.press("Home")
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "56")

    const box = await handle.boundingBox()
    expect(box).not.toBeNull()
    if (box === null) return
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    await page.mouse.down()
    await page.mouse.move(box.x + box.width / 2 + 16, box.y + box.height / 2)
    await page.mouse.up()
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "60")

    await handle.dblclick()
    await expect(shell).toHaveAttribute("data-sidebar-width-step", "62")
    await expect(page.locator("[style]")).toHaveCount(0)
  })

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

  test("keeps Board, List, Table, and Map strict-CSP, axe-clean, and on one main landmark", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    const response = await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    expectStrictCsp(response)
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.locator("[style]")).toHaveCount(0)
    await expect(page.locator("style")).toHaveCount(0)
    await expectTasksSurfaceAxeClean(page)

    const views = page.getByRole("group", { name: "任务视图" })
    await views.getByRole("link", { name: "列表", exact: true }).click()
    await expect(page.getByTestId("task-list").locator('[data-display-variant="list"]')).toBeVisible()
    await expectTasksSurfaceAxeClean(page)

    await views.getByRole("link", { name: "表格", exact: true }).click()
    await expect(page.getByTestId("task-list").locator('[data-display-variant="table"]')).toBeVisible()
    await expectTasksSurfaceAxeClean(page)

    await views.getByRole("link", { name: "关系图", exact: true }).click()
    await expect(page.getByTestId("task-map")).toBeVisible()
    await expectTasksSurfaceAxeClean(page)
  })

  test("keeps one shared task toolbar with URL q awareness on desktop and 430px", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board?q=agent", { waitUntil: "domcontentloaded" })

    const toolbar = page.getByTestId("tasks-workspace-toolbar")
    await expect(toolbar).toHaveCount(1)
    await expect(toolbar).toHaveAttribute("role", "toolbar")
    await expect(page.getByTestId("list-search")).toHaveValue("agent")
    await expect(page.getByTestId("list-search")).toBeDisabled()
    await expect(toolbar.getByRole("button", { name: /筛选:/ })).toBeEnabled()
    await expect(toolbar.locator('[data-view-filter="board"]')).toHaveCount(1)
    await expect(page.getByTestId("list-search")).toHaveAttribute("data-search-support", "url-only")
    await expect(toolbar.locator("[data-frame]")).toHaveCount(0)

    const views = page.getByRole("group", { name: "任务视图" })
    await views.getByRole("link", { name: "列表", exact: true }).click()
    await expect(page.getByTestId("list-search")).toHaveValue("agent")
    await expect(page.getByTestId("list-search")).toBeEnabled()
    await expect(toolbar.getByRole("button", { name: "筛选", exact: true })).toBeEnabled()
    await expect(toolbar.locator('[data-view-filter="list"]')).toHaveCount(1)
    await page.getByTestId("list-search").fill("agent next")
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?q=agent(?:%20|\+)next$/)

    await page.setViewportSize({ width: 430, height: 900 })
    await expect(toolbar).toBeVisible()
    await expectNoPageOverflow(page)
    await expect(toolbar).toHaveAttribute("aria-label", "任务工具栏")
    await expect(toolbar.getByRole("search")).toHaveAttribute("aria-label", "任务搜索")
  })

  test("keeps the 430px shell usable with a drawer, inspector fullscreen dialog, and no page overflow", async ({ page }) => {
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
    await page.getByTestId("projects-sidebar-close").click()
    await expect(drawer).toHaveAttribute("data-open", "false")
    await expect(page.getByTestId("resource-header-menu")).toBeFocused()

    const taskOpener = page.getByRole("button", { name: /ready task$/i }).first()
    await taskOpener.click()
    const inspectorDialog = page.getByTestId("task-inspector-dialog")
    await expect(inspectorDialog).toHaveAttribute("role", "dialog")
    await expect(inspectorDialog).toHaveAttribute("aria-modal", "true")
    await expect(inspectorDialog).toHaveAttribute("data-mode", "fullscreen")
    await expect(page.getByTestId("task-inspector-scrim")).toBeVisible()
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await expect(page.getByTestId("task-inspector")).toHaveAttribute("data-mode", "fullscreen")
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

  test("keeps a nested task action dialog trapped without closing the inspector modal", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.setViewportSize({ width: 430, height: 900 })
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })

    const taskOpener = page.getByRole("button", { name: /ready task$/i }).first()
    await taskOpener.click()
    const inspector = page.getByTestId("task-inspector-dialog")
    await expect(inspector).toHaveAttribute("role", "dialog")
    const block = page.getByTestId("task-inspector").getByRole("button", { name: "阻塞", exact: true })
    await block.click()

    const actionDialog = page.locator('[role="dialog"]').filter({ has: page.locator("textarea[name='action-reason']") }).last()
    await expect(actionDialog).toBeVisible()
    const textarea = actionDialog.locator("textarea[name='action-reason']")
    const cancel = actionDialog.getByRole("button", { name: "取消", exact: true })
    const submit = actionDialog.getByRole("button", { name: "阻塞", exact: true })
    await expect(textarea).toBeFocused()

    await textarea.fill("需要确认")
    await expect(submit).toBeEnabled()

    await cancel.focus()
    await page.keyboard.press("Tab")
    await expect(submit).toBeFocused()
    await page.keyboard.press("Tab")
    await expect(textarea).toBeFocused()
    await page.keyboard.press("Shift+Tab")
    await expect(submit).toBeFocused()

    await page.keyboard.press("Escape")
    await expect(actionDialog).toHaveCount(0)
    await expect(inspector).toBeVisible()
    await expect(block).toBeFocused()
    await expect(page.getByTestId("task-inspector-dialog")).toHaveAttribute("data-mode", "fullscreen")
  })

  test("maps Inspector presentation to each shell mode with focus, URL, overflow, and axe evidence", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)

    for (const [width, mode, modal] of [[320, "fullscreen", true], [430, "fullscreen", true], [768, "dialog", true], [1024, "side-peek", false]] as const) {
      await page.setViewportSize({ width, height: 900 })
      await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
      const opener = page.getByRole("button", { name: /ready task$/i }).first()
      await expect(opener).toBeVisible()
      await opener.click()

      const inspectorDialog = page.getByTestId("task-inspector-dialog")
      await expect(inspectorDialog).toHaveAttribute("data-mode", mode)
      await expect(page.getByTestId("task-inspector")).toHaveAttribute("data-mode", mode)
      await expect(page).toHaveURL(/\/app\/boards\/default\/board\?task=t_default_ready$/)
      await expectTasksSurfaceAxeClean(page)

      if (modal) {
        await expect(inspectorDialog).toHaveAttribute("role", "dialog")
        await expect(inspectorDialog).toHaveAttribute("aria-modal", "true")
        await expect(page.getByTestId("task-inspector-scrim")).toBeVisible()
        await expect(page.getByTestId("task-inspector-mobile-close")).toBeFocused()
        await expectNoPageOverflow(page)
        if (width === 768) {
          await page.getByTestId("task-inspector-scrim").click({ position: { x: 8, y: 8 } })
        } else {
          await page.keyboard.press("Escape")
        }
      } else {
        await expect(inspectorDialog).not.toHaveAttribute("role")
        await expect(inspectorDialog).not.toHaveAttribute("aria-modal")
        await expect(page.getByTestId("task-inspector-scrim")).toHaveCount(0)
        await expect(page.getByTestId("task-inspector").getByRole("heading", { name: /Default project ready task/ })).toBeFocused()
        await page.getByRole("button", { name: "关闭任务检查器" }).click()
      }

      await expect(page).toHaveURL(/\/app\/boards\/default\/board$/)
      await expect(opener).toBeFocused()
    }
  })

  test("keeps product rail and task view controls keyboard reachable", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board?q=agent", { waitUntil: "domcontentloaded" })

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

    await page.goto("/app/boards/default/board?q=agent", { waitUntil: "domcontentloaded" })
    const viewSwitcher = page.getByRole("group", { name: "任务视图" })
    const list = viewSwitcher.getByRole("link", { name: "列表", exact: true })
    await list.focus()
    await expect(list).toBeFocused()
    await list.press("Enter")
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?q=agent$/)
  })

  test("keeps diagnostics and Settings on the retained canonical board session", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect.poll(() => page.evaluate(() => Reflect.get(window, "__kanbanSseConnectionCount"))).toBe(1)
    await page.evaluate(() => Reflect.set(window, "__planeSpaSentinel", "retained"))

    const tasksChrome = page.getByTestId("tasks-workspace-chrome")
    await expect(page.getByText("更多", { exact: true })).toHaveCount(1)
    await tasksChrome.locator("summary").filter({ hasText: "更多" }).click()
    await tasksChrome.locator('a[href="/app/boards/default/health"]').click()
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
