import { expect, test, type Page } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"

type TaskStatus = "todo" | "ready"

function task(status: TaskStatus, title = "Draft") {
  return {
    id: "t_todo",
    board_id: "b_default",
    board_slug: "default",
    ref: "default#1",
    seq: 1,
    title,
    description: null,
    status,
    status_reason: null,
    assignee: null,
    priority: 2,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "test",
    created_at: 1,
    updated_at: 2,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result: null,
    metadata: {},
    lock_version: 1,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "not_required",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
  }
}

async function wireBoard(page: Page, options: { readonly updateStatus?: number; readonly updateBody?: unknown } = {}) {
  await installRuntimeFixture(page)
  let status: TaskStatus = "todo"
  await page.route("**/api/v1/stream/events**", (route) => route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }))
  await page.route("**/api/v1/**", async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    if (request.method() === "POST" || request.method() === "PATCH") {
      if (url.pathname.endsWith("/tasks/t_todo")) {
        await route.fulfill({
          status: options.updateStatus ?? 200,
          contentType: "application/json",
          body: JSON.stringify(options.updateBody ?? { data: task(status) }),
        })
        return
      }
      if (url.pathname.endsWith("/promote")) {
        status = "ready"
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: task("ready") }) })
        return
      }
    }
    if (url.pathname === "/api/v1/boards") {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 2, archived_at: null }] }) })
      return
    }
    if (url.pathname === "/api/v1/boards/default/columns") {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: [
        { id: "c_todo", board_id: "b_default", status: "todo", title: "Todo", position: 1, hidden: false, wip_limit: null, created_at: 1, updated_at: 2 },
        { id: "c_ready", board_id: "b_default", status: "ready", title: "Ready", position: 2, hidden: false, wip_limit: null, created_at: 1, updated_at: 2 },
      ] }) })
      return
    }
    if (url.pathname === "/api/v1/boards/default/tasks/by-status") {
      const requested = url.searchParams.get("status")
      const visible = requested === status ? [task(status)] : []
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: { statuses: [{ status: requested, tasks: visible, page: { limit: 1000, offset: 0, total: visible.length } }] }, meta: { limit: 1000, offset: 0 } }) })
      return
    }
    await route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "fixture" } }) })
  })
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("board-task")).toBeVisible()
}

test.describe("board task mutation DOM behavior", () => {
  test("uses a native modal dialog, Escape, focus return, and does not let buttons steal card keys", async ({ page }) => {
    await wireBoard(page)
    const edit = page.getByTestId("task-edit-t_todo")
    await edit.click()
    const dialog = page.getByRole("dialog")
    await expect(dialog).toBeVisible()
    await expect(dialog.locator("input[name='task-title']")).toBeFocused()
    expect(await dialog.evaluate((node) => node.matches(":modal"))).toBe(true)
    await page.keyboard.press("Escape")
    await expect(dialog).not.toBeVisible()
    await expect(edit).toBeFocused()

    await edit.press("Enter")
    await expect(page.getByRole("dialog")).toBeVisible()
    await expect(page.getByTestId("board-task")).toHaveAttribute("aria-grabbed", "false")
  })

  test("shows pending, rolls back failed edits, and keeps transport details out of the DOM", async ({ page }) => {
    let release: (() => void) | null = null
    await wireBoard(page, { updateStatus: 503, updateBody: { error: { code: "internal", message: "SECRET transport detail" } } })
    await page.unroute("**/api/v1/**")
    await page.route("**/api/v1/**", async (route) => {
      const request = route.request()
      const url = new URL(request.url())
      if ((request.method() === "PATCH" || request.method() === "POST") && url.pathname.endsWith("/tasks/t_todo")) {
        await new Promise<void>((resolve) => { release = resolve })
        await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { code: "internal", message: "SECRET transport detail" } }) })
        return
      }
      await route.fallback()
    })
    const edit = page.getByTestId("task-edit-t_todo")
    await edit.click()
    await page.getByTestId("task-title-input").fill("Changed")
    await page.getByRole("button", { name: "保存" }).click()
    await expect(page.getByRole("button", { name: "正在保存…" })).toBeDisabled()
    release?.()
    await expect(page.getByTestId("mutation-notice")).toContainText("任务操作失败")
    await expect(page.getByTestId("mutation-notice")).not.toContainText("SECRET")
    await expect(page.getByTestId("board-task")).toContainText("Draft")
  })

  test("keeps the edit input open for a canonical conflict and offers an explicit retry", async ({ page }) => {
    await wireBoard(page, { updateStatus: 409, updateBody: { error: { code: "conflict", message: "SECRET conflict detail" } } })
    const edit = page.getByTestId("task-edit-t_todo")
    await edit.click()
    await page.getByTestId("task-title-input").fill("Concurrent edit")
    await page.getByRole("button", { name: "保存" }).click()
    await expect(page.getByTestId("mutation-notice")).toContainText(/任务已被其他操作更新|canonical 看板暂时无法重新读取/)
    await expect(page.getByTestId("mutation-notice")).not.toContainText("SECRET")
    await expect(page.getByTestId("task-title-input")).toHaveValue("Concurrent edit")
    await expect(page.getByTestId("mutation-retry")).toBeVisible()
  })

  test("retries create with one client task id and idempotency key", async ({ page }) => {
    await wireBoard(page)
    const bodies: Array<Record<string, unknown>> = []
    await page.route("**/api/v1/boards/default/tasks", async (route) => {
      bodies.push(JSON.parse(route.request().postData() ?? "{}") as Record<string, unknown>)
      await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { code: "internal", message: "SECRET create detail" } }) })
    })
    await page.getByTestId("task-create").click()
    await page.getByTestId("task-title-input").fill("New task")
    await page.getByRole("button", { name: "创建" }).click()
    await expect(page.getByTestId("mutation-retry")).toBeVisible()
    await page.getByTestId("mutation-retry").click()
    await expect.poll(() => bodies.length).toBe(2)
    expect(bodies[0]?.task_id).toMatch(/^t_/)
    expect(bodies[0]?.idempotency_key).toBe(`task.create:${bodies[0]?.task_id}`)
    expect(bodies[1]?.task_id).toBe(bodies[0]?.task_id)
    expect(bodies[1]?.idempotency_key).toBe(bodies[0]?.idempotency_key)
  })

  test("rejects external text/plain drops and performs keyboard movement through the action path", async ({ page }) => {
    await wireBoard(page)
    const target = page.getByTestId("board-drop-target-ready")
    await target.evaluate((node) => {
      const data = new DataTransfer()
      data.setData("text/plain", "t_todo")
      node.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer: data }))
    })
    await expect(page.getByTestId("task-drag-announcement")).toContainText("未识别")

    const card = page.getByTestId("board-task")
    await card.focus()
    await card.press("Space")
    await expect(card).toHaveAttribute("aria-grabbed", "true")
    const promote = page.waitForRequest((request) => request.url().includes("/transitions/promote"))
    await card.press("ArrowRight")
    await promote
  })
})
