import { expect, test } from "@playwright/test"

import { installExplorerFixture } from "./explorer-fixture"

const boardPath = "/app/boards/default"
const taskId = "t_ready"

test.describe("Inspector integration seam", () => {
  test("keeps the draft after a version conflict has refreshed canonical data", async ({ page }) => {
    const fixture = await installExplorerFixture(page)
    let conflict = true
    await page.route(`**/api/v1/tasks/${taskId}`, async route => {
      if (route.request().method() === "PATCH" && conflict) {
        conflict = false
        await route.fulfill({ status: 409, contentType: "application/json", body: JSON.stringify({ error: { code: "claim_conflict", message: "lock_version 不匹配" } }) })
      } else await route.fallback()
    })
    await page.goto(`${boardPath}/list?task=${taskId}`)
    const title = page.getByRole("textbox", { name: "任务标题", exact: true })
    await title.fill("冲突后仍保留的标题")
    await title.press("Tab")
    await expect(page.getByTestId("inspector-mutation-error")).toBeVisible()
    await expect(title).toBeEnabled()
    await expect(title).toHaveValue("冲突后仍保留的标题")
    expect(fixture.readyTask().title).not.toBe("冲突后仍保留的标题")
    await title.focus()
    await title.press("Tab")
    await expect.poll(() => fixture.readyTask().title).toBe("冲突后仍保留的标题")
  })

  test("an SSE refresh carries the write reload forward before the next field can save", async ({ page }) => {
    const fixture = await installExplorerFixture(page)
    await page.goto(`${boardPath}/list?task=${taskId}`)
    await fixture.waitForSseConnection(0)
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    let heldReads = 0
    let release!: () => void
    const held = new Promise<void>(resolve => { release = resolve })
    const versions: number[] = []
    await page.route(`**/api/v1/tasks/${taskId}`, async route => {
      if (route.request().method() === "PATCH") versions.push(route.request().postDataJSON().expected_lock_version)
      else if (versions.length === 1) {
        heldReads += 1
        await held
      }
      try { await route.fallback() } catch { /* 同一项目的更新会中止被替换的读取。 */ }
    })
    const title = page.getByRole("textbox", { name: "任务标题", exact: true })
    const description = page.getByRole("textbox", { name: "编辑任务说明" })
    await title.fill("标题先保存")
    await title.press("Tab")
    await expect.poll(() => heldReads).toBeGreaterThan(0)
    await fixture.emitTaskUpdated()
    await expect.poll(() => heldReads).toBeGreaterThan(1)
    await expect(description).toBeDisabled()
    release()
    await expect(description).toBeEnabled()
    await description.fill("使用回读后的版本继续保存说明")
    await description.press("Tab")
    await expect.poll(() => fixture.readyTask().description).toBe("使用回读后的版本继续保存说明")
    expect(versions).toEqual([1, 2])
    await expect(page.getByTestId("inspector-mutation-error")).toHaveCount(0)
  })

  test("a failed metadata refresh does not release a still-running task reload", async ({ page }) => {
    const fixture = await installExplorerFixture(page)
    await page.goto(`${boardPath}/list?task=${taskId}`)
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    let release!: () => void
    const held = new Promise<void>(resolve => { release = resolve })
    const versions: number[] = []
    await page.route(`**/api/v1/boards/default/columns`, async route => {
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { code: "internal_error", message: "metadata refresh unavailable" } }) })
    })
    await page.route(`**/api/v1/tasks/${taskId}`, async route => {
      if (route.request().method() === "PATCH") versions.push(route.request().postDataJSON().expected_lock_version)
      else if (versions.length === 1) await held
      try { await route.fallback() } catch { /* 卸载或同项目刷新可能中止旧请求。 */ }
    })
    const title = page.getByRole("textbox", { name: "任务标题", exact: true })
    const description = page.getByRole("textbox", { name: "编辑任务说明" })
    const metadataFailure = page.waitForResponse(response => response.url().endsWith("/boards/default/columns") && response.status() === 503)
    await title.fill("元数据失败仍等待任务")
    await title.press("Tab")
    await (await metadataFailure).finished()
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => resolve())))
    await expect(description).toBeDisabled()
    release()
    await expect(description).toBeEnabled()
    await description.fill("继续使用最新任务版本")
    await description.press("Tab")
    await expect.poll(() => fixture.readyTask().description).toBe("继续使用最新任务版本")
    expect(versions).toEqual([1, 2])
    await expect(page.getByTestId("inspector-mutation-error")).toHaveCount(0)
  })

  test("keeps detail writes scoped, and defers suggestions/attachment bytes until requested", async ({ page }) => {
    const fixture = await installExplorerFixture(page, { withAssets: true })
    await page.goto(`${boardPath}/list?task=${taskId}`, { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.getByText("标签与附件", { exact: true }).first().click()
    await expect(page.getByTestId("inspector-assets")).toBeVisible()

    const suggestionRequests = () => fixture.apiRequests.filter((request) => request.includes("/labels/suggestions"))
    const downloadRequests = () => fixture.apiRequests.filter((request) => request.endsWith("/attachments/a_fixture"))
    expect(suggestionRequests()).toHaveLength(0)
    expect(downloadRequests()).toHaveLength(0)

    await page.getByRole("textbox", { name: "任务标题" }).fill("Renamed in Inspector")
    await page.getByRole("textbox", { name: "任务标题", exact: true }).press("Tab")
    await expect(page).toHaveURL(new RegExp(`/list\\?task=${taskId}$`))
    await expect(page.getByRole("textbox", { name: "任务标题", exact: true })).toHaveValue("Renamed in Inspector")
    expect(fixture.apiRequests.some((request) => request === `/api/v1/tasks/${taskId}`)).toBeTruthy()

    await page.getByTestId("label-suggestion-request").click()
    await expect.poll(() => suggestionRequests().length).toBe(1)
    await expect(page.getByRole("button", { name: "刷新建议" })).toBeVisible()

    await page.getByTestId("attachment-download").click()
    await expect.poll(() => downloadRequests().length).toBe(1)
    await expect(page).toHaveURL(new RegExp(`/list\\?task=${taskId}$`))
  })

  test("reconciles a List create plus first required step into the visible list", async ({ page }) => {
    const fixture = await installExplorerFixture(page)
    await page.goto(`${boardPath}/list`, { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-list")).toBeVisible()
    await page.getByTestId("task-create").click()
    await page.getByTestId("task-title-input").fill("Created with first step")
    await page.getByText("执行计划", { exact: true }).click()
    await page.getByTestId("first-required-step-input").fill("Initial handoff")
    await page.getByTestId("task-mutation-dialog").getByRole("button", { name: "创建任务", exact: true }).click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?task=t_[^&]+$/)
    await expect(page.getByRole("button", { name: "Created with first step" })).toBeVisible()
    expect(fixture.apiRequests.some((request) => request.match(/\/api\/v1\/tasks\/t_[^/]+\/steps$/))).toBeTruthy()
  })

  test("keeps a failed label draft editable and scopes successful attachment actions", async ({ page }) => {
    const fixture = await installExplorerFixture(page, { withAssets: true, failLabelAddOnce: true })
    await page.goto(`${boardPath}/list?task=${taskId}`, { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.getByText("标签与附件", { exact: true }).first().click()
    await expect(page.getByTestId("attachment-row")).toHaveCount(1)

    const labelInput = page.getByRole("textbox", { name: "标签名称" })
    await labelInput.fill("first-label")
    await page.getByTestId("label-add").click()
    await expect(page.getByTestId("inspector-mutation-error").first()).toBeVisible()
    await expect(labelInput).toHaveValue("first-label")
    await expect(labelInput).toBeEnabled()

    await labelInput.fill("fresh-label")
    await page.getByTestId("label-add").click()
    await expect(page.getByTestId("inspector-labels")).toContainText("fresh-label")
    await expect(page.getByTestId("inspector-labels")).not.toContainText("first-label")
    expect(fixture.writeRequests.filter((request) => request === `/api/v1/tasks/${taskId}/labels`)).toHaveLength(2)

    const downloadRequests = () => fixture.apiRequests.filter((request) => request.endsWith("/attachments/a_fixture"))
    await page.getByTestId("attachment-download").click()
    await expect.poll(() => downloadRequests().length).toBe(1)
    await page.getByTestId("attachment-delete").click()
    await expect(page.getByTestId("attachment-row")).toHaveCount(0)
    await expect(page).toHaveURL(new RegExp(`/list\\?task=${taskId}$`))
  })

  test("waits for the scoped inspector reload before reporting a committed label", async ({ page }) => {
    const fixture = await installExplorerFixture(page, { failInspectorReadsAfterLabelAdd: 4 })
    await page.goto(`${boardPath}/list?task=${taskId}`, { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await page.getByText("标签与附件", { exact: true }).first().click()

    const labelInput = page.getByRole("textbox", { name: "标签名称" })
    await labelInput.fill("reload-gated")
    await page.getByTestId("label-add").click()
    await expect(page.getByTestId("task-inspector")).toContainText("stale")
    await expect(page.getByTestId("inspector-mutation-error").first()).toBeVisible()
    await expect(page.getByTestId("inspector-labels")).not.toContainText("reload-gated")

    fixture.failNextInspectorReads(0)
    await page.getByTestId("task-inspector").getByRole("button", { name: "重试", exact: true }).first().click()
    await expect(page.getByTestId("inspector-labels")).toContainText("reload-gated")
    await expect(page.getByTestId("task-inspector")).not.toContainText("stale")
  })
})
