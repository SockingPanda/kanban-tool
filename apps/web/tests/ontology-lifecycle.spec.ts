import { expect, test, type Page, type Route } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"

const BOARD_ID = "b_default"
const BOARD_SLUG = "default"
const SIGNAL_A = "los_a"
const SIGNAL_B = "los_b"
const ACTION_FAILURE_MESSAGE = "A lifecycle action failed on purpose"

type Scenario = "retry" | "switch-while-pending"

type ActionRequest = {
  readonly action_type?: string
  readonly signal_ids?: readonly string[]
  readonly reason?: string
  readonly idempotency_key?: string | null
}

type OntologyFixture = {
  readonly actionRequests: ActionRequest[]
  readonly completedActions: () => number
  readonly releasePendingAction: () => void
  readonly snapshotReadCounts: () => ReadCounts
}

type ReadCounts = {
  readonly signals: number
  readonly review: number
  readonly detailA: number
  readonly detailB: number
}

function signalRecord(id: string, title: string) {
  return {
    id,
    observation_id: `obs_${id}`,
    board_id: BOARD_ID,
    kind: "false_negative",
    status: "open",
    target_label_id: `label_${id}`,
    target_label_name_snapshot: title,
    proposed_action: "add_positive_atom",
    candidate_atom_polarity: "positive",
    candidate_atom_kind: "applies_when",
    candidate_text: `${title} candidate atom`,
    candidate_content_hash: `hash_${id}`,
    proposed_label_name: null,
    proposed_label_name_normalized: null,
    agent_selected: true,
    suggest_state: "absent",
    suggest_score: 0.75,
    suggest_rank: 1,
    final_selected: true,
    rationale: `${title} rationale`,
    confidence: 0.9,
    signal_key: `key_${id}`,
    superseded_by_signal_id: null,
    status_reason: null,
    created_at: 1,
    updated_at: 1,
    reviewed_at: null,
    closed_at: null,
    related_labels: [],
    proposal: {},
  }
}

function signalDetail(signal: ReturnType<typeof signalRecord>) {
  return {
    data: {
      signal,
      observation: {
        id: signal.observation_id,
        board_id: BOARD_ID,
        task_id: `task_${signal.id}`,
        task_ref_snapshot: `${BOARD_SLUG}#${signal.id}`,
        suggest_input_hash: `input_${signal.id}`,
        suggest_coverage: 0.8,
        suggest_coverage_cosine: 0.8,
        suggest_residual_norm: 0.1,
        suggest_needs_new_label: false,
        suggest_degraded: false,
        capture_fingerprint: `fingerprint_${signal.id}`,
        created_by: "playwright",
        created_by_type: "user",
        agent_type: null,
        created_at: 1,
        signals: [signal],
        task_snapshot: {},
        agent_candidates: [],
        suggestion_snapshot: {},
        final_decision: {},
        diagnostics: [],
      },
      actions: [],
    },
  }
}

function reviewGroup(signal: ReturnType<typeof signalRecord>) {
  return {
    group_by: "label",
    key: signal.target_label_id,
    label_id: signal.target_label_id,
    label_name: signal.target_label_name_snapshot,
    candidate_atom_polarity: signal.candidate_atom_polarity,
    candidate_atom_kind: signal.candidate_atom_kind,
    candidate_text: signal.candidate_text,
    candidate_content_hash: signal.candidate_content_hash,
    proposed_label_name: null,
    proposed_label_name_normalized: null,
    cluster_key: null,
    cluster_reason: null,
    task_count: 1,
    signal_count: 1,
    open_count: 1,
    confirmed_count: 0,
    resolved_count: 0,
    rejected_count: 0,
    superseded_count: 0,
    degraded_count: 0,
    average_score: signal.suggest_score,
    median_score: signal.suggest_score,
    oldest_signal_at: 1,
    latest_signal_at: 1,
    sample_task_refs: [`${BOARD_SLUG}#${signal.id}`],
    signal_ids: [signal.id],
    action_count: 0,
    action_ids: [],
    proposal_ids: [],
    labels: [{ id: signal.target_label_id, name: signal.target_label_name_snapshot }],
    candidate_atom_variants: [],
  }
}

function actionResponse(request: ActionRequest) {
  return {
    data: {
      id: "loa_playwright",
      board_id: BOARD_ID,
      parent_action_id: null,
      action_type: "confirm",
      reason: request.reason ?? "",
      target_label_id: null,
      result_label_id: null,
      result_atom_id: null,
      result_atom_content_hash: null,
      result_proposal_id: null,
      canonical_before_hash: null,
      canonical_after_hash: null,
      change: {},
      validation_requirement: "none",
      validation_status: "not_required",
      validation_effective_outcome: "not_required",
      validation_latest_attempt_id: null,
      validation: {},
      created_by: "playwright",
      created_by_type: "user",
      agent_type: null,
      created_at: 2,
      signal_ids: [...(request.signal_ids ?? [SIGNAL_A])],
    },
  }
}

async function fulfillJson(route: Route, payload: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(payload),
  })
}

function taskWindow(url: URL) {
  const status = url.searchParams.get("status") ?? "todo"
  const limit = Number(url.searchParams.get("limit") ?? 1_000)
  const offset = Number(url.searchParams.get("offset") ?? 0)
  return {
    data: {
      statuses: [{ status, tasks: [], page: { limit, offset, total: 0 } }],
    },
    meta: { limit, offset },
  }
}

async function installOntologyFixture(page: Page, scenario: Scenario): Promise<OntologyFixture> {
  const signals = [signalRecord(SIGNAL_A, "Signal A"), signalRecord(SIGNAL_B, "Signal B")]
  const signalById = new Map(signals.map((signal) => [signal.id, signal]))
  const actionRequests: ActionRequest[] = []
  let completedActionCount = 0
  let releasePendingAction!: () => void
  const pendingAction = new Promise<void>((resolve) => {
    releasePendingAction = resolve
  })
  const readCounts = { signals: 0, review: 0, detailA: 0, detailB: 0 }

  await page.route("**/api/v1/**", async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const { pathname } = url

    if (pathname === "/api/v1/stream/events") {
      await route.abort()
      return
    }
    if (request.method() === "GET" && pathname === "/api/v1/boards") {
      await fulfillJson(route, {
        data: [{
          id: BOARD_ID,
          slug: BOARD_SLUG,
          name: "Playwright board",
          description: null,
          created_at: 1,
          updated_at: 1,
          archived_at: null,
        }],
      })
      return
    }
    if (request.method() === "GET" && pathname === `/api/v1/boards/${BOARD_SLUG}/columns`) {
      await fulfillJson(route, {
        data: [{
          id: "col_todo",
          board_id: BOARD_ID,
          status: "todo",
          title: "Todo",
          position: 1,
          hidden: false,
          wip_limit: null,
          created_at: 1,
          updated_at: 1,
        }],
      })
      return
    }
    if (request.method() === "GET" && pathname === `/api/v1/boards/${BOARD_SLUG}/tasks/by-status`) {
      await fulfillJson(route, taskWindow(url))
      return
    }
    if (request.method() === "GET" && pathname === "/api/v1/events") {
      await fulfillJson(route, { data: [], meta: { next_after: 0 } })
      return
    }
    if (request.method() === "GET" && pathname === `/api/v1/boards/${BOARD_ID}/label-ontology/signals`) {
      readCounts.signals += 1
      await fulfillJson(route, {
        data: signals,
        meta: { include_all: false, limit: 100 },
      })
      return
    }
    if (request.method() === "GET" && pathname === `/api/v1/boards/${BOARD_ID}/label-ontology/review`) {
      readCounts.review += 1
      await fulfillJson(route, {
        data: signals.map(reviewGroup),
        meta: { group_by: "label", include_all: false, limit: 100 },
      })
      return
    }
    if (request.method() === "GET" && pathname.startsWith(`/api/v1/label-ontology/signals/`)) {
      const signalId = pathname.split("/").at(-1) ?? ""
      const signal = signalById.get(signalId)
      if (signal === undefined) {
        await fulfillJson(route, { error: { code: "not_found", message: "signal not found" } }, 404)
        return
      }
      if (signalId === SIGNAL_A) readCounts.detailA += 1
      if (signalId === SIGNAL_B) readCounts.detailB += 1
      await fulfillJson(route, signalDetail(signal))
      return
    }
    if (request.method() === "POST" && pathname === `/api/v1/boards/${BOARD_ID}/label-ontology/actions`) {
      const body = request.postDataJSON() as ActionRequest
      actionRequests.push(body)
      if (scenario === "switch-while-pending" && actionRequests.length === 1) {
        await pendingAction
        await fulfillJson(route, { error: { code: "internal", message: ACTION_FAILURE_MESSAGE } }, 500)
        completedActionCount += 1
        return
      }
      if (scenario === "retry" && actionRequests.length === 1) {
        await fulfillJson(route, { error: { code: "internal", message: ACTION_FAILURE_MESSAGE } }, 500)
        completedActionCount += 1
        return
      }
      await fulfillJson(route, actionResponse(body))
      completedActionCount += 1
      return
    }

    await fulfillJson(route, { error: { code: "not_found", message: `Unhandled fixture path: ${pathname}` } }, 404)
  })

  return {
    actionRequests,
    completedActions: () => completedActionCount,
    releasePendingAction,
    snapshotReadCounts: () => ({ ...readCounts }),
  }
}

async function openOntology(page: Page, signalId = SIGNAL_A): Promise<void> {
  await page.goto(`/app/boards/${BOARD_SLUG}/ontology?signal=${signalId}`, { waitUntil: "domcontentloaded" })
  await expect(page.getByTestId("ontology-screen")).toBeVisible()
  await expect(page.getByRole("heading", { name: signalId === SIGNAL_A ? "Signal A" : "Signal B", exact: true })).toBeVisible()
}

test.describe("Ontology lifecycle browser fences", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem("kb:web:locale", "en")
    })
    await installRuntimeFixture(page)
  })

  test("retries a failed A action with the frozen reason and idempotency key", async ({ page }) => {
    const fixture = await installOntologyFixture(page, "retry")
    await openOntology(page)

    const reason = page.getByRole("textbox", { name: "Review action reason" })
    await reason.fill("Original A reason")
    await page.getByRole("button", { name: "Confirm signal" }).click()
    await expect.poll(() => fixture.actionRequests.length).toBe(1)
    await expect(page.getByRole("button", { name: "Retry action" })).toBeVisible()

    await page.getByRole("button", { name: "Retry action" }).click()
    await expect.poll(() => fixture.actionRequests.length).toBe(2)
    await expect.poll(() => fixture.completedActions()).toBe(2)
    await expect(page.getByRole("button", { name: "Retry action" })).toHaveCount(0)

    const [first, second] = fixture.actionRequests
    expect(first).toMatchObject({
      action_type: "confirm",
      signal_ids: [SIGNAL_A],
      reason: "Original A reason",
    })
    expect(second).toMatchObject({
      action_type: "confirm",
      signal_ids: [SIGNAL_A],
      reason: "Original A reason",
    })
    expect(first?.idempotency_key).toMatch(/^ontology-lifecycle:/)
    expect(second?.idempotency_key).toBe(first?.idempotency_key)
  })

  test("does not project a stale A failure or refresh B after switching during pending action", async ({ page }) => {
    const fixture = await installOntologyFixture(page, "switch-while-pending")
    await openOntology(page)

    await page.getByRole("textbox", { name: "Review action reason" }).fill("Original A reason")
    const confirm = page.getByRole("button", { name: "Confirm signal" })
    await confirm.click()
    await expect.poll(() => fixture.actionRequests.length).toBe(1)
    await expect(confirm).toBeDisabled()

    await page.locator("button").filter({ hasText: "Signal B" }).click()
    await expect(page).toHaveURL(/signal=los_b/)
    await expect(page.getByRole("heading", { name: "Signal B", exact: true })).toBeVisible()
    const bReason = page.getByRole("textbox", { name: "Review action reason" })
    await expect(bReason).toHaveValue("")
    await bReason.fill("B reason")
    await expect(confirm).toBeEnabled()

    const countsBeforeASettles = fixture.snapshotReadCounts()
    fixture.releasePendingAction()
    await expect.poll(() => fixture.completedActions()).toBe(1)
    await page.waitForTimeout(50)

    await expect(page.getByRole("heading", { name: "Signal B", exact: true })).toBeVisible()
    await expect(page.getByRole("button", { name: "Retry action" })).toHaveCount(0)
    await expect(page.getByTestId("ontology-screen")).not.toContainText("Lifecycle action failed")
    await expect(confirm).toBeEnabled()
    expect(fixture.actionRequests[0]).toMatchObject({
      action_type: "confirm",
      signal_ids: [SIGNAL_A],
      reason: "Original A reason",
    })
    expect(fixture.snapshotReadCounts()).toEqual(countsBeforeASettles)
  })

  test("updates the feature offline banner when browser connectivity events fire", async ({ page }) => {
    await installOntologyFixture(page, "retry")
    await openOntology(page)

    await page.evaluate(() => window.dispatchEvent(new Event("offline")))
    await expect(page.getByTestId("ontology-screen")).toContainText("You are offline")

    await page.evaluate(() => window.dispatchEvent(new Event("online")))
    await expect(page.getByTestId("ontology-screen")).not.toContainText("You are offline")
  })
})
