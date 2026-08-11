import { expect, test, type Page } from "@playwright/test"

import { auditAxe, evidenceContext, evidenceRunId, expectFocusVisible, hostIdentityEvidence, type AxeEvidence, type AxeIncompleteDisposition, type BrowserContextObservation, type KeyboardEvidence, writeEvidence } from "./a11y-proof-support"
import { installExplorerFixture } from "./explorer-fixture"

test.describe.configure({ mode: "serial" })

const axeEvidence: AxeEvidence[] = []
const keyboardEvidence: KeyboardEvidence[] = []
const pageErrors: string[] = []
const expectedAxeLabels = [
  "shell.board-ready",
  "settings.ready",
  "board.ready",
  "board.mutation-dialog",
  "list.ready",
  "map.ready",
  "inspector.ready",
  "inspector.edit",
  "events.online",
  "events.offline",
  "events.recovered",
  "health.ready",
  "maintenance.ready",
  "maintenance.confirm",
] as const
const expectedKeyboardLabels = [
  "shell.settings-nav",
  "settings.theme",
  "settings.actor",
  "board.seed-title",
  "board.create-button",
  "list.seed-title",
  "map.node",
  "inspector.opener",
  "health.refresh",
  "maintenance.run",
] as const

const manualAxeDispositions: Record<string, AxeIncompleteDisposition> = {
  "shell.board-ready:color-contrast": {
    reviewed: true,
    result: "manual-pass",
    rationale: "Axe 4.12 reports a geometry-only elmPartiallyObscured check; observed board headings/count/empty status nodes have computed foreground #171717 or #525252 on opaque #ffffff, ratios 17.93:1 or 7.81:1, and no computed error.",
  },
  "board.ready:color-contrast": {
    reviewed: true,
    result: "manual-pass",
    rationale: "Axe 4.12 reports a geometry-only elmPartiallyObscured check; observed board headings/count/empty status nodes have computed foreground #171717 or #525252 on opaque #ffffff, ratios 17.93:1 or 7.81:1, and no computed error.",
  },
  "map.ready:color-contrast": {
    reviewed: true,
    result: "manual-pass",
    rationale: "Axe 4.12 classifies the visible − and ↺ glyphs as nonBmp; both enabled buttons expose the exact accessible names 缩小关系图 and 重置关系图缩放, with computed #171717 on opaque #ffffff at 12.16px/400 and a 17.93:1 ratio.",
  },
}

function dispositionFor(label: string, ruleId: string): AxeIncompleteDisposition | null {
  return manualAxeDispositions[`${label}:${ruleId}`] ?? null
}

function browserContextMatchesExpected(context: BrowserContextObservation): boolean {
  return context.document_language === "zh-CN"
    && context.navigator_language === "zh-CN"
    && ["zh-cn", "zh-hans-cn"].includes(context.intl_locale.toLowerCase())
    && context.color_scheme_light
    && !context.color_scheme_dark
    && context.reduced_motion_reduce
    && context.theme_state === "system"
    && context.astryx_theme === "neutral"
}

async function goto(page: Page, path: string, ready: string): Promise<void> {
  await page.goto(path, { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId(ready)).toBeVisible({ timeout: 30_000 })
}

async function audit(page: Page, label: string): Promise<void> {
  await auditAxe(page, label, axeEvidence, dispositionFor)
}

test.beforeEach(async ({ page }) => {
  await installExplorerFixture(page)
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
  page.on("pageerror", (error) => pageErrors.push(`${page.url()}: ${error.message}`))
  page.on("crash", () => pageErrors.push(`${page.url()}: page crashed`))
})

test.afterAll(async ({ browser }, testInfo) => {
  void browser
  const actualLabels = axeEvidence.map((run) => run.label).sort()
  const expectedLabels = [...expectedAxeLabels].sort()
  const axeLabelsComplete = actualLabels.length === expectedLabels.length && actualLabels.every((label, index) => label === expectedLabels[index])
  const incompleteRules = axeEvidence.flatMap((run) => run.incomplete)
  const incompleteDispositionComplete = incompleteRules.every((rule) => rule.disposition?.reviewed === true && rule.disposition.rationale.length > 0)
  const incompleteEvidenceComplete = incompleteRules.every((rule) => rule.nodes.every((node) => node.computed?.error === null && node.computed.accessible_name !== null && node.computed.font_size !== null && node.computed.font_weight !== null && node.computed.opacity === "1" && node.computed.contrast_ratio !== null && node.computed.contrast_ratio >= 4.5))
  const contextComplete = axeEvidence.length === expectedLabels.length && axeEvidence.every((run) => browserContextMatchesExpected(run.context))
  const axeGate = axeLabelsComplete
    && pageErrors.length === 0
    && contextComplete
    && incompleteDispositionComplete
    && incompleteEvidenceComplete
    && axeEvidence.every((run) => Object.values(run.impactCounts).every((count) => count === 0) && run.violations.length === 0)
  const actualKeyboardLabels = keyboardEvidence.map((run) => run.label).sort()
  const expectedKeyboard = [...expectedKeyboardLabels].sort()
  const keyboardLabelsComplete = actualKeyboardLabels.length === expectedKeyboard.length
    && actualKeyboardLabels.every((label, index) => label === expectedKeyboard[index])
  const keyboardGate = keyboardLabelsComplete
    && keyboardEvidence.every((run) => run.focusVisible)
  await writeEvidence(testInfo, "keyboard", {
    schema_version: 1,
    run_id: evidenceRunId(),
    lane: "real-kanban-serve",
    browser: testInfo.project.name,
    viewport: testInfo.project.use.viewport,
    ...evidenceContext,
    host: hostIdentityEvidence(),
    axe_gate: axeGate ? "passed" : "failed",
    incomplete_disposition_gate: incompleteDispositionComplete ? "passed" : "failed",
    incomplete_evidence_gate: incompleteEvidenceComplete ? "passed" : "failed",
    keyboard_gate: keyboardGate ? "passed" : "failed",
    expected_axe_labels: expectedAxeLabels,
    expected_keyboard_labels: expectedKeyboardLabels,
    axe_runs: axeEvidence,
    keyboard: keyboardEvidence,
    page_errors: pageErrors,
  })
  expect(actualLabels, "a11y axe evidence labels").toEqual(expectedLabels)
  expect(actualKeyboardLabels, "keyboard evidence labels").toEqual(expectedKeyboard)
  expect(keyboardEvidence.every((run) => run.focusVisible), "keyboard focus indicators").toBe(true)
  expect(pageErrors, "page errors/crashes").toEqual([])
  if (process.env.KANBAN_A11Y_MODE === "formal") {
    expect(incompleteDispositionComplete, "axe incomplete manual dispositions").toBe(true)
    expect(incompleteEvidenceComplete, "axe incomplete computed evidence").toBe(true)
    expect(contextComplete, "axe browser context").toBe(true)
  }
})

test("shell, board and settings keep landmarks and visible keyboard focus", async ({ page }) => {
  await goto(page, "/app/boards/default/board", "board-view")
  await audit(page, "shell.board-ready")
  await expectFocusVisible(page, "shell.settings-nav", keyboardEvidence, page.getByTestId("product-rail-settings"))
  await page.getByTestId("product-rail-settings").click()
  await expect(page).toHaveURL(/\/app\/settings$/)
  await expect(page.getByTestId("settings-page")).toBeVisible()
  await audit(page, "settings.ready")
  await expectFocusVisible(page, "settings.theme", keyboardEvidence, page.getByTestId("appearance-theme"))
  await expectFocusVisible(page, "settings.actor", keyboardEvidence, page.getByTestId("identity-actor"))
})

test("board list and map routes expose keyboard-reachable task surfaces", async ({ page }) => {
  await goto(page, "/app/boards/default/board", "board-view")
  await audit(page, "board.ready")
  const seedTitle = page.getByRole("button", { name: "Ready task", exact: true }).first()
  await expect(seedTitle).toBeVisible()
  await expectFocusVisible(page, "board.seed-title", keyboardEvidence, seedTitle)
  await seedTitle.press("Enter")
  await expect(page.getByTestId("task-inspector")).toBeVisible()
  await page.getByRole("button", { name: "关闭任务检查器" }).click()
  await expect(seedTitle).toBeFocused()
  const createButton = page.getByTestId("task-create")
  await expectFocusVisible(page, "board.create-button", keyboardEvidence, createButton)
  await createButton.press("Enter")
  await expect(page.getByTestId("task-mutation-dialog")).toBeVisible()
  await audit(page, "board.mutation-dialog")
  await expect(page.getByTestId("task-title-input")).toBeFocused()
  const mutationDialog = page.getByTestId("task-mutation-dialog")
  const focusable = mutationDialog.locator("button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled])")
  const firstFocusable = focusable.first()
  const lastFocusable = focusable.last()
  await lastFocusable.focus()
  await lastFocusable.press("Tab")
  await expect(firstFocusable).toBeFocused()
  await firstFocusable.press("Shift+Tab")
  await expect(lastFocusable).toBeFocused()
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("task-mutation-dialog")).toHaveCount(0)
  await expect(createButton).toBeFocused()

  await page.goto("/app/boards/default/list", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("task-list")).toBeVisible()
  await audit(page, "list.ready")
  await expectFocusVisible(page, "list.seed-title", keyboardEvidence, page.getByRole("button", { name: "Ready task", exact: true }).first())

  await page.goto("/app/boards/default/map?filter=all", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("task-map")).toBeVisible()
  await audit(page, "map.ready")
  await expectFocusVisible(page, "map.node", keyboardEvidence, page.getByTestId("task-map-node").getByRole("button").first())
})

test("inspector close returns focus and events recover after offline without refresh", async ({ page, browser }) => {
  await goto(page, "/app/boards/default/list", "task-list")
  const seedOpener = page.getByRole("button", { name: "Ready task", exact: true })
  await expectFocusVisible(page, "inspector.opener", keyboardEvidence, seedOpener)
  await seedOpener.click()
  await expect(page.getByTestId("task-inspector")).toBeVisible()
  await audit(page, "inspector.ready")
  await page.getByTestId("task-inspector").getByRole("button", { name: "编辑任务" }).click()
  await expect(page.getByTestId("inspector-edit-form")).toBeVisible()
  await audit(page, "inspector.edit")
  await page.getByTestId("inspector-edit-form").getByRole("button", { name: "取消" }).click()
  await expect(page.getByTestId("inspector-edit-form")).toHaveCount(0)
  await page.getByRole("button", { name: "关闭任务检查器" }).click()
  await expect(seedOpener).toBeFocused()

  const context = await browser.newContext({ baseURL: process.env.KANBAN_A11Y_BASE_URL, locale: "zh-CN", viewport: { width: 1440, height: 900 }, colorScheme: "light", reducedMotion: "reduce" })
  const events = await context.newPage()
  events.on("pageerror", (error) => pageErrors.push(`${events.url()}: ${error.message}`))
  events.on("crash", () => pageErrors.push(`${events.url()}: page crashed`))
  try {
    await goto(events, "/app/boards/default/events", "events-ready")
    await expect(events.getByTestId("event-row")).not.toHaveCount(0)
    await auditAxe(events, "events.online", axeEvidence, dispositionFor)
    await context.setOffline(true)
    await expect(events.getByTestId("events-stale-notice")).toBeVisible()
    await auditAxe(events, "events.offline", axeEvidence, dispositionFor)
    await context.setOffline(false)
    await expect.poll(() => events.getByTestId("events-stale-notice").count()).toBe(0)
    await expect(events.getByTestId("events-ready")).toBeVisible()
    await auditAxe(events, "events.recovered", axeEvidence, dispositionFor)
  } finally {
    await context.close()
  }
})

test("health and maintenance confirmation remain accessible with keyboard", async ({ page }) => {
  await goto(page, "/app/boards/default/health", "health-page")
  await expect(page.getByTestId("health-metrics")).toBeVisible()
  await audit(page, "health.ready")
  await expectFocusVisible(page, "health.refresh", keyboardEvidence, page.getByTestId("health-refresh"))

  await goto(page, "/app/boards/default/maintenance", "maintenance-page")
  await expect(page.getByTestId("maintenance-status")).toBeVisible()
  await audit(page, "maintenance.ready")
  const opener = page.getByTestId("maintenance-run-submit")
  await expectFocusVisible(page, "maintenance.run", keyboardEvidence, opener)
  await opener.click()
  const confirmation = page.getByRole("alertdialog")
  await expect(confirmation).toBeVisible()
  await audit(page, "maintenance.confirm")
  await expect(confirmation.getByRole("button", { name: "取消" })).toBeFocused()
  await page.keyboard.press("Escape")
  await expect(confirmation).toHaveCount(0)
  await expect(opener).toBeFocused()
})
