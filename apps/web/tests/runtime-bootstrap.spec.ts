import { readFileSync } from "node:fs"

import { expect, test } from "@playwright/test"

import type { WebRuntimeConfig } from "../src/lib/runtime"

const validRuntime = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json", import.meta.url), "utf8"),
) as WebRuntimeConfig

test.describe("同源 Web runtime bootstrap", () => {
  test("在 validated runtime fixture 成功后才挂载 App", async ({ page }) => {
    const runtimeRequests: string[] = []
    await page.route("**/app/runtime.json", async (route) => {
      runtimeRequests.push(route.request().url())
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(validRuntime),
      })
    })

    await page.goto("/app/", { waitUntil: "networkidle" })

    await expect(page.getByRole("heading", { name: "Astryx foundation lab" })).toBeVisible()
    await expect(page.locator("#main-content")).toHaveAttribute("data-runtime-actor", validRuntime.actor)
    await expect(page.locator("#main-content")).toHaveAttribute("data-runtime-default-board", validRuntime.defaultBoard)
    expect(runtimeRequests).toEqual(["http://127.0.0.1:4173/app/runtime.json"])
  })

  test("does not mount App when runtime HTTP request fails", async ({ page }) => {
    await page.route("**/app/runtime.json", async (route) => {
      await route.fulfill({ status: 503, body: "service unavailable" })
    })

    await page.goto("/app/", { waitUntil: "networkidle" })

    await expect(page.getByTestId("runtime-startup-error")).toBeVisible()
    await expect(page.getByRole("heading", { name: "Astryx foundation lab" })).toHaveCount(0)
    await expect(page.getByTestId("runtime-startup-error")).toContainText("HTTP 503")
  })

  test("does not mount App when runtime schema drifts", async ({ page }) => {
    await page.route("**/app/runtime.json", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ ...validRuntime, unexpected: true }),
      })
    })

    await page.goto("/app/", { waitUntil: "networkidle" })

    await expect(page.getByTestId("runtime-startup-error")).toBeVisible()
    await expect(page.getByTestId("runtime-startup-error")).toContainText("不符合当前协议")
    await expect(page.getByRole("heading", { name: "Astryx foundation lab" })).toHaveCount(0)
  })
})
