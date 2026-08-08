import { expect, test, type Page, type Response } from "@playwright/test"

type FoundationPage = Page & { foundationErrors?: string[] }

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

test.describe("Astryx foundation lab", () => {
  test.beforeEach(async ({ page }, testInfo) => {
    const errors: string[] = []
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(`console: ${message.text()}`)
    })
    page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`))
    ;(page as FoundationPage).foundationErrors = errors

    if (testInfo.project.name === "webkit") {
      testInfo.annotations.push({
        type: "note",
        description: "WebKit engine proxy only; packaged WebKitGTK is not asserted.",
      })
    }

    const response = await page.goto("/app/", { waitUntil: "networkidle" })
    expectStrictCsp(response)
  })

  test.afterEach(async ({ page }) => {
    const errors = (page as FoundationPage).foundationErrors ?? []
    expect(errors, errors.join("\n")).toEqual([])
  })

  test("renders the foundation seam with official components under strict CSP", async ({ page }) => {
    await expect(page).toHaveTitle("Kanban Tool · Astryx Foundation Lab")
    await expect(page.getByRole("heading", { name: "Astryx foundation lab" })).toBeVisible()
    await expect(page.getByTestId("astryx-button")).toBeVisible()
    await expect(page.getByTestId("foundation-text-input")).toBeVisible()
    await expect(page.getByTestId("astryx-card")).toBeVisible()
    await expect(page.getByTestId("astryx-table")).toBeVisible()
    await expect(page.getByTestId("astryx-vstack")).toBeVisible()
    await expect(page.getByRole("dialog")).toBeHidden()
    await expect(page.getByRole("group", { name: "Foundation controls" })).toBeVisible()

    const skipLink = page.getByRole("link", { name: "跳转到主要内容" })
    await skipLink.focus()
    await skipLink.press("Enter")
    await expect(page.locator("#main-content")).toBeFocused()

    await expect(page.getByTestId("foundation-text-input")).toHaveAttribute("type", "search")
    await expect(page.getByTestId("foundation-text-input")).toHaveAttribute("name", "eventQuery")
    await expect(page.getByTestId("foundation-text-input")).toHaveAttribute("autocomplete", "off")
    await expect(page.getByTestId("foundation-text-input")).toHaveAttribute("placeholder", "task.created…")

    const tableWrap = page.getByTestId("astryx-table")
    const tableCard = page.getByTestId("astryx-table-card")
    const secondRow = tableWrap.locator("tbody tr").nth(1)
    await expect(secondRow).toBeVisible()
    await expect(secondRow).toBeInViewport()
    const [tableBounds, cardBounds, rowBounds] = await Promise.all([
      tableWrap.boundingBox(),
      tableCard.boundingBox(),
      secondRow.boundingBox(),
    ])
    expect(tableBounds).not.toBeNull()
    expect(cardBounds).not.toBeNull()
    expect(rowBounds).not.toBeNull()
    if (!tableBounds || !cardBounds || !rowBounds) throw new Error("Table layout bounds are unavailable")
    const layoutTolerance = 1
    expect(rowBounds.y).toBeGreaterThanOrEqual(tableBounds.y - layoutTolerance)
    expect(rowBounds.y + rowBounds.height).toBeLessThanOrEqual(tableBounds.y + tableBounds.height + layoutTolerance)
    expect(rowBounds.y).toBeGreaterThanOrEqual(cardBounds.y - layoutTolerance)
    expect(rowBounds.y + rowBounds.height).toBeLessThanOrEqual(cardBounds.y + cardBounds.height + layoutTolerance)

    const padding = await page.getByTestId("astryx-button").evaluate((element) => {
      return Number.parseFloat(getComputedStyle(element).paddingInlineStart)
    })
    expect(padding).toBeGreaterThan(0)

    const explicitSeamStyles = await page.locator("[data-testid]").evaluateAll((elements) =>
      elements
        .filter((element) => element.hasAttribute("style"))
        .map((element) => element.getAttribute("data-testid")),
    )
    expect(explicitSeamStyles).toEqual([])
    await expect(page.locator("[style]")).toHaveCount(0)
    await expect(page.locator("style")).toHaveCount(0)
    await expect(page.locator("[data-testid=platform-features]")).toContainText("popover:")
    await expect(page.locator("[data-testid=platform-features]")).toContainText("anchor positioning:")
  })

  test("keeps light/dark and long Chinese/English copy observable", async ({ page }) => {
    const root = page.locator("html")
    await expect(root).toHaveAttribute("data-theme", "light")
    await expect(root).toHaveAttribute("lang", "zh-CN")
    await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#f1f1f1")

    await page.getByRole("button", { name: "切换主题" }).click()
    await expect(root).toHaveAttribute("data-theme", "dark")
    await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#1b1b1b")

    await page.getByRole("button", { name: "切换语言" }).click()
    await expect(root).toHaveAttribute("lang", "en")
    await expect(page.getByTestId("long-copy")).toContainText("This operator console keeps")
    await expect(page.getByTestId("long-copy")).toContainText("persistent SSE")

    const overlayTrigger = page.getByRole("button", { name: "Open overlay" })
    await overlayTrigger.click()
    const dialog = page.getByRole("dialog", { name: "Overlay verification" })
    await expect(dialog).toBeVisible()
    expect(await dialog.evaluate((element) => element.matches(":modal"))).toBe(true)
    const closeOverlay = page.getByRole("button", { name: "Close overlay" })
    await expect(closeOverlay).toBeFocused()
    await page.keyboard.press("Tab")
    expect(await dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true)
    await overlayTrigger.focus()
    await expect(overlayTrigger).not.toBeFocused()
    await expect(page.getByRole("dialog")).toContainText("Astryx overlay seam")
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    await expect(overlayTrigger).toBeFocused()
  })

  test("has no browser console or CSP errors", async ({ page }) => {
    const response = await page.reload({ waitUntil: "networkidle" })
    expectStrictCsp(response)
  })

  test("keeps strict CSP on documents, assets, and SPA fallback", async ({ page }) => {
    const documentResponse = await page.goto("/app/", { waitUntil: "networkidle" })
    expectStrictCsp(documentResponse)

    const assetUrls = await page.locator("script[src], link[rel=stylesheet][href]").evaluateAll((elements) =>
      elements.map((element) => (element as HTMLScriptElement | HTMLLinkElement).src || (element as HTMLLinkElement).href),
    )
    expect(assetUrls.length).toBeGreaterThan(0)
    for (const assetUrl of assetUrls) {
      const response = await page.request.get(assetUrl)
      expect(response.status()).toBe(200)
      expectStrictCsp(response)
    }

    const fallbackResponse = await page.request.get("/app/foundation/missing-route")
    expect(fallbackResponse.status()).toBe(200)
    expectStrictCsp(fallbackResponse)
    expect(await fallbackResponse.text()).toContain('id="root"')
  })

  test("matches the Astryx visual baseline", async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== "chromium", "Visual baseline is authored by the fixed Chromium project.")
    await expect(page).toHaveScreenshot("foundation-light.png", { animations: "disabled" })
    await page.getByRole("button", { name: "切换主题" }).click()
    await expect(page).toHaveScreenshot("foundation-dark.png", { animations: "disabled" })
  })
})
