import { expect, test } from "@playwright/test"

import { installRpcFixture } from "./rpc-fixture"
import { installBoardFixture, installRuntimeFixture } from "./runtime-fixture"

const healthFixture = {
  data: {
    ok: true,
    db: "turso",
    version: "3.0.0",
    db_path: "/tmp/kanban.db",
    db_fingerprint: "sha256:settings",
  },
}

test.describe("Kanban Settings", () => {
  test.beforeEach(async ({ page }) => {
    await installRuntimeFixture(page)
    ;(await installRpcFixture(page)).handle("GetHealth", () => healthFixture)
  })

  test("persists appearance, language and actor preferences in the Web namespace", async ({ page }) => {
    const rpc = await installRpcFixture(page)
    await page.goto("/app/settings", { waitUntil: "networkidle" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
    await page.getByText("外观与语言", { exact: true }).click()
    await page.getByTestId("appearance-theme").selectOption("system")
    await page.getByTestId("appearance-density").selectOption("compact")
    await page.getByTestId("settings-locale").selectOption("en")
    await page.getByTestId("identity-actor").fill("browser-reviewer")
    await page.getByTestId("identity-actor-save").click()
    await page.getByTestId("identity-actor-reset").click()

    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light")
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact")
    await expect(page.locator("html")).toHaveAttribute("lang", "en")
    await expect(page.getByTestId("identity-actor-saved")).toBeVisible()
    await expect(page.evaluate(() => localStorage.getItem("kb:web:actor"))).resolves.toBe("")
    expect(rpc.subscriptions.flat().some(query => query.query.case !== "getHealth")).toBe(false)
    await expect
      .poll(() => page.evaluate(() => Object.keys(localStorage).sort()))
      .toEqual(["kb:web:actor", "kb:web:density", "kb:web:locale", "kb:web:sidebar", "kb:web:theme"])
  })

  test("rejects unsafe actor input without persisting it", async ({ page }) => {
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByTestId("identity-actor").fill("   ")
    await page.getByTestId("identity-actor").blur()

    await expect(page.getByTestId("identity-actor-error")).toBeVisible()
    await expect(page.getByTestId("identity-actor-save")).toBeDisabled()
    await expect(page.evaluate(() => localStorage.getItem("kb:web:actor"))).resolves.not.toBe("   ")
  })

  test("copies validated diagnostics safely and opens the canonical health route", async ({ page }) => {
    await page.goto("/app/boards/default/board", { waitUntil: "networkidle" })
    await page.getByTestId("nav-settings").click()
    await page.getByTestId("settings-page").waitFor()
    await page.evaluate(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: async () => undefined },
      })
    })
    await page.getByText("连接与诊断", { exact: true }).click()
    await page.getByTestId("diagnostics-copy").click()
    await expect(page.getByTestId("diagnostics-feedback")).toContainText("已复制")
    await page.getByTestId("diagnostics-health-link").click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/health$/)
  })

  test("keeps clipboard failure and no-board state actionable", async ({ page }) => {
    await page.evaluate(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: () => { throw new Error("denied") } },
      })
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByText("连接与诊断", { exact: true }).click()
    await page.getByTestId("diagnostics-copy").click()
    await expect(page.getByTestId("diagnostics-feedback")).toContainText("复制")

    await page.route("**/app/runtime.json", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          apiBaseUrl: "",
          webBasePath: "/app/",
          actor: "local",
          defaultBoard: "",
          serverVersion: "3.0.0",
          protocolVersion: "v1",
          webBuildId: "dev",
        }),
      })
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByText("连接与诊断", { exact: true }).click()
    await expect(page.getByTestId("settings-no-board")).toBeVisible()
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
  })

  test("retains the canonical board session when moving Board to Settings", async ({ page }) => {
    const fixture = await installBoardFixture(page)
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("nav-settings")).toBeVisible()
    await expect.poll(() => fixture.rpc.activeConnectionCount()).toBe(1)

    await page.getByTestId("nav-settings").click()
    await expect(page).toHaveURL(/\/app\/settings$/)
    await page.getByText("连接与诊断", { exact: true }).click()
    await expect(page.getByTestId("connection-reconnect")).toBeEnabled()
    await page.getByTestId("connection-reconnect").click()
    await expect(page.getByTestId("connection-feedback")).toContainText("仍在连接")
    await expect.poll(() => fixture.rpc.activeConnectionCount()).toBe(1)

    await page.keyboard.press("Escape")
    await expect(page).toHaveURL(/\/app\/boards\/default\/list$/)
    await expect.poll(() => fixture.rpc.activeConnectionCount()).toBe(1)
  })
})
