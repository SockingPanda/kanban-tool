import { mkdir, writeFile } from "node:fs/promises"

import { expect, test, type APIRequestContext } from "@playwright/test"

const baseURL = process.env.KANBAN_RELEASE_BASE_URL
const evidenceDirectory = process.env.KANBAN_RELEASE_EVIDENCE_DIR ?? "../../output/release"
if (baseURL === undefined || baseURL.length === 0) throw new Error("real-host release proof requires KANBAN_RELEASE_BASE_URL")

let readyFlowCount = 0
const flowIds: string[] = []
let firstFailure: string | null = null
let hostIdentityVerified = false

test.afterAll(async ({ request }, testInfo) => {
  void request
  await mkdir(evidenceDirectory, { recursive: true })
  await writeFile(
    `${evidenceDirectory}/browser-${testInfo.project.name}.json`,
    `${JSON.stringify({
      schema_version: 1,
      real_host: hostIdentityVerified,
      base_url: baseURL,
      browser: testInfo.project.name,
      chromium: testInfo.project.name === "chromium" ? (firstFailure === null ? "passed" : "failed") : "not-applicable",
      firefox: testInfo.project.name === "firefox" ? (firstFailure === null ? "passed" : "failed") : "not-applicable",
      ready_flow_count: readyFlowCount,
      flow_ids: flowIds,
      failure: firstFailure,
    }, null, 2)}\n`,
    "utf8",
  )
})

async function assertHostIdentity(request: APIRequestContext) {
  const healthResponse = await request.get(`${baseURL}/health`)
  expect(healthResponse.ok()).toBe(true)
  const health = await healthResponse.json() as { data?: { ok?: boolean; db?: string; version?: string; db_fingerprint?: string } }
  expect(health.data?.ok).toBe(true)
  expect(health.data?.db).toBe("turso")
  expect(health.data?.version).toBe("3.0.0")
  expect(health.data?.db_fingerprint).toMatch(/^turso:/)

  const runtimeResponse = await request.get(`${baseURL}/app/runtime.json`)
  expect(runtimeResponse.ok()).toBe(true)
  expect(runtimeResponse.headers()["content-type"]).toContain("application/json")
  const runtime = await runtimeResponse.json() as { serverVersion?: string; protocolVersion?: string; webBuildId?: string; webBasePath?: string }
  expect(runtime.serverVersion).toBe("3.0.0")
  expect(runtime.protocolVersion).toBe("v1")
  expect(runtime.webBasePath).toBe("/app/")

  const manifestResponse = await request.get(`${baseURL}/app/manifest.json`)
  expect(manifestResponse.ok()).toBe(true)
  const manifest = await manifestResponse.json() as { serverVersion?: string; protocolVersion?: string; buildId?: string; entrypoint?: string }
  expect(manifest.serverVersion).toBe(runtime.serverVersion)
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

test.beforeEach(async ({ page }, testInfo) => {
  testInfo.attachments.push({ name: "lane", contentType: "text/plain", body: Buffer.from("real-kanban-serve") })
  page.on("pageerror", (error) => {
    if (firstFailure === null) firstFailure = `pageerror: ${error.message}`
  })
})

test("#shell.runtime real host runtime/manifest identity and negative origin checks", async ({ request }) => {
  // capability: shell.runtime
  await assertHostIdentity(request)
  flowIds.push("shell.runtime")
  readyFlowCount += 1
})

test("#task.collection real canonical task create/read after launcher restart", async ({ page, request }, testInfo) => {
  // capability: task.collection
  await assertHostIdentity(request)
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready")
  await expect(page.getByText("Seed release task", { exact: true })).toBeVisible()
  await page.getByTestId("task-create").click()
  await expect(page.getByTestId("task-mutation-dialog")).toBeVisible()
  const title = `Stage09 real task ${testInfo.project.name}`
  await page.getByTestId("task-title-input").fill(title)
  await page.getByTestId("task-mutation-dialog").getByRole("button", { name: "创建", exact: true }).click()
  await expect(page).toHaveURL(/\/app\/boards\/default\/board\?task=t_[^&]+$/)
  await expect(page.getByTestId("task-inspector")).toContainText(title)
  const recoveredURL = page.url()
  // The launcher has already restarted kanban serve against the same DB; reload only proves route recovery.
  await page.reload({ waitUntil: "domcontentloaded" })
  await expect(page).toHaveURL(recoveredURL)
  await expect(page.getByTestId("task-inspector")).toContainText(title)
  flowIds.push("task.collection")
  readyFlowCount += 1
})

test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status !== testInfo.expectedStatus && firstFailure === null) firstFailure = `${testInfo.title}: ${testInfo.status}`
  void page
})
