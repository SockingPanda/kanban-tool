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
type SseSample = {
  readonly sample: number
  readonly task_id: string
  readonly mutation_to_event_ms: number
  readonly disconnected_catch_up: boolean
}

type SseStreamRequest = {
  readonly url: string
  readonly at_ms: number
  readonly last_event_id: string | null
  readonly after: string | null
}

type ConfirmedTaskCursor = {
  readonly cursor: string
  readonly event_id: string
}

type ListUiEvidence = {
  readonly row_count: number
  readonly range: string
  readonly first_task_id: string | null
  readonly last_task_id: string | null
}

type BoardReadyEvidence = {
  readonly navigation_start_ms: number
  readonly ready_ms: number
  readonly budget_ms: number
  readonly within_budget: boolean
}

type BoardColumnEvidence = {
  readonly column_id: string
  readonly total: number
  readonly page: number
  readonly total_pages: number
  readonly range_start: number
  readonly range_end: number
  readonly range_total: number
  readonly rendered_card_count: number
}

type BoardWindowEvidence = {
  readonly column_id: string
  readonly page_size: number
  readonly page1: {
    readonly page: number
    readonly range_start: number
    readonly range_end: number
    readonly total: number
    readonly first_task_id: string | null
    readonly last_task_id: string | null
  }
  readonly page2: {
    readonly page: number
    readonly range_start: number
    readonly range_end: number
    readonly total: number
    readonly first_task_id: string | null
    readonly last_task_id: string | null
  }
}

type FunctionalUiEvidence = {
  readonly board_total: number
  readonly board_rendered_count: number
  readonly board_columns: readonly BoardColumnEvidence[]
  readonly board_window: BoardWindowEvidence
  readonly board_ready: BoardReadyEvidence
  readonly list_total: number
  readonly page1: ListUiEvidence
  readonly page2: ListUiEvidence
}

type StressUiEvidence = {
  readonly board_total: number
  readonly board_rendered_count: number
  readonly board_columns: readonly BoardColumnEvidence[]
  readonly board_window: BoardWindowEvidence
  readonly board_ready: BoardReadyEvidence
  readonly list_total: number
  readonly before_limit: number
  readonly before: ListUiEvidence
  readonly after_limit: number
  readonly after: ListUiEvidence
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
const sseSamples: SseSample[] = []
const taskCounts: Array<{ readonly target: number; readonly total: number }> = []
const fixtureSeedRecords: FixtureSeedRecord[] = []
const sseStreamRequests: SseStreamRequest[] = []
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
let sseKeyPathPassed = false
let sseReconnectPassed = false
let sseStaleCleared = false
let sseReconnectEvidence: {
  readonly before_count: number
  readonly request_count_after_disconnect: number
  readonly request_at_ms: number | null
  readonly event_seen_at_ms: number | null
  readonly last_event_id: string | null
  readonly after: string | null
  readonly confirmed_cursor: string | null
  readonly confirmed_task_id: string | null
  readonly confirmed_event_id: string | null
} | null = null

const phaseTarget = phase.includes("5k") ? 5_000 : phase.includes("2k") ? 2_000 : null
const SSE_P95_BUDGET_MS = 1_000
const SSE_CATCH_UP_P95_BUDGET_MS = 3_000
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
  const response = await request.get(`${baseURL}/api/v1/boards/default/tasks?limit=1&offset=0&sort=seq`)
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
    const response = await request.get(`${baseURL}/api/v1/events?board=default&after=${after}&limit=100`)
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

async function listUiEvidence(page: Page): Promise<ListUiEvidence> {
  const list = page.getByTestId("task-list")
  const range = (await list.locator("header p").last().innerText()).trim()
  const rows = page.getByTestId("task-row")
  const rowCount = await rows.count()
  return {
    row_count: rowCount,
    range,
    first_task_id: rowCount > 0 ? await rows.first().getAttribute("data-task-id") : null,
    last_task_id: rowCount > 0 ? await rows.last().getAttribute("data-task-id") : null,
  }
}

function listTotalFromRange(range: string): number {
  const match = range.match(/\/\s*(\d+)$/)
  if (!match) throw new Error(`task list range did not expose a total: ${range}`)
  const total = Number(match[1])
  if (!Number.isSafeInteger(total) || total < 0) throw new Error(`task list total is not a safe integer: ${range}`)
  return total
}

function parseBoardRange(text: string): { readonly start: number; readonly end: number; readonly total: number } {
  const match = text.trim().match(/^(\d+)–(\d+)\s*\/\s*(\d+)$/)
  if (!match) throw new Error(`board page range did not expose a visible range: ${text}`)
  const start = Number(match[1])
  const end = Number(match[2])
  const total = Number(match[3])
  if (![start, end, total].every((value) => Number.isSafeInteger(value) && value >= 0)) {
    throw new Error(`board page range contains an unsafe number: ${text}`)
  }
  return { start, end, total }
}

async function boardColumnEvidence(page: Page): Promise<{ readonly total: number; readonly rendered: number; readonly columns: readonly BoardColumnEvidence[] }> {
  const boardTotalElement = page.getByTestId("board-task-total")
  await expect(boardTotalElement).toBeVisible({ timeout: UI_ACTION_TIMEOUT_MS })
  const boardTotalText = (await boardTotalElement.innerText()).trim()
  const boardTotal = Number(await boardTotalElement.getAttribute("data-total"))
  if (!Number.isSafeInteger(boardTotal) || boardTotal < 0 || !boardTotalText.includes(String(boardTotal))) {
    throw new Error(`board total is not a visible safe integer: ${boardTotalText}`)
  }
  const columns = page.getByTestId("board-column")
  const columnEvidence: BoardColumnEvidence[] = []
  for (let index = 0; index < await columns.count(); index += 1) {
    const column = columns.nth(index)
    const columnId = await column.getAttribute("data-column-id")
    const totalElement = column.getByTestId("board-column-total")
    const pageElement = column.getByTestId("board-column-page")
    const total = Number(await totalElement.getAttribute("data-total"))
    const renderedCardCount = await column.getByTestId("board-task").count()
    if (!Number.isSafeInteger(total) || total < 0 || total > 5_000) {
      throw new Error(`board column ${columnId ?? "unknown"} exposed an unsafe total`)
    }
    if (await pageElement.count() === 0) {
      if (total > 100 || renderedCardCount !== total) {
        throw new Error(`non-paginated board column ${columnId ?? "unknown"} exceeded its bounded window`)
      }
      columnEvidence.push({
        column_id: columnId ?? "",
        total,
        page: 1,
        total_pages: 1,
        range_start: total === 0 ? 0 : 1,
        range_end: total,
        range_total: total,
        rendered_card_count: renderedCardCount,
      })
      continue
    }
    const pageNumber = Number(await pageElement.getAttribute("data-page"))
    const totalPages = Number(await pageElement.getAttribute("data-total-pages"))
    const rangeStart = Number(await pageElement.getAttribute("data-range-start"))
    const rangeEnd = Number(await pageElement.getAttribute("data-range-end"))
    const rangeTotal = Number(await pageElement.getAttribute("data-total"))
    const range = parseBoardRange(await pageElement.innerText())
    if (
      columnId === null
      || ![total, pageNumber, totalPages, rangeStart, rangeEnd, rangeTotal].every((value) => Number.isSafeInteger(value) && value >= 0)
      || range.start !== rangeStart
      || range.end !== rangeEnd
      || range.total !== rangeTotal
      || total !== rangeTotal
    ) throw new Error(`board column ${columnId ?? "unknown"} exposed inconsistent total/range metadata`)
    columnEvidence.push({
      column_id: columnId,
      total,
      page: pageNumber,
      total_pages: totalPages,
      range_start: rangeStart,
      range_end: rangeEnd,
      range_total: rangeTotal,
      rendered_card_count: renderedCardCount,
    })
  }
  return {
    total: boardTotal,
    rendered: await page.getByTestId("board-task").count(),
    columns: columnEvidence,
  }
}

async function boardWindowSnapshot(column: Locator): Promise<BoardWindowEvidence["page1"]> {
  const pageElement = column.getByTestId("board-column-page")
  const page = Number(await pageElement.getAttribute("data-page"))
  const rangeStart = Number(await pageElement.getAttribute("data-range-start"))
  const rangeEnd = Number(await pageElement.getAttribute("data-range-end"))
  const total = Number(await pageElement.getAttribute("data-total"))
  const range = parseBoardRange(await pageElement.innerText())
  const cards = column.getByTestId("board-task")
  const rendered = await cards.count()
  if (rendered === 0 || range.start !== rangeStart || range.end !== rangeEnd || range.total !== total) {
    throw new Error("board window did not expose a consistent visible range")
  }
  return {
    page,
    range_start: rangeStart,
    range_end: rangeEnd,
    total,
    first_task_id: await cards.first().getAttribute("data-task-id"),
    last_task_id: await cards.last().getAttribute("data-task-id"),
  }
}

async function boardWindowEvidence(page: Page): Promise<BoardWindowEvidence> {
  const columns = page.getByTestId("board-column")
  for (let index = 0; index < await columns.count(); index += 1) {
    const column = columns.nth(index)
    const pageElement = column.getByTestId("board-column-page")
    if (await pageElement.count() === 0) continue
    const totalPages = Number(await pageElement.getAttribute("data-total-pages"))
    if (!Number.isSafeInteger(totalPages) || totalPages <= 1) continue
    const columnId = await column.getAttribute("data-column-id")
    if (columnId === null) throw new Error("paginated board column did not expose its identity")
    const page1 = await boardWindowSnapshot(column)
    const next = column.getByTestId("board-page-next")
    await expect(next).toBeEnabled({ timeout: UI_ACTION_TIMEOUT_MS })
    await next.click()
    await expect(pageElement).toHaveAttribute("data-page", "2", { timeout: UI_ACTION_TIMEOUT_MS })
    const page2 = await boardWindowSnapshot(column)
    expect(page2.page).toBeGreaterThan(page1.page)
    expect(page2.range_start).toBeGreaterThan(page1.range_start)
    expect(page2.first_task_id).not.toBe(page1.first_task_id)
    expect(page2.last_task_id).not.toBe(page1.last_task_id)
    return {
      column_id: columnId,
      page_size: page1.range_end - page1.range_start + 1,
      page1,
      page2,
    }
  }
  throw new Error("board did not expose a paginated visible column")
}

async function createTaskBatch(request: APIRequestContext, ids: readonly string[]): Promise<number> {
  let created = 0
  for (let offset = 0; offset < ids.length; offset += FIXTURE_CONCURRENCY) {
    const batch = ids.slice(offset, offset + FIXTURE_CONCURRENCY)
    await Promise.all(batch.map(async (taskId) => {
      try {
        const response = await request.post(`${baseURL}/api/v1/boards/default/tasks`, {
          data: {
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
          },
        })
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
  await dialog.getByRole("button", { name: "创建", exact: true }).click()
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
  const normalSseLatencies = sseSamples
    .filter((sample) => !sample.disconnected_catch_up)
    .map((sample) => sample.mutation_to_event_ms)
  const catchUpSseLatencies = sseSamples
    .filter((sample) => sample.disconnected_catch_up)
    .map((sample) => sample.mutation_to_event_ms)
  const sseSummary = sseSamples.length === 0 ? null : {
    latency_ms: summarize(sseSamples.map((sample) => sample.mutation_to_event_ms)),
    mutation_to_event_ms: summarize(normalSseLatencies),
    disconnected_catch_up_ms: summarize(catchUpSseLatencies),
    samples: sseSamples,
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
  const sseLatencyBudgetStatus = sseSummary === null
    ? "not_run"
    : testInfo.project.name !== "chromium"
      ? "not_applicable"
      : (sseSummary.mutation_to_event_ms?.p95 ?? Number.POSITIVE_INFINITY) <= SSE_P95_BUDGET_MS
        && (sseSummary.disconnected_catch_up_ms?.p95 ?? Number.POSITIVE_INFINITY) <= SSE_CATCH_UP_P95_BUDGET_MS
        && noBrowserErrors
        && firstFailure === null
        ? "passed"
        : "failed"
  const sseKeyPathStatus = sseSummary === null
    ? "not_run"
    : sseKeyPathPassed && sseReconnectPassed && sseStaleCleared && noBrowserErrors && firstFailure === null
      ? "passed"
      : "failed"
  await writeEvidence(`release-09d-${phase}-${testInfo.project.name}.json`, {
    schema_version: 1,
    stage: "stage09-release-proof-09D",
    phase,
    browser: testInfo.project.name,
    ...hostEvidence(),
    gates: {
      performance: phase === "small-perf-sse"
        ? testInfo.project.name !== "chromium" ? "not-applicable" : performanceThresholdsPassed ? "passed" : "failed"
        : "not-run",
      sse: sseSummary === null ? "not-run" : sseKeyPathStatus === "passed"
        && (sseLatencyBudgetStatus === "passed" || sseLatencyBudgetStatus === "not_applicable")
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
    sse: sseSummary,
    sse_contract: {
      latency_budget_status: sseLatencyBudgetStatus,
      key_path_status: sseKeyPathStatus,
      budgets_ms: {
        mutation_to_event_p95: SSE_P95_BUDGET_MS,
        disconnected_catch_up_p95: SSE_CATCH_UP_P95_BUDGET_MS,
      },
    },
    fixture_seed: {
      records: fixtureSeedRecords,
      response_error_count: fixtureSeedResponseErrors,
    },
    ui: uiEvidence.functional_2k === null && uiEvidence.stress_5k === null ? null : uiEvidence,
    map: mapEvidence,
    sse_stream_requests: sseStreamRequests,
    sse_reconnect: {
      new_request_after_disconnect: sseReconnectPassed,
      stale_notice_cleared: sseStaleCleared,
      ...(sseReconnectEvidence ?? {
        before_count: 0,
        request_count_after_disconnect: 0,
        request_at_ms: null,
        event_seen_at_ms: null,
        last_event_id: null,
        after: null,
        confirmed_cursor: null,
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
      await samplePage.getByRole("link", { name: "列表", exact: true }).click()
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

test("09D persistent SSE UI mutation latency and disconnect catch-up", async ({ browser, request }, testInfo) => {
  const sampleCount = testInfo.project.name === "chromium" ? 20 : 1
  const contextB = await browser.newContext({ baseURL, viewport: { width: 1440, height: 900 } })
  const pageB = await contextB.newPage()
  const contextA = await browser.newContext({ baseURL, viewport: { width: 1440, height: 900 } })
  const pageA = await contextA.newPage()
  const errors: BrowserError[] = []
  attachBrowserErrors(pageB, errors)
  attachBrowserErrors(pageA, errors)
  const streamRequests: SseStreamRequest[] = []
  pageB.on("request", (request) => {
    if (request.url().includes("/api/v1/stream/events")) {
      const url = new URL(request.url())
      const entry = {
        url: request.url(),
        at_ms: Date.now(),
        last_event_id: request.headers()["last-event-id"] ?? null,
        after: url.searchParams.get("after"),
      }
      streamRequests.push(entry)
      sseStreamRequests.push(entry)
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
      const title = `Stage09 09D SSE ${testInfo.project.name} ${sample}`
      const mutation = await createTaskFromUi(pageA, title)
      const eventSeenAt = await eventVisibleAt(pageB, mutation.taskId)
      sseSamples.push({ sample, task_id: mutation.taskId, mutation_to_event_ms: eventSeenAt - mutation.mutationStarted, disconnected_catch_up: false })
    }

    const lastNormalTaskId = sseSamples.at(-1)?.task_id
    if (!lastNormalTaskId) throw new Error("normal SSE samples did not produce a confirmed task")
    const confirmed = await confirmedTaskCursor(request, lastNormalTaskId)
    const streamRequestsBeforeDisconnect = streamRequests.length
    await contextB.setOffline(true)
    await waitForOfflineNotice(pageB)
    await pageA.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
    await assertBoardReady(pageA)
    const catchUpMutation = await createTaskFromUi(pageA, `Stage09 09D SSE catch-up ${testInfo.project.name}`)
    await expect(pageB.getByTestId("event-row").filter({ hasText: catchUpMutation.taskId })).toHaveCount(0)
    await contextB.setOffline(false)
    await expect.poll(async () => streamRequests.length, { timeout: 120_000 }).toBeGreaterThan(streamRequestsBeforeDisconnect)
    const reconnectRequest = streamRequests.at(-1)
    if (!reconnectRequest) throw new Error("SSE reconnect request was not captured")
    expect(reconnectRequest.last_event_id).not.toBeNull()
    expect(reconnectRequest.after).not.toBeNull()
    expect(reconnectRequest.last_event_id).toBe(confirmed.cursor)
    expect(reconnectRequest.after).toBe(confirmed.cursor)
    const catchUpEventSeenAt = await eventVisibleAt(pageB, catchUpMutation.taskId)
    sseSamples.push({ sample: sampleCount + 1, task_id: catchUpMutation.taskId, mutation_to_event_ms: catchUpEventSeenAt - catchUpMutation.mutationStarted, disconnected_catch_up: true })
    expect(reconnectRequest.at_ms).toBeLessThanOrEqual(catchUpEventSeenAt)
    await expect(pageB.getByTestId("events-ready")).toBeVisible({ timeout: 120_000 })
    await expect(pageB.getByTestId("events-stale-notice")).toHaveCount(0)
    await expect(pageB.getByTestId("events-offline")).toHaveCount(0)
    sseReconnectPassed = streamRequests.length > streamRequestsBeforeDisconnect
      && reconnectRequest.last_event_id === confirmed.cursor
      && reconnectRequest.after === confirmed.cursor
    sseStaleCleared = true
    sseReconnectEvidence = {
      before_count: streamRequestsBeforeDisconnect,
      request_count_after_disconnect: streamRequests.length,
      request_at_ms: reconnectRequest.at_ms,
      event_seen_at_ms: catchUpEventSeenAt,
      last_event_id: reconnectRequest.last_event_id,
      after: reconnectRequest.after,
      confirmed_cursor: confirmed.cursor,
      confirmed_task_id: lastNormalTaskId,
      confirmed_event_id: confirmed.event_id,
    }
    const normalLatencies = sseSamples
      .filter((sample) => !sample.disconnected_catch_up)
      .map((sample) => sample.mutation_to_event_ms)
    const catchUpLatencies = sseSamples
      .filter((sample) => sample.disconnected_catch_up)
      .map((sample) => sample.mutation_to_event_ms)
    expect(normalLatencies).toHaveLength(sampleCount)
    expect(catchUpLatencies).toHaveLength(1)
    if (testInfo.project.name === "chromium") {
      expect(percentile(normalLatencies, 0.95)).toBeLessThanOrEqual(SSE_P95_BUDGET_MS)
      expect(percentile(catchUpLatencies, 0.95)).toBeLessThanOrEqual(SSE_CATCH_UP_P95_BUDGET_MS)
    }
    expect(streamRequests.length).toBeGreaterThan(0)
    expect(errors).toEqual([])
    sseKeyPathPassed = true
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
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  const boardReady = await assertBoardReady(page)
  expect(boardReady.budget_ms).toBe(BOARD_READY_BUDGET_MS)
  expect(boardReady.within_budget).toBe(true)
  const board = await boardColumnEvidence(page)
  expect(board.total).toBe(total)
  expect(board.columns.reduce((sum, column) => sum + column.total, 0)).toBe(total)
  expect(board.columns.every((column) => column.rendered_card_count <= 100)).toBe(true)
  expect(board.rendered).toBeLessThanOrEqual(Math.max(1, board.columns.length) * 100)
  const boardWindow = await boardWindowEvidence(page)
  await page.goto("/app/boards/default/list", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("task-list")).toContainText("2000")
  await expect(page.getByTestId("task-row")).toHaveCount(100)
  const page1 = await listUiEvidence(page)
  const listTotal = listTotalFromRange(page1.range)
  expect(listTotal).toBe(2_000)
  await page.getByRole("button", { name: "下一页", exact: true }).click()
  await expect(page.getByTestId("task-row")).toHaveCount(100)
  const page2 = await listUiEvidence(page)
  expect(page2.range).toContain("101–200")
  uiEvidence.functional_2k = {
    board_total: board.total,
    board_rendered_count: board.rendered,
    board_columns: board.columns,
    board_window: boardWindow,
    board_ready: boardReady,
    list_total: listTotal,
    page1,
    page2,
  }
  expect(browserErrors.slice(errorCountBefore)).toEqual([])
})

test("09D stress_5k real board list map no-crash bounded interaction", async ({ page, request }) => {
  test.setTimeout(FIXTURE_TEST_TIMEOUT_MS)
  const errorCountBefore = browserErrors.length
  const total = await ensureTaskTotal(request, 5_000)
  expect(total).toBe(5_000)
  await page.goto("/app/boards/default/board", { waitUntil: "domcontentloaded" })
  const boardReady = await assertBoardReady(page)
  expect(boardReady.budget_ms).toBe(BOARD_READY_BUDGET_MS)
  expect(boardReady.within_budget).toBe(true)
  const board = await boardColumnEvidence(page)
  expect(board.total).toBe(total)
  expect(board.columns.reduce((sum, column) => sum + column.total, 0)).toBe(total)
  expect(board.columns.every((column) => column.rendered_card_count <= 100)).toBe(true)
  expect(board.rendered).toBeLessThanOrEqual(Math.max(1, board.columns.length) * 100)
  const boardWindow = await boardWindowEvidence(page)
  await page.goto("/app/boards/default/list", { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("task-list")).toContainText("5000", { timeout: UI_ACTION_TIMEOUT_MS })
  await expect(page.getByTestId("task-row")).toHaveCount(100, { timeout: UI_ACTION_TIMEOUT_MS })
  const before = await listUiEvidence(page)
  const listTotal = listTotalFromRange(before.range)
  expect(listTotal).toBe(5_000)
  const listLimit = page.getByTestId("list-limit")
  const beforeLimit = Number(await listLimit.inputValue())
  expect(beforeLimit).toBe(100)
  await listLimit.selectOption("200")
  await expect(page.getByTestId("task-row")).toHaveCount(200, { timeout: UI_ACTION_TIMEOUT_MS })
  await expect(listLimit).toHaveValue("200", { timeout: UI_ACTION_TIMEOUT_MS })
  const afterLimit = Number(await listLimit.inputValue())
  expect(afterLimit).toBe(200)
  const after = await listUiEvidence(page)
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
  const mapResponse = await request.get(`${baseURL}/api/v1/boards/default/task-map?active_only=true&context_depth=1&include_done_context=false&include_archived_context=false&hide_isolated=false&limit_nodes=240`)
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
    board_total: board.total,
    board_rendered_count: board.rendered,
    board_columns: board.columns,
    board_window: boardWindow,
    board_ready: boardReady,
    list_total: listTotal,
    before_limit: beforeLimit,
    before,
    after_limit: afterLimit,
    after,
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
