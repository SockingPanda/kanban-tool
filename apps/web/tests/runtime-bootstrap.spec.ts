import { readFileSync } from "node:fs"

import { expect, test } from "@playwright/test"

import type { WebRuntimeConfig } from "../src/lib/runtime"

const validRuntime = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json", import.meta.url), "utf8"),
) as WebRuntimeConfig

test.describe("同源 Web runtime bootstrap", () => {
  test("在 runtime fetch 之前同步应用持久化的首屏偏好", async ({ page }) => {
    let releaseRuntime!: () => void
    const runtimeGate = new Promise<void>((resolve) => {
      releaseRuntime = resolve
    })
    await page.addInitScript(() => {
      localStorage.setItem("kb:web:theme", "dark")
      localStorage.setItem("kb:web:locale", "en")
    })
    await page.route("**/app/runtime.json", async (route) => {
      await runtimeGate
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(validRuntime),
      })
    })

    const navigation = page.goto("/app/", { waitUntil: "domcontentloaded" })
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("lang", "en")
    await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#1b1b1b")
    await expect(page.locator("#bootstrap-loading-copy")).toHaveText("Loading kanban-tool…")
    releaseRuntime()
    await navigation
  })

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

    await page.goto("/app/", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("product-shell")).toBeVisible()
    await expect(page.locator("[data-runtime-actor]")).toHaveAttribute("data-runtime-actor", validRuntime.actor)
    await expect(page.locator("[data-runtime-default-board]")).toHaveAttribute("data-runtime-default-board", validRuntime.defaultBoard)
    expect(runtimeRequests).toEqual(["http://127.0.0.1:4173/app/runtime.json"])
  })

  test("does not mount App when runtime HTTP request fails", async ({ page }) => {
    await page.route("**/app/runtime.json", async (route) => {
      await route.fulfill({ status: 503, body: "service unavailable" })
    })

    await page.goto("/app/", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("runtime-startup-error")).toBeVisible()
    await expect(page.getByTestId("product-shell")).toHaveCount(0)
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

    await page.goto("/app/", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("runtime-startup-error")).toBeVisible()
    await expect(page.getByTestId("runtime-startup-error")).toContainText("不符合当前协议")
    await expect(page.getByTestId("product-shell")).toHaveCount(0)
  })
})
