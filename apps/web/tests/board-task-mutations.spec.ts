import { expect, test, type Page } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"

type TaskStatus = "todo" | "ready" | "running" | "blocked"

function task(status: TaskStatus, title = "Draft", boardId = "b_default", boardSlug = "default", id = "t_todo") {
  return {
    id,
    board_id: boardId,
    board_slug: boardSlug,
    ref: `${boardSlug}#${id === "t_todo" ? 1 : 2}`,
    seq: id === "t_todo" ? 1 : 2,
    title,
    description: "Draft specification",
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

function claimResponse(boardId = "b_default", boardSlug = "default") {
  return {
    data: {
      claim_token: "claim-a",
      claim_expires_at: 4102444800000,
      task: task("running", "Draft", boardId, boardSlug),
      run: {
        id: "r_todo",
        task_id: "t_todo",
        status: "running",
        worker_profile: "manual",
        worker_pid: null,
        claim_owner: "local",
        started_at: 1,
        finished_at: null,
        exit_code: null,
        summary: null,
        error: null,
        has_log: false,
        metadata: {},
      },
    },
  }
}

async function wireBoard(page: Page, options: {
  readonly updateStatus?: number
  readonly updateBody?: unknown
  readonly initialStatus?: TaskStatus
  readonly boardId?: string
  readonly boardSlug?: string
  readonly secondaryBoard?: { readonly boardId: string; readonly boardSlug: string; readonly status: TaskStatus }
  readonly secondTask?: { readonly taskId: string; readonly title?: string; readonly status?: TaskStatus }
} = {}) {
  await installRuntimeFixture(page)
  const boardId = options.boardId ?? "b_default"
  const boardSlug = options.boardSlug ?? "default"
  const boardSpecs = [
    { boardId, boardSlug, status: options.initialStatus ?? "todo" as TaskStatus },
    ...(options.secondaryBoard === undefined ? [] : [options.secondaryBoard]),
  ]
  let status: TaskStatus = options.initialStatus ?? "todo"
  await page.route("**/api/v1/stream/events**", (route) => route.fulfill({ status: 200, contentType: "text/event-stream", body: "" }))
  await page.route("**/api/v1/**", async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    if (request.method() === "POST" || request.method() === "PATCH") {
      if (url.pathname.endsWith("/tasks/t_todo")) {
        await route.fulfill({
          status: options.updateStatus ?? 200,
          contentType: "application/json",
          body: JSON.stringify(options.updateBody ?? { data: task(status, "Draft", boardId, boardSlug) }),
        })
        return
      }
      if (url.pathname.endsWith("/promote")) {
        status = "ready"
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: task("ready", "Draft", boardId, boardSlug) }) })
        return
      }
    }
    if (url.pathname === "/api/v1/boards") {
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: boardSpecs.map((spec) => ({ id: spec.boardId, slug: spec.boardSlug, name: spec.boardSlug === "default" ? "Default" : "Other", description: null, created_at: 1, updated_at: 2, archived_at: null })) }) })
      return
    }
    const columnsMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/columns$/)
    if (columnsMatch !== null) {
      const spec = boardSpecs.find((candidate) => candidate.boardSlug === columnsMatch[1])
      if (spec === undefined) { await route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "fixture" } }) }); return }
      const boardStatus = spec.boardSlug === boardSlug ? status : spec.status
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: [
        { id: "c_todo", board_id: spec.boardId, status: "todo", title: "Todo", position: 1, hidden: false, wip_limit: null, created_at: 1, updated_at: 2 },
        { id: "c_ready", board_id: spec.boardId, status: "ready", title: "Ready", position: 2, hidden: false, wip_limit: null, created_at: 1, updated_at: 2 },
        ...(boardStatus === "running" ? [{ id: "c_running", board_id: spec.boardId, status: "running", title: "Running", position: 3, hidden: false, wip_limit: null, created_at: 1, updated_at: 2 }] : []),
      ] }) })
      return
    }
    const tasksMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/tasks\/by-status$/)
    if (tasksMatch !== null) {
      const spec = boardSpecs.find((candidate) => candidate.boardSlug === tasksMatch[1])
      if (spec === undefined) { await route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "fixture" } }) }); return }
      const requested = url.searchParams.get("status")
      const boardStatus = spec.boardSlug === boardSlug ? status : spec.status
      const visible = requested === boardStatus
        ? [
            task(boardStatus, "Draft", spec.boardId, spec.boardSlug),
            ...(options.secondTask !== undefined && spec.boardSlug === boardSlug && (options.secondTask.status ?? "todo") === boardStatus
              ? [task(boardStatus, options.secondTask.title ?? "Second task", spec.boardId, spec.boardSlug, options.secondTask.taskId)]
              : []),
          ]
        : []
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: { statuses: [{ status: requested, tasks: visible, page: { limit: 1000, offset: 0, total: visible.length } }] }, meta: { limit: 1000, offset: 0 } }) })
      return
    }
    await route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "fixture" } }) })
  })
  await page.goto(`/app/boards/${boardSlug}/board`, { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("board-task").first()).toBeVisible()
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
    await expect(page.getByTestId("task-mutation-dialog").getByRole("button", { name: "正在保存…" })).toBeDisabled()
    release?.()
    release = null
    await expect(page.getByTestId("mutation-notice")).toContainText("任务操作失败")
    await expect(page.getByTestId("mutation-notice")).not.toContainText("SECRET")
    await expect(page.getByTestId("board-task")).toContainText("Draft")
    await page.getByTestId("mutation-retry").click()
    await expect(page.getByTestId("task-mutation-pending")).toBeAttached()
    await expect.poll(() => release !== null).toBe(true)
    release?.()
    await expect(page.getByTestId("mutation-retry")).toBeVisible()
  })

  test("settles a successful mutation while the app is mounted through StrictMode", async ({ page }) => {
    let release: (() => void) | null = null
    await wireBoard(page)
    await page.route("**/api/v1/tasks/t_todo", async (route) => {
      const request = route.request()
      const url = new URL(request.url())
      if (request.method() === "PATCH" && url.pathname.endsWith("/tasks/t_todo")) {
        await new Promise<void>((resolve) => { release = resolve })
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: task("todo", "Changed") }) })
        return
      }
      await route.fallback()
    })
    await page.getByTestId("task-edit-t_todo").click()
    await page.getByTestId("task-title-input").fill("Changed")
    await page.getByRole("button", { name: "保存" }).click()
    await expect(page.getByTestId("task-mutation-dialog").getByRole("button", { name: "正在保存…" })).toBeDisabled()
    release?.()
    await expect(page.getByRole("dialog")).not.toBeVisible()
    await expect(page.getByTestId("task-mutation-dialog").getByRole("button", { name: "正在保存…" })).toHaveCount(0)
  })

  test("accepts a legal internal pointer drag through the native drag lifecycle", async ({ page }) => {
    await wireBoard(page)
    const card = page.getByTestId("board-task")
    const target = page.getByTestId("board-drop-target-ready")
    const promote = page.waitForRequest((request) => request.url().endsWith("/api/v1/tasks/t_todo/transitions/promote"))
    await card.dragTo(target)
    await promote
    await expect(page.getByTestId("task-drag-announcement")).toContainText("移动任务")
  })

  test("serializes task mutations across cards and releases the next dialog after settle", async ({ page }) => {
    let release: (() => void) | null = null
    await wireBoard(page, { secondTask: { taskId: "t_other", title: "Second task" } })
    await page.route("**/api/v1/tasks/t_todo", async (route) => {
      if (route.request().method() !== "PATCH") {
        await route.fallback()
        return
      }
      await new Promise<void>((resolve) => { release = resolve })
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: task("todo", "Pending") }) })
    })

    await page.getByTestId("task-edit-t_todo").click()
    await page.getByTestId("task-title-input").fill("Pending")
    await page.getByRole("button", { name: "保存" }).click()
    await expect(page.getByTestId("task-mutation-pending")).toBeAttached()

    const secondEdit = page.getByTestId("task-edit-t_other")
    await expect(secondEdit).toBeDisabled()
    await secondEdit.dispatchEvent("click")
    await expect(page.getByTestId("task-title-input")).toHaveValue("Pending")

    release?.()
    await expect(page.getByRole("dialog")).not.toBeVisible()
    await secondEdit.click()
    await expect(page.getByTestId("task-title-input")).toHaveValue("Second task")
  })

  test("does not leak a deferred claim token across a board identity switch", async ({ page }) => {
    await wireBoard(page, { initialStatus: "ready", secondaryBoard: { boardId: "b_other", boardSlug: "other", status: "running" } })
    let releaseClaim: (() => void) | null = null
    let claimStarted: (() => void) | null = null
    const claimStartedPromise = new Promise<void>((resolve) => { claimStarted = resolve })
    await page.route("**/api/v1/tasks/t_todo/transitions/claim", async (route) => {
      claimStarted?.()
      await new Promise<void>((resolve) => { releaseClaim = resolve })
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(claimResponse()) })
    })
    await page.getByTestId("task-transition-claim-t_todo").click()
    await claimStartedPromise

    await page.goto("/app/boards/other/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByText("b_other · other")).toBeVisible()
    releaseClaim?.()
    await expect(page.getByTestId("task-transition-submit-review-t_todo")).toHaveCount(0)
  })

  test("keeps the edit input open for a canonical conflict and offers an explicit retry", async ({ page }) => {
    await wireBoard(page, { updateStatus: 409, updateBody: { error: { code: "conflict", message: "SECRET conflict detail" } } })
    await page.waitForTimeout(250)
    const updateBodies: Array<Record<string, unknown>> = []
    page.on("request", (request) => {
      if (request.method() === "PATCH" && request.url().endsWith("/api/v1/tasks/t_todo")) {
        updateBodies.push(JSON.parse(request.postData() ?? "{}") as Record<string, unknown>)
      }
    })
    const edit = page.getByTestId("task-edit-t_todo")
    await edit.click()
    await page.getByTestId("task-title-input").fill("Concurrent edit")
    await page.getByRole("button", { name: "保存" }).click()
    await expect(page.getByTestId("mutation-notice")).toContainText(/任务已被其他操作更新|canonical 看板暂时无法重新读取/)
    await expect(page.getByTestId("mutation-notice")).not.toContainText("SECRET")
    await expect(page.getByTestId("task-title-input")).toHaveValue("Concurrent edit")
    await expect(page.getByTestId("mutation-retry")).toBeVisible()
    await page.getByTestId("task-title-input").fill("Concurrent edit revised")
    await page.getByTestId("mutation-retry").click()
    await expect.poll(() => updateBodies.length).toBe(2)
    expect(updateBodies[1]?.title).toBe("Concurrent edit revised")
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

  test("retains a created task id when its first required step fails and retries only the step", async ({ page }) => {
    await wireBoard(page)
    const createBodies: Array<Record<string, unknown>> = []
    const stepBodies: Array<Record<string, unknown>> = []
    await page.route("**/api/v1/boards/default/tasks", async (route) => {
      const body = JSON.parse(route.request().postData() ?? "{}") as Record<string, unknown>
      createBodies.push(body)
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: task("todo", String(body.title ?? "Created task")) }) })
    })
    await page.route("**/api/v1/tasks/*/steps", async (route) => {
      const body = JSON.parse(route.request().postData() ?? "{}") as Record<string, unknown>
      stepBodies.push(body)
      if (stepBodies.length === 1) {
        await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { code: "internal", message: "SECRET step detail" } }) })
        return
      }
      const pathSegments = new URL(route.request().url()).pathname.split("/")
      const taskId = pathSegments[pathSegments.length - 2]
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            task_id: taskId,
            steps: [],
            execution_plan: { board_id: "b_default", task_id: taskId, state: "planned", reason: null, updated_by: "web-user", updated_at: 1 },
          },
        }),
      })
    })
    await page.getByTestId("task-create").click()
    await page.getByTestId("task-title-input").fill("Created with step")
    await page.getByTestId("task-description-input").fill("Description")
    await page.getByTestId("first-required-step-input").fill("Verify output")
    await page.getByRole("button", { name: "创建" }).click()
    await expect(page.getByTestId("mutation-retry")).toBeVisible()
    await expect(page.getByTestId("mutation-retry")).toHaveText("重试添加首个步骤")
    await page.getByTestId("mutation-retry").click()
    await expect.poll(() => stepBodies.length).toBe(2)
    expect(createBodies).toHaveLength(1)
    expect(createBodies[0]?.title).toBe("Created with step")
    expect(createBodies[0]?.description).toBe("Description")
    expect(stepBodies[0]?.title).toBe("Verify output")
    expect(stepBodies[1]?.title).toBe("Verify output")
    expect(stepBodies[1]?.idempotency_key).toBe(stepBodies[0]?.idempotency_key)
  })

  test("rejects external text/plain drops and performs keyboard movement through the action path", async ({ page }) => {
    await wireBoard(page)
    const target = page.getByTestId("board-drop-target-ready")
    const card = page.getByTestId("board-task")
    await card.focus()
    await card.press("Space")
    await expect(card).toHaveAttribute("aria-grabbed", "true")
    await target.evaluate((node) => {
      const data = new DataTransfer()
      data.setData("text/plain", "t_todo")
      node.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer: data }))
    })
    await expect(page.getByTestId("task-drag-announcement")).toContainText("未识别")

    const promote = page.waitForRequest((request) => request.url().includes("/transitions/promote"))
    await card.press("ArrowRight")
    await promote
  })
})
