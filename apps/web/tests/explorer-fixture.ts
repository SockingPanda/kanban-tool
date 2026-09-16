import type { Page } from "@playwright/test"

import { installRuntimeFixture } from "./runtime-fixture"
import { installRpcFixture, unavailable, type RpcFixture } from "./rpc-fixture"

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
  readonly withAssets?: boolean
  readonly failLabelAddOnce?: boolean
  readonly failInspectorReadsAfterLabelAdd?: number
}

export type ExplorerFixture = {
  readonly rpc: RpcFixture
  readonly readyTask: () => Record<string, unknown>
  readonly setReadyTaskStatus: (status: TaskStatus) => void
  readonly apiRequests: string[]
  readonly writeRequests: string[]
  readonly getQueryConnectionCount: () => Promise<number>
  readonly waitForQueryConnection: (afterCount: number) => Promise<void>
  readonly emitHeartbeat: () => Promise<void>
  readonly emitTaskUpdated: () => Promise<void>
  readonly releaseList: () => void
  readonly failNextInspectorReads: (count?: number) => void
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

/** 单一 fixture 状态产生正式 unary/QueryResult DTO，不绕过浏览器的生产读写路径。 */
export async function installExplorerFixture(page: Page, options: ExplorerFixtureOptions = {}): Promise<ExplorerFixture> {
  await installRuntimeFixture(page)
  const rpc = await installRpcFixture(page)
  const apiRequests: string[] = [], writeRequests: string[] = []
  const readyTask = fixtureTask("ready", 1, "Ready task", TASK_ID)
  const listTasks = [readyTask, fixtureTask("todo", 2, "Todo task")]
  const attachments: Record<string, unknown>[] = options.withAssets ? [{ id: "a_fixture", board_id: BOARD_ID, task_id: TASK_ID, filename: "fixture.txt", rel_path: "attachments/fixture.txt", content_type: "text/plain", size_bytes: 7, sha256: null, created_by: "playwright", created_at: 1 }] : []
  const stepsByTask = new Map<string, Record<string, unknown>[]>([[TASK_ID, []]])
  let events: FixtureEvent[] = options.emptyEvents ? [] : [fixtureEvent(1, "task.created")]
  let labelAddFailuresRemaining = options.failLabelAddOnce ? 1 : 0
  let inspectorReadFailuresRemaining = 0
  let inspectorReadFailuresAfterLabelAdd = options.failInspectorReadsAfterLabelAdd ?? 0
  let releaseList: () => void = () => undefined
  const listGate = options.delayList ? new Promise<void>(resolve => { releaseList = resolve }) : Promise.resolve()
  const executionPlan = (id: string) => ({ board_id: BOARD_ID, task_id: id, state: "planned", reason: null, updated_by: "playwright", updated_at: 1 })
  const step = (id: string, title: string, position: number, required: boolean, body: unknown = null) => ({ id: 'step_' + position, parent_task_id: id, title, body, linked_task: null, position, required, status: "todo", resolution_note: null, resolved_by: null, resolved_at: null, created_by: "playwright", created_at: 1, updated_by: "playwright", updated_at: 1 })
  const summary = () => ({ id: TASK_ID, board_id: BOARD_ID, board_slug: BOARD_SLUG, ref: TASK_REF, title: "Task Inspector fixture", status: "ready" })
  rpc.handle("*", async call => {
    apiRequests.push(call.method)
    if (call.kind === "unary" && !["SuggestTaskLabels", "DownloadAttachment", "DownloadFile"].includes(call.method)) writeRequests.push(call.method)
    const { query, input, path } = call
    const taskId = String(path.task_id ?? TASK_ID)
    switch (call.method) {
      case "ListBoards": return { data: options.emptyBoards ? [] : [{ id: BOARD_ID, slug: BOARD_SLUG, name: "Default Board", description: null, created_at: 1, updated_at: 2, archived_at: null }] }
      case "ListBoardColumns": return { data: TASK_STATUSES.map((status, index) => ({ id: 'col_' + status, board_id: BOARD_ID, status, title: status[0]?.toUpperCase() + status.slice(1), position: index + 1, hidden: status === "archived", wip_limit: null, created_at: 1, updated_at: 2 })) }
      case "ListTasksByStatus": {
        const status = query.status as TaskStatus, limit = Number(query.limit ?? 1000), offset = Number(query.offset ?? 0)
        const tasks = offset === 0 ? [...(status === readyTask.status ? [readyTask] : []), ...(status !== "ready" ? [fixtureTask(status, TASK_STATUSES.indexOf(status) + 1, status + " task")] : [])] : []
        return { data: { statuses: [{ status, tasks, page: { limit, offset, total: tasks.length } }] }, meta: { limit, offset } }
      }
      case "ListTasks": {
        if (options.failList) throw unavailable()
        await listGate
        const tasks = options.emptyList ? [] : listTasks
        return { data: tasks, meta: { limit: Number(query.limit ?? 100), offset: Number(query.offset ?? 0), total: tasks.length } }
      }
      case "CreateTask": {
        const id = String(input.task_id ?? "t_created")
        const created = fixtureTask("todo", 3, String(input.title ?? "Created task"), id)
        listTasks.push(created); stepsByTask.set(id, [])
        return { data: created }
      }
      case "GetTask": {
        if (options.failInspector) throw unavailable()
        if (inspectorReadFailuresRemaining > 0) { inspectorReadFailuresRemaining -= 1; throw unavailable() }
        return { data: listTasks.find(task => task.id === taskId) ?? readyTask }
      }
      case "UpdateTask": {
        for (const key of ["title", "description", "assignee", "priority", "scheduled_at", "due_at"]) if (Object.hasOwn(input, key)) readyTask[key] = input[key]
        readyTask.lock_version = Number(readyTask.lock_version) + 1
        readyTask.updated_at = Number(readyTask.updated_at) + 1
        return { data: readyTask }
      }
      case "BoardTaskMap": case "TaskNeighborhood": {
        if (call.method === "BoardTaskMap" && options.failMap) throw unavailable()
        const neighborhood = call.method === "TaskNeighborhood"
        const nodes = options.emptyMap && !neighborhood ? [] : [{ task: readyTask, role: neighborhood ? "center" : "active", context_only: false }]
        return { data: { ...(neighborhood ? { center_task_id: TASK_ID } : {}), nodes, edges: [], meta: { depth: neighborhood ? 1 : 0, context_depth: Number(query.context_depth ?? (neighborhood ? 0 : 1)), generated_at: 1, node_count: nodes.length, edge_count: 0, truncated: false, active_statuses: ["ready", "running", "review", "blocked"], active_only: true, include_done_context: neighborhood || query.include_done_context === true, include_archived_context: query.include_archived_context === true, hide_isolated: query.hide_isolated === true, limit_nodes: Number(query.limit_nodes ?? (neighborhood ? 40 : 240)) } } }
      }
      case "ListDependencies": return { data: { task: summary(), parents: [], children: [], edges: [] } }
      case "ListSteps": return { data: { task_id: taskId, steps: [...(stepsByTask.get(taskId) ?? []), ...(taskId === TASK_ID ? [{ ...step(taskId, "Verify browser path", 1, true, "Open and close the inspector"), id: "step_fixture" }] : [])], execution_plan: executionPlan(taskId) } }
      case "CreateStep": {
        const steps = stepsByTask.get(taskId) ?? []
        steps.push(step(taskId, String(input.title ?? "Created step"), steps.length + 1, input.required === true, input.body ?? null)); stepsByTask.set(taskId, steps)
        return { data: { task_id: taskId, steps, execution_plan: executionPlan(taskId) } }
      }
      case "ListRuns": return { data: options.emptyRuns ? [] : [
        { id: "r_active", task_id: TASK_ID, status: "running", worker_profile: "manual", worker_pid: null, claim_owner: "runner", started_at: 2, finished_at: null, exit_code: null, summary: null, error: null, has_log: false, metadata: {} },
        { id: "r_finished", task_id: TASK_ID, status: "succeeded", worker_profile: "manual", worker_pid: null, claim_owner: "runner", started_at: 1, finished_at: 2, exit_code: 0, summary: null, error: null, has_log: true, metadata: {} },
      ] }
      case "GetRunLog": return { data: { run_id: "r_finished", content: "playwright fixture log", truncated: false } }
      case "ListComments": return { data: [] }
      case "CreateComment": return { data: { id: "comment_fixture", board_id: BOARD_ID, task_id: TASK_ID, author: "playwright", author_type: "user", agent_type: null, body: "fixture comment", kind: "note", metadata: {}, created_at: 1 } }
      case "ListAttachments": return { data: attachments }
      case "DownloadFile":
      case "DownloadAttachment": return { attachment: attachments.find(item => item.id === path.attachment_id), content: new TextEncoder().encode("fixture") }
      case "DeleteAttachment": { const index = attachments.findIndex(item => item.id === path.attachment_id); if (index >= 0) attachments.splice(index, 1); return { data: { deleted: true } } }
      case "ListTaskLabels": return { data: readyTask.labels }
      case "SuggestTaskLabels": return { data: { task_id: TASK_ID, board_id: BOARD_ID, selected_labels: [], candidates: [{ label_id: "l_fixture", label_name: "fixture", score: 0.9, weight: 1, already_applied: false, evidence_atoms: [], negative_evidence_atoms: [] }], coverage: 0.5, coverage_cosine: 0.5, residual_norm: 0.1, needs_new_label: false, reason_codes: [], degraded: false, diagnostics: [] } }
      case "AddTaskLabel": {
        if (labelAddFailuresRemaining > 0) { labelAddFailuresRemaining -= 1; throw unavailable() }
        const name = String(input.name ?? "fixture"), labels = Array.isArray(readyTask.labels) ? readyTask.labels : []
        readyTask.labels = [...labels, { id: 'l_' + name, board_id: BOARD_ID, name, color: null, created_at: 1, updated_at: 1 }]
        readyTask.lock_version = Number(readyTask.lock_version) + 1
        if (inspectorReadFailuresAfterLabelAdd > 0) { inspectorReadFailuresRemaining = inspectorReadFailuresAfterLabelAdd; inspectorReadFailuresAfterLabelAdd = 0 }
        return { data: readyTask, meta: null }
      }
      case "RemoveTaskLabel": return { data: readyTask }
      case "ListEvents": case "RecentEvents": {
        const after = Number(query.after ?? 0), limit = Number(query.limit ?? 150)
        const selected = events.filter(event => event.id > after && (!query.task_id || event.task_id === query.task_id))
        const result = call.method === "RecentEvents" ? selected.slice(-limit) : selected.slice(0, limit)
        return { data: result, meta: { next_after: result.at(-1)?.id ?? after } }
      }
    }
  })
  return {
    rpc, apiRequests, writeRequests,
    readyTask: () => ({ ...readyTask }), setReadyTaskStatus: status => { readyTask.status = status },
    getQueryConnectionCount: () => rpc.connectionCount(), waitForQueryConnection: count => rpc.waitForConnection(count),
    releaseList: () => releaseList(), emitHeartbeat: () => rpc.heartbeat(),
    emitTaskUpdated: async () => { events = [...events, fixtureEvent(2, "task.updated")]; await rpc.publish() },
    failNextInspectorReads(count = 1) { inspectorReadFailuresRemaining = Math.max(0, Math.floor(count)) },
  }
}
