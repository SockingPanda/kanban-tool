import { expect, test } from "@playwright/test"

test("keeps controlled date state when the complete candidate violates a time bound", async ({ page }) => {
  await page.goto("/app/")
  await page.waitForTimeout(1000)
  await page.addScriptTag({ type: "module", url: "/app/src/ui/astryx/datetime/DateTimeInput.controlled.harness.tsx" })

  const date = page.locator('input[data-astryx-segment="date"]')
  const time = page.locator('input[data-astryx-segment="time"]')
  const lastChange = page.getByTestId("last-change")

  await expect(date).toHaveValue("2026-08-10")
  await expect(time).toHaveValue("08:00")
  await date.fill("2026-08-09")

  await expect(date).toHaveValue("2026-08-10")
  await expect(time).toHaveValue("08:00")
  await expect(lastChange).toHaveText("none")

  await time.fill("09:00")
  await expect(lastChange).toHaveText("2026-08-10T09:00")
  await date.fill("2026-08-09")
  await expect(date).toHaveValue("2026-08-09")
  await expect(lastChange).toHaveText("2026-08-09T09:00")
})
