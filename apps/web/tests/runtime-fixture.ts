import { readFileSync } from "node:fs"

import type { Page } from "@playwright/test"

import { installRpcFixture, type RpcFixture } from "./rpc-fixture"

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
  readonly rpc: RpcFixture
  readonly apiRequests: string[]
  setReadyTaskTitle(title: string): void
  getQueryConnectionCount(): Promise<number>
  waitForQueryConnection(afterCount: number): Promise<void>
  cancelQueryConnection(connectionCount: number): Promise<void>
  closeQuery(): Promise<void>
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

/** Preview 使用正式 QueryService 完整查询；业务 fixture 只负责产生领域 DTO。 */
export async function installBoardFixture(page: Page, options: BoardFixtureOptions = {}): Promise<BoardFixture> {
  await installRuntimeFixture(page)
  const rpc = await installRpcFixture(page)
  const apiRequests: string[] = []
  let readyTaskTitle = "Ready task"
  rpc.handle("*", call => {
    apiRequests.push(call.method)
    switch (call.method) {
      case "ListBoards": return { data: options.emptyBoards ? [] : [{ id: BOARD_ID, slug: BOARD_SLUG, name: "Default Board", description: null, created_at: 1, updated_at: 2, archived_at: null }] }
      case "ListBoardColumns": return { data: TASK_STATUSES.map((status, index) => ({ id: 'col_' + status, board_id: BOARD_ID, status, title: status[0]?.toUpperCase() + status.slice(1), position: index + 1, hidden: status === "archived", wip_limit: null, created_at: 1, updated_at: 2 })) }
      case "ListTasks": {
        const tasks = TASK_STATUSES.filter(status => status !== "archived").map((status, index) => task(status, index + 1, status === "ready" ? readyTaskTitle : status + " task"))
        return { data: tasks, meta: { limit: Number(call.query.limit ?? 100), offset: Number(call.query.offset ?? 0), total: tasks.length } }
      }
      case "ListTasksByStatus": {
        const status = call.query.status as TaskStatus
        const tasks = [task(status, TASK_STATUSES.indexOf(status) + 1, status === "ready" ? readyTaskTitle : status + " task")]
        const limit = Number(call.query.limit ?? 1000), offset = Number(call.query.offset ?? 0)
        return { data: { statuses: [{ status, tasks, page: { limit, offset, total: tasks.length } }] }, meta: { limit, offset } }
      }
      case "ListEvents": case "RecentEvents": return { data: [], meta: { next_after: Number(call.query.after ?? 0) } }
    }
  })
  return {
    rpc, apiRequests,
    setReadyTaskTitle(title) { readyTaskTitle = title },
    getQueryConnectionCount: () => rpc.connectionCount(),
    waitForQueryConnection: count => rpc.waitForConnection(count),
    cancelQueryConnection: count => rpc.close(count),
    closeQuery: () => rpc.close(),
    emitHeartbeat: () => rpc.heartbeat(),
    emitTaskUpdated: () => rpc.publish(),
  }
}
