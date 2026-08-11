import { expect, test, type Locator, type Page } from "@playwright/test"

import { evidenceContext, evidenceRunId, hostIdentityEvidence, observeBrowserContext, screenshotMaskSelectors, type BrowserContextObservation, writeEvidence } from "./a11y-proof-support"

test.describe.configure({ mode: "serial" })

const visualEvidence: Array<{
  readonly name: string
  readonly masks: readonly string[]
  readonly viewport: string
  readonly context: BrowserContextObservation
}> = []
const visualFailures: string[] = []
const pageErrors: string[] = []
const expectedVisualNames = [
  "board-ready",
  "inspector-ready",
  "inspector-mutation-dialog",
  "events-offline",
  "events-recovered",
  "maintenance-confirm",
  "settings-ready",
] as const

async function goto(page: Page, path: string, ready: string): Promise<void> {
  await page.goto(path, { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId(ready)).toBeVisible({ timeout: 30_000 })
}

async function resetCaptureScroll(page: Page): Promise<void> {
  await page.evaluate(() => {
    const scrollables = [
      document.scrollingElement,
      document.body,
      document.querySelector<HTMLElement>("#astryx-app-shell-main"),
      ...Array.from(document.querySelectorAll<HTMLElement>("main, [data-testid='task-list'], [data-testid='task-map']")),
    ]
    for (const element of scrollables) {
      if (element === null) continue
      element.scrollTop = 0
      element.scrollLeft = 0
    }
  })
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())))
}

async function capture(page: Page, name: string, masks: readonly string[] = []): Promise<void> {
  await resetCaptureScroll(page)
  const maskLocators = masks.map((selector) => page.locator(selector))
  await expect(page).toHaveScreenshot(`${name}.png`, {
    fullPage: true,
    caret: "hide",
    mask: maskLocators,
    maxDiffPixelRatio: 0.001,
  })
  visualEvidence.push({ name, masks: screenshotMaskSelectors(masks), viewport: `${(await page.viewportSize())?.width}x${(await page.viewportSize())?.height}`, context: await observeBrowserContext(page) })
}

async function captureLocator(locator: Locator, name: string): Promise<void> {
  await resetCaptureScroll(locator.page())
  await expect(locator).toHaveScreenshot(`${name}.png`, { caret: "hide", maxDiffPixelRatio: 0.001 })
  const viewport = locator.page().viewportSize()
  visualEvidence.push({ name, masks: [], viewport: `${viewport?.width}x${viewport?.height}`, context: await observeBrowserContext(locator.page()) })
}

function browserContextMatchesExpected(context: BrowserContextObservation): boolean {
  return context.document_language === "zh-CN"
    && context.navigator_language === "zh-CN"
    && ["zh-cn", "zh-hans-cn"].includes(context.intl_locale.toLowerCase())
    && context.color_scheme_light
    && !context.color_scheme_dark
    && context.reduced_motion_reduce
    && context.theme_state === "dark"
    && context.astryx_theme === "neutral"
}

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
  page.on("pageerror", (error) => pageErrors.push(`${page.url()}: ${error.message}`))
  page.on("crash", () => pageErrors.push(`${page.url()}: page crashed`))
})

test.afterEach(async ({ page }, testInfo) => {
  void page
  if (testInfo.status !== testInfo.expectedStatus) visualFailures.push(`${testInfo.title}: ${testInfo.status}`)
})

test.afterAll(async ({ browser }, testInfo) => {
  void browser
  const actualNames = visualEvidence.map((shot) => shot.name).sort()
  const expectedNames = [...expectedVisualNames].sort()
  const namesComplete = actualNames.length === expectedNames.length && actualNames.every((name, index) => name === expectedNames[index])
  const contextGate = visualEvidence.length === expectedNames.length && visualEvidence.every((shot) => browserContextMatchesExpected(shot.context))
  const visualGate = namesComplete && contextGate && visualFailures.length === 0 && pageErrors.length === 0
  await writeEvidence(testInfo, "visual", {
    schema_version: 1,
    run_id: evidenceRunId(),
    lane: "real-kanban-serve",
    browser: testInfo.project.name,
    viewport: testInfo.project.use.viewport,
    ...evidenceContext,
    host: hostIdentityEvidence(),
    visual_gate: visualGate ? "passed" : "failed",
    expected_screenshot_names: expectedVisualNames,
    baseline_policy: "chromium-only-fixed-1440x900-reduced-motion",
    screenshots: visualEvidence,
    failures: visualFailures,
    page_errors: pageErrors,
  })
  expect(actualNames, "visual screenshot evidence names").toEqual(expectedNames)
  expect(visualFailures, "visual test failures").toEqual([])
  expect(pageErrors, "page errors/crashes").toEqual([])
  if (process.env.KANBAN_A11Y_MODE === "formal") {
    expect(visualEvidence.every((shot) => browserContextMatchesExpected(shot.context)), "visual browser context").toBe(true)
  }
})

test("captures deterministic board-ready and inspector dialog baselines", async ({ page }) => {
  await goto(page, "/app/boards/default/board", "board-view")
  await capture(page, "board-ready")

  await page.getByTestId("board-task").filter({ hasText: "A11y Seed Task" }).getByRole("button", { name: "A11y Seed Task", exact: true }).click()
  await expect(page.getByTestId("task-inspector")).toBeVisible()
  await capture(page, "inspector-ready")
  await page.getByRole("button", { name: "关闭任务检查器" }).click()

  await page.getByTestId("task-create").click()
  await expect(page.getByTestId("task-mutation-dialog")).toBeVisible()
  await capture(page, "inspector-mutation-dialog")
  await page.keyboard.press("Escape")
})

test("captures events offline and recovered baselines", async ({ page, context }) => {
  await goto(page, "/app/boards/default/events", "events-ready")
  await expect(page.getByTestId("event-row")).not.toHaveCount(0)
  await context.setOffline(true)
  await expect(page.getByTestId("events-stale-notice")).toBeVisible()
  await capture(page, "events-offline", ["[data-testid='event-row'] time"])
  await context.setOffline(false)
  await expect.poll(() => page.getByTestId("events-stale-notice").count()).toBe(0)
  await expect(page.getByTestId("events-ready")).toBeVisible()
  await capture(page, "events-recovered", ["[data-testid='event-row'] time"])
})

test("captures maintenance confirmation and settings baselines", async ({ page }) => {
  await goto(page, "/app/boards/default/maintenance", "maintenance-page")
  await expect(page.getByTestId("maintenance-status")).toBeVisible()
  await page.getByTestId("maintenance-run-submit").click()
  const confirmation = page.getByRole("alertdialog")
  await expect(confirmation).toBeVisible()
  await captureLocator(confirmation, "maintenance-confirm")
  await page.keyboard.press("Escape")

  await goto(page, "/app/settings", "settings-page")
  await expect(page.getByTestId("settings-connection")).toBeVisible()
  await capture(page, "settings-ready", ["[data-testid='connection-web-build']", "[data-testid='settings-health']"])
})
