import { constants } from "node:fs"
import { mkdir, open, rename } from "node:fs/promises"

import { expect, test, type APIRequestContext } from "@playwright/test"

const baseURL = process.env.KANBAN_RELEASE_BASE_URL
const runId = process.env.KANBAN_RELEASE_RUN_ID
const evidenceDirectory = process.env.KANBAN_RELEASE_EVIDENCE_DIR ?? "../../output/release"
if (baseURL === undefined || baseURL.length === 0) throw new Error("real-host release proof requires KANBAN_RELEASE_BASE_URL")
if (runId === undefined || runId.length === 0) throw new Error("real-host release proof requires KANBAN_RELEASE_RUN_ID")

let readyFlowCount = 0
const flowIds: string[] = []
const createdTaskIds: string[] = []
const createdTaskTitles: string[] = []
const createdTaskProjects: string[] = []
const createdTaskBoardSlugs: string[] = []
let firstFailure: string | null = null
let hostIdentityVerified = false

function markFlows(ids: readonly string[]): void {
  for (const id of ids) {
    if (flowIds.includes(id)) throw new Error(`release flow recorded more than once: ${id}`)
    flowIds.push(id)
    readyFlowCount += 1
  }
}

async function writeEvidence(fileName: string, value: unknown): Promise<void> {
  await mkdir(evidenceDirectory, { recursive: true })
  const destination = `${evidenceDirectory}/${fileName}`
  const temporary = `${destination}.${process.pid}.${Date.now()}.tmp`
  const handle = await open(
    temporary,
    constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW,
    0o600,
  )
  try {
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, "utf8")
    await handle.sync()
  } finally {
    await handle.close()
  }
  await rename(temporary, destination)
}

test.afterAll(async ({ request }, testInfo) => {
  void request
  await writeEvidence(`browser-${testInfo.project.name}.json`, {
    schema_version: 2,
    run_id: runId,
    real_host: hostIdentityVerified,
    base_url: baseURL,
    browser: testInfo.project.name,
    chromium: testInfo.project.name === "chromium" ? (firstFailure === null ? "passed" : "failed") : "not-applicable",
    firefox: testInfo.project.name === "firefox" ? (firstFailure === null ? "passed" : "failed") : "not-applicable",
    ready_flow_count: readyFlowCount,
    flow_ids: flowIds,
    created_task_ids: createdTaskIds,
    created_task_titles: createdTaskTitles,
    created_task_projects: createdTaskProjects,
    created_task_board_slugs: createdTaskBoardSlugs,
    failure: firstFailure,
  })
})

async function assertHostIdentity(request: APIRequestContext) {
  const healthResponse = await request.get(`${baseURL}/health`)
  expect(healthResponse.ok()).toBe(true)
  const health = await healthResponse.json() as { data?: { ok?: boolean; db?: string; version?: string; db_fingerprint?: string } }
  const healthVersion = health.data?.version
  expect(health.data?.ok).toBe(true)
  expect(health.data?.db).toBe("turso")
  expect(healthVersion).toMatch(/^\d+\.\d+\.\d+$/)
  expect(health.data?.db_fingerprint).toMatch(/^turso:/)

  const runtimeResponse = await request.get(`${baseURL}/app/runtime.json`)
  expect(runtimeResponse.ok()).toBe(true)
  expect(runtimeResponse.headers()["content-type"]).toContain("application/json")
  const runtime = await runtimeResponse.json() as { serverVersion?: string; protocolVersion?: string; webBuildId?: string; webBasePath?: string }
  expect(runtime.serverVersion).toBe(healthVersion)
  expect(runtime.protocolVersion).toBeTruthy()
  expect(runtime.webBasePath).toBe("/app/")

  const manifestResponse = await request.get(`${baseURL}/app/manifest.json`)
  expect(manifestResponse.ok()).toBe(true)
  const manifest = await manifestResponse.json() as { serverVersion?: string; protocolVersion?: string; buildId?: string; entrypoint?: string }
  expect(manifest.serverVersion).toBe(healthVersion)
  expect(manifest.protocolVersion).toBe(runtime.protocolVersion)
  expect(manifest.entrypoint).toBe("index.html")
  expect(manifest.buildId).toBe(runtime.webBuildId)

  const hostileHost = await request.get(`${baseURL}/health`, { headers: { Host: "evil.invalid" } })
  expect([400, 403]).toContain(hostileHost.status())
  const hostileOrigin = await request.get(`${baseURL}/health`, { headers: { Origin: "https://evil.invalid" } })
  if (hostileOrigin.status() === 200) expect(hostileOrigin.headers()["access-control-allow-origin"]).toBeUndefined()
  else expect([400, 403]).toContain(hostileOrigin.status())
  const entrypoint = await request.get(`${baseURL}/app/`)
  expect(entrypoint.headers()["content-security-policy"]).toContain("default-src 'self'")
  hostIdentityVerified = true
  return runtime
}

function taskApiPath(taskId: string, suffix = ""): string {
  return `${baseURL}/api/v1/tasks/${encodeURIComponent(taskId)}${suffix}`
}

async function canonicalGet<T>(request: APIRequestContext, path: string): Promise<T> {
  const response = await request.get(path)
  expect(response.ok()).toBe(true)
  return await response.json() as T
}

test.beforeEach(async ({ page }, testInfo) => {
  testInfo.attachments.push({ name: "lane", contentType: "text/plain", body: Buffer.from("real-kanban-serve") })
  page.on("pageerror", (error) => {
    if (firstFailure === null) firstFailure = `pageerror: ${error.message}`
  })
})

test("#shell.runtime real host runtime/manifest identity and negative origin checks", async ({ request }) => {
  // capability: shell.runtime
  await assertHostIdentity(request)
  markFlows(["shell.runtime"])
})

test("#task.collection #task.create #task.detail #events.view real UI create, persistent SSE catch-up and deep-link recovery", async ({ browser, page, request }, testInfo) => {
  // capabilities: task.collection, task.create, task.detail, events.view
  await assertHostIdentity(request)

  const contextB = await browser.newContext({ baseURL })
  const pageB = await contextB.newPage()
  pageB.on("pageerror", (error) => {
    if (firstFailure === null) firstFailure = `pageerror: ${error.message}`
  })

  try {
    await pageB.goto("/app/boards/default/events", { waitUntil: "domcontentloaded" })
    await expect(pageB.getByTestId("events-ready")).toBeVisible()
    await expect(pageB.getByTestId("event-row").filter({ hasText: "t_release_seed" })).toBeVisible()
    const eventRowsBeforeOffline = await pageB.getByTestId("event-row").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-event-id")))
    await contextB.setOffline(true)
    await expect(pageB.getByTestId("events-stale-notice")).toContainText("当前离线")

    // Page A remains online and performs the only canonical mutation through the UI.
    await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
    await expect(page.getByText("Seed release task", { exact: true })).toBeVisible()
    await page.getByTestId("task-create").click()
    await expect(page.getByTestId("task-mutation-dialog")).toBeVisible()
    const title = `Stage09 real task ${testInfo.project.name}`
    await page.getByTestId("task-title-input").fill(title)
    await page.getByTestId("task-mutation-dialog").getByRole("button", { name: "创建", exact: true }).click()
    await expect(page).toHaveURL(/\/app\/boards\/default\/board\?task=t_[^&]+$/)
    const taskId = new URL(page.url()).searchParams.get("task")
    expect(taskId).toMatch(/^t_[A-Za-z0-9_-]+$/)
    await expect(page.getByTestId("task-inspector")).toContainText(title)

    const taskResponse = await request.get(`${baseURL}/api/v1/tasks/${encodeURIComponent(taskId!)}`)
    expect(taskResponse.ok()).toBe(true)
    const taskBody = await taskResponse.json() as { data?: { id?: string; title?: string; board_slug?: string } }
    expect(taskBody.data?.id).toBe(taskId)
    expect(taskBody.data?.title).toBe(title)
    expect(taskBody.data?.board_slug).toBe("default")
    createdTaskIds.push(taskId!)
    createdTaskTitles.push(title)
    createdTaskProjects.push(testInfo.project.name)
    createdTaskBoardSlugs.push(taskBody.data?.board_slug ?? "")

    // The offline page must retain its pre-gap event window until reconnect.
    await expect.poll(() => pageB.getByTestId("event-row").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-event-id")))).toEqual(eventRowsBeforeOffline)
    await expect(pageB.getByTestId("event-row").filter({ hasText: taskId! })).toHaveCount(0)
    await contextB.setOffline(false)
    // Reconnect/catch-up is observed without page reload or an Events refresh click.
    await expect(pageB.getByTestId("event-row").filter({ hasText: taskId! })).toBeVisible()
    await pageB.getByTestId("nav-board").click()
    await expect(pageB).toHaveURL(/\/app\/boards\/default\/board$/)
    await expect(pageB.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
    await expect(pageB.getByTestId("board-task").filter({ hasText: title })).toBeVisible()
    await expect(pageB.getByTestId("board-sync-banner")).toHaveAttribute("data-sync-state", "live")

    // The launcher has already restarted kanban serve against the same DB; seed deep-link reload proves route recovery.
    await page.goto("/app/boards/default/board?task=t_release_seed", { waitUntil: "domcontentloaded" })
    await expect(page).toHaveURL(/\/app\/boards\/default\/board\?task=t_release_seed$/)
    await expect(page.getByTestId("task-inspector")).toContainText("Seed release task")
    await page.reload({ waitUntil: "domcontentloaded" })
    await expect(page).toHaveURL(/\/app\/boards\/default\/board\?task=t_release_seed$/)
    await expect(page.getByTestId("task-inspector")).toContainText("Seed release task")
    markFlows(["task.collection", "task.create", "task.detail", "events.view"])
  } finally {
    await contextB.close()
  }
})

test("#board.view #list.view #map.view #runs.view real read surfaces and selected-task deep links", async ({ page }, testInfo) => {
  // capabilities: board.view, list.view, map.view, runs.view
  const title = `Stage09 real task ${testInfo.project.name}`
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
  await expect(page.getByText("Seed release task", { exact: true })).toBeVisible()
  const taskCard = page.getByTestId("board-task").filter({ hasText: title }).first()
  await expect(taskCard).toBeVisible()
  const taskId = await taskCard.getAttribute("data-task-id")
  expect(taskId).toMatch(/^t_[A-Za-z0-9_-]+$/)

  await page.goto("/app/boards/default/list", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("task-list")).toBeVisible()
  await expect(page.getByTestId("task-row").filter({ hasText: title })).toBeVisible()

  await page.goto(`/app/boards/default/map?filter=all&task=${encodeURIComponent(taskId!)}`, { waitUntil: "domcontentloaded" })
  await expect(page).toHaveURL(new RegExp(`/app/boards/default/map\\?filter=all&task=${taskId}$`))
  await expect(page.getByTestId("task-map")).toBeVisible()
  await expect(page.getByTestId("task-map-node").filter({ hasText: title })).toBeVisible()

  await page.goto("/app/boards/default/runs?task=t_release_seed", { waitUntil: "domcontentloaded" })
  await expect(page).toHaveURL(/\/app\/boards\/default\/runs\?task=t_release_seed$/)
  await expect(page.getByTestId("runs-empty")).toBeVisible()
  markFlows(["board.view", "list.view", "map.view", "runs.view"])
})

test("#task.transition #task.comments #task.dependencies #task.steps #task.labels #task.attachments real Inspector mutations and canonical readback", async ({ page, request }, testInfo) => {
  // capabilities: task.transition, task.comments, task.dependencies, task.steps, task.labels, task.attachments
  const title = `Stage09 real task ${testInfo.project.name}`
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
  const taskCard = page.getByTestId("board-task").filter({ hasText: title }).first()
  await expect(taskCard).toBeVisible()
  const taskId = await taskCard.getAttribute("data-task-id")
  expect(taskId).toMatch(/^t_[A-Za-z0-9_-]+$/)

  await page.goto(`/app/boards/default/board?task=${encodeURIComponent(taskId!)}`, { waitUntil: "domcontentloaded" })
  await expect(page).toHaveURL(new RegExp(`/app/boards/default/board\\?task=${taskId}$`))
  await expect(page.getByTestId("task-inspector")).toBeVisible()
  await expect(page.getByTestId("task-inspector-relations")).toBeVisible()
  await expect(page.getByTestId("inspector-assets")).toBeVisible()

  const description = `Stage09 release proof description ${testInfo.project.name}`
  await expect(page.locator('button[data-action="specify"]')).toBeEnabled()
  await page.locator('button[data-action="specify"]').click()
  const actionDialog = page.getByRole("dialog")
  await expect(actionDialog).toBeVisible()
  await actionDialog.locator('textarea[name="action-description"]').fill(description)
  await actionDialog.getByRole("button", { name: "指定", exact: true }).click()
  await expect(actionDialog).toBeHidden()
  await expect(page.locator('button[data-action="promote"]')).toBeVisible()
  const transitionedTask = await canonicalGet<{ data?: { id?: string; status?: string; description?: string | null } }>(request, taskApiPath(taskId!))
  expect(transitionedTask.data?.id).toBe(taskId)
  expect(transitionedTask.data?.status).toBe("todo")
  expect(transitionedTask.data?.description).toBe(description)
  markFlows(["task.transition"])

  const commentBody = `Stage09 release proof comment ${testInfo.project.name}`
  const commentBodyInput = page.locator('textarea[name="comment-body"]')
  await page.locator('select[name="comment-kind"]').selectOption("note")
  await commentBodyInput.fill(commentBody)
  await page.getByRole("button", { name: "添加评论", exact: true }).click()
  await expect(commentBodyInput).toHaveValue("")
  await expect(page.getByTestId("task-inspector-comments").locator("ul li").filter({ hasText: commentBody })).toBeVisible()
  const comments = await canonicalGet<{ data?: readonly { task_id?: string; kind?: string; body?: string }[] }>(request, taskApiPath(taskId!, "/comments"))
  expect(comments.data).toEqual(expect.arrayContaining([
    expect.objectContaining({ task_id: taskId, kind: "note", body: commentBody }),
  ]))
  markFlows(["task.comments"])

  const stepTitle = `Stage09 release proof step ${testInfo.project.name}`
  const stepBody = `Step body ${testInfo.project.name}`
  await page.getByTestId("task-inspector-step-title").fill(stepTitle)
  await page.locator('textarea[name="step-body"]').fill(stepBody)
  await expect(page.getByTestId("task-inspector-create-step")).toBeEnabled()
  await page.getByTestId("task-inspector-create-step").click()
  await expect(page.getByTestId("task-inspector-steps")).toContainText(stepTitle)
  const steps = await canonicalGet<{ data?: { task_id?: string; steps?: readonly { title?: string; body?: string | null; required?: boolean }[] } }>(request, taskApiPath(taskId!, "/steps"))
  expect(steps.data?.task_id).toBe(taskId)
  expect(steps.data?.steps).toEqual(expect.arrayContaining([
    expect.objectContaining({ title: stepTitle, body: stepBody, required: true }),
  ]))
  markFlows(["task.steps"])

  const labelName = "Stage09 release"
  const labelInput = page.getByRole("textbox", { name: "标签名称" })
  await labelInput.fill(labelName)
  await page.getByTestId("label-add").click()
  await expect(page.getByTestId("inspector-labels")).toContainText(labelName)
  const taskLabels = await canonicalGet<{ data?: readonly { id?: string; board_id?: string; name?: string; color?: string | null }[] }>(request, taskApiPath(taskId!, "/labels"))
  expect(taskLabels.data).toEqual(expect.arrayContaining([
    expect.objectContaining({ name: labelName, color: "#4F46E5" }),
  ]))
  markFlows(["task.labels"])

  const attachmentName = `stage09-${testInfo.project.name}.txt`
  const attachmentBytes = Buffer.from(`Stage09 attachment bytes ${testInfo.project.name}\n`, "utf8")
  await page.getByTestId("attachment-file").setInputFiles({ name: attachmentName, mimeType: "text/plain", buffer: attachmentBytes })
  await page.getByTestId("attachment-upload").click()
  const attachmentRow = page.getByTestId("attachment-row").filter({ hasText: attachmentName })
  await expect(attachmentRow).toBeVisible()
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page.getByTestId("attachment-download").click(),
  ])
  expect(download.suggestedFilename()).toBe(attachmentName)
  const stream = await download.createReadStream()
  if (stream === null) throw new Error("attachment download did not expose a readable stream")
  const chunks: Buffer[] = []
  for await (const chunk of stream) chunks.push(Buffer.from(chunk))
  expect(Buffer.concat(chunks)).toEqual(attachmentBytes)
  const attachments = await canonicalGet<{ data?: readonly { task_id?: string; filename?: string; content_type?: string | null; size_bytes?: number }[] }>(request, taskApiPath(taskId!, "/attachments"))
  expect(attachments.data).toEqual(expect.arrayContaining([
    expect.objectContaining({ task_id: taskId, filename: attachmentName, content_type: "text/plain", size_bytes: attachmentBytes.byteLength }),
  ]))
  markFlows(["task.attachments"])

  await page.locator('input[name="dependency-parent"]').fill("t_release_seed")
  await page.getByRole("button", { name: "添加父依赖", exact: true }).click()
  await expect(page.getByTestId("task-inspector-dependencies")).toContainText("Seed release task")
  const dependencies = await canonicalGet<{ data?: { task?: { id?: string }; parents?: readonly { id?: string; title?: string }[]; edges?: readonly { parent?: { id?: string }; child?: { id?: string } }[] } }>(request, taskApiPath(taskId!, "/dependencies"))
  expect(dependencies.data?.task?.id).toBe(taskId)
  expect(dependencies.data?.parents).toEqual(expect.arrayContaining([
    expect.objectContaining({ id: "t_release_seed", title: "Seed release task" }),
  ]))
  expect(dependencies.data?.edges).toEqual(expect.arrayContaining([
    expect.objectContaining({ parent: expect.objectContaining({ id: "t_release_seed" }), child: expect.objectContaining({ id: taskId }) }),
  ]))
  markFlows(["task.dependencies"])
})

test("#signals.view #ontology.view #health.view #maintenance.view #settings.view real operator and feature read surfaces", async ({ page }) => {
  // capabilities: signals.view, ontology.view, health.view, maintenance.view, settings.view
  await page.goto("/app/boards/default/signals?status=all", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("signals-screen")).toBeVisible()
  await expect(page.getByText("没有返回信号。", { exact: true })).toBeVisible()

  await page.goto("/app/boards/default/ontology?include_all=true&group_by=label", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("ontology-screen")).toBeVisible()
  await expect(page.getByText("没有返回本体信号行。", { exact: true })).toBeVisible()

  await page.goto("/app/boards/default/health", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("health-page")).toBeVisible()
  await expect(page.getByTestId("health-metrics")).toBeVisible()

  await page.goto("/app/boards/default/maintenance", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("maintenance-page")).toBeVisible()

  await page.goto("/app/settings", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("settings-page")).toBeVisible()
  await expect(page.getByTestId("settings-connection")).toBeVisible()
  markFlows(["signals.view", "ontology.view", "health.view", "maintenance.view", "settings.view"])
})

test.afterEach(async ({ page }, testInfo) => {
  void page
  if (testInfo.status !== testInfo.expectedStatus && firstFailure === null) firstFailure = `${testInfo.title}: ${testInfo.status}`
})
