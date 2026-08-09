import type { Page, Route } from "@playwright/test"

import { installPersistentSse, installRuntimeFixture } from "./runtime-fixture"

const BOARD_ID = "b_default"
const BOARD_SLUG = "default"
const TASK_ID = "t_ready"
const TASK_REF = "default#1"
const TASK_STATUSES = ["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"] as const

type TaskStatus = (typeof TASK_STATUSES)[number]

export type ExplorerFixtureOptions = {
  readonly emptyBoards?: boolean
  readonly emptyList?: boolean
  readonly emptyMap?: boolean
  readonly emptyRuns?: boolean
  readonly emptyEvents?: boolean
  readonly failList?: boolean
  readonly failMap?: boolean
  readonly failInspector?: boolean
  readonly delayList?: boolean
}

export type ExplorerFixture = {
  readonly apiRequests: string[]
  readonly getSseConnectionCount: () => Promise<number>
  readonly waitForSseConnection: (afterCount: number) => Promise<void>
  readonly emitHeartbeat: () => Promise<void>
  readonly emitTaskUpdated: () => Promise<void>
  readonly releaseList: () => void
}

type FixtureEvent = {
  readonly id: number
  readonly event_id: string
  readonly board_id: string
  readonly task_id: string | null
  readonly run_id: string | null
  readonly kind: "task.created" | "task.updated"
  readonly actor: string | null
  readonly payload: Record<string, unknown>
  readonly created_at: number
}

function fixtureTask(
  status: TaskStatus,
  position: number,
  title: string,
  id = `t_${status}`,
): Record<string, unknown> {
  return {
    id,
    board_id: BOARD_ID,
    board_slug: BOARD_SLUG,
    ref: status === "ready" ? TASK_REF : `default#${position}`,
    seq: position,
    title,
    description: status === "ready" ? "Task Inspector fixture" : null,
    status,
    status_reason: null,
    assignee: status === "ready" ? "playwright" : null,
    priority: status === "ready" ? 2 : 1,
    position,
    scheduled_at: null,
    due_at: null,
    created_by: "playwright",
    created_at: 1,
    updated_at: 2,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: status === "ready" ? "r_finished" : null,
    retry_count: 0,
    max_retries: 2,
    result_summary: null,
    result: null,
    metadata: { source: "playwright" },
    lock_version: 1,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "planned",
    required_step_count: status === "ready" ? 1 : 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
  }
}

function fixtureEvent(id: number, kind: FixtureEvent["kind"], taskId: string | null = TASK_ID): FixtureEvent {
  return {
    id,
    event_id: `evt-playwright-${id}`,
    board_id: BOARD_ID,
    task_id: taskId,
    run_id: null,
    kind,
    actor: "playwright",
    payload: kind === "task.created" ? { status: "ready" } : {},
    created_at: 1_700_000_000 + id,
  }
}

function sseFrame(eventName: string, data: unknown, id: number | null = null): string {
  const lines = [`event: ${eventName}`]
  if (id !== null) lines.push(`id: ${id}`)
  lines.push(`data: ${JSON.stringify(data)}`, "", "")
  return lines.join("\n")
}

async function fulfillJson(route: Route, payload: unknown): Promise<void> {
  await route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(payload),
  })
}

async function fulfillUnavailable(route: Route): Promise<void> {
  await route.fulfill({
    status: 503,
    contentType: "text/plain",
    body: "service unavailable",
  })
}

function boardSummary() {
  return {
    id: TASK_ID,
    board_id: BOARD_ID,
    board_slug: BOARD_SLUG,
    ref: TASK_REF,
    title: "Task Inspector fixture",
    status: "ready",
  }
}

/**
 * Browser-only coherent API fixture. It serves the same generated response shapes
 * consumed by BoardLive and Explorer, and replaces only the network boundary.
 */
export async function installExplorerFixture(page: Page, options: ExplorerFixtureOptions = {}): Promise<ExplorerFixture> {
  await installRuntimeFixture(page)
  await installPersistentSse(page)
  const apiRequests: string[] = []
  const readyTask = fixtureTask("ready", 1, "Ready task", TASK_ID)
  const listTasks = [readyTask, fixtureTask("todo", 2, "Todo task")]
  let events: FixtureEvent[] = options.emptyEvents ? [] : [fixtureEvent(1, "task.created")]
  let releaseList: () => void = () => undefined
  const listGate = options.delayList
    ? new Promise<void>((resolve) => {
        releaseList = resolve
      })
    : Promise.resolve()

  await page.route("**/api/v1/**", async (route) => {
    const url = new URL(route.request().url())
    apiRequests.push(`${url.pathname}${url.search}`)

    if (url.pathname === "/api/v1/boards") {
      await fulfillJson(route, options.emptyBoards ? { data: [] } : {
        data: [{
          id: BOARD_ID,
          slug: BOARD_SLUG,
          name: "Default Board",
          description: null,
          created_at: 1,
          updated_at: 2,
          archived_at: null,
        }],
      })
      return
    }

    if (url.pathname === `/api/v1/boards/${BOARD_SLUG}/columns`) {
      await fulfillJson(route, {
        data: TASK_STATUSES.map((status, index) => ({
          id: `col_${status}`,
          board_id: BOARD_ID,
          status,
          title: status[0]?.toUpperCase() + status.slice(1),
          position: index + 1,
          hidden: status === "archived",
          wip_limit: null,
          created_at: 1,
          updated_at: 2,
        })),
      })
      return
    }

    if (url.pathname === `/api/v1/boards/${BOARD_SLUG}/tasks/by-status`) {
      const status = url.searchParams.get("status") as TaskStatus | null
      const limit = Number(url.searchParams.get("limit") ?? 1000)
      const offset = Number(url.searchParams.get("offset") ?? 0)
      const validStatus = status !== null && TASK_STATUSES.includes(status)
      const tasks = validStatus && offset === 0
        ? [status === "ready" ? readyTask : fixtureTask(status, TASK_STATUSES.indexOf(status) + 1, `${status} task`)]
        : []
      await fulfillJson(route, {
        data: { statuses: [{ status: validStatus ? status : "ready", tasks, page: { limit, offset, total: tasks.length } }] },
        meta: { limit, offset },
      })
      return
    }

    if (url.pathname === `/api/v1/boards/${BOARD_SLUG}/tasks`) {
      if (route.request().method() === "POST") {
        const body = JSON.parse(route.request().postData() ?? "{}") as { readonly task_id?: unknown; readonly title?: unknown }
        const taskId = typeof body.task_id === "string" ? body.task_id : "t_created"
        await fulfillJson(route, { data: fixtureTask("todo", 3, typeof body.title === "string" ? body.title : "Created task", taskId) })
        return
      }
      if (options.failList) {
        await fulfillUnavailable(route)
        return
      }
      await listGate
      const limit = Number(url.searchParams.get("limit") ?? 100)
      const offset = Number(url.searchParams.get("offset") ?? 0)
      const visibleTasks = options.emptyList ? [] : listTasks
      await fulfillJson(route, { data: visibleTasks, meta: { limit, offset, total: visibleTasks.length } })
      return
    }

    if (url.pathname === `/api/v1/boards/${BOARD_SLUG}/task-map`) {
      if (options.failMap) {
        await fulfillUnavailable(route)
        return
      }
      const includeDoneContext = url.searchParams.get("include_done_context") === "true"
      const includeArchivedContext = url.searchParams.get("include_archived_context") === "true"
      const hideIsolated = url.searchParams.get("hide_isolated") === "true"
      const contextDepth = Number(url.searchParams.get("context_depth") ?? 1)
      const limitNodes = Number(url.searchParams.get("limit_nodes") ?? 240)
      const nodes = options.emptyMap
        ? []
        : [{ task: readyTask, role: "active", context_only: false }]
      await fulfillJson(route, {
        data: {
          nodes,
          edges: [],
          meta: {
            depth: 0,
            context_depth: contextDepth,
            generated_at: 1,
            node_count: nodes.length,
            edge_count: 0,
            truncated: false,
            active_statuses: ["ready", "running", "review", "blocked"],
            active_only: true,
            include_done_context: includeDoneContext,
            include_archived_context: includeArchivedContext,
            hide_isolated: hideIsolated,
            limit_nodes: limitNodes,
          },
        },
      })
      return
    }

    if (url.pathname === `/api/v1/tasks/${TASK_ID}`) {
      if (options.failInspector) {
        await fulfillUnavailable(route)
        return
      }
      await fulfillJson(route, { data: readyTask })
      return
    }

    if (url.pathname === `/api/v1/tasks/${TASK_ID}/neighborhood`) {
      const neighborhoodTask = fixtureTask("ready", 1, "Ready task", TASK_ID)
      await fulfillJson(route, {
        data: {
          center_task_id: TASK_ID,
          nodes: [{ task: neighborhoodTask, role: "center", context_only: false }],
          edges: [],
          meta: {
            depth: 1,
            context_depth: 0,
            generated_at: 1,
            node_count: 1,
            edge_count: 0,
            truncated: false,
            active_statuses: ["ready", "running", "review", "blocked"],
            active_only: true,
            include_done_context: true,
            include_archived_context: false,
            hide_isolated: false,
            limit_nodes: 40,
          },
        },
      })
      return
    }

    if (url.pathname === `/api/v1/tasks/${TASK_ID}/dependencies`) {
      await fulfillJson(route, { data: { task: boardSummary(), parents: [], children: [], edges: [] } })
      return
    }

    if (url.pathname === `/api/v1/tasks/${TASK_ID}/steps`) {
      await fulfillJson(route, {
        data: {
          task_id: TASK_ID,
          steps: [{
            id: "step_fixture",
            parent_task_id: TASK_ID,
            title: "Verify browser path",
            body: "Open and close the inspector",
            linked_task: null,
            position: 1,
            required: true,
            status: "todo",
            resolution_note: null,
            resolved_by: null,
            resolved_at: null,
            created_by: "playwright",
            created_at: 1,
            updated_by: "playwright",
            updated_at: 1,
          }],
          execution_plan: {
            board_id: BOARD_ID,
            task_id: TASK_ID,
            state: "planned",
            reason: null,
            updated_by: "playwright",
            updated_at: 1,
          },
        },
      })
      return
    }

    if (url.pathname === `/api/v1/tasks/${TASK_ID}/runs`) {
      const runs = options.emptyRuns
        ? []
        : [
            { id: "r_active", task_id: TASK_ID, status: "running", worker_profile: "manual", worker_pid: null, claim_owner: "runner", started_at: 2, finished_at: null, exit_code: null, summary: null, error: null, has_log: false, metadata: {} },
            { id: "r_finished", task_id: TASK_ID, status: "succeeded", worker_profile: "manual", worker_pid: null, claim_owner: "runner", started_at: 1, finished_at: 2, exit_code: 0, summary: null, error: null, has_log: true, metadata: {} },
          ]
      await fulfillJson(route, { data: runs })
      return
    }

    if (url.pathname === "/api/v1/runs/r_finished/log") {
      await fulfillJson(route, { data: { run_id: "r_finished", content: "playwright fixture log", truncated: false } })
      return
    }

    if (url.pathname === `/api/v1/tasks/${TASK_ID}/comments`) {
      await fulfillJson(route, { data: [] })
      return
    }

    if (url.pathname === "/api/v1/events") {
      const after = Number(url.searchParams.get("after") ?? 0)
      const taskId = url.searchParams.get("task_id")
      const selected = events.filter((event) => event.id > after && (taskId === null || event.task_id === taskId))
      const limit = Number(url.searchParams.get("limit") ?? 150)
      const page = selected.slice(0, limit)
      const nextAfter = page.length === 0 ? after : page[page.length - 1]?.id ?? after
      await fulfillJson(route, { data: page, meta: { next_after: nextAfter } })
      return
    }

    await route.fulfill({ status: 404, contentType: "text/plain", body: "fixture route not found" })
  })

  async function emit(frame: string): Promise<void> {
    await page.evaluate((value) => {
      const push = (window as unknown as { __kanbanPushSse?: (next: string) => void }).__kanbanPushSse
      if (push === undefined) throw new Error("SSE fixture is not installed")
      push(value)
    }, frame)
  }

  return {
    apiRequests,
    getSseConnectionCount: () => page.evaluate(() => (window as unknown as { __kanbanSseConnectionCount?: number }).__kanbanSseConnectionCount ?? 0),
    waitForSseConnection: (afterCount) => page.waitForFunction((count) => ((window as unknown as { __kanbanSseConnectionCount?: number }).__kanbanSseConnectionCount ?? 0) > count, afterCount),
    releaseList: () => releaseList(),
    emitHeartbeat: () => emit(sseFrame("kb-heartbeat", {})),
    emitTaskUpdated: async () => {
      const updated = fixtureEvent(2, "task.updated")
      events = [...events, updated]
      await emit(sseFrame("task.updated", updated, updated.id))
    },
  }
}
