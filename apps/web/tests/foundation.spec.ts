import { expect, test, type Response } from "@playwright/test"

import { installExplorerFixture } from "./explorer-fixture"

const strictCspDirectives = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self'",
  "img-src 'self' data:",
  "font-src 'self'",
  "connect-src 'self'",
  "object-src 'none'",
  "base-uri 'self'",
  "frame-ancestors 'none'",
]

function expectStrictCsp(response: Pick<Response, "headers"> | null) {
  expect(response).not.toBeNull()
  const csp = response?.headers()["content-security-policy"] ?? ""
  expect(csp).not.toBe("")
  expect(csp).not.toContain("unsafe-inline")
  for (const directive of strictCspDirectives) expect(csp).toContain(directive)
}

test.describe("纸本应用壳层", () => {
  test.beforeEach(async ({ page }) => {
    await installExplorerFixture(page)
  })

  test("renders a strict-CSP shell with one keyboard-reachable main landmark", async ({ page }) => {
    const response = await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    expectStrictCsp(response)

    await expect(page).toHaveTitle("Kanban Tool · Workspace")
    await expect(page.getByTestId("product-shell")).toBeVisible()
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.getByRole("navigation", { name: "项目页面" })).toBeVisible()
    await expect(page.locator("main:visible")).toHaveCount(1)

    const skipLink = page.getByRole("link", { name: "跳到主要内容" })
    await skipLink.focus()
    await skipLink.press("Enter")
    await expect(page.locator("#main-content")).toBeFocused()
    await expect(page.locator("[style]")).toHaveCount(0)
    await expect(page.locator("style")).toHaveCount(0)
  })

  test("persists theme, density, locale, actor, and sidebar preferences in kb:web keys", async ({ page }) => {
    await page.goto("/app/settings", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await expect(page.locator("html")).not.toHaveAttribute("data-theme")
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN")
    await page.getByTestId("appearance-theme").click()
    await page.getByRole("option", { name: "深色", exact: true }).click()
    await page.getByTestId("appearance-density").click()
    await page.getByRole("option", { name: "紧凑", exact: true }).click()
    await page.getByTestId("settings-locale").click()
    await page.getByRole("option", { name: "English", exact: true }).click()
    await page.getByRole("tab", { name: "Identity", exact: true }).click()
    await page.getByTestId("identity-actor").fill("playwright")
    await page.getByTestId("identity-save").click()
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("lang", "en")
    await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#1b1b1b")

    await expect
      .poll(() => page.evaluate(() => Object.keys(localStorage).sort()))
      .toEqual(["kb:web:actor", "kb:web:density", "kb:web:locale", "kb:web:sidebar", "kb:web:theme"])
  })

  test("navigates between board and settings without a router dependency", async ({ page }) => {
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    await page.getByTestId("nav-settings").click()
    await expect(page).toHaveURL(/\/app\/settings$/)
    await expect(page.getByTestId("nav-board")).toBeEnabled()
    const brand = page.getByRole("link", { name: "返回任务" })
    await expect(brand).toHaveText("Kanban")
    await expect(brand).toHaveAttribute("href", "/app/boards/default/list")
    await brand.click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/list$/)
    await expect(page.getByTestId("task-list")).toBeVisible()
    await page.goBack()
    await expect(page).toHaveURL(/\/app\/settings$/)
  })

  test("Kanban 名称链接保留修饰键的新标签页行为", async ({ page, context }) => {
    await page.goto("/app/boards/default/list")
    await expect(page.getByTestId("task-list")).toBeVisible()
    await page.getByTestId("nav-settings").click()
    const brand = page.getByRole("link", { name: "返回任务" })
    const popupPromise = context.waitForEvent("page")
    await brand.click({ modifiers: ["ControlOrMeta"] })
    const popup = await popupPromise
    try {
      await expect(popup).toHaveURL(/\/app\/boards\/default\/list$/)
      await expect(page).toHaveURL(/\/app\/settings$/)
      await expect(brand).toHaveText("Kanban")
    } finally {
      await popup.close()
    }
  })

  test("surfaces a rejected navigation without an unhandled page error", async ({ page }) => {
    const pageErrors: Error[] = []
    page.on("pageerror", (error) => pageErrors.push(error))
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    await page.evaluate(() => {
      history.pushState = () => {
        throw new Error("navigation rejected")
      }
    })

    await page.getByTestId("nav-settings").click()
    await expect(page.getByTestId("shell-error")).toBeVisible()
    expect(pageErrors).toEqual([])
  })

  test("keeps board and settings entries available in collapsed desktop navigation", async ({ page }) => {
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toBeVisible()
    await page.getByRole("button", { name: "收起侧边栏" }).click()
    await expect(page.getByRole("button", { name: "展开侧边栏" })).toBeVisible()
    await expect(page.getByTestId("nav-board")).toBeVisible()
    await expect(page.getByTestId("nav-settings")).toBeVisible()
    await page.getByTestId("nav-board").hover()
    await page.getByTestId("nav-settings").focus()
    await page.getByRole("button", { name: "展开侧边栏" }).click()
    await page.getByRole("button", { name: "收起侧边栏" }).click()
    await expect(page.locator("[style]")).toHaveCount(0)
  })

  test("keeps strict CSP on assets and the SPA fallback", async ({ page }) => {
    const response = await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    expectStrictCsp(response)
    await expect(page.getByTestId("board-view")).toBeVisible()
    const assetUrls = await page.locator("script[src], link[rel=stylesheet][href]").evaluateAll((elements) =>
      elements.map((element) => (element as HTMLScriptElement | HTMLLinkElement).src || (element as HTMLLinkElement).href),
    )
    expect(assetUrls.length).toBeGreaterThan(0)
    for (const assetUrl of assetUrls) expectStrictCsp(await page.request.get(assetUrl))

    const fallbackResponse = await page.request.get("/app/missing-route")
    expect(fallbackResponse.status()).toBe(200)
    expectStrictCsp(fallbackResponse)
    await expect(fallbackResponse.text()).resolves.toContain('id="root"')
  })

  test("renders an explicit 404 boundary for an unknown route", async ({ page }) => {
    await page.goto("/app/unknown", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("shell-not-found")).toBeVisible()
    await expect(page.getByRole("heading", { name: "页面不存在" })).toBeVisible()
  })

  test("renders an explicit invalid-slug boundary without rewriting the address", async ({ page }) => {
    await page.goto("/app/boards/Bad%20Board/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("shell-route-error")).toBeVisible()
    await expect(page).toHaveURL(/\/app\/boards\/Bad%20Board\/board$/)
  })
})
