import { isInteger, type Integer } from '../../domain/integer'
import type { RpcCall } from "./rpc-transport";
import type { WebRuntimeConfig } from "../../lib/runtime";

import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../../domain/board-slug";

import { asCanonicalBoardId, type CanonicalBoardId } from "../../domain/board-id";

import { parseApiListBoardsQuery } from "../../lib/api/generated/contracts/api-list-boards-query";

import { type ApiListBoardsResponseContract } from "../../lib/api/generated/contracts/api-list-boards-response";

import { parseApiListTasksPath } from "../../lib/api/generated/contracts/api-list-tasks-path";

import { parseApiListTasksQuery, type ApiListTasksQueryContract } from "../../lib/api/generated/contracts/api-list-tasks-query";

import { type ApiListTasksResponseContract } from "../../lib/api/generated/contracts/api-list-tasks-response";

import { parseApiBoardTaskMapPath } from "../../lib/api/generated/contracts/api-board-task-map-path";

import { parseApiBoardTaskMapQuery } from "../../lib/api/generated/contracts/api-board-task-map-query";

import { type ApiBoardTaskMapResponseContract } from "../../lib/api/generated/contracts/api-board-task-map-response";

import { parseApiGetTaskPath } from "../../lib/api/generated/contracts/api-get-task-path";

import { type ApiGetTaskResponseContract } from "../../lib/api/generated/contracts/api-get-task-response";

import { parseApiTaskNeighborhoodPath } from "../../lib/api/generated/contracts/api-task-neighborhood-path";

import { parseApiTaskNeighborhoodQuery } from "../../lib/api/generated/contracts/api-task-neighborhood-query";

import { type ApiTaskNeighborhoodResponseContract } from "../../lib/api/generated/contracts/api-task-neighborhood-response";

import { parseApiListDependenciesPath } from "../../lib/api/generated/contracts/api-list-dependencies-path";

import { type ApiListDependenciesResponseContract } from "../../lib/api/generated/contracts/api-list-dependencies-response";

import { parseApiListStepsPath } from "../../lib/api/generated/contracts/api-list-steps-path";

import { type ApiListStepsResponseContract } from "../../lib/api/generated/contracts/api-list-steps-response";

import { parseApiListRunsPath } from "../../lib/api/generated/contracts/api-list-runs-path";

import { type ApiListRunsResponseContract } from "../../lib/api/generated/contracts/api-list-runs-response";

import { parseApiGetRunLogPath } from "../../lib/api/generated/contracts/api-get-run-log-path";

import { type ApiGetRunLogResponseContract } from "../../lib/api/generated/contracts/api-get-run-log-response";

import { parseApiListCommentsPath } from "../../lib/api/generated/contracts/api-list-comments-path";

import { type ApiListCommentsResponseContract } from "../../lib/api/generated/contracts/api-list-comments-response";

import { parseApiListTaskLabelsPath } from "../../lib/api/generated/contracts/api-list-task-labels-path";

import { type ApiListTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-list-task-labels-response";

import { parseApiListAttachmentsPath } from "../../lib/api/generated/contracts/api-list-attachments-path";

import { type ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response";


import { type ApiListEventsResponseContract } from "../../lib/api/generated/contracts/api-list-events-response";

import { ContractValidationError } from "../../lib/api/generated/runtime";

import { RpcTransportError, type RpcTransportErrorKind, type RpcTransport, type RpcTransportOptions } from "./rpc-transport";

export type TaskListStatus = NonNullable<ApiListTasksQueryContract["status"]>[number]

export type TaskListSort = NonNullable<ApiListTasksQueryContract["sort"]>

export type TaskListPlanFilter = NonNullable<ApiListTasksQueryContract["plan_filter"]>[number]

export interface TaskListQueryState {
  readonly status: readonly TaskListStatus[]
  readonly priority: readonly number[]
  readonly plan: readonly TaskListPlanFilter[]
  readonly search: string
  readonly sort: TaskListSort
  readonly page: number
  readonly limit: number
  readonly includeArchived: boolean
}

export const defaultTaskListQuery: TaskListQueryState = Object.freeze({
  status: [],
  priority: [],
  plan: [],
  search: "",
  sort: "updated_at",
  page: 1,
  limit: 100,
  includeArchived: false,
})

export const taskStatuses = new Set<TaskListStatus>([
  "triage",
  "todo",
  "scheduled",
  "ready",
  "running",
  "blocked",
  "review",
  "done",
  "archived",
])

export const taskSorts = new Set<TaskListSort>([
  "seq",
  "-seq",
  "title",
  "-title",
  "status",
  "-status",
  "position",
  "-position",
  "priority",
  "-priority",
  "assignee",
  "-assignee",
  "scheduled_at",
  "-scheduled_at",
  "due_at",
  "-due_at",
  "created_at",
  "-created_at",
  "updated_at",
  "-updated_at",
])

export const taskPlanFilters = new Set<TaskListPlanFilter>([
  "plan_needed",
  "has_steps",
  "incomplete_required_steps",
])

export function unique<T>(values: readonly T[]): T[] {
  return [...new Set(values)]
}

export function parseInteger(value: string | null, fallback: number, minimum: number, maximum: number): number {
  if (value === null || !/^\d+$/.test(value)) return fallback
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed >= minimum && parsed <= maximum ? parsed : fallback
}

export function parseSearch(value: string | null): string {
  if (value === null || value.includes("\u0000")) return ""
  const trimmed = value.trim()
  return trimmed.length <= 1024 ? trimmed : trimmed.slice(0, 1024)
}

export function serializeTaskListQuery(query: TaskListQueryState): string {
  const params = new URLSearchParams()
  for (const status of query.status) params.append("status", status)
  for (const priority of query.priority) params.append("priority", String(priority))
  for (const plan of query.plan) params.append("plan", plan)
  if (query.search.trim()) params.set("q", query.search.trim())
  if (query.sort !== defaultTaskListQuery.sort) params.set("sort", query.sort)
  if (query.page !== defaultTaskListQuery.page) params.set("page", String(query.page))
  if (query.limit !== defaultTaskListQuery.limit) params.set("limit", String(query.limit))
  if (query.includeArchived) params.set("include_archived", "true")
  const encoded = params.toString()
  return encoded ? `?${encoded}` : ""
}

export function parseContract<T>(contractId: string, parser: (value: unknown) => T, payload: unknown): T {
  try {
    return parser(payload)
  } catch (cause) {
    if (cause instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", `Web API 响应不符合 ${contractId} contract。`, { contractId, cause })
    }
    throw cause
  }
}

export type ExplorerReadErrorKind =
  | "empty"
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_contract"
  | "anomaly"
  | "cross_origin"
  | "malformed_url"
  | "invalid_content_type"
  | "response_too_large"

export class ExplorerReadError extends Error {
  readonly kind: ExplorerReadErrorKind
  readonly reason: "board-not-found" | "task-not-found" | null
  readonly status: number | null
  readonly contractId: string | null
  readonly apiError: RpcTransportError["apiError"]

  constructor(
    kind: ExplorerReadErrorKind,
    message: string,
    options: {
      reason?: ExplorerReadError["reason"]
      status?: number
      contractId?: string
      apiError?: ExplorerReadError["apiError"]
      cause?: unknown
    } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "ExplorerReadError"
    this.kind = kind
    this.reason = options.reason ?? null
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
    this.apiError = options.apiError ?? null
  }

  get code(): string | null {
    return this.apiError?.code ?? null
  }
}

export interface ExplorerReadDependencies extends RpcTransportOptions {
  /** 读模型通过具名 RPC 请求业务 DTO。 */
  readonly transport?: RpcTransport
}

export const MAX_EXPLORER_TOTAL_PROTOBUF_BYTES = 64 * 1024 * 1024

export class ExplorerReadBudget {
  private totalBytes = 0

  consume(bytes: number): void {
    if (!Number.isSafeInteger(bytes) || bytes < 0 || this.totalBytes > MAX_EXPLORER_TOTAL_PROTOBUF_BYTES - bytes) {
      throw new ExplorerReadError("anomaly", `Explorer read Protobuf 超过 ${MAX_EXPLORER_TOTAL_PROTOBUF_BYTES} 字节预算。`)
    }
    this.totalBytes += bytes
  }
}

export interface ExplorerReadOptions extends ExplorerReadDependencies {
  readonly signal?: AbortSignal
  readonly includeArchived?: boolean
  /** 同一次展示映射共用的 Protobuf 结果预算。 */
  readonly budget?: ExplorerReadBudget
}

export interface ExplorerBoardIdentity {
  readonly selector: string
  readonly id: CanonicalBoardId
  readonly slug: CanonicalBoardSlug
  readonly name: string
}

export interface ExplorerTaskListPage {
  readonly board: ExplorerBoardIdentity
  readonly tasks: readonly ApiListTasksResponseContract["data"][number][]
  readonly meta: ApiListTasksResponseContract["meta"]
}

export const BOARD_EVENTS_PAGE_LIMIT = 150

export type ExplorerEvent = ApiListEventsResponseContract["data"][number]

export interface BoardEventsReadModel {
  readonly board: ExplorerBoardIdentity
  readonly taskId: string | null
  readonly events: readonly ExplorerEvent[]
  readonly meta: {
    readonly count: number
    readonly nextAfter: Integer
    readonly limit: number
  }
}

export function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

export function throwIfAborted(signal: AbortSignal | undefined): void {
  if (!signal?.aborted) return
  const error = new Error("Explorer 事件读取已取消。")
  error.name = "AbortError"
  throw error
}

export function explorerErrorKind(kind: RpcTransportErrorKind): ExplorerReadErrorKind {
  switch (kind) {
    case "cross_origin":
    case "malformed_url":
    case "offline":
    case "http":
    case "invalid_json":
    case "invalid_content_type":
    case "response_too_large":
      return kind
    case "invalid_headers":
    case "invalid_bytes":
      return "anomaly"
  }
}

export function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof ExplorerReadError) throw error
  if (error instanceof RpcTransportError) {
    throw new ExplorerReadError(explorerErrorKind(error.kind), error.message, {
      status: error.status ?? undefined,
      apiError: error.apiError,
      cause: error,
    })
  }
  throw error
}

export function boardListPath(includeArchived: boolean): RpcCall {
  const query = parseApiListBoardsQuery({ include_archived: includeArchived })
  return { method: "ListBoards", query }
}

export function validBoardId(value: string): CanonicalBoardId | null {
  if (value.trim() !== value || !value.startsWith("b_") || value.length <= 2 || hasUnsafeIdentityCharacters(value)) return null
  try {
    return asCanonicalBoardId(value)
  } catch {
    return null
  }
}

export function hasUnsafeIdentityCharacters(value: string): boolean {
  if (/[\\/?#]/.test(value)) return true
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) return true
  }
  return false
}

export function resolveBoard(boards: ApiListBoardsResponseContract["data"], selector: string): ExplorerBoardIdentity {
  const requested = selector.trim()
  const identities = boards.map((candidate) => {
    const id = validBoardId(candidate.id)
    const slug = parseCanonicalBoardSlug(candidate.slug)
    if (!id || !slug || candidate.name.trim().length === 0) {
      throw new ExplorerReadError("anomaly", "看板列表包含无效 canonical identity。")
    }
    return { candidate, id, slug }
  })
  const selectors = new Set<string>()
  for (const identity of identities) {
    if (selectors.has(identity.id) || selectors.has(identity.slug)) {
      throw new ExplorerReadError("anomaly", "看板列表包含重复 canonical identity。")
    }
    selectors.add(identity.id)
    selectors.add(identity.slug)
  }
  const matches = identities.filter(({ candidate }) => candidate.id === requested || candidate.slug === requested)
  if (matches.length === 0) {
    throw new ExplorerReadError("empty", "请求的看板 selector 未在看板列表中精确匹配。", {
      reason: "board-not-found",
    })
  }
  if (matches.length !== 1) throw new ExplorerReadError("anomaly", "请求的看板 selector 不唯一。")
  const identity = matches[0]
  if (identity === undefined) throw new ExplorerReadError("empty", "请求的看板 selector 未在看板列表中精确匹配。", { reason: "board-not-found" })
  return Object.freeze({ selector: requested, id: identity.id, slug: identity.slug, name: identity.candidate.name.trim() })
}

export function safeEventCursor(value: unknown): value is Integer {
  return isInteger(value) && value >= 0
}

export function validateEventBatch(
  events: readonly ExplorerEvent[],
  board: ExplorerBoardIdentity,
  taskId: string | null,
  after: Integer,
  nextAfter: Integer,
  limit: number,
): void {
  if (!safeEventCursor(after) || !safeEventCursor(nextAfter)) {
    throw new ExplorerReadError("anomaly", "事件响应的 cursor 不是非负 64 位整数。")
  }
  if (!Number.isSafeInteger(limit) || limit < 0 || events.length > limit) {
    throw new ExplorerReadError("anomaly", "事件响应超过请求的 page limit。")
  }
  let previousId = after
  const ids = new Set<Integer>()
  const eventIds = new Set<string>()
  for (const event of events) {
    if (!safeEventCursor(event.id) || event.id <= previousId) {
      throw new ExplorerReadError("anomaly", "事件响应的 id 必须严格按数字递增。")
    }
    if (event.board_id !== board.id) {
      throw new ExplorerReadError("anomaly", `事件 ${String(event.id)} 不属于当前 canonical board。`)
    }
    if (taskId !== null && event.task_id !== taskId) {
      throw new ExplorerReadError("anomaly", `事件 ${String(event.id)} 越过当前 task scope。`)
    }
    if (event.event_id.trim().length === 0) {
      throw new ExplorerReadError("anomaly", `事件 ${String(event.id)} 缺少 event_id。`)
    }
    if (ids.has(event.id) || eventIds.has(event.event_id)) {
      throw new ExplorerReadError("anomaly", `事件 ${String(event.id)} 在响应中重复。`)
    }
    ids.add(event.id)
    eventIds.add(event.event_id)
    previousId = event.id
  }
  if (events.length === 0) {
    if (nextAfter !== after) throw new ExplorerReadError("anomaly", "空事件页不得推进 next_after。")
  } else if (nextAfter !== previousId) {
    throw new ExplorerReadError("anomaly", "事件响应的 next_after 必须等于最后一个 event id。")
  }
}

export function buildTaskListRequest(board: string, query: TaskListQueryState): RpcCall {
  try {
    const path = parseApiListTasksPath({ board })
    const offset = (query.page - 1) * query.limit
    if (!Number.isSafeInteger(offset) || offset < 0) throw new ExplorerReadError("anomaly", "列表页码超出安全范围。")
    const parsed = parseApiListTasksQuery({
      status: [...query.status],
      priority: [...query.priority] as ApiListTasksQueryContract["priority"],
      label: [],
      plan_filter: [...query.plan],
      assignee: null,
      q: query.search.trim() || null,
      include_archived: query.includeArchived,
      limit: query.limit,
      offset,
      sort: query.sort,
    })
    return { method: "ListTasks", path, query: parsed }
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "列表查询不符合 api.list-tasks.query contract。", { contractId: "api.list-tasks.query", cause: error })
    }
    throw error
  }
}

export function validateTaskBoard(
  task: ApiListTasksResponseContract["data"][number],
  board: ExplorerBoardIdentity,
): void {
  if (task.board_id !== board.id || task.board_slug !== board.slug) {
    throw new ExplorerReadError("anomaly", `任务 ${task.id} 不属于当前 canonical board。`)
  }
}

export interface TaskMapQueryOptions {
  readonly activeOnly: boolean
  readonly contextDepth: number
  readonly includeDoneContext: boolean
  readonly includeArchivedContext: boolean
  readonly hideIsolated: boolean
  readonly limitNodes: number
}

export type ExplorerTaskMap = ApiBoardTaskMapResponseContract["data"]

export interface ExplorerTaskMapReadModel {
  readonly board: ExplorerBoardIdentity
  readonly map: ExplorerTaskMap
}

export const defaultTaskMapQuery: TaskMapQueryOptions = Object.freeze({
  activeOnly: true,
  contextDepth: 1,
  includeDoneContext: true,
  includeArchivedContext: false,
  hideIsolated: false,
  limitNodes: 240,
})

export function validateTaskMapOptions(options: TaskMapQueryOptions): void {
  if (!Number.isSafeInteger(options.contextDepth) || options.contextDepth < 0 || options.contextDepth > 8) {
    throw new ExplorerReadError("anomaly", "任务关系图 context depth 超出服务端安全范围。")
  }
  if (!Number.isSafeInteger(options.limitNodes) || options.limitNodes < 1 || options.limitNodes > 1000) {
    throw new ExplorerReadError("anomaly", "任务关系图节点上限必须是 1 到 1000 的安全整数。")
  }
}

export function validateProvidedBoardIdentity(identity: ExplorerBoardIdentity, selector: string): ExplorerBoardIdentity {
  const id = validBoardId(identity.id)
  const slug = parseCanonicalBoardSlug(identity.slug)
  if (!id || !slug || identity.name.trim().length === 0) {
    throw new ExplorerReadError("anomaly", "Map 使用了无效 canonical board identity。")
  }
  const requested = selector.trim()
  if (requested !== id && requested !== slug) {
    throw new ExplorerReadError("anomaly", "Map board identity 与当前 route selector 不一致。")
  }
  return Object.freeze({ selector: requested, id, slug, name: identity.name.trim() })
}

export function buildTaskMapRequest(board: string, options: TaskMapQueryOptions = defaultTaskMapQuery): RpcCall {
  try {
    validateTaskMapOptions(options)
    const path = parseApiBoardTaskMapPath({ board })
    const query = parseApiBoardTaskMapQuery({
      active_only: options.activeOnly,
      context_depth: options.contextDepth,
      include_done_context: options.includeDoneContext,
      include_archived_context: options.includeArchivedContext,
      hide_isolated: options.hideIsolated,
      limit_nodes: options.limitNodes,
    })
    return { method: "BoardTaskMap", path, query }
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "任务关系图查询不符合 generated contract。", { contractId: "api.board-task-map.query", cause: error })
    }
    throw error
  }
}

export function validateMapBoard(map: ExplorerTaskMap, board: ExplorerBoardIdentity, options: TaskMapQueryOptions): void {
  const nodeIds = new Set<string>()
  for (const node of map.nodes) {
    validateTaskBoard(node.task, board)
    if (node.task.id.trim().length === 0 || nodeIds.has(node.task.id)) {
      throw new ExplorerReadError("anomaly", "任务关系图包含重复或空 task node id。")
    }
    nodeIds.add(node.task.id)
  }
  for (const edge of map.edges) {
    if (
      edge.source_task_id.trim().length === 0
      || edge.target_task_id.trim().length === 0
      || !nodeIds.has(edge.source_task_id)
      || !nodeIds.has(edge.target_task_id)
    ) {
      throw new ExplorerReadError("anomaly", "任务关系图 edge 未能在当前响应 node 集合中闭合。")
    }
  }
  const { meta } = map
  if (
    meta.depth !== 0
    || meta.context_depth !== options.contextDepth
    || meta.active_only !== options.activeOnly
    || meta.include_done_context !== options.includeDoneContext
    || meta.include_archived_context !== options.includeArchivedContext
    || meta.hide_isolated !== options.hideIsolated
    || meta.limit_nodes !== options.limitNodes
    || meta.node_count !== map.nodes.length
    || meta.edge_count !== map.edges.length
    || map.nodes.length > options.limitNodes
  ) {
    throw new ExplorerReadError("anomaly", "任务关系图 meta 与请求或实际数组不一致。")
  }
}

export interface TaskRunsReadModel {
  readonly taskId: string
  readonly runs: readonly ApiListRunsResponseContract["data"][number][]
  readonly selectedRunId: string | null
  readonly log: ApiGetRunLogResponseContract["data"] | null
}

export function hasUnsafeTaskSelector(value: string): boolean {
  if (value.trim() !== value || value.length === 0 || /[\\/?#]/.test(value)) return true
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || codePoint === 0x7f) return true
  }
  return false
}

export function validateCanonicalTaskSelector(value: string): void {
  if (hasUnsafeTaskSelector(value) || !value.startsWith("t_") || value.length <= 2) {
    throw new ExplorerReadError("anomaly", "请求的 task selector 必须是 canonical t_ identity。", { reason: "task-not-found" })
  }
}

export function validateRunSelector(value: string): void {
  if (hasUnsafeTaskSelector(value) || !value.startsWith("r_") || value.length <= 2) {
    throw new ExplorerReadError("anomaly", "Run selector 必须是 canonical r_ identity。")
  }
}

export function buildTaskRunsRequest(taskId: string): RpcCall {
  try {
    validateCanonicalTaskSelector(taskId)
    const path = parseApiListRunsPath({ task_id: taskId })
    return { method: "ListRuns", path }
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "Runs 请求不符合 generated path contract。", { contractId: "api.list-runs.path", cause: error })
    }
    throw error
  }
}

export function buildRunLogRequest(runId: string): RpcCall {
  try {
    validateRunSelector(runId)
    const path = parseApiGetRunLogPath({ run_id: runId })
    return { method: "GetRunLog", path }
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "Run log 请求不符合 generated path contract。", { contractId: "api.get-run-log.path", cause: error })
    }
    throw error
  }
}

export function validateRunScope(
  runs: readonly ApiListRunsResponseContract["data"][number][],
  taskId: string,
): void {
  for (const run of runs) {
    if (run.task_id !== taskId) throw new ExplorerReadError("anomaly", "Runs 响应越过了当前 task scope。")
  }
}

export interface TaskInspectorReadModel {
  readonly board: ExplorerBoardIdentity
  readonly task: TaskInspectorTask
  readonly neighborhood: ApiTaskNeighborhoodResponseContract["data"] | null
  readonly dependencies: ApiListDependenciesResponseContract["data"]
  readonly steps: ApiListStepsResponseContract["data"]
  readonly runs: ApiListRunsResponseContract["data"]
  readonly comments: ApiListCommentsResponseContract["data"]
  readonly attachments: ApiListAttachmentsResponseContract["data"]
  readonly events: ApiListEventsResponseContract["data"]
  readonly runtime: Pick<WebRuntimeConfig, "actor" | "apiBaseUrl" | "serverVersion" | "protocolVersion" | "webBuildId">
}

export interface TaskInspectorReadOptions extends ExplorerReadOptions {
  /** Detail sections are fetched by the inspector when their disclosure opens. */
  readonly includeNeighborhood?: boolean
  readonly includeRuns?: boolean
  readonly includeEvents?: boolean
  /** Let mounted UI defer attachment metadata while keeping bytes downloads lazy. */
  readonly includeAttachments?: boolean
}

export type TaskInspectorTask = Omit<ApiGetTaskResponseContract["data"], "labels"> & {
  readonly labels: ApiListTaskLabelsResponseContract["data"]
}

export interface TaskInspectorRequests {
  readonly task: RpcCall
  readonly labels: RpcCall
  readonly neighborhood: RpcCall
  readonly dependencies: RpcCall
  readonly steps: RpcCall
  readonly runs: RpcCall
  readonly comments: RpcCall
  readonly attachments: RpcCall
}

export function buildTaskInspectorRequests(taskId: string): TaskInspectorRequests {
  try {
    validateCanonicalTaskSelector(taskId)
    const task = parseApiGetTaskPath({ task_id: taskId }).task_id
    const neighborhoodPath = parseApiTaskNeighborhoodPath({ task_id: task })
    const neighborhoodQuery = parseApiTaskNeighborhoodQuery({ depth: 1, include_archived_context: false, limit_nodes: 40 })
    return {
      task: { method: "GetTask", path: parseApiGetTaskPath({ task_id: task }) },
      labels: { method: "ListTaskLabels", path: parseApiListTaskLabelsPath({ task_id: task }) },
      neighborhood: { method: "TaskNeighborhood", path: neighborhoodPath, query: neighborhoodQuery },
      dependencies: { method: "ListDependencies", path: parseApiListDependenciesPath({ task_id: task }) },
      steps: { method: "ListSteps", path: parseApiListStepsPath({ task_id: task }) },
      runs: { method: "ListRuns", path: parseApiListRunsPath({ task_id: task }) },
      comments: { method: "ListComments", path: parseApiListCommentsPath({ task_id: task }) },
      attachments: { method: "ListAttachments", path: parseApiListAttachmentsPath({ task_id: task }) },
    }
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "任务 Inspector 请求不符合 generated path/query contract。", { contractId: "api.get-task.path", cause: error })
    }
    throw error
  }
}

export function validateInspectorTask(
  task: ApiGetTaskResponseContract["data"],
  board: ExplorerBoardIdentity,
  expectedId: string,
): void {
  if (task.id !== expectedId) throw new ExplorerReadError("anomaly", "任务 Inspector 响应 id 与请求不一致。")
  if (task.board_id !== board.id || task.board_slug !== board.slug) {
    throw new ExplorerReadError("anomaly", `任务 ${task.id} 不属于当前 canonical board。`, { reason: "task-not-found" })
  }
}

export function validateInspectorLabels(
  labels: ApiListTaskLabelsResponseContract["data"],
  board: ExplorerBoardIdentity,
): void {
  const ids = new Set<string>()
  for (const label of labels) {
    if (label.board_id !== board.id || label.id.trim().length === 0 || label.name.trim().length === 0 || ids.has(label.id)) {
      throw new ExplorerReadError("anomaly", "任务标签响应越过当前 board scope 或包含重复/空 label。")
    }
    ids.add(label.id)
  }
}

export function validateNeighborhoodScope(
  neighborhood: ApiTaskNeighborhoodResponseContract["data"],
  board: ExplorerBoardIdentity,
  taskId: string,
): void {
  if (neighborhood.center_task_id !== taskId) throw new ExplorerReadError("anomaly", "任务邻域响应中心 task id 不一致。")
  const nodeIds = new Set<string>()
  let centerCount = 0
  for (const node of neighborhood.nodes) {
    validateTaskBoard(node.task, board)
    const nodeId = node.task.id
    if (nodeId.trim().length === 0 || nodeIds.has(nodeId)) {
      throw new ExplorerReadError("anomaly", "任务邻域包含重复或空 task node id。")
    }
    nodeIds.add(nodeId)
    if (node.role === "center") {
      centerCount += 1
      if (nodeId !== taskId) throw new ExplorerReadError("anomaly", "任务邻域 center node 越过当前 task scope。")
    }
  }
  if (centerCount !== 1 || !nodeIds.has(taskId)) throw new ExplorerReadError("anomaly", "任务邻域缺少唯一的当前 task center node。")
  const edgeIds = new Set<string>()
  for (const edge of neighborhood.edges) {
    if (edge.id.trim().length === 0 || edgeIds.has(edge.id)) {
      throw new ExplorerReadError("anomaly", "任务邻域包含重复或空 edge id。")
    }
    if (!nodeIds.has(edge.source_task_id) || !nodeIds.has(edge.target_task_id)) {
      throw new ExplorerReadError("anomaly", "任务邻域 edge 越过当前 node scope。")
    }
    edgeIds.add(edge.id)
  }
  if (neighborhood.meta.node_count !== neighborhood.nodes.length || neighborhood.meta.edge_count !== neighborhood.edges.length) {
    throw new ExplorerReadError("anomaly", "任务邻域 meta 与 node/edge 数量不一致。")
  }
}

export function validateInspectorScope(
  model: Pick<TaskInspectorReadModel, "neighborhood" | "dependencies" | "steps" | "runs" | "comments" | "attachments" | "events" | "task">,
  board: ExplorerBoardIdentity,
  taskId: string,
): void {
  validateInspectorLabels(model.task.labels, board)
  if (model.neighborhood !== null) validateNeighborhoodScope(model.neighborhood, board, taskId)
  if (model.dependencies.task.id !== taskId) throw new ExplorerReadError("anomaly", "任务依赖响应 task id 不一致。")
  if (model.steps.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务步骤响应 task id 不一致。")
  for (const task of [...model.dependencies.parents, ...model.dependencies.children]) validateTaskBoard(task, board)
  for (const event of model.events) {
    if (event.board_id !== board.id || event.task_id !== taskId) {
      throw new ExplorerReadError("anomaly", "任务事件响应越过了当前 board/task scope。")
    }
  }
  for (const run of model.runs) if (run.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务运行记录越过了当前 task scope。")
  for (const comment of model.comments) {
    if (comment.board_id !== board.id || comment.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务评论响应越过了当前 board/task scope。")
  }
  for (const attachment of model.attachments) {
    if (attachment.board_id !== board.id || attachment.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务附件响应越过了当前 board/task scope。")
  }
}

export function parseTaskListQuery(input: string | URLSearchParams): TaskListQueryState {
  const params = typeof input === "string" ? new URLSearchParams(input.startsWith("?") ? input.slice(1) : input) : input
  const status = unique(params.getAll("status").filter((value): value is TaskListStatus => taskStatuses.has(value as TaskListStatus)))
  const priorityValues: number[] = []
  for (const raw of params.getAll('priority')) {
    const value = Number(raw)
    if (Number.isSafeInteger(value) && value >= 0 && value <= 3) priorityValues.push(value)
  }
  const priority = unique(priorityValues)
  const plan = unique(params.getAll("plan").filter((value): value is TaskListPlanFilter => taskPlanFilters.has(value as TaskListPlanFilter)))
  const sortValue = params.get("sort")
  const sort = sortValue && taskSorts.has(sortValue as TaskListSort) ? sortValue as TaskListSort : defaultTaskListQuery.sort
  return Object.freeze({
    status,
    priority,
    plan,
    search: parseSearch(params.get("q")),
    sort,
    page: parseInteger(params.get("page"), defaultTaskListQuery.page, 1, Number.MAX_SAFE_INTEGER),
    limit: parseInteger(params.get("limit"), defaultTaskListQuery.limit, 1, 1000),
    includeArchived: params.get("include_archived") === "true",
  })
}
