import { spawn, type ChildProcess } from "node:child_process"
import { expect, test, type WorkerInfo } from "@playwright/test"

let fixtureServer: ChildProcess | undefined
let fixtureBaseURL = ""

async function waitForFixture(url: string): Promise<void> {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url)
      if (response.ok) return
    } catch {
      // Vite is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`selector fixture did not start at ${url}`)
}

test.describe("CSP-safe selector browser contracts", () => {
  test.describe.configure({ mode: "serial" })

  test.beforeAll(async ({ browser }, workerInfo: WorkerInfo) => {
    void browser
    const port = 1422 + (process.pid % 1000) + workerInfo.workerIndex
    fixtureBaseURL = `http://127.0.0.1:${port}/app/`
    fixtureServer = spawn("pnpm", ["vite", "--host", "127.0.0.1", "--port", String(port)], {
      cwd: import.meta.dirname.replace(/\/tests$/, ""),
      stdio: "ignore",
    })
    await waitForFixture(`${fixtureBaseURL}tests/fixtures/astryx-selectors.fixture.html`)
  })

  test.afterAll(() => {
    fixtureServer?.kill()
    fixtureServer = undefined
  })

  test.beforeEach(async ({ page }) => {
    await page.goto(`${fixtureBaseURL}tests/fixtures/astryx-selectors.fixture.html`)
  })

  test("keeps search editing keys native and uses two-stage Escape", async ({ page }) => {
    const trigger = page.getByTestId("multi-selector")
    await trigger.click()
    const search = page.getByRole("combobox", { name: "Filter statuses" })
    await search.fill("ab")
    await search.press("Home")
    await page.keyboard.type("x")
    await search.press("End")
    await page.keyboard.type("y")
    await page.keyboard.type(" ")
    await expect(search).toHaveValue("xaby ")

    await search.press("Escape")
    await expect(trigger).toHaveAttribute("aria-expanded", "false")
    await trigger.press("Escape")
    await trigger.click()
    await expect(page.getByRole("combobox", { name: "Filter statuses" })).toHaveValue("")
    await search.fill("   ")
    await expect(page.getByRole("option", { name: "Select all statuses" })).toBeVisible()
    await page.getByRole("option", { name: "Select all statuses" }).click()
    await expect(page.locator('input[type="hidden"][name="statuses"]')).toHaveCount(2)
    await page.getByTestId("outside").click()
    await expect(trigger).toHaveAttribute("aria-expanded", "false")
  })

  test("keeps active descendant IDs truthful for hover and source swaps", async ({ page }) => {
    const trigger = page.getByTestId("multi-selector")
    await trigger.click()
    const search = page.getByRole("combobox", { name: "Filter statuses" })
    await expect(search).toHaveAttribute("aria-activedescendant", /.+/)
    const activeId = await search.getAttribute("aria-activedescendant")
    expect(activeId).not.toBeNull()
    await expect(page.locator(`#${activeId}`)).toHaveAttribute("role", "option")
    await search.press("ArrowDown")
    await search.press("ArrowDown")
    const skippedDisabledId = await search.getAttribute("aria-activedescendant")
    expect(skippedDisabledId).not.toBeNull()
    await expect(page.locator(`#${skippedDisabledId}`)).toHaveText("Done")
    const hoveredOption = page.getByRole("option").nth(1)
    await hoveredOption.hover()
    await expect(search).toHaveAttribute("aria-activedescendant", await hoveredOption.getAttribute("id") ?? "")

    await page.getByTestId("toggle-multi-disabled").click()
    await expect(trigger).toBeDisabled()
    await expect(trigger).toHaveAttribute("aria-expanded", "false")

    const typeahead = page.getByTestId("typeahead")
    await typeahead.fill("old")
    await page.getByTestId("swap-source").click()
    await expect(typeahead).toHaveAttribute("aria-expanded", "false")
    await page.getByTestId("resolve-old").click()
    await expect(page.getByText("Old result")).toHaveCount(0)
    await typeahead.fill("new")
    await expect(page.getByText("New result")).toBeVisible()
    const typeaheadActiveId = await typeahead.getAttribute("aria-activedescendant")
    expect(typeaheadActiveId).not.toBeNull()
    await expect(page.locator(`#${typeaheadActiveId}`)).toHaveAttribute("role", "option")
  })

  test("closes and invalidates controls when disabled flips with inline callbacks", async ({ page }) => {
    const typeahead = page.getByTestId("typeahead")
    await page.getByTestId("swap-source").click()
    await typeahead.fill("new")
    await expect(page.getByText("New result")).toBeVisible()
    await expect(page.getByTestId("open-events")).toHaveText("1")
    await page.getByTestId("toggle-typeahead-disabled").click()
    await expect(typeahead).toBeDisabled()
    await expect(typeahead).toHaveAttribute("aria-expanded", "false")
    await expect(page.getByText("New result")).toHaveCount(0)
    await expect(page.getByTestId("open-events")).toHaveText("2")
  })

  test("closes Typeahead on outside blur and keeps its two-stage Escape contract", async ({ page }) => {
    const typeahead = page.getByTestId("typeahead")
    await page.getByTestId("swap-source").click()
    await typeahead.fill("new")
    await expect(page.getByText("New result")).toBeVisible()
    await page.getByTestId("outside").click()
    await expect(typeahead).toHaveAttribute("aria-expanded", "false")
    await expect(page.getByTestId("open-events")).toHaveText("2")

    await typeahead.fill("new")
    await expect(page.getByText("New result")).toBeVisible()
    await typeahead.press("Escape")
    await expect(typeahead).toHaveAttribute("aria-expanded", "false")
    await expect(typeahead).toHaveValue("new")
    await typeahead.press("Escape")
    await expect(typeahead).toHaveValue("")
    await expect(page.getByTestId("open-events")).toHaveText("4")
  })
})
