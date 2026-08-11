import { expect, test, type Page, type Route } from "@playwright/test"

import { installPersistentSse, installRuntimeFixture } from "./runtime-fixture"

const STATUSES = ["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"] as const
type TaskStatus = (typeof STATUSES)[number]

type BoardSeed = {
  readonly id: string
  readonly slug: string
  readonly name: string
  readonly taskTitle: string
}

const BOARDS: readonly BoardSeed[] = [
  { id: "b_alpha", slug: "alpha", name: "Alpha Board", taskTitle: "Alpha ready task" },
  { id: "b_beta", slug: "beta", name: "Beta Board", taskTitle: "Beta ready task" },
]

type AgentFirstFixtureOptions = {
  readonly emptyBoards?: boolean
  readonly deferAlphaReads?: boolean
}

type AgentFirstFixture = {
  readonly apiRequests: string[]
  readonly alphaReadStarted: Promise<void>
  readonly releaseAlphaReads: () => void
}

function boardForSlug(slug: string): BoardSeed | undefined {
  return BOARDS.find((board) => board.slug === slug)
}

function boardRows() {
  return BOARDS.map((board) => ({
    id: board.id,
    slug: board.slug,
    name: board.name,
    description: null,
    created_at: 1,
    updated_at: 2,
    archived_at: null,
  }))
}

function columnsFor(board: BoardSeed) {
  return STATUSES.map((status, index) => ({
    id: `col_${board.slug}_${status}`,
    board_id: board.id,
    status,
    title: status[0]?.toUpperCase() + status.slice(1),
    position: index + 1,
    hidden: status === "archived",
    wip_limit: null,
    created_at: 1,
    updated_at: 2,
  }))
}

function taskFor(board: BoardSeed, status: TaskStatus) {
  if (status !== "ready") return []
  return [{
    id: `t_${board.slug}_ready`,
    board_id: board.id,
    board_slug: board.slug,
    ref: `${board.slug}#1`,
    seq: 1,
    title: board.taskTitle,
    description: "Primary task context remains concise while secondary facts are available on demand.",
    status,
    status_reason: "Waiting for the next agent step",
    assignee: `agent-${board.slug}`,
    priority: 2,
    position: 1,
    scheduled_at: 1_710_000_000,
    due_at: 1_711_000_000,
    created_by: "playwright",
    created_at: 1,
    updated_at: 2,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: 1_710_500_000,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result: null,
    metadata: {},
    lock_version: 1,
    dependency_blocked: true,
    unfinished_parent_count: 2,
    execution_plan_state: "planned",
    required_step_count: 3,
    completed_required_step_count: 1,
    optional_step_count: 1,
    labels: [{
      id: `label_${board.slug}_agent`,
      board_id: board.id,
      name: "agent-first",
      color: "#6b7280",
      created_at: 1,
      updated_at: 2,
    }],
  }]
}

async function fulfillJson(route: Route, payload: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(payload),
  })
}

/**
 * Keep this lane source-backed: the mock only returns the active API contracts
 * used by the shell/board read path, while writes and feature APIs stay outside
 * this read-only acceptance surface.
 */
async function installAgentFirstFixture(page: Page, options: AgentFirstFixtureOptions = {}): Promise<AgentFirstFixture> {
  await installRuntimeFixture(page)
  await installPersistentSse(page)
  const apiRequests: string[] = []
  let alphaReadStartedResolve: () => void = () => undefined
  const alphaReadStarted = new Promise<void>((resolve) => {
    alphaReadStartedResolve = resolve
  })
  let releaseAlphaReads: () => void = () => undefined
  const alphaReadGate = new Promise<void>((resolve) => {
    releaseAlphaReads = resolve
  })
  if (options.deferAlphaReads !== true) {
    alphaReadStartedResolve()
    releaseAlphaReads()
  }
  let alphaReadObserved = false

  const waitForAlphaRelease = async (board: BoardSeed): Promise<void> => {
    if (options.deferAlphaReads !== true || board.slug !== "alpha") return
    if (!alphaReadObserved) {
      alphaReadObserved = true
      alphaReadStartedResolve()
    }
    await alphaReadGate
  }

  await page.route("**/health", async (route) => {
    await fulfillJson(route, {
      data: {
        ok: true,
        db: "fixture",
        version: "fixture",
        db_path: "fixture.db",
        db_fingerprint: "fixture-fingerprint",
      },
    })
  })

  await page.route("**/api/v1/**", async (route) => {
    const url = new URL(route.request().url())
    apiRequests.push(`${url.pathname}${url.search}`)

    if (url.pathname === "/api/v1/boards") {
      await fulfillJson(route, { data: options.emptyBoards ? [] : boardRows() })
      return
    }

    const columnsMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/columns$/)
    if (columnsMatch) {
      const board = boardForSlug(decodeURIComponent(columnsMatch[1] ?? ""))
      if (board === undefined) {
        await fulfillJson(route, { error: { code: "not_found", message: "fixture board not found" } }, 404)
        return
      }
      await waitForAlphaRelease(board)
      await fulfillJson(route, { data: columnsFor(board) })
      return
    }

    const tasksMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/tasks\/by-status$/)
    if (tasksMatch) {
      const board = boardForSlug(decodeURIComponent(tasksMatch[1] ?? ""))
      const status = url.searchParams.get("status") as TaskStatus | null
      const limit = Number(url.searchParams.get("limit") ?? 1000)
      const offset = Number(url.searchParams.get("offset") ?? 0)
      if (board === undefined || status === null || !STATUSES.includes(status)) {
        await fulfillJson(route, { error: { code: "not_found", message: "fixture task window not found" } }, 404)
        return
      }
      await waitForAlphaRelease(board)
      const tasks = offset === 0 ? taskFor(board, status) : []
      await fulfillJson(route, {
        data: {
          statuses: [{ status, tasks, page: { limit, offset, total: tasks.length } }],
        },
        meta: { limit, offset },
      })
      return
    }

    if (url.pathname === "/api/v1/events") {
      await fulfillJson(route, { data: [], meta: { next_after: 0 } })
      return
    }

    await fulfillJson(route, { error: { code: "not_found", message: "fixture route not found" } }, 404)
  })

  return { apiRequests, alphaReadStarted, releaseAlphaReads }
}

async function expectNoPageOverflow(page: Page): Promise<void> {
  const widths = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    document: document.documentElement.scrollWidth,
    body: document.body.scrollWidth,
  }))
  expect(widths.document).toBeLessThanOrEqual(widths.viewport)
  expect(widths.body).toBeLessThanOrEqual(widths.viewport)
}

test.describe("agent-first Plane shell and board acceptance", () => {
  test("switches between canonical boards without leaking the previous board task", async ({ page }) => {
    const fixture = await installAgentFirstFixture(page, { deferAlphaReads: true })
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("board-switcher")).toBeVisible()
    await fixture.alphaReadStarted

    const switcher = page.getByTestId("board-switcher-select")
    await expect(switcher).toBeVisible()
    await expect(page.getByTestId("board-switcher-search")).toBeVisible()
    await switcher.selectOption("beta")

    await expect(page).toHaveURL(/\/app\/boards\/beta\/board$/)
    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.getByRole("heading", { name: "Beta Board" })).toBeVisible()
    await expect(page.getByTestId("board-identity-slug")).toHaveText("beta")
    await expect(page.getByTestId("board-task").filter({ hasText: "Beta ready task" })).toBeVisible()
    fixture.releaseAlphaReads()
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())))
    await expect(page.getByTestId("board-identity-slug")).toHaveText("beta")
    await expect(page.getByTestId("board-task").filter({ hasText: "Alpha ready task" })).toHaveCount(0)

    const identityDetails = page.getByTestId("board-identity-details")
    await expect(identityDetails).not.toHaveAttribute("open", "")
    await identityDetails.locator("summary").press("Enter")
    await expect(identityDetails).toHaveAttribute("open", "")
    await expect(identityDetails).toContainText("b_beta")

    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards/beta/columns"))).toBe(true)
    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards/beta/tasks/by-status"))).toBe(true)
  })

  test("keeps every shell destination reachable at a narrow viewport and confines board overflow to its track", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.getByTestId("compact-app-nav")).toBeVisible()
    await expectNoPageOverflow(page)

    const boardTrack = page.getByTestId("board-view").locator('[role="region"][tabindex="0"]')
    await expect(boardTrack).toBeVisible()
    const trackWidths = await boardTrack.evaluate((element) => ({ client: element.clientWidth, scroll: element.scrollWidth }))
    expect(trackWidths.scroll).toBeGreaterThan(trackWidths.client)

    const destinations: readonly [string, string][] = [
      ["compact-nav-signals", "signals-screen"],
      ["compact-nav-ontology", "ontology-screen"],
      ["compact-nav-health", "health-page"],
      ["compact-nav-maintenance", "maintenance-page"],
      ["compact-nav-settings", "settings-page"],
    ]
    for (const [navTestId, pageTestId] of destinations) {
      await page.getByTestId(navTestId).click()
      await expect(page.getByTestId(pageTestId)).toBeVisible()
      await expect(page.getByTestId("compact-app-nav")).toBeVisible()
      await expectNoPageOverflow(page)
    }

    await page.getByTestId("compact-nav-board").click()
    await expect(page).toHaveURL(/\/app\/boards\/alpha\/board$/)
    await expect(page.getByTestId("board-view")).toBeVisible()
  })

  test("keeps the global board switcher available on a direct Settings route", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.goto("/app/settings", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    const switcher = page.getByTestId("board-switcher-select")
    await expect(switcher).toBeVisible()
    await switcher.selectOption("beta")

    await expect(page).toHaveURL(/\/app\/boards\/beta\/board$/)
    await expect(page.getByTestId("board-identity-slug")).toHaveText("beta")
    await expect(page.getByTestId("board-task").filter({ hasText: "Beta ready task" })).toBeVisible()
  })

  test("searches and groups current, recent, and all canonical projects from the keyboard", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    const switcher = page.getByTestId("board-switcher-select")
    await expect(switcher.locator('optgroup[label="当前项目"]')).toHaveCount(1)
    await expect(switcher.locator('optgroup[label="所有项目"]')).toHaveCount(1)

    const search = page.getByTestId("board-switcher-search")
    await search.focus()
    await search.fill("Beta")
    await expect(switcher.locator('option[value="beta"]')).toHaveCount(1)
    await switcher.selectOption("beta")
    await expect(page).toHaveURL(/\/app\/boards\/beta\/board$/)

    await page.getByTestId("board-switcher-search").fill("")
    await expect(page.getByTestId("board-switcher-select").locator('optgroup[label="最近项目"]')).toHaveCount(0)
    await page.getByTestId("board-switcher-select").selectOption("alpha")
    await expect(page).toHaveURL(/\/app\/boards\/alpha\/board$/)
    await expect(page.getByTestId("board-switcher-select").locator('optgroup[label="最近项目"]')).toHaveCount(1)
  })

  test("shows primary task facts first and expands secondary facts from the keyboard", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    const task = page.getByTestId("board-task").filter({ hasText: "Alpha ready task" })
    await expect(task).toBeVisible()
    await expect(task.getByRole("heading")).toContainText("Alpha ready task")
    await expect(task).toContainText("ready")
    await expect(task).toContainText("P2")

    const secondary = task.getByTestId("board-task-secondary")
    await expect(secondary).toBeVisible()
    await expect(secondary).not.toHaveAttribute("open", "")
    const disclosure = secondary.locator("summary")
    await expect(disclosure).toBeVisible()
    await disclosure.focus()
    await page.keyboard.press("Enter")
    await expect(secondary).toHaveAttribute("open", "")
    await expect(secondary.getByTestId("board-task-status-reason")).toBeVisible()
  })

  test("keeps a board-list empty response as a local retryable boundary", async ({ page }) => {
    await installAgentFirstFixture(page, { emptyBoards: true })
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    const boundary = page.getByTestId("board-error")
    await expect(boundary).toBeVisible()
    await expect(boundary.getByRole("heading", { name: "看板加载失败" })).toBeVisible()
    await expect(boundary.getByRole("button", { name: "重试" })).toBeVisible()
    await expect(page.getByTestId("board-switcher-empty")).toBeVisible()
    await expect(page.getByTestId("board-switcher-retry")).toBeVisible()
    await expect(page.getByTestId("board-view")).toHaveCount(0)
  })
})
