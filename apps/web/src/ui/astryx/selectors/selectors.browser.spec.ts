import { expect, test } from "@playwright/test"

test.describe("CSP-safe selector browser contracts", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("src/ui/astryx/selectors/selectors.browser.html")
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
  })

  test("keeps active descendant IDs truthful for hover and source swaps", async ({ page }) => {
    const trigger = page.getByTestId("multi-selector")
    await trigger.click()
    const search = page.getByRole("combobox", { name: "Filter statuses" })
    await expect(search).toHaveAttribute("aria-activedescendant", /.+/)
    const activeId = await search.getAttribute("aria-activedescendant")
    expect(activeId).not.toBeNull()
    await expect(page.locator(`#${activeId}`)).toHaveAttribute("role", "option")
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

  test("closes and invalidates controls when disabled flips", async ({ page }) => {
    const typeahead = page.getByTestId("typeahead")
    await page.getByTestId("swap-source").click()
    await typeahead.fill("new")
    await expect(page.getByText("New result")).toBeVisible()
    await page.getByTestId("toggle-typeahead-disabled").click()
    await expect(typeahead).toBeDisabled()
    await expect(typeahead).toHaveAttribute("aria-expanded", "false")
    await expect(page.getByText("New result")).toHaveCount(0)
  })
})
