import { readFileSync } from "node:fs"

import type { Page, Route } from "@playwright/test"

import type { WebRuntimeConfig } from "../src/lib/runtime"

const validRuntime = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json", import.meta.url), "utf8"),
) as WebRuntimeConfig

/** 仅用于 Preview 测试 harness：生产 runtime.json 仍由 kanban serve 提供。 */
export async function installRuntimeFixture(page: Page) {
  await page.route("**/app/runtime.json", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(validRuntime),
    })
  })
}

export interface BoardFixtureOptions {
  readonly emptyBoards?: boolean
}

export interface BoardFixture {
  readonly apiRequests: string[]
  setReadyTaskTitle(title: string): void
  getSseConnectionCount(): Promise<number>
  waitForSseConnection(afterCount: number): Promise<void>
  cancelSseConnection(connectionCount: number): Promise<void>
  closeSse(): Promise<void>
  emitHeartbeat(): Promise<void>
  emitTaskUpdated(): Promise<void>
}

const BOARD_ID = "b_default"
const BOARD_SLUG = "default"
const TASK_STATUSES = ["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"] as const

type TaskStatus = (typeof TASK_STATUSES)[number]

function task(status: TaskStatus, position: number, title: string) {
  return {
    id: `t_${status}`,
    board_id: BOARD_ID,
    board_slug: BOARD_SLUG,
    ref: `default#${position}`,
    seq: position,
    title,
    description: null,
    status,
    status_reason: null,
    assignee: null,
    priority: 1,
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

function sseFrame(eventName: string, data: unknown, id: number | null = null): string {
  const lines = [`event: ${eventName}`]
  if (id !== null) lines.push(`id: ${id}`)
  lines.push(`data: ${JSON.stringify(data)}`, "", "")
  return lines.join("\n")
}

async function fulfillJSON(route: Route, payload: unknown): Promise<void> {
  await route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(payload),
  })
}

/**
 * Install coherent board API fixtures and a real fetch-backed SSE stream.
 * The production WebSyncController and FetchSseTransport stay untouched.
 */
export async function installBoardFixture(page: Page, options: BoardFixtureOptions = {}): Promise<BoardFixture> {
  await installRuntimeFixture(page)
  const apiRequests: string[] = []
  let readyTaskTitle = "Ready task"

  await page.route("**/api/v1/**", async (route) => {
    const url = new URL(route.request().url())
    apiRequests.push(`${url.pathname}${url.search}`)

    if (url.pathname === "/api/v1/boards") {
      await fulfillJSON(route, options.emptyBoards ? { data: [] } : {
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
      await fulfillJSON(route, {
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
        ? [task(status, TASK_STATUSES.indexOf(status) + 1, status === "ready" ? readyTaskTitle : `${status} task`)]
        : []
      await fulfillJSON(route, {
        data: { statuses: [{ status: validStatus ? status : "ready", tasks, page: { limit, offset, total: tasks.length } }] },
        meta: { limit, offset },
      })
      return
    }

    if (url.pathname === "/api/v1/events") {
      const after = Number(url.searchParams.get("after") ?? 0)
      await fulfillJSON(route, { data: [], meta: { next_after: Number.isSafeInteger(after) ? after : 0 } })
      return
    }

    await route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found", message: "fixture route not found" } }) })
  })

  await page.addInitScript(() => {
    let streamController: ReadableStreamDefaultController<Uint8Array> | null = null
    let streamConnectionCount = 0
    const streamCancels = new Map<number, () => void>()
    let activeClose: (() => void) | null = null
    const pendingFrames: string[] = []
    const encoder = new TextEncoder()
    const pushFrame = (frame: string) => {
      if (streamController === null) pendingFrames.push(frame)
      else streamController.enqueue(encoder.encode(frame))
    }
    Object.defineProperty(window, "__kanbanPushSse", { configurable: true, value: pushFrame })
    Object.defineProperty(window, "__kanbanCloseSse", { configurable: true, value: () => activeClose?.() })
    Object.defineProperty(window, "__kanbanLateCancelSse", {
      configurable: true,
      value: (connectionCount: number) => {
        const cancel = streamCancels.get(connectionCount)
        if (cancel === undefined) throw new Error(`SSE fixture connection ${connectionCount} is not installed`)
        cancel()
      },
    })
    Object.defineProperty(window, "__kanbanSseConnectionCount", { configurable: true, get: () => streamConnectionCount })

    const nativeFetch = window.fetch.bind(window)
    window.fetch = async (input, init) => {
      const inputURL = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
      const target = new URL(inputURL, window.location.href)
      if (target.pathname !== "/api/v1/stream/events") return nativeFetch(input, init)

      let ownController: ReadableStreamDefaultController<Uint8Array> | null = null
      let ownClosed = false
      const connectionCount = streamConnectionCount + 1
      const clearOwnController = () => {
        const controller = ownController
        if (controller !== null && streamController === controller) streamController = null
        if (activeClose === closeOwn) activeClose = null
        ownController = null
      }
      const closeOwn = () => {
        const controller = ownController
        if (controller === null || ownClosed) return
        ownClosed = true
        if (streamController === controller) streamController = null
        controller.error(new Error("fixture SSE disconnect"))
      }
      streamCancels.set(connectionCount, clearOwnController)
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          ownController = controller
          streamController = controller
          streamConnectionCount += 1
          for (const frame of pendingFrames.splice(0)) controller.enqueue(encoder.encode(frame))
        },
        cancel() {
          clearOwnController()
        },
      })
      activeClose = closeOwn
      const response = new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } })
      Object.defineProperty(response, "url", { configurable: true, value: target.toString() })
      return response
    }
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
    setReadyTaskTitle(title) {
      readyTaskTitle = title
    },
    getSseConnectionCount() {
      return page.evaluate(() => (window as unknown as { __kanbanSseConnectionCount?: number }).__kanbanSseConnectionCount ?? 0)
    },
    waitForSseConnection(afterCount) {
      return page.waitForFunction((count) => ((window as unknown as { __kanbanSseConnectionCount?: number }).__kanbanSseConnectionCount ?? 0) > count, afterCount)
    },
    cancelSseConnection(connectionCount) {
      return page.evaluate((count) => {
        const cancel = (window as unknown as { __kanbanLateCancelSse?: (value: number) => void }).__kanbanLateCancelSse
        if (cancel === undefined) throw new Error("SSE fixture is not installed")
        cancel(count)
      }, connectionCount)
    },
    closeSse() {
      return page.evaluate(() => {
        const close = (window as unknown as { __kanbanCloseSse?: () => void }).__kanbanCloseSse
        if (close === undefined) throw new Error("SSE fixture is not installed")
        close()
      })
    },
    emitHeartbeat() {
      return emit(sseFrame("kb-heartbeat", {}))
    },
    emitTaskUpdated() {
      return emit(sseFrame("task.updated", {
        id: 1,
        event_id: "evt-playwright-1",
        board_id: BOARD_ID,
        task_id: "t_ready",
        run_id: null,
        kind: "task.updated",
        actor: "playwright",
        payload: {},
        created_at: 3,
      }, 1))
    },
  }
}
