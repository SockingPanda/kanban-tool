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

    const picker = page.getByTestId("project-picker")
    await expect(picker).toBeVisible()
    await fixture.alphaReadStarted

    const search = picker.getByRole("combobox")
    await expect(search).toBeVisible()
    await search.fill("Beta")
    const betaOption = picker.getByRole("option", { name: /Beta Board/ })
    await expect(betaOption).toBeVisible()
    await betaOption.click()

    await expect(page).toHaveURL(/\/app\/boards\/beta\/overview$/)
    await expect(page.getByTestId("project-overview")).toBeVisible()
    await page.getByTestId("project-overview-open-tasks").click()
    await expect(page).toHaveURL(/\/app\/boards\/beta\/board$/)
    const boardView = page.getByTestId("board-view")
    await expect(boardView).toBeVisible()
    await expect(boardView.getByRole("heading", { name: "Beta Board" })).toHaveCount(1)
    await expect(boardView).toHaveAttribute("data-board-id", "b_beta")
    await expect(boardView).toHaveAttribute("data-board-slug", "beta")
    await expect(page.getByTestId("board-task").filter({ hasText: "Beta ready task" })).toBeVisible()
    fixture.releaseAlphaReads()
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())))
    await expect(boardView).toHaveAttribute("data-board-id", "b_beta")
    await expect(boardView).toHaveAttribute("data-board-slug", "beta")
    await expect(page.getByTestId("board-task").filter({ hasText: "Alpha ready task" })).toHaveCount(0)

    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards/beta/columns"))).toBe(true)
    expect(fixture.apiRequests.some((request) => request.startsWith("/api/v1/boards/beta/tasks/by-status"))).toBe(true)
  })

  test("keeps mobile shell destinations reachable and confines board overflow to its track", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("board-view")).toBeVisible()
    await expect(page.getByTestId("resource-header-menu")).toBeVisible()
    const sidebar = page.getByTestId("projects-sidebar")
    await expect(sidebar).toHaveAttribute("data-open", "false")
    await expectNoPageOverflow(page)

    const boardTrack = page.getByTestId("board-view").locator('[role="region"][tabindex="0"]')
    await expect(boardTrack).toBeVisible()
    const trackWidths = await boardTrack.evaluate((element) => ({ client: element.clientWidth, scroll: element.scrollWidth }))
    expect(trackWidths.scroll).toBeGreaterThan(trackWidths.client)

    await page.getByTestId("resource-header-menu").click()
    await expect(sidebar).toHaveAttribute("data-open", "true")
    await expect(sidebar.getByTestId("project-tree-tasks")).toBeVisible()
    await sidebar.getByTestId("project-tree-overview").click()
    await expect(page.getByTestId("project-overview")).toBeVisible()
    await expect(sidebar).toHaveAttribute("data-open", "false")

    const destinations: readonly [string, string][] = [
      ["信号", "signals-screen"],
      ["本体", "ontology-screen"],
      ["健康", "health-page"],
      ["维护", "maintenance-page"],
    ]
    for (const [label, pageTestId] of destinations) {
      const more = page.getByTestId("resource-header").locator("details")
      if (await more.getAttribute("open") === null) await more.locator("summary").click()
      const item = more.getByRole("link", { name: label, exact: true })
      await expect(item).toBeVisible()
      await item.click()
      await expect(page.getByTestId(pageTestId)).toBeVisible()
      await expect(page.getByTestId("resource-header-menu")).toBeVisible()
      await expectNoPageOverflow(page)
    }

    // Mobile hides the product rail by contract; use the context drawer's Projects entry.
    await page.getByTestId("resource-header-menu").click()
    await sidebar.getByTestId("projects-sidebar-projects").click()
    await expect(page.getByTestId("projects-collection")).toBeVisible()
    await page.getByTestId("projects-collection-project-alpha").click()
    await page.getByTestId("project-overview-open-tasks").click()
    await expect(page).toHaveURL(/\/app\/boards\/alpha\/board$/)
    await expect(page.getByTestId("board-view")).toBeVisible()
  })

  test("keeps the global project picker available on a direct Settings route", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.goto("/app/settings", { waitUntil: "domcontentloaded" })

    await expect(page.getByTestId("settings-page")).toBeVisible()
    const picker = page.getByTestId("project-picker")
    const search = picker.getByRole("combobox")
    await search.fill("Beta")
    await picker.getByRole("option", { name: /Beta Board/ }).click()

    await expect(page).toHaveURL(/\/app\/boards\/beta\/overview$/)
    await page.getByTestId("project-overview-open-tasks").click()
    await expect(page).toHaveURL(/\/app\/boards\/beta\/board$/)
    const boardView = page.getByTestId("board-view")
    await expect(boardView).toHaveAttribute("data-board-id", "b_beta")
    await expect(boardView).toHaveAttribute("data-board-slug", "beta")
    await expect(page.getByTestId("board-task").filter({ hasText: "Beta ready task" })).toBeVisible()
  })

  test("searches canonical projects from the keyboard combobox and listbox", async ({ page }) => {
    await installAgentFirstFixture(page)
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    const picker = page.getByTestId("project-picker")
    const search = picker.getByRole("combobox")
    await search.focus()
    await search.fill("Beta")
    await expect(picker.getByRole("listbox")).toBeVisible()
    await expect(picker.getByRole("option", { name: /Beta Board/ })).toHaveCount(1)
    await search.press("Enter")
    await expect(page).toHaveURL(/\/app\/boards\/beta\/overview$/)

    await page.getByTestId("product-rail-projects").click()
    await page.getByTestId("projects-collection-project-alpha").click()
    await expect(page).toHaveURL(/\/app\/boards\/alpha\/overview$/)
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

  test("keeps an empty project snapshot at an explicit project boundary", async ({ page }) => {
    await installAgentFirstFixture(page, { emptyBoards: true })
    await page.goto("/app/boards/alpha/board", { waitUntil: "domcontentloaded" })

    const boundary = page.getByTestId("shell-project-not-found")
    await expect(boundary).toBeVisible()
    await expect(boundary.getByRole("heading", { name: "页面不存在" })).toBeVisible()
    await expect(page.getByTestId("project-picker-empty")).toBeVisible()
    await expect(page.getByTestId("board-view")).toHaveCount(0)
  })
})
