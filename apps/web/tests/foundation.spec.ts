import { expect, test, type Response } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"

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

test.describe("Astryx product shell", () => {
  test.beforeEach(async ({ page }) => {
    await installRuntimeFixture(page)
  })

  test("renders a strict-CSP shell with one keyboard-reachable main landmark", async ({ page }) => {
    const response = await page.goto("/app/boards/default/board", { waitUntil: "networkidle" })
    expectStrictCsp(response)

    await expect(page).toHaveTitle("Astryx Kanban · Workspace")
    await expect(page.getByTestId("product-shell")).toBeVisible()
    await expect(page.getByTestId("board-placeholder")).toBeVisible()
    await expect(page.getByRole("navigation", { name: "Side navigation" })).toBeVisible()
    await expect(page.getByRole("main")).toHaveCount(1)

    const skipLink = page.getByRole("link", { name: "跳转到主要内容" })
    await skipLink.focus()
    await skipLink.press("Enter")
    await expect(page.locator("#astryx-app-shell-main")).toBeFocused()
    await expect(page.locator("[style]")).toHaveCount(0)
    await expect(page.locator("style")).toHaveCount(0)
  })

  test("persists theme, locale, and sidebar preferences in kb:web keys", async ({ page }) => {
    await page.goto("/app/settings", { waitUntil: "networkidle" })

    await expect(page.locator("html")).toHaveAttribute("data-theme", "light")
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN")
    await page.getByTestId("theme-preference").selectOption("dark")
    await page.getByTestId("locale-preference").selectOption("en")
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("lang", "en")
    await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#1b1b1b")

    await expect
      .poll(() => page.evaluate(() => Object.keys(localStorage).sort()))
      .toEqual(["kb:web:locale", "kb:web:sidebar", "kb:web:theme"])
  })

  test("navigates between board and settings without a router dependency", async ({ page }) => {
    await page.goto("/app/boards/default/board", { waitUntil: "networkidle" })
    await page.getByTestId("nav-settings").click()
    await expect(page).toHaveURL(/\/app\/settings$/)
    await expect(page.getByTestId("nav-board")).toBeDisabled()
    await page.goBack()
    await expect(page).toHaveURL(/\/app\/boards\/default\/board$/)
  })

  test("keeps strict CSP on assets and the SPA fallback", async ({ page }) => {
    const response = await page.goto("/app/boards/default/board", { waitUntil: "networkidle" })
    expectStrictCsp(response)
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
    await page.goto("/app/unknown", { waitUntil: "networkidle" })
    await expect(page.getByTestId("shell-not-found")).toBeVisible()
    await expect(page.getByRole("heading", { name: "页面不存在" })).toBeVisible()
  })
})
