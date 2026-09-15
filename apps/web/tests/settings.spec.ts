import { expect, test } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"
import { installExplorerFixture } from "./explorer-fixture"

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
    await page.route("http://127.0.0.1:4173/health", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(healthFixture) })
    })
  })

  test("persists appearance, language and actor preferences in the Web namespace", async ({ page }) => {
    let streamRequests = 0
    await page.route("**/api/v1/stream/events**", async (route) => {
      streamRequests += 1
      await route.abort()
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await page.getByRole("tab", { name: "连接与诊断" }).click()
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
    await page.getByRole("tab", { name: "外观", exact: true }).click()
    await page.getByTestId("appearance-theme").click()
    await page.getByRole("option", { name: "跟随系统", exact: true }).click()
    await page.getByTestId("appearance-density").click()
    await page.getByRole("option", { name: "紧凑", exact: true }).click()
    await page.getByTestId("settings-locale").click()
    await page.getByRole("option", { name: "English", exact: true }).click()
    await page.getByRole("tab", { name: "Identity", exact: true }).click()
    await page.getByTestId("identity-actor").fill("browser-reviewer")
    await page.getByTestId("identity-save").click()
    await expect.poll(() => page.evaluate(() => localStorage.getItem("kb:web:actor"))).toBe("browser-reviewer")
    await page.getByTestId("identity-reset").click()

    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light")
    await expect(page.locator("html")).not.toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact")
    await expect(page.locator("html")).toHaveAttribute("lang", "en")
    await expect(page.getByRole("status")).toHaveText("Operation name saved.")
    await expect(page.evaluate(() => localStorage.getItem("kb:web:actor"))).resolves.toBe("")
    expect(streamRequests).toBe(0)
    await expect
      .poll(() => page.evaluate(() => Object.keys(localStorage).sort()))
      .toEqual(["kb:web:actor", "kb:web:density", "kb:web:locale", "kb:web:sidebar", "kb:web:theme"])
  })

  test("rejects unsafe actor input without persisting it", async ({ page }) => {
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByRole("tab", { name: "操作身份", exact: true }).click()
    await page.getByTestId("identity-actor").fill("   ")
    await page.getByTestId("identity-save").click()

    await expect(page.getByTestId("identity-actor")).toHaveAttribute("aria-invalid", "true")
    await expect(page.getByRole("alert")).toBeVisible()
    await expect(page.evaluate(() => localStorage.getItem("kb:web:actor"))).resolves.not.toBe("   ")
  })

  test("copies validated diagnostics safely and opens the canonical health route", async ({ page }) => {
    await installExplorerFixture(page)
    await page.goto("/app/boards/default/board", { waitUntil: "networkidle" })
    await page.getByTestId("nav-settings").click()
    await page.getByTestId("settings-page").waitFor()
    await page.evaluate(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: async () => undefined },
      })
    })
    await page.getByRole("tab", { name: "连接与诊断" }).click()
    await page.getByTestId("diagnostics-copy").click()
    await expect(page.getByRole("status").filter({ hasText: "已复制" })).toBeVisible()
    await page.getByTestId("diagnostics-health-link").click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/health$/)
  })

  test("keeps clipboard failure and no-board state actionable", async ({ page }) => {
    await page.addInitScript(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: () => { throw new Error("denied") } },
      })
    })
    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByRole("tab", { name: "连接与诊断" }).click()
    await page.getByTestId("diagnostics-copy").click()
    await expect(page.getByRole("status").filter({ hasText: "复制" })).toBeVisible()

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
    await page.getByRole("tab", { name: "连接与诊断" }).click()
    await expect(page.getByTestId("settings-no-board")).toBeVisible()
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
  })

  test("retains the canonical board session when moving Board to Settings", async ({ page }) => {
    const fixture = await installExplorerFixture(page)

    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("nav-settings")).toBeVisible()
    await expect.poll(fixture.getSseConnectionCount).toBe(1)

    await page.getByTestId("nav-settings").click()
    await expect(page).toHaveURL(/\/app\/settings$/)
    await page.getByRole("tab", { name: "连接与诊断" }).click()
    await expect(page.getByTestId("connection-reconnect")).toBeEnabled()
    await page.getByTestId("connection-reconnect").click()
    await expect(page.getByRole("status").filter({ hasText: "仍在连接" })).toBeVisible()
    await expect.poll(fixture.getSseConnectionCount).toBe(1)

    await page.getByRole("link", { name: "返回任务" }).click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/list$/)
    await expect.poll(fixture.getSseConnectionCount).toBe(1)
  })

  test("设置下拉框展开时屏蔽全局快捷键，关闭后恢复", async ({ page }) => {
    await installExplorerFixture(page)
    await page.addInitScript(() => localStorage.setItem("kb:web:locale", "en"))
    await page.goto("/app/boards/default/list")
    await expect(page.getByTestId("task-list")).toBeVisible()
    await page.getByTestId("nav-settings").click()
    await page.getByTestId("appearance-density").click()
    const options = page.getByRole("listbox", { name: "Density" })
    await expect(options).toBeFocused()
    await options.press("c")
    await options.press("ControlOrMeta+k")
    await expect(page).toHaveURL(/\/app\/settings$/)
    await expect(options).toBeFocused()
    await options.press("ArrowDown")
    await options.press("Enter")
    await expect.poll(() => page.evaluate(() => localStorage.getItem("kb:web:density"))).toBe("compact")
    await page.getByRole("tab", { name: "Appearance", exact: true }).focus()
    await page.keyboard.press("c")
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?create=1$/)
    await expect(page.getByTestId("task-mutation-dialog")).toBeVisible()
  })
})
