import { expect, test } from "@playwright/test"

import type { ApiHealthResponseContract } from "../src/lib/api/generated/contracts/api-health-response"
import healthFixturePayload from "../src/lib/api/generated/fixtures/api-health-response.valid.json" with { type: "json" }
import { installRpcFixture, unavailable, InvalidQueryResult } from "./rpc-fixture"

import { installRuntimeFixture } from "./runtime-fixture"

// Playwright's Node loader cannot resolve Vite's `virtual:` validator module;
// the browser read path validates this generated fixture through parseApiHealthResponse.
const validatedHealthFixture = healthFixturePayload satisfies ApiHealthResponseContract
const healthFixture = {
  ...validatedHealthFixture,
  data: {
    ...validatedHealthFixture.data,
    db: "turso",
    db_fingerprint: `turso:${validatedHealthFixture.data.db_fingerprint}`,
  },
} satisfies ApiHealthResponseContract

test.describe("Health operator workflow", () => {
  test.beforeEach(async ({ page }) => {
    await installRuntimeFixture(page)
  })

  test("loads typed health metrics and runtime identity", async ({ page }) => {
    const rpc = await installRpcFixture(page)
    ;(await installRpcFixture(page)).handle("GetHealth", async () => {
      return healthFixture
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-page")).toBeVisible()
    await expect(page.getByTestId("health-metric-ok")).toContainText("true")
    await expect(page.getByTestId("health-metric-db")).toContainText("turso")
    await expect(page.getByTestId("health-metric-db-fingerprint")).toContainText(healthFixture.data.db_fingerprint)
    await expect(page.getByTestId("health-runtime")).toContainText("local")
    await expect(page.getByTestId("nav-settings")).toHaveAttribute("aria-current", "page")
    expect(rpc.calls.filter(call => ["ListBoards", "ListBoardColumns", "ListTasks"].includes(call.method))).toEqual([])
  })

  test("keeps an actionable local error when health request fails", async ({ page }) => {
    let requestCount = 0
    let releaseRetry!: () => void
    const retryResponse = new Promise<void>((resolve) => {
      releaseRetry = resolve
    })
    ;(await installRpcFixture(page)).handle("GetHealth", async () => {
      requestCount += 1
      if (requestCount > 1) await retryResponse
      throw unavailable("kanban serve unavailable")
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-error")).toBeVisible()
    await expect(page.getByTestId("health-error")).toContainText("HTTP 503")
    await expect(page.getByTestId("health-error-retry")).toBeEnabled()
    await page.getByTestId("health-error-retry").click()
    await expect(page.getByTestId("health-error-retry")).toBeDisabled()
    await expect(page.getByTestId("health-error-retry")).toHaveText("加载中…")
    releaseRetry()
    await expect(page.getByTestId("health-error-retry")).toBeEnabled()
  })

  test("uses a safe fallback for a malformed health response", async ({ page }) => {
    ;(await installRpcFixture(page)).handle("GetHealth", async () => {
      return new InvalidQueryResult()
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })

    await expect(page.getByTestId("health-error")).toBeVisible()
    await expect(page.getByTestId("health-error-detail")).toContainText("无法识别")
    await expect(page.getByTestId("health-error")).not.toContainText("/tmp/kanban.db")
  })

  test("keeps the last report visible when refresh becomes stale", async ({ page }) => {
    let requestCount = 0
    ;(await installRpcFixture(page)).handle("GetHealth", async () => {
      requestCount += 1
      if (requestCount === 1) {
        return healthFixture
      } else {
        throw unavailable("kanban serve unavailable")
      }
    })

    await page.goto("/app/boards/default/health", { waitUntil: "networkidle" })
    await expect(page.getByTestId("health-metric-db-fingerprint")).toContainText(healthFixture.data.db_fingerprint)

    await page.getByTestId("health-refresh").click()

    await expect(page.getByTestId("health-stale")).toBeVisible()
    await expect(page.getByTestId("health-metric-db-fingerprint")).toContainText(healthFixture.data.db_fingerprint)
    await expect(page.getByTestId("health-stale")).toContainText("503")
  })

  test("loads settings health diagnostics without a board query", async ({ page }) => {
    const rpc = await installRpcFixture(page)
    ;(await installRpcFixture(page)).handle("GetHealth", async () => {
      return healthFixture
    })

    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByText("连接与诊断", { exact: true }).click()

    await expect(page.getByTestId("settings-page")).toBeVisible()
    await page.getByText("技术详情", { exact: true }).click()
    await expect(page.getByText(healthFixture.data.db_fingerprint, { exact: true })).toBeVisible()
    expect(rpc.calls.filter(call => ["ListBoards", "ListBoardColumns", "ListTasks"].includes(call.method))).toEqual([])
  })

  test("无项目设置页可独立重试健康检查，等待时防重复并恢复状态", async ({ page }) => {
    let requestCount = 0
    let releaseRetry: (() => void) | undefined
    const retryResponse = new Promise<void>(resolve => { releaseRetry = resolve })
    ;(await installRpcFixture(page)).handle("GetHealth", async () => {
      requestCount += 1
      if (requestCount === 1) {
        throw unavailable("kanban serve unavailable")
      }
      await retryResponse
      return healthFixture
    })

    await page.goto("/app/settings", { waitUntil: "networkidle" })
    await page.getByText("连接与诊断", { exact: true }).click()

    await expect(page.getByTestId("settings-health-error")).toBeVisible()
    await expect(page.getByTestId("settings-health-error")).toContainText("kanban serve")
    await expect(page.getByTestId("settings-health-error")).toContainText("然后重试")
    await expect(page.getByTestId("connection-reconnect")).toBeDisabled()
    await expect(page.getByTestId("diagnostics-health-link")).toBeDisabled()
    await expect(page.getByTestId("settings-health-retry")).toBeEnabled()
    try {
      await page.getByTestId("settings-health-retry").click()
      await expect(page.getByTestId("settings-health-retry")).toBeDisabled()
      await expect(page.getByTestId("settings-health-status")).toHaveText("正在检查")
      await expect.poll(() => requestCount).toBe(2)
    } finally {
      releaseRetry?.()
    }
    await expect(page.getByTestId("settings-health-error")).toHaveCount(0)
    await expect(page.getByTestId("settings-health-status")).toHaveText("服务可用")
    expect(requestCount).toBe(2)
    await page.reload()
    await page.getByRole("tab", { name: "连接与诊断" }).click()
    await expect(page.getByTestId("settings-health-status")).toHaveText("服务可用")
  })
})
