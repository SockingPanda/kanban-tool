import { fromBinary } from "@bufbuild/protobuf"
import { WatchQueriesRequestSchema, type QueryCursor } from "../src/generated/rpc/kanban/v1/query_pb"
import { installQueryProbe } from "./release-query-probe"
import { rpcRequest } from "./release-rpc"
import { constants } from "node:fs"
import { lstat, mkdir, open, rename, rm } from "node:fs/promises"
import { join } from "node:path"

import { expect, test, type APIRequestContext, type BrowserContext, type Locator, type Page } from "@playwright/test"

const baseURL = process.env.KANBAN_RELEASE_09D_BASE_URL
const runId = process.env.KANBAN_RELEASE_09D_RUN_ID
const evidenceDirectory = process.env.KANBAN_RELEASE_09D_EVIDENCE_DIR
const phase = process.env.KANBAN_RELEASE_09D_PHASE ?? "unspecified"
if (!baseURL) throw new Error("09D real-host proof requires KANBAN_RELEASE_09D_BASE_URL")
if (!runId) throw new Error("09D real-host proof requires KANBAN_RELEASE_09D_RUN_ID")
if (!evidenceDirectory) throw new Error("09D real-host proof requires KANBAN_RELEASE_09D_EVIDENCE_DIR")

type BrowserError = { readonly kind: "pageerror" | "crash"; readonly message: string }
type MetricSample = {
  readonly sample: number
  readonly lcp_ms: number
  readonly inp_ms: number
  readonly inp_interaction_id: number | null
  readonly cls: number
  readonly lcp_entry_count: number
  readonly cls_entry_count: number
  readonly observer_supported: boolean
}
type QuerySample = {
  readonly sample: number
  readonly task_id: string
  readonly mutation_to_event_ms: number
  readonly disconnected_catch_up: boolean
}

type CursorEvidence = { readonly epoch: string; readonly scope: string; readonly revision: string }
function cursorEvidence(cursor: QueryCursor | undefined): CursorEvidence | null { return cursor ? { epoch: cursor.epoch, scope: cursor.scope, revision: String(cursor.revision) } : null }

type QueryStreamRequest = {
  readonly client_query_id: string
  readonly url: string
  readonly at_ms: number
  readonly resume: CursorEvidence | null
}

type ConfirmedTaskCursor = {
  readonly cursor: string
  readonly event_id: string
}

type CanonicalTask = { readonly id: string; readonly status: string; readonly title: string }
type WindowQuery = { readonly page: number; readonly limit: number; readonly sort: string; readonly status: readonly string[]; readonly q: string }
type TaskWindowEvidence = {
  readonly view: "list" | "board"
  readonly url: string
  readonly query: WindowQuery
  readonly footer_text: string
  readonly displayed_count: number
  readonly displayed_total: number
  readonly pager_text: string
  readonly page: number
  readonly total_pages: number
  readonly previous_enabled: boolean
  readonly next_enabled: boolean
  readonly task_ids: readonly string[]
  readonly canonical: { readonly total: number; readonly offset: number; readonly limit: number; readonly tasks: readonly CanonicalTask[] }
  readonly columns: readonly BoardColumnEvidence[] | null
}

type BoardReadyEvidence = {
  readonly navigation_start_ms: number
  readonly ready_ms: number
  readonly budget_ms: number
  readonly within_budget: boolean
}

type BoardColumnEvidence = {
  readonly label: string
  readonly count_text: string
  readonly displayed_count: number
  readonly task_ids: readonly string[]
  readonly task_statuses: readonly string[]
}

type FunctionalUiEvidence = {
  readonly stats: { readonly text: string; readonly total: number; readonly canonical_total: number }
  readonly board_ready: BoardReadyEvidence
  readonly board_pages: readonly TaskWindowEvidence[]
  readonly list_pages: readonly TaskWindowEvidence[]
  readonly filter_sort: {
    readonly status_text: string
    readonly search_text: string
    readonly sort_text: string
    readonly pages: readonly TaskWindowEvidence[]
    readonly changed_sort_text: string
    readonly changed_sort: TaskWindowEvidence
    readonly cleared: TaskWindowEvidence
  }
}

type StressUiEvidence = FunctionalUiEvidence & {
  readonly page_size: {
    readonly options: readonly string[]
    readonly before_text: string
    readonly before_pages: readonly TaskWindowEvidence[]
    readonly after_text: string
    readonly after: TaskWindowEvidence
  }
  readonly map: {
    readonly node_count: number
    readonly edge_count: number
    readonly truncated: boolean
    readonly limit_nodes: number
    readonly zoom_before: number
    readonly zoom_after: number
  }
}

type FixtureSeedRecord = {
  readonly target: number
  readonly requested: number
  created: number
  final_total: number | null
  duration_ms: number
  readonly concurrency: number
}

type InteractionTiming = {
  readonly duration: number
  readonly interactionId: number | null
}

type UiMutation = {
  readonly taskId: string
  readonly mutationStarted: number
}

const browserErrors: BrowserError[] = []
const metricSamples: MetricSample[] = []
const querySamples: QuerySample[] = []
const taskCounts: Array<{ readonly target: number; readonly total: number }> = []
const fixtureSeedRecords: FixtureSeedRecord[] = []
const queryStreamRequests: QueryStreamRequest[] = []
let mapEvidence: {
  readonly node_count: number
  readonly edge_count: number
  readonly truncated: boolean
  readonly limit_nodes: number
  readonly ui_node_count: number
  readonly ui_edge_count: number
  readonly ui_truncated: boolean
  readonly zoom_before: number
  readonly zoom_after: number
} | null = null
const uiEvidence: { functional_2k: FunctionalUiEvidence | null; stress_5k: StressUiEvidence | null } = {
  functional_2k: null,
  stress_5k: null,
}
let fixtureSeedResponseErrors = 0
let firstFailure: string | null = null
let queryKeyPathPassed = false
let queryReconnectPassed = false
let queryStaleCleared = false
let queryReconnectEvidence: {
  readonly before_count: number
  readonly request_count_after_disconnect: number
  readonly request_at_ms: number | null
  readonly event_seen_at_ms: number | null
  readonly resume: CursorEvidence | null
  readonly confirmed_cursor: CursorEvidence | null
  readonly recovery_cursor: CursorEvidence | null
  readonly recovery_mode: "resume" | "snapshot" | null
  readonly confirmed_task_id: string | null
  readonly confirmed_event_id: string | null
} | null = null

const phaseTarget = phase.includes("5k") ? 5_000 : phase.includes("2k") ? 2_000 : null
const QUERY_P95_BUDGET_MS = 1_000
const QUERY_CATCH_UP_P95_BUDGET_MS = 3_000
const FIXTURE_CONCURRENCY = 16
const FIXTURE_TEST_TIMEOUT_MS = 30 * 60_000
const BOARD_READY_BUDGET_MS = 120_000
const UI_ACTION_TIMEOUT_MS = 120_000

function percentile(values: readonly number[], fraction: number): number {
  if (values.length === 0) throw new Error("cannot calculate percentile for empty sample set")
  const sorted = [...values].sort((left, right) => left - right)
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * fraction) - 1))
  return sorted[index]!
}

function summarize(values: readonly number[]) {
  if (values.length === 0) return null
  return {
    count: values.length,
    p50: percentile(values, 0.5),
    p95: percentile(values, 0.95),
    max: Math.max(...values),
    samples: [...values],
  }
}

function envNumber(name: string): number | null {
  const value = process.env[name]
  if (value === undefined || value.trim() === "") return null
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) || Number.isFinite(parsed) ? parsed : null
}

function hostEvidence() {
  const hostArgvTokens = (() => {
    const value = process.env.KANBAN_RELEASE_09D_HOST_ARGV_JSON
    if (!value) return null
    try { return JSON.parse(value) as unknown } catch { return null }
  })()
  return {
    run_id: runId,
    formal: process.env.KANBAN_RELEASE_09D_FORMAL === "true",
    base_url: baseURL,
    port: envNumber("KANBAN_RELEASE_09D_PORT"),
    start_sha: process.env.KANBAN_RELEASE_09D_START_SHA ?? null,
    start_clean: process.env.KANBAN_RELEASE_09D_START_CLEAN === "true",
    host_pid: envNumber("KANBAN_RELEASE_09D_HOST_PID"),
    host_starttime: process.env.KANBAN_RELEASE_09D_HOST_STARTTIME ?? null,
    host_argv: process.env.KANBAN_RELEASE_09D_HOST_ARGV ?? null,
    host_argv_tokens: hostArgvTokens,
    binary_sha256: process.env.KANBAN_RELEASE_09D_BINARY_SHA256 ?? null,
    artifact_build_id: process.env.KANBAN_RELEASE_09D_ARTIFACT_BUILD_ID ?? null,
    artifact_manifest_sha256: process.env.KANBAN_RELEASE_09D_ARTIFACT_MANIFEST_SHA256 ?? null,
    db_path: process.env.KANBAN_RELEASE_09D_DB_PATH ?? null,
    db_identity_before: process.env.KANBAN_RELEASE_09D_DB_IDENTITY_BEFORE ?? null,
  }
}

async function writeEvidence(fileName: string, value: unknown): Promise<void> {
  await mkdir(evidenceDirectory!, { recursive: true, mode: 0o700 })
  const destination = join(evidenceDirectory!, fileName)
  const temporary = `${destination}.${process.pid}.${Date.now()}.tmp`
  try {
    const existing = await lstat(destination)
    if (existing.isSymbolicLink() || !existing.isFile() || existing.nlink !== 1) {
      throw new Error("evidence destination is not a regular single-link file")
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  }
  const handle = await open(
    temporary,
    constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW,
    0o600,
  )
  try {
    const stat = await handle.stat()
    if (!stat.isFile() || stat.nlink !== 1) throw new Error("evidence temporary is not a single regular file")
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, "utf8")
    await handle.sync()
    await handle.close()
    await rename(temporary, destination)
    const written = await lstat(destination)
    if (written.isSymbolicLink() || !written.isFile() || written.nlink !== 1) {
      throw new Error("evidence destination became unsafe after atomic rename")
    }
  } catch (error) {
    await rm(temporary, { force: true }).catch(() => undefined)
    throw error
  } finally {
    await handle.close().catch(() => undefined)
  }
}

function attachBrowserErrors(page: Page, errors: BrowserError[]): void {
  page.on("pageerror", (error) => {
    const entry = { kind: "pageerror" as const, message: error.message }
    errors.push(entry)
    if (errors !== browserErrors) browserErrors.push(entry)
  })
  page.on("crash", () => {
    const entry = { kind: "crash" as const, message: "Playwright page crashed" }
    errors.push(entry)
    if (errors !== browserErrors) browserErrors.push(entry)
  })
}

async function listTaskTotal(request: APIRequestContext): Promise<number> {
  void request
  const response = await rpcRequest("ListTasks", { path: { board: "default" }, query: { limit: 1, offset: 0, sort: "seq" } })
  expect(response.ok()).toBe(true)
  const body = await response.json() as { meta?: { total?: unknown } }
  const total = body.meta?.total
  if (typeof total !== "number" || !Number.isSafeInteger(total) || total < 0) {
    throw new Error("task list response did not contain a safe integer total")
  }
  return total
}

async function confirmedTaskCursor(request: APIRequestContext, taskId: string): Promise<ConfirmedTaskCursor> {
  let after = 0
  let selected: { id: number; event_id: string } | null = null
  for (let page = 0; page < 100; page += 1) {
    const response = await rpcRequest("ListEvents", { query: { board: "default", after: after, limit: 100 } })
    expect(response.ok()).toBe(true)
    const body = await response.json() as {
      data?: Array<{ id?: unknown; event_id?: unknown; task_id?: unknown }>
      meta?: { next_after?: unknown }
    }
    for (const event of body.data ?? []) {
      if (event.task_id !== taskId || typeof event.id !== "number" || !Number.isSafeInteger(event.id) || typeof event.event_id !== "string") continue
      if (selected === null || event.id > selected.id) selected = { id: event.id, event_id: event.event_id }
    }
    const nextAfter = body.meta?.next_after
    if (typeof nextAfter !== "number" || !Number.isSafeInteger(nextAfter) || nextAfter <= after) break
    after = nextAfter
  }
  if (selected === null) throw new Error(`canonical event cursor was not found for ${taskId}`)
  return { cursor: String(selected.id), event_id: selected.event_id }
}

const listStatusOrder = ["running", "blocked", "review", "ready", "scheduled", "todo", "triage", "done", "archived"]
const boardColumns = [
  { label: "待开始", statuses: ["triage", "todo", "scheduled", "ready"] },
  { label: "进行中", statuses: ["running", "blocked"] },
  { label: "待验收", statuses: ["review"] },
  { label: "已完成", statuses: ["done", "archived"] },
]
const defaultWindowQuery: WindowQuery = { page: 1, limit: 100, sort: "updated_at", status: [], q: "" }

function queryFromUrl(url: string): WindowQuery {
  const params = new URL(url).searchParams
  return {
    page: Number(params.get("page") ?? 1), limit: Number(params.get("limit") ?? 100),
    sort: params.get("sort") ?? "updated_at", status: params.getAll("status"), q: params.get("q") ?? "",
  }
}

async function taskIds(locator: Locator): Promise<string[]> {
  return locator.evaluateAll(elements => elements.map(element => element.getAttribute("data-task-id") ?? ""))
}

async function canonicalWindow(query: WindowQuery): Promise<TaskWindowEvidence["canonical"]> {
  const response = await rpcRequest("ListTasks", {
    path: { board: "default" },
    query: { limit: query.limit, offset: (query.page - 1) * query.limit, sort: query.sort, status: [...query.status], q: query.q || null },
  })
  expect(response.ok()).toBe(true)
  const body = await response.json() as { data: CanonicalTask[]; meta: { total: number; offset: number; limit: number } }
  expect(Number.isSafeInteger(body.meta.total)).toBe(true)
  expect(body.meta.offset).toBe((query.page - 1) * query.limit)
  expect(body.meta.limit).toBe(query.limit)
  return { ...body.meta, tasks: body.data.map(({ id, status, title }) => ({ id, status, title })) }
}

async function taskWindow(page: Page, view: "list" | "board", query: WindowQuery, total?: number): Promise<TaskWindowEvidence> {
  await expect.poll(() => queryFromUrl(page.url()), { timeout: UI_ACTION_TIMEOUT_MS }).toEqual(query)
  const canonical = await canonicalWindow(query)
  if (total !== undefined) expect(canonical.total).toBe(total)
  const ids = canonical.tasks.map(task => task.id)
  expect(ids.length).toBe(Math.min(query.limit, Math.max(0, canonical.total - canonical.offset)))
  expect(new Set(ids).size).toBe(ids.length)
  const rows = page.getByTestId(view === "list" ? "task-row" : "board-task")
  // Atlas 列表按状态分组；每组内保留 canonical 排序，看板按展示列分组。
  const expectedIds = view === "list"
    ? listStatusOrder.flatMap(status => canonical.tasks.filter(task => task.status === status).map(task => task.id))
    : boardColumns.flatMap(column => canonical.tasks.filter(task => column.statuses.includes(task.status)).map(task => task.id))
  await expect.poll(async () => {
    const rendered = await taskIds(rows)
    return view === "list" ? rendered : rendered.sort()
  }, { timeout: UI_ACTION_TIMEOUT_MS }).toEqual(view === "list" ? expectedIds : [...expectedIds].sort())
  const footer = page.locator(".table-footer")
  await expect(footer).toContainText(`显示 ${ids.length} / ${canonical.total} 个任务`)
  const footerText = (await footer.innerText()).trim()
  const counts = footerText.match(/^显示 (\d+) \/ (\d+) 个任务/)
  if (!counts) throw new Error(`Atlas footer did not expose displayed count and total: ${footerText}`)
  const pager = page.locator(".paper-pagination > span").last()
  const totalPages = Math.max(1, Math.ceil(canonical.total / query.limit))
  await expect(pager).toHaveText(`${query.page} / ${totalPages}`)
  const pagerText = (await pager.innerText()).trim()
  const pageNumbers = pagerText.match(/^(\d+) \/ (\d+)$/)
  if (!pageNumbers) throw new Error(`Atlas pager did not expose its current page: ${pagerText}`)
  const previous = page.getByRole("button", { name: "上一页", exact: true })
  const next = page.getByRole("button", { name: "下一页", exact: true })
  expect(await previous.isEnabled()).toBe(query.page > 1)
  expect(await next.isEnabled()).toBe(query.page < totalPages)
  const columns: BoardColumnEvidence[] = []
  if (view === "board") {
    await expect(page.locator(".board-column")).toHaveCount(boardColumns.length)
    for (const expected of boardColumns) {
      const column = page.getByRole("region", { name: expected.label, exact: true })
      const cards = column.getByTestId("board-task")
      const expectedTasks = canonical.tasks.filter(task => expected.statuses.includes(task.status))
      const countText = (await column.locator("header > span").innerText()).trim()
      const columnIds = await taskIds(cards)
      const statuses = await cards.evaluateAll(elements => elements.map(element => element.getAttribute("data-status") ?? ""))
      expect(countText).toBe(String(expectedTasks.length))
      expect([...columnIds].sort()).toEqual(expectedTasks.map(task => task.id).sort())
      expect(statuses).toEqual(columnIds.map(id => expectedTasks.find(task => task.id === id)!.status))
      columns.push({ label: await column.getAttribute("aria-label") ?? "", count_text: countText, displayed_count: Number(countText), task_ids: columnIds, task_statuses: statuses })
    }
  }
  return {
    view, url: page.url(), query: queryFromUrl(page.url()), footer_text: footerText,
    displayed_count: Number(counts[1]), displayed_total: Number(counts[2]),
    pager_text: pagerText, page: Number(pageNumbers[1]), total_pages: Number(pageNumbers[2]),
    previous_enabled: await previous.isEnabled(), next_enabled: await next.isEnabled(),
    task_ids: await taskIds(rows), canonical, columns: view === "board" ? columns : null,
  }
}

async function selectChoice(page: Page, label: string, option: string): Promise<void> {
  const control = page.getByRole("combobox", { name: label, exact: true })
  if (label === "排序" || label === "每页") {
    const filters = page.locator(".task-extra-filters")
    if (!await filters.evaluate(element => element.hasAttribute("open"))) await filters.locator("summary").click()
  }
  await control.click()
  await page.getByRole("option", { name: option, exact: true }).click()
  await expect(control).toContainText(option)
}

async function nextWindow(page: Page, view: "list" | "board", previous: TaskWindowEvidence): Promise<TaskWindowEvidence> {
  await page.getByRole("button", { name: "下一页", exact: true }).click()
  const next = await taskWindow(page, view, { ...previous.query, page: previous.page + 1 }, previous.displayed_total)
  expect(next.task_ids.filter(id => previous.task_ids.includes(id))).toEqual([])
  return next
}

async function loadUiEvidence(page: Page, total: number): Promise<FunctionalUiEvidence> {
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  const ready = await assertBoardReady(page)
  expect(ready.within_budget).toBe(true)
  const statsResponse = await rpcRequest("GetStats", { query: { board: "default" } })
  expect(statsResponse.ok()).toBe(true)
  const statsBody = await statsResponse.json() as { data: { status_counts: Array<{ status: string; count: number }> } }
  const canonicalTotal = statsBody.data.status_counts.reduce((sum, row) => sum + row.count, 0)
  expect(canonicalTotal).toBe(total)
  const statsElement = page.locator(".page-inline-stats > span").first()
  await expect(statsElement).toHaveText(`${total} 个任务`)
  const statsText = (await statsElement.innerText()).trim()
  const statsTotal = Number(statsText.match(/^(\d+)\s+个任务$/)?.[1])
  expect(statsTotal).toBe(total)
  const firstBoard = await taskWindow(page, "board", defaultWindowQuery, total)
  const secondBoard = await nextWindow(page, "board", firstBoard)
  // 返回也必须恢复同一 canonical 窗口，不能只证明页码变了。
  await page.getByRole("button", { name: "上一页", exact: true }).click()
  expect((await taskWindow(page, "board", defaultWindowQuery, total)).task_ids).toEqual(firstBoard.task_ids)
  await page.getByRole("button", { name: "列表", exact: true }).click()
  const listPages = [await taskWindow(page, "list", defaultWindowQuery, total)]
  for (let number = 2; number <= Math.ceil(total / 100); number += 1) {
    listPages.push(await nextWindow(page, "list", listPages.at(-1)!))
  }
  const allIds = listPages.flatMap(window => window.task_ids)
  expect(allIds).toHaveLength(total)
  expect(new Set(allIds).size).toBe(total)
  expect(new Set(allIds)).toEqual(new Set(listPages.flatMap(window => window.canonical.tasks.map(task => task.id))))
  await selectChoice(page, "按状态筛选任务", "待开始")
  await page.getByTestId("list-search").fill("load task 1")
  await selectChoice(page, "排序", "标题")
  const filteredQuery: WindowQuery = { ...defaultWindowQuery, status: ["todo"], q: "load task 1", sort: "title" }
  const firstFiltered = await taskWindow(page, "list", filteredQuery, 1_000)
  const secondFiltered = await nextWindow(page, "list", firstFiltered)
  expect(firstFiltered.canonical.tasks.every(task => task.status === "todo" && task.title.includes("load task 1"))).toBe(true)
  const statusText = await page.getByRole("combobox", { name: "按状态筛选任务", exact: true }).innerText()
  const searchText = await page.getByTestId("list-search").inputValue()
  const sortText = await page.getByRole("combobox", { name: "排序", exact: true }).innerText()
  await selectChoice(page, "排序", "最近更新")
  const changedSort = await taskWindow(page, "list", { ...filteredQuery, sort: "updated_at" }, 1_000)
  expect(changedSort.task_ids).not.toEqual(firstFiltered.task_ids)
  const changedSortText = await page.getByRole("combobox", { name: "排序", exact: true }).innerText()
  await page.getByTestId("list-search").fill("")
  await selectChoice(page, "按状态筛选任务", "所有状态")
  const cleared = await taskWindow(page, "list", defaultWindowQuery, total)
  expect(cleared.task_ids).toEqual(listPages[0]!.task_ids)
  return {
    stats: { text: statsText, total: statsTotal, canonical_total: canonicalTotal },
    board_ready: ready, board_pages: [firstBoard, secondBoard], list_pages: listPages,
    filter_sort: { status_text: statusText, search_text: searchText, sort_text: sortText, pages: [firstFiltered, secondFiltered], changed_sort_text: changedSortText, changed_sort: changedSort, cleared },
  }
}

async function createTaskBatch(request: APIRequestContext, ids: readonly string[]): Promise<number> {
  let created = 0
  for (let offset = 0; offset < ids.length; offset += FIXTURE_CONCURRENCY) {
    const batch = ids.slice(offset, offset + FIXTURE_CONCURRENCY)
    await Promise.all(batch.map(async (taskId) => {
      try {
        const response = await rpcRequest("CreateTask", { path: { board: "default" }, input: {
            task_id: taskId,
            idempotency_key: `release-09d:create:${taskId}`,
            title: `Stage09 09D load task ${taskId.slice(-4)}`,
            description: "Deterministic Stage09 performance fixture.",
            status: "todo",
            priority: 1,
            metadata: { fixture: "stage09-09d", task_id: taskId },
            labels: [],
            depends_on: [],
            actor: "release-09d",
          } })
        if (![200, 201].includes(response.status())) {
          fixtureSeedResponseErrors += 1
          throw new Error(`fixture task ${taskId} response status ${response.status()}`)
        }
        created += 1
      } catch (error) {
        if (!(error instanceof Error && error.message.startsWith("fixture task ") && error.message.includes("response status"))) {
          fixtureSeedResponseErrors += 1
        }
        throw error
      }
    }))
  }
  return created
}

async function ensureTaskTotal(request: APIRequestContext, target: number): Promise<number> {
  const started = Date.now()
  const current = await listTaskTotal(request)
  if (current > target) throw new Error(`fixture already contains ${current} tasks; target is ${target}`)
  const ids = Array.from({ length: target - current }, (_, index) => {
    const sequence = String(current + index).padStart(4, "0")
    return `t_release_09d_${sequence}`
  })
  const record: FixtureSeedRecord = {
    target,
    requested: ids.length,
    created: 0,
    final_total: null,
    duration_ms: 0,
    concurrency: FIXTURE_CONCURRENCY,
  }
  fixtureSeedRecords.push(record)
  try {
    record.created = await createTaskBatch(request, ids)
    const total = await listTaskTotal(request)
    record.final_total = total
    if (total !== target) throw new Error(`fixture task total ${total} does not equal target ${target}`)
    taskCounts.push({ target, total })
    return total
  } finally {
    record.duration_ms = Date.now() - started
  }
}

async function assertBoardReady(page: Page, timeout = BOARD_READY_BUDGET_MS): Promise<BoardReadyEvidence> {
  await expect(page.getByTestId("board-view")).toHaveAttribute("data-state", "ready", { timeout })
  const timing = await page.evaluate(() => {
    const navigation = performance.getEntriesByType("navigation")[0]
    const navigationStartMs = navigation?.startTime ?? 0
    return { navigationStartMs, readyMs: performance.now() - navigationStartMs }
  })
  return {
    navigation_start_ms: timing.navigationStartMs,
    ready_ms: timing.readyMs,
    budget_ms: BOARD_READY_BUDGET_MS,
    within_budget: timing.readyMs <= BOARD_READY_BUDGET_MS,
  }
}

async function installBufferedVitals(context: BrowserContext): Promise<void> {
  await context.addInitScript(() => {
    type VitalsState = {
      readonly lcpSupported: boolean
      readonly clsSupported: boolean
      readonly lcpEntries: PerformanceEntry[]
      readonly clsEntries: PerformanceEntry[]
      readonly lcpObserver: PerformanceObserver | null
      readonly clsObserver: PerformanceObserver | null
    }
    type VitalsWindow = Window & { __release09dVitals?: VitalsState }
    const target = window as VitalsWindow
    const supported = PerformanceObserver.supportedEntryTypes
    const lcpSupported = supported.includes("largest-contentful-paint")
    const clsSupported = supported.includes("layout-shift")
    if (!lcpSupported || !clsSupported) {
      target.__release09dVitals = {
        lcpSupported,
        clsSupported,
        lcpEntries: [],
        clsEntries: [],
        lcpObserver: null,
        clsObserver: null,
      }
      return
    }
    const lcpEntries: PerformanceEntry[] = []
    const clsEntries: PerformanceEntry[] = []
    const lcpObserver = new PerformanceObserver((list) => lcpEntries.push(...list.getEntries()))
    const clsObserver = new PerformanceObserver((list) => clsEntries.push(...list.getEntries()))
    lcpObserver.observe({ type: "largest-contentful-paint", buffered: true })
    clsObserver.observe({ type: "layout-shift", buffered: true })
    target.__release09dVitals = { lcpSupported, clsSupported, lcpEntries, clsEntries, lcpObserver, clsObserver }
  })
}

async function readBufferedVitals(page: Page): Promise<{
  readonly lcp: number | null
  readonly cls: number | null
  readonly lcpSupported: boolean
  readonly clsSupported: boolean
  readonly lcpEntryCount: number
  readonly clsEntryCount: number
}> {
  return page.evaluate(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
    type VitalsState = {
      readonly lcpSupported: boolean
      readonly clsSupported: boolean
      readonly lcpEntries: PerformanceEntry[]
      readonly clsEntries: PerformanceEntry[]
      readonly lcpObserver: PerformanceObserver | null
      readonly clsObserver: PerformanceObserver | null
    }
    type VitalsWindow = Window & { __release09dVitals?: VitalsState }
    const state = (window as VitalsWindow).__release09dVitals
    state?.lcpObserver?.disconnect()
    state?.clsObserver?.disconnect()
    if (!state) return { lcp: null, cls: null, lcpSupported: false, clsSupported: false, lcpEntryCount: 0, clsEntryCount: 0 }
    const lcp = state.lcpEntries.at(-1)?.startTime ?? null
    const cls = state.clsEntries.reduce((sum, entry) => {
      const value = entry as PerformanceEntry & { readonly value?: number; readonly hadRecentInput?: boolean }
      return sum + (value.hadRecentInput ? 0 : (value.value ?? 0))
    }, 0)
    return {
      lcp,
      cls,
      lcpSupported: state.lcpSupported,
      clsSupported: state.clsSupported,
      lcpEntryCount: state.lcpEntries.length,
      clsEntryCount: state.clsEntries.length,
    }
  })
}

async function installInteractionObserver(page: Page): Promise<void> {
  await page.evaluate(() => {
    type EventTimingEntry = PerformanceEntry & { readonly interactionId?: number }
    type EventTimingState = {
      readonly supported: boolean
      readonly entries: Array<{ readonly duration: number; readonly interactionId: number | null }>
      readonly observer: PerformanceObserver | null
    }
    type EventTimingWindow = Window & { __release09dEventTiming?: EventTimingState }
    const target = window as EventTimingWindow
    if (!PerformanceObserver.supportedEntryTypes.includes("event")) {
      target.__release09dEventTiming = { supported: false, entries: [], observer: null }
      return
    }
    const entries: Array<{ readonly duration: number; readonly interactionId: number | null }> = []
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        const eventTiming = entry as EventTimingEntry
        entries.push({
          duration: eventTiming.duration,
          interactionId: typeof eventTiming.interactionId === "number" && Number.isFinite(eventTiming.interactionId)
            ? eventTiming.interactionId
            : null,
        })
      }
    })
    observer.observe({ type: "event", buffered: false, durationThreshold: 16 } as PerformanceObserverInit)
    target.__release09dEventTiming = { supported: true, entries, observer }
  })
}

async function readInteractionDuration(page: Page): Promise<InteractionTiming | null> {
  return page.evaluate(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
    type EventTimingState = {
      readonly supported: boolean
      readonly entries: Array<{ readonly duration: number; readonly interactionId: number | null }>
      readonly observer: PerformanceObserver | null
    }
    type EventTimingWindow = Window & { __release09dEventTiming?: EventTimingState }
    const state = (window as EventTimingWindow).__release09dEventTiming
    state?.observer?.disconnect()
    const interactions = state?.entries.filter((entry) => entry.interactionId !== null && entry.interactionId > 0) ?? []
    if (!state?.supported || interactions.length === 0) return null
    const entry = interactions.reduce((longest, candidate) => (
      candidate.duration > longest.duration ? candidate : longest
    ))
    return { duration: entry.duration, interactionId: entry.interactionId }
  })
}

async function createTaskFromUi(page: Page, title: string): Promise<UiMutation> {
  await page.getByTestId("task-create").click()
  const dialog = page.getByTestId("task-mutation-dialog")
  await expect(dialog).toBeVisible()
  await page.getByTestId("task-title-input").fill(title)
  await page.evaluate(() => {
    type MutationClockWindow = Window & { __release09dSubmitClock?: number | null }
    const target = window as MutationClockWindow
    target.__release09dSubmitClock = null
    document.addEventListener("click", (event) => {
      const element = event.target instanceof Element ? event.target : null
      const submit = element?.closest("button[type=submit]")
      if (submit !== null && submit !== undefined && submit.closest('[data-testid="task-mutation-dialog"]') !== null) {
        target.__release09dSubmitClock = performance.timeOrigin + performance.now()
      }
    }, { capture: true, once: true })
  })
  await dialog.getByRole("button", { name: "创建任务", exact: true }).click()
  const mutationStarted = await page.evaluate(() => {
    type MutationClockWindow = Window & { __release09dSubmitClock?: number | null }
    const value = (window as MutationClockWindow).__release09dSubmitClock
    if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("submit click timestamp was not captured")
    return value
  })
  await expect(page).toHaveURL(/\/app\/boards\/default\/board\?task=t_[^&]+$/)
  const taskId = new URL(page.url()).searchParams.get("task")
  if (!taskId) throw new Error("UI task creation did not produce a canonical task id")
  return { taskId, mutationStarted }
}

async function eventVisibleAt(page: Page, taskId: string): Promise<number> {
  await expect(page.getByTestId("event-row").filter({ hasText: taskId })).toBeVisible({ timeout: 120_000 })
  return page.evaluate((id) => {
    const row = Array.from(document.querySelectorAll<HTMLElement>('[data-testid="event-row"]'))
      .find((candidate) => candidate.textContent?.includes(id))
    if (!row) throw new Error(`event row disappeared before timestamp capture: ${id}`)
    return performance.timeOrigin + performance.now()
  }, taskId)
}

async function waitForOfflineNotice(page: Page): Promise<void> {
  await expect.poll(async () => (
    await page.getByTestId("events-stale-notice").count()
    + await page.getByTestId("events-offline").count()
  )).toBeGreaterThan(0)
}

test.afterAll(async ({ request }, testInfo) => {
  let finalTotal: number | null = null
  try {
    finalTotal = await listTaskTotal(request)
  } catch (error) {
    firstFailure ??= `final task count failed: ${error instanceof Error ? error.message : String(error)}`
  }
  const initialBrotliFiles = (() => {
    const value = process.env.KANBAN_RELEASE_09D_INITIAL_BROTLI_FILES
    if (!value) return null
    try { return JSON.parse(value) as unknown } catch { return null }
  })()
  const metricSummary = metricSamples.length === 0 ? null : {
    lcp_ms: summarize(metricSamples.map((sample) => sample.lcp_ms)),
    inp_ms: summarize(metricSamples.map((sample) => sample.inp_ms)),
    cls: summarize(metricSamples.map((sample) => sample.cls)),
    samples: metricSamples,
  }
  const normalQueryLatencies = querySamples
    .filter((sample) => !sample.disconnected_catch_up)
    .map((sample) => sample.mutation_to_event_ms)
  const catchUpQueryLatencies = querySamples
    .filter((sample) => sample.disconnected_catch_up)
    .map((sample) => sample.mutation_to_event_ms)
  const querySummary = querySamples.length === 0 ? null : {
    latency_ms: summarize(querySamples.map((sample) => sample.mutation_to_event_ms)),
    mutation_to_event_ms: summarize(normalQueryLatencies),
    disconnected_catch_up_ms: summarize(catchUpQueryLatencies),
    samples: querySamples,
  }
  const initialBrotliBytes = Number(process.env.KANBAN_RELEASE_09D_INITIAL_BROTLI_BYTES)
  const initialBrotliWithinBudget = Number.isSafeInteger(initialBrotliBytes)
    && initialBrotliBytes >= 0
    && initialBrotliBytes <= 750 * 1024
  const noBrowserErrors = browserErrors.length === 0
  const performanceThresholdsPassed = metricSummary !== null
    && metricSamples.length === 20
    && (metricSummary.lcp_ms?.p95 ?? Number.POSITIVE_INFINITY) <= 2_000
    && (metricSummary.inp_ms?.p95 ?? Number.POSITIVE_INFINITY) <= 200
    && (metricSummary.cls?.p95 ?? Number.POSITIVE_INFINITY) <= 0.1
    && initialBrotliWithinBudget
    && noBrowserErrors
    && firstFailure === null
  const queryLatencyBudgetStatus = querySummary === null
    ? "not_run"
    : testInfo.project.name !== "chromium"
      ? "not_applicable"
      : (querySummary.mutation_to_event_ms?.p95 ?? Number.POSITIVE_INFINITY) <= QUERY_P95_BUDGET_MS
        && (querySummary.disconnected_catch_up_ms?.p95 ?? Number.POSITIVE_INFINITY) <= QUERY_CATCH_UP_P95_BUDGET_MS
        && noBrowserErrors
        && firstFailure === null
        ? "passed"
        : "failed"
  const queryKeyPathStatus = querySummary === null
    ? "not_run"
    : queryKeyPathPassed && queryReconnectPassed && queryStaleCleared && noBrowserErrors && firstFailure === null
      ? "passed"
      : "failed"
  await writeEvidence(`release-09d-${phase}-${testInfo.project.name}.json`, {
    schema_version: 1,
    stage: "stage09-release-proof-09D",
    phase,
    browser: testInfo.project.name,
    ...hostEvidence(),
    gates: {
      performance: phase === "small-perf-query"
        ? testInfo.project.name !== "chromium" ? "not-applicable" : performanceThresholdsPassed ? "passed" : "failed"
        : "not-run",
      query: querySummary === null ? "not-run" : queryKeyPathStatus === "passed"
        && (queryLatencyBudgetStatus === "passed" || queryLatencyBudgetStatus === "not_applicable")
          ? "passed"
          : "failed",
      functional_2k: phaseTarget === 2_000 && finalTotal === 2_000 && uiEvidence.functional_2k !== null && noBrowserErrors && firstFailure === null ? "passed" : phaseTarget === 2_000 ? "failed" : "not-run",
      stress_5k: phaseTarget === 5_000 && finalTotal === 5_000 && uiEvidence.stress_5k !== null && mapEvidence !== null && noBrowserErrors && firstFailure === null ? "passed" : phaseTarget === 5_000 ? "failed" : "not-run",
    },
    initial_brotli: process.env.KANBAN_RELEASE_09D_INITIAL_BROTLI_BYTES ? {
      bytes: Number(process.env.KANBAN_RELEASE_09D_INITIAL_BROTLI_BYTES),
      budget_bytes: 750 * 1024,
      files: initialBrotliFiles,
    } : null,
    performance: metricSummary,
    query: querySummary,
    query_contract: {
      latency_budget_status: queryLatencyBudgetStatus,
      key_path_status: queryKeyPathStatus,
      budgets_ms: {
        mutation_to_event_p95: QUERY_P95_BUDGET_MS,
        disconnected_catch_up_p95: QUERY_CATCH_UP_P95_BUDGET_MS,
      },
    },
    fixture_seed: {
      records: fixtureSeedRecords,
      response_error_count: fixtureSeedResponseErrors,
    },
    ui: uiEvidence.functional_2k === null && uiEvidence.stress_5k === null ? null : uiEvidence,
    map: mapEvidence,
    query_stream_requests: queryStreamRequests,
    query_reconnect: {
      new_request_after_disconnect: queryReconnectPassed,
      stale_notice_cleared: queryStaleCleared,
      ...(queryReconnectEvidence ?? {
        before_count: 0,
        request_count_after_disconnect: 0,
        request_at_ms: null,
        event_seen_at_ms: null,
        resume: null,
        confirmed_cursor: null,
        recovery_cursor: null,
        recovery_mode: null,
        confirmed_task_id: null,
        confirmed_event_id: null,
      }),
    },
    task_counts: taskCounts,
    final_task_total: finalTotal,
    browser_errors: browserErrors,
    failure: firstFailure,
  })
})

test.beforeEach(async ({ page }, testInfo) => {
  attachBrowserErrors(page, browserErrors)
  testInfo.attachments.push({ name: "execution-surface", contentType: "text/plain", body: Buffer.from("real-kanban-serve") })
  testInfo.annotations.push({ type: "browser-error-count", description: String(browserErrors.length) })
})

test.afterEach(async ({ page }, testInfo) => {
  void page
  if (testInfo.status !== testInfo.expectedStatus && firstFailure === null) {
    firstFailure = `${testInfo.title}: ${testInfo.status}`
  }
})

test("09D performance real browser Web Vitals and initial Brotli budget", async ({ browser }, testInfo) => {
  if (testInfo.project.name !== "chromium") {
    testInfo.annotations.push({ type: "not-applicable", description: "Web Vitals are measured only in Chromium" })
    return
  }
  const sampleCount = 20
  for (let sample = 1; sample <= sampleCount; sample += 1) {
    const context = await browser.newContext({ baseURL, viewport: { width: 1440, height: 900 } })
    await installBufferedVitals(context)
    const samplePage = await context.newPage()
    const errors: BrowserError[] = []
    attachBrowserErrors(samplePage, errors)
    try {
      await samplePage.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
      await assertBoardReady(samplePage)
      const vitals = await readBufferedVitals(samplePage)
      if (!vitals.lcpSupported || !vitals.clsSupported || vitals.lcp === null || vitals.cls === null) {
        throw new Error("real browser did not expose buffered LCP/CLS entries")
      }
      await installInteractionObserver(samplePage)
      await samplePage.getByRole("button", { name: "列表", exact: true }).click()
      await expect(samplePage.getByTestId("task-list")).toBeVisible()
      const inp = await readInteractionDuration(samplePage)
      if (inp === null) throw new Error("real browser did not expose EventTiming for the real UI click")
      metricSamples.push({
        sample,
        lcp_ms: vitals.lcp,
        inp_ms: inp.duration,
        inp_interaction_id: inp.interactionId,
        cls: vitals.cls,
        lcp_entry_count: vitals.lcpEntryCount,
        cls_entry_count: vitals.clsEntryCount,
        observer_supported: vitals.lcpSupported && vitals.clsSupported,
      })
      expect(errors).toEqual([])
    } finally {
      await context.close()
    }
  }
  expect(metricSamples).toHaveLength(sampleCount)
  const lcp = summarize(metricSamples.map((sample) => sample.lcp_ms))!
  const inp = summarize(metricSamples.map((sample) => sample.inp_ms))!
  const cls = summarize(metricSamples.map((sample) => sample.cls))!
  expect(lcp.p95).toBeLessThanOrEqual(2_000)
  expect(inp.p95).toBeLessThanOrEqual(200)
  expect(cls.p95).toBeLessThanOrEqual(0.1)
  const brotliBytes = Number(process.env.KANBAN_RELEASE_09D_INITIAL_BROTLI_BYTES)
  if (!Number.isSafeInteger(brotliBytes) || brotliBytes < 0) throw new Error("initial Brotli evidence is unavailable")
  expect(brotliBytes).toBeLessThanOrEqual(750 * 1024)
})

test("09D persistent QueryService UI mutation latency and disconnect catch-up", async ({ browser, request }, testInfo) => {
  const sampleCount = testInfo.project.name === "chromium" ? 20 : 1
  const contextB = await browser.newContext({ baseURL, viewport: { width: 1440, height: 900 } })
  const pageB = await contextB.newPage()
  const contextA = await browser.newContext({ baseURL, viewport: { width: 1440, height: 900 } })
  const pageA = await contextA.newPage()
  const errors: BrowserError[] = []
  attachBrowserErrors(pageB, errors)
  attachBrowserErrors(pageA, errors)
  const probe = await installQueryProbe(pageB)
  const streamRequests: QueryStreamRequest[] = []
  pageB.on("request", (request) => {
    if (request.url().endsWith("/kanban.v1.QueryService/WatchQueries")) {
      const queries = fromBinary(WatchQueriesRequestSchema, request.postDataBuffer()!.subarray(5)).queries
      const subscription = queries.find(query => query.query.case === "recentEvents")
      if (!subscription) return
      const entry = {
        client_query_id: subscription.clientQueryId,
        url: request.url(),
        at_ms: Date.now(),
        resume: cursorEvidence(subscription.resume),
      }
      streamRequests.push(entry)
      queryStreamRequests.push(entry)
    }
  })
  try {
    await pageB.goto("/app/boards/default/events", { waitUntil: "domcontentloaded" })
    await expect(pageB.getByTestId("events-ready")).toBeVisible()
    await expect.poll(async () => await pageB.getByTestId("event-row").count(), { timeout: 120_000 }).toBeGreaterThan(0)
    await pageA.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await assertBoardReady(pageA)
    for (let sample = 1; sample <= sampleCount; sample += 1) {
      await pageA.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
      await assertBoardReady(pageA)
      const title = `Stage09 09D QUERY ${testInfo.project.name} ${sample}`
      const mutation = await createTaskFromUi(pageA, title)
      const eventSeenAt = await eventVisibleAt(pageB, mutation.taskId)
      querySamples.push({ sample, task_id: mutation.taskId, mutation_to_event_ms: eventSeenAt - mutation.mutationStarted, disconnected_catch_up: false })
    }

    const lastNormalTaskId = querySamples.at(-1)?.task_id
    if (!lastNormalTaskId) throw new Error("normal QUERY samples did not produce a confirmed task")
    const confirmed = await confirmedTaskCursor(request, lastNormalTaskId)
    const confirmedProjection = cursorEvidence(probe.committed("recentEvents"))
    expect(confirmedProjection).not.toBeNull()
    const streamRequestsBeforeDisconnect = streamRequests.length
    await contextB.setOffline(true)
    await waitForOfflineNotice(pageB)
    await pageA.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await assertBoardReady(pageA)
    const catchUpMutation = await createTaskFromUi(pageA, `Stage09 09D QUERY catch-up ${testInfo.project.name}`)
    await expect(pageB.getByTestId("event-row").filter({ hasText: catchUpMutation.taskId })).toHaveCount(0)
    await contextB.setOffline(false)
    await expect.poll(async () => streamRequests.length, { timeout: 120_000 }).toBeGreaterThan(streamRequestsBeforeDisconnect)
    const reconnectRequest = streamRequests.at(-1)
    if (!reconnectRequest) throw new Error("QUERY reconnect request was not captured")
    if (reconnectRequest.resume) expect(reconnectRequest.resume).toEqual(confirmedProjection)
    const catchUpEventSeenAt = await eventVisibleAt(pageB, catchUpMutation.taskId)
    querySamples.push({ sample: sampleCount + 1, task_id: catchUpMutation.taskId, mutation_to_event_ms: catchUpEventSeenAt - catchUpMutation.mutationStarted, disconnected_catch_up: true })
    expect(reconnectRequest.at_ms).toBeLessThanOrEqual(catchUpEventSeenAt)
    await expect(pageB.getByTestId("events-ready")).toBeVisible({ timeout: 120_000 })
    await expect(pageB.getByTestId("events-stale-notice")).toHaveCount(0)
    await expect(pageB.getByTestId("events-offline")).toHaveCount(0)
    // 离线 UI 可释放最后一个查询消费者；新 owner 必须接收完整 snapshot。
    if (!reconnectRequest.resume) {
      const connection = probe.requests.at(-1)!.connection
      expect(probe.frames.some(item => item.connection === connection && item.definition?.query.case === 'recentEvents' && item.frame.body.case === 'begin' && item.frame.body.value.snapshot)).toBe(true)
    }
    queryReconnectPassed = streamRequests.length > streamRequestsBeforeDisconnect
    queryStaleCleared = true
    queryReconnectEvidence = {
      before_count: streamRequestsBeforeDisconnect,
      request_count_after_disconnect: streamRequests.length,
      request_at_ms: reconnectRequest.at_ms,
      event_seen_at_ms: catchUpEventSeenAt,
      resume: reconnectRequest.resume,
      confirmed_cursor: confirmedProjection,
      recovery_cursor: cursorEvidence(probe.committed("recentEvents")),
      recovery_mode: reconnectRequest.resume ? "resume" : "snapshot",
      confirmed_task_id: lastNormalTaskId,
      confirmed_event_id: confirmed.event_id,
    }
    const normalLatencies = querySamples
      .filter((sample) => !sample.disconnected_catch_up)
      .map((sample) => sample.mutation_to_event_ms)
    const catchUpLatencies = querySamples
      .filter((sample) => sample.disconnected_catch_up)
      .map((sample) => sample.mutation_to_event_ms)
    expect(normalLatencies).toHaveLength(sampleCount)
    expect(catchUpLatencies).toHaveLength(1)
    if (testInfo.project.name === "chromium") {
      expect(percentile(normalLatencies, 0.95)).toBeLessThanOrEqual(QUERY_P95_BUDGET_MS)
      expect(percentile(catchUpLatencies, 0.95)).toBeLessThanOrEqual(QUERY_CATCH_UP_P95_BUDGET_MS)
    }
    expect(streamRequests.length).toBeGreaterThan(0)
    expect(errors).toEqual([])
    queryKeyPathPassed = true
  } finally {
    await contextA.close()
    await contextB.close()
  }
})

test("09D functional_2k real board and list pagination", async ({ page, request }) => {
  test.setTimeout(FIXTURE_TEST_TIMEOUT_MS)
  const errorCountBefore = browserErrors.length
  const total = await ensureTaskTotal(request, 2_000)
  expect(total).toBe(2_000)
  uiEvidence.functional_2k = await loadUiEvidence(page, total)
  expect(browserErrors.slice(errorCountBefore)).toEqual([])
})

test("09D stress_5k real board list map no-crash bounded interaction", async ({ page, request }) => {
  test.setTimeout(FIXTURE_TEST_TIMEOUT_MS)
  const errorCountBefore = browserErrors.length
  const total = await ensureTaskTotal(request, 5_000)
  expect(total).toBe(5_000)
  const load = await loadUiEvidence(page, total)
  const limit = page.getByRole("combobox", { name: "每页", exact: true })
  await limit.click()
  const options = await page.getByRole("option").allTextContents()
  expect(options).toEqual(["每页 10 项", "每页 25 项", "每页 50 项", "每页 100 项"])
  await page.getByRole("option", { name: "每页 50 项", exact: true }).click()
  const beforeText = await limit.innerText()
  const first50 = await taskWindow(page, "list", { ...defaultWindowQuery, limit: 50 }, total)
  const second50 = await nextWindow(page, "list", first50)
  await selectChoice(page, "每页", "每页 100 项")
  const afterText = await limit.innerText()
  const after100 = await taskWindow(page, "list", defaultWindowQuery, total)
  expect(new Set([...first50.task_ids, ...second50.task_ids])).toEqual(new Set(after100.task_ids))
  await page.goto("/app/boards/default/map?filter=all", { waitUntil: "domcontentloaded" })
  const taskMap = page.getByTestId("task-map")
  await expect(taskMap).toBeVisible({ timeout: UI_ACTION_TIMEOUT_MS })
  await expect(page.getByTestId("task-map-loading")).toHaveCount(0, { timeout: UI_ACTION_TIMEOUT_MS })
  await expect(page.getByTestId("task-map-error")).toHaveCount(0, { timeout: UI_ACTION_TIMEOUT_MS })
  await expect(page.getByTestId("task-map-chunk-error")).toHaveCount(0, { timeout: UI_ACTION_TIMEOUT_MS })
  await expect(page.getByTestId("task-map-refresh-error")).toHaveCount(0, { timeout: UI_ACTION_TIMEOUT_MS })
  const mapNodes = page.getByTestId("task-map-node")
  await expect(mapNodes.first()).toBeVisible({ timeout: UI_ACTION_TIMEOUT_MS })
  const mapNodeCount = await mapNodes.count()
  const mapEdgeCount = await page.getByTestId("task-map-edge").count()
  const mapTruncated = await page.getByTestId("task-map-truncated").count() === 1
  expect(mapNodeCount).toBeGreaterThan(0)
  expect(mapNodeCount).toBeLessThanOrEqual(240)
  const mapResponse = await rpcRequest("BoardTaskMap", { path: { board: "default" }, query: { active_only: true, context_depth: 1, include_done_context: false, include_archived_context: false, hide_isolated: false, limit_nodes: 240 } })
  expect(mapResponse.ok()).toBe(true)
  const mapBody = await mapResponse.json() as {
    data?: { nodes?: unknown[]; edges?: unknown[]; meta?: { node_count?: unknown; edge_count?: unknown; truncated?: unknown; limit_nodes?: unknown } }
  }
  const mapMeta = mapBody.data?.meta
  if (
    !mapMeta
    || typeof mapMeta.node_count !== "number"
    || typeof mapMeta.edge_count !== "number"
    || typeof mapMeta.truncated !== "boolean"
    || mapMeta.limit_nodes !== 240
  ) throw new Error("task map response did not expose canonical meta")
  expect(mapNodeCount).toBe(mapMeta.node_count)
  expect(mapEdgeCount).toBe(mapMeta.edge_count)
  expect(mapTruncated).toBe(mapMeta.truncated)
  await expect(page.getByTestId("task-map-zoom")).toHaveText("100%", { timeout: UI_ACTION_TIMEOUT_MS })
  const mapZoomBefore = Number((await page.getByTestId("task-map-zoom").innerText()).replace("%", ""))
  expect(mapZoomBefore).toBe(100)
  await page.getByRole("button", { name: "放大关系图", exact: true }).click()
  await expect(page.getByTestId("task-map-zoom")).toHaveText("115%", { timeout: UI_ACTION_TIMEOUT_MS })
  const mapZoomAfter = Number((await page.getByTestId("task-map-zoom").innerText()).replace("%", ""))
  expect(mapZoomAfter).toBe(115)
  mapEvidence = {
    node_count: mapMeta.node_count,
    edge_count: mapMeta.edge_count,
    truncated: mapMeta.truncated,
    limit_nodes: mapMeta.limit_nodes,
    ui_node_count: mapNodeCount,
    ui_edge_count: mapEdgeCount,
    ui_truncated: mapTruncated,
    zoom_before: mapZoomBefore,
    zoom_after: mapZoomAfter,
  }
  uiEvidence.stress_5k = {
    ...load,
    page_size: { options, before_text: beforeText, before_pages: [first50, second50], after_text: afterText, after: after100 },
    map: {
      node_count: mapMeta.node_count,
      edge_count: mapMeta.edge_count,
      truncated: mapMeta.truncated,
      limit_nodes: mapMeta.limit_nodes,
      zoom_before: mapZoomBefore,
      zoom_after: mapZoomAfter,
    },
  }
  expect(browserErrors.slice(errorCountBefore)).toEqual([])
})
