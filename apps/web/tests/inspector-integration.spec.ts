import { expect, test } from "@playwright/test"

import { installExplorerFixture } from "./explorer-fixture"

const boardPath = "/app/boards/default"
const taskId = "t_ready"

test.describe("Inspector integration seam", () => {
  test("keeps detail writes scoped, and defers suggestions/attachment bytes until requested", async ({ page }) => {
    const fixture = await installExplorerFixture(page, { withAssets: true })
    await page.goto(`${boardPath}/list?task=${taskId}`, { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await expect(page.getByTestId("inspector-assets")).toBeVisible()

    const suggestionRequests = () => fixture.apiRequests.filter((request) => request.includes("/labels/suggestions"))
    const downloadRequests = () => fixture.apiRequests.filter((request) => request.endsWith("/attachments/a_fixture"))
    expect(suggestionRequests()).toHaveLength(0)
    expect(downloadRequests()).toHaveLength(0)

    await page.getByRole("button", { name: "编辑任务" }).click()
    await page.getByRole("textbox", { name: "任务标题" }).fill("Renamed in Inspector")
    await page.getByRole("button", { name: "保存", exact: true }).click()
    await expect(page).toHaveURL(new RegExp(`/list\\?task=${taskId}$`))
    await expect(page.getByTestId("task-inspector").getByRole("heading", { name: "Renamed in Inspector" })).toBeVisible()
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
    await page.getByTestId("first-required-step-input").fill("Initial handoff")
    await page.getByTestId("task-mutation-dialog").getByRole("button", { name: "创建", exact: true }).click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?task=t_[^&]+$/)
    await expect(page.getByRole("button", { name: "Created with first step" })).toBeVisible()
    expect(fixture.apiRequests.some((request) => request.match(/\/api\/v1\/tasks\/t_[^/]+\/steps$/))).toBeTruthy()
  })

  test("keeps a failed label draft editable and scopes successful attachment actions", async ({ page }) => {
    const fixture = await installExplorerFixture(page, { withAssets: true, failLabelAddOnce: true })
    await page.goto(`${boardPath}/list?task=${taskId}`, { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("task-inspector")).toBeVisible()
    await expect(page.getByTestId("attachment-row")).toHaveCount(1)

    const labelInput = page.getByRole("textbox", { name: "标签名称" })
    await labelInput.fill("first-label")
    await page.getByTestId("label-add").click()
    await expect(page.getByTestId("inspector-mutation-error")).toBeVisible()
    await expect(labelInput).toHaveValue("first-label")
    await expect(labelInput).toBeEnabled()

    await labelInput.fill("fresh-label")
    await page.getByTestId("label-add").click()
    await expect(page.getByTestId("inspector-labels")).toContainText("fresh-label")
    await expect(page.getByTestId("inspector-labels")).not.toContainText("first-label")
    expect(fixture.apiRequests.filter((request) => request === `/api/v1/tasks/${taskId}/labels`)).toHaveLength(2)

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

    const labelInput = page.getByRole("textbox", { name: "标签名称" })
    await labelInput.fill("reload-gated")
    await page.getByTestId("label-add").click()
    await expect(page.getByTestId("inspector-mutation-error")).toContainText("写入已提交")
    await expect(page.getByTestId("inspector-labels")).not.toContainText("reload-gated")

    fixture.failNextInspectorReads(0)
    await page.getByTestId("inspector-retry").click()
    await expect(page.getByTestId("inspector-labels")).toContainText("reload-gated")
    await expect(page.getByTestId("inspector-mutation-error")).toHaveCount(0)
  })
})
