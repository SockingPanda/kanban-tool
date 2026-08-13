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

  test("keeps archive state and search in the Projects URL while retaining archived identity", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/?archive=archived&q=Archive", { waitUntil: "domcontentloaded" })

    await expect(page).toHaveURL(/\/app\/\?archive=archived&q=Archive$/)
    await expect(page.getByTestId("projects-collection")).toHaveAttribute("data-archive", "archived")
    await expect(page.getByTestId("projects-search")).toHaveValue("Archive")
    await expect(page.getByTestId("projects-collection-project-archived")).toBeVisible()
    await expect(page.getByTestId("projects-collection-project-default")).toHaveCount(0)

    await page.getByTestId("projects-archive-active").click()
    await expect(page).toHaveURL(/\/app\/\?q=Archive$/)
    await expect(page.getByTestId("projects-collection")).toHaveAttribute("data-archive", "active")

    await page.getByTestId("projects-search").fill("Default")
    await expect(page).toHaveURL(/\/app\/\?q=Default$/)
    await expect(page.getByTestId("projects-collection-project-default")).toBeVisible()
    await expect(page.getByTestId("projects-collection-project-archived")).toHaveCount(0)
  })

  test("keeps the current archived project visible as an identity-only overview", async ({ page }) => {
    await installPlaneAcceptanceFixture(page)
    await page.goto("/app/boards/archived/overview", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("project-overview")).toBeVisible()
    await expect(page.getByTestId("project-overview")).toHaveAttribute("data-archived", "true")
    await expect(page.getByTestId("project-tree-project")).toContainText("Archived project")
    await expect(page.getByTestId("project-overview-open-tasks")).toHaveCount(0)
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
