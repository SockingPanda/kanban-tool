import AxeBuilder from "@axe-core/playwright"
import { expect, test } from "@playwright/test"

import { installPlaneAcceptanceFixture } from "./plane-acceptance-fixture"
import { expectStrictCsp } from "./plane-acceptance-support"

test.describe("Plane-only Projects navigation acceptance", () => {
  test("keeps /app/ as the Projects collection instead of resolving runtime defaultBoard", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    const response = await page.goto("/app/", { waitUntil: "domcontentloaded" })
    expectStrictCsp(response)

    await expect(page).toHaveURL(/\/app\/$/)
    await expect(page.getByTestId("projects-collection")).toBeVisible()
    await expect(page.getByTestId("projects-collection-project-default")).toBeVisible()
    await expect(page.getByTestId("projects-collection-project-agent-first")).toBeVisible()
    expect(page.url()).not.toContain("/app/home")

    const defaultProject = page.getByTestId("projects-collection-project-default")
    await expect(defaultProject).toHaveAttribute("href", "/app/boards/default/overview")
    await defaultProject.click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/overview$/)
  })

  test("uses the canonical project overview route and keeps the product rail limited to Projects and Settings", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/default/overview", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("project-overview")).toBeVisible()
    await expect(page.getByTestId("projects-sidebar")).toBeVisible()
    await expect(page.getByTestId("project-tree-overview")).toHaveAttribute("aria-current", "page")
    await expect(page.getByTestId("project-tree-tasks")).toBeVisible()

    const rail = page.getByTestId("product-rail")
    await expect(rail).toBeVisible()
    await expect(rail.getByTestId("product-rail-projects")).toBeVisible()
    await expect(rail.getByTestId("product-rail-settings")).toBeVisible()
    await expect(rail.getByRole("link")).toHaveCount(2)
    await expect(rail.getByTestId("product-rail-projects")).toHaveAttribute("aria-current", "page")
  })

  test("keeps the overview surface strict-CSP, axe-clean, and free of runtime style tags", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    const response = await page.goto("/app/boards/default/overview", { waitUntil: "domcontentloaded" })
    expectStrictCsp(response)
    await expect(page.getByTestId("project-overview")).toBeVisible()

    await expect(page.locator("[style]")).toHaveCount(0)
    await expect(page.locator("style")).toHaveCount(0)
    const axe = await new AxeBuilder({ page }).analyze()
    expect(axe.violations).toEqual([])
  })
})
