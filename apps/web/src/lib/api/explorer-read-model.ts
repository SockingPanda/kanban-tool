import type { WebRuntimeConfig } from "../runtime"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../board-slug"
import { asCanonicalBoardId, type CanonicalBoardId } from "../sync/contracts"
import { parseApiListBoardsQuery } from "./generated/contracts/api-list-boards-query"
import { parseApiListBoardsResponse, type ApiListBoardsResponseContract } from "./generated/contracts/api-list-boards-response"
import { parseApiListTasksPath } from "./generated/contracts/api-list-tasks-path"
import {
  parseApiListTasksQuery,
  type ApiListTasksQueryContract,
} from "./generated/contracts/api-list-tasks-query"
import {
  parseApiListTasksResponse,
  type ApiListTasksResponseContract,
} from "./generated/contracts/api-list-tasks-response"
import { parseApiBoardTaskMapPath } from "./generated/contracts/api-board-task-map-path"
import { parseApiBoardTaskMapQuery } from "./generated/contracts/api-board-task-map-query"
import { parseApiBoardTaskMapResponse, type ApiBoardTaskMapResponseContract } from "./generated/contracts/api-board-task-map-response"
import { parseApiGetTaskPath } from "./generated/contracts/api-get-task-path"
import { parseApiGetTaskResponse, type ApiGetTaskResponseContract } from "./generated/contracts/api-get-task-response"
import { parseApiTaskNeighborhoodPath } from "./generated/contracts/api-task-neighborhood-path"
import { parseApiTaskNeighborhoodQuery } from "./generated/contracts/api-task-neighborhood-query"
import { parseApiTaskNeighborhoodResponse, type ApiTaskNeighborhoodResponseContract } from "./generated/contracts/api-task-neighborhood-response"
import { parseApiListDependenciesPath } from "./generated/contracts/api-list-dependencies-path"
import { parseApiListDependenciesResponse, type ApiListDependenciesResponseContract } from "./generated/contracts/api-list-dependencies-response"
import { parseApiListStepsPath } from "./generated/contracts/api-list-steps-path"
import { parseApiListStepsResponse, type ApiListStepsResponseContract } from "./generated/contracts/api-list-steps-response"
import { parseApiListRunsPath } from "./generated/contracts/api-list-runs-path"
import { parseApiListRunsResponse, type ApiListRunsResponseContract } from "./generated/contracts/api-list-runs-response"
import { parseApiGetRunLogPath } from "./generated/contracts/api-get-run-log-path"
import { parseApiGetRunLogResponse, type ApiGetRunLogResponseContract } from "./generated/contracts/api-get-run-log-response"
import { parseApiListCommentsPath } from "./generated/contracts/api-list-comments-path"
import { parseApiListCommentsResponse, type ApiListCommentsResponseContract } from "./generated/contracts/api-list-comments-response"
import { parseApiListEventsQuery } from "./generated/contracts/api-list-events-query"
import { parseApiListEventsResponse, type ApiListEventsResponseContract } from "./generated/contracts/api-list-events-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpTransport,
  type HttpTransportOptions,
  type HttpTransportResponse,
} from "./http-transport"

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

const taskStatuses = new Set<TaskListStatus>([
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
const taskSorts = new Set<TaskListSort>([
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
const taskPlanFilters = new Set<TaskListPlanFilter>([
  "plan_needed",
  "has_steps",
  "incomplete_required_steps",
])

function unique<T>(values: readonly T[]): T[] {
  return [...new Set(values)]
}

function parseInteger(value: string | null, fallback: number, minimum: number, maximum: number): number {
  if (value === null || !/^\d+$/.test(value)) return fallback
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed >= minimum && parsed <= maximum ? parsed : fallback
}

function parseSearch(value: string | null): string {
  if (value === null || value.includes("\u0000")) return ""
  const trimmed = value.trim()
  return trimmed.length <= 1024 ? trimmed : trimmed.slice(0, 1024)
}

export function parseTaskListQuery(input: string | URLSearchParams): TaskListQueryState {
  const params = typeof input === "string" ? new URLSearchParams(input.startsWith("?") ? input.slice(1) : input) : input
  const status = unique(params.getAll("status").filter((value): value is TaskListStatus => taskStatuses.has(value as TaskListStatus)))
  const priority = unique(
    params
      .getAll("priority")
      .map((value) => Number(value))
      .filter((value) => Number.isSafeInteger(value) && value >= 0 && value <= 3),
  )
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

function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

function parseContract<T>(contractId: string, parser: (value: unknown) => T, payload: unknown): T {
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
  readonly apiError: HttpTransportError["apiError"]

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

export interface ExplorerReadDependencies extends HttpTransportOptions {
  readonly transport?: HttpTransport
}

export const MAX_EXPLORER_TOTAL_JSON_BYTES = 64 * 1024 * 1024

class ExplorerReadBudget {
  private totalBytes = 0

  consume(bytes: number): void {
    if (!Number.isSafeInteger(bytes) || bytes < 0 || this.totalBytes > MAX_EXPLORER_TOTAL_JSON_BYTES - bytes) {
      throw new ExplorerReadError("anomaly", `Explorer read raw JSON 超过 ${MAX_EXPLORER_TOTAL_JSON_BYTES} 字节预算。`)
    }
    this.totalBytes += bytes
  }
}

export interface ExplorerReadOptions extends ExplorerReadDependencies {
  readonly signal?: AbortSignal
  readonly includeArchived?: boolean
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

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof ExplorerReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new ExplorerReadError(error.kind, error.message, {
      status: error.status ?? undefined,
      apiError: error.apiError,
      cause: error,
    })
  }
  throw error
}

async function getPayload(
  transport: HttpTransport,
  path: string,
  signal: AbortSignal | undefined,
  budget = new ExplorerReadBudget(),
): Promise<unknown> {
  try {
    const response: HttpTransportResponse = await transport.get(path, signal)
    if (
      response === null
      || typeof response !== "object"
      || !("payload" in response)
      || typeof response.bytes !== "number"
      || !Number.isSafeInteger(response.bytes)
      || response.bytes < 0
    ) {
      throw new ExplorerReadError("anomaly", "Web API transport 返回了无效响应。")
    }
    budget.consume(response.bytes)
    return response.payload
  } catch (error) {
    return wrapTransportError(error)
  }
}

function boardListPath(includeArchived: boolean): string {
  const query = parseApiListBoardsQuery({ include_archived: includeArchived })
  const params = new URLSearchParams()
  if (query.include_archived !== undefined) params.set("include_archived", String(query.include_archived))
  return `/api/v1/boards?${params.toString()}`
}

function validBoardId(value: string): CanonicalBoardId | null {
  if (value.trim() !== value || value.length === 0 || hasUnsafeIdentityCharacters(value)) return null
  try {
    return asCanonicalBoardId(value)
  } catch {
    return null
  }
}

function hasUnsafeIdentityCharacters(value: string): boolean {
  if (/[\\/?#]/.test(value)) return true
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || codePoint === 0x7f) return true
  }
  return false
}

function resolveBoard(boards: ApiListBoardsResponseContract["data"], selector: string): ExplorerBoardIdentity {
  const requested = selector.trim()
  const identities = boards.map((candidate) => {
    const id = validBoardId(candidate.id)
    const slug = parseCanonicalBoardSlug(candidate.slug)
    if (!id || !slug || candidate.name.trim().length === 0) {
      throw new ExplorerReadError("anomaly", "看板列表包含无效 canonical identity。")
    }
    return { candidate, id, slug }
  })
  const ids = new Set<string>()
  const slugs = new Set<string>()
  for (const identity of identities) {
    if (ids.has(identity.id) || slugs.has(identity.slug)) {
      throw new ExplorerReadError("anomaly", "看板列表包含重复 canonical identity。")
    }
    ids.add(identity.id)
    slugs.add(identity.slug)
  }
  const identity = identities.find(({ candidate }) => candidate.id === requested || candidate.slug === requested)
  if (!identity) {
    throw new ExplorerReadError("empty", "请求的看板 selector 未在看板列表中精确匹配。", {
      reason: "board-not-found",
    })
  }
  return Object.freeze({ selector: requested, id: identity.id, slug: identity.slug, name: identity.candidate.name.trim() })
}

export async function loadExplorerBoardIdentity(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: ExplorerReadOptions = {},
): Promise<ExplorerBoardIdentity> {
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: HttpTransport
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  try {
    const response = parseContract(
      "api.list-boards.response",
      parseApiListBoardsResponse,
      await getPayload(transport, boardListPath(options.includeArchived ?? false), options.signal, budget),
    )
    if (response.data.length === 0) throw new ExplorerReadError("empty", "kanban serve 未返回可用看板。", { reason: "board-not-found" })
    return resolveBoard(response.data, selector)
  } catch (error) {
    return wrapTransportError(error)
  }
}

export function buildTaskListRequest(board: string, query: TaskListQueryState): string {
  let path: string
  try {
    path = `/api/v1/boards/${encodedSegment(parseApiListTasksPath({ board }).board)}/tasks`
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
    const params = new URLSearchParams()
    for (const status of parsed.status ?? []) params.append("status", status)
    for (const priority of parsed.priority ?? []) params.append("priority", String(priority))
    for (const plan of parsed.plan_filter ?? []) params.append("plan_filter", plan)
    if (parsed.q) params.set("q", parsed.q)
    if (parsed.include_archived !== undefined) params.set("include_archived", String(parsed.include_archived))
    if (parsed.limit !== undefined) params.set("limit", String(parsed.limit))
    if (parsed.offset !== undefined) params.set("offset", String(parsed.offset))
    if (parsed.sort !== undefined) params.set("sort", parsed.sort)
    return `${path}?${params.toString()}`
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "列表查询不符合 api.list-tasks.query contract。", { contractId: "api.list-tasks.query", cause: error })
    }
    throw error
  }
}

function validateTaskBoard(
  task: ApiListTasksResponseContract["data"][number],
  board: ExplorerBoardIdentity,
): void {
  if (task.board_id !== board.id || task.board_slug !== board.slug) {
    throw new ExplorerReadError("anomaly", `任务 ${task.id} 不属于当前 canonical board。`)
  }
}

export async function loadTaskListPage(
  runtime: WebRuntimeConfig,
  selector: string,
  query: TaskListQueryState,
  options: ExplorerReadOptions = {},
): Promise<ExplorerTaskListPage> {
  const budget = options.budget ?? new ExplorerReadBudget()
  const board = await loadExplorerBoardIdentity(runtime, selector, { ...options, budget })
  let transport: HttpTransport
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  try {
    const response = parseContract(
      "api.list-tasks.response",
      parseApiListTasksResponse,
      await getPayload(transport, buildTaskListRequest(board.slug, query), options.signal, budget),
    )
    for (const task of response.data) validateTaskBoard(task, board)
    return Object.freeze({ board, tasks: Object.freeze(response.data.map((task) => Object.freeze({ ...task }))), meta: response.meta })
  } catch (error) {
    return wrapTransportError(error)
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

export function buildTaskMapRequest(board: string, options: TaskMapQueryOptions = defaultTaskMapQuery): string {
  try {
    const path = parseApiBoardTaskMapPath({ board }).board
    const query = parseApiBoardTaskMapQuery({
      active_only: options.activeOnly,
      context_depth: options.contextDepth,
      include_done_context: options.includeDoneContext,
      include_archived_context: options.includeArchivedContext,
      hide_isolated: options.hideIsolated,
      limit_nodes: options.limitNodes,
    })
    const params = new URLSearchParams()
    if (query.active_only !== undefined) params.set("active_only", String(query.active_only))
    if (query.context_depth !== undefined) params.set("context_depth", String(query.context_depth))
    if (query.include_done_context !== undefined) params.set("include_done_context", String(query.include_done_context))
    if (query.include_archived_context !== undefined) params.set("include_archived_context", String(query.include_archived_context))
    if (query.hide_isolated !== undefined) params.set("hide_isolated", String(query.hide_isolated))
    if (query.limit_nodes !== undefined) params.set("limit_nodes", String(query.limit_nodes))
    return `/api/v1/boards/${encodedSegment(path)}/task-map?${params.toString()}`
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "任务关系图查询不符合 generated contract。", { contractId: "api.board-task-map.query", cause: error })
    }
    throw error
  }
}

function validateMapBoard(map: ExplorerTaskMap, board: ExplorerBoardIdentity): void {
  for (const node of map.nodes) validateTaskBoard(node.task, board)
  for (const edge of map.edges) {
    if (edge.source_task_id.trim().length === 0 || edge.target_task_id.trim().length === 0) {
      throw new ExplorerReadError("anomaly", "任务关系图包含空 task id edge。")
    }
  }
}

export async function loadTaskMap(
  runtime: WebRuntimeConfig,
  selector: string,
  options: ExplorerReadOptions & Partial<TaskMapQueryOptions> = {},
): Promise<ExplorerTaskMapReadModel> {
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: HttpTransport
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  try {
    const board = await loadExplorerBoardIdentity(runtime, selector, { ...options, transport, budget })
    const map = parseContract(
      "api.board-task-map.response",
      parseApiBoardTaskMapResponse,
      await getPayload(transport, buildTaskMapRequest(board.slug, {
        activeOnly: options.activeOnly ?? defaultTaskMapQuery.activeOnly,
        contextDepth: options.contextDepth ?? defaultTaskMapQuery.contextDepth,
        includeDoneContext: options.includeDoneContext ?? defaultTaskMapQuery.includeDoneContext,
        includeArchivedContext: options.includeArchivedContext ?? defaultTaskMapQuery.includeArchivedContext,
        hideIsolated: options.hideIsolated ?? defaultTaskMapQuery.hideIsolated,
        limitNodes: options.limitNodes ?? defaultTaskMapQuery.limitNodes,
      }), options.signal, budget),
    ).data
    validateMapBoard(map, board)
    return Object.freeze({ board, map })
  } catch (error) {
    return wrapTransportError(error)
  }
}

export interface TaskRunsReadModel {
  readonly taskId: string
  readonly runs: readonly ApiListRunsResponseContract["data"][number][]
  readonly selectedRunId: string | null
  readonly log: ApiGetRunLogResponseContract["data"] | null
}

function hasUnsafeTaskSelector(value: string): boolean {
  if (value.trim() !== value || value.length === 0 || /[\\/?#]/.test(value)) return true
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || codePoint === 0x7f) return true
  }
  return false
}

function validateTaskSelector(value: string): void {
  if (hasUnsafeTaskSelector(value)) throw new ExplorerReadError("anomaly", "Runs 请求的 task selector 无效。")
}

export function buildTaskRunsRequest(taskId: string): string {
  try {
    validateTaskSelector(taskId)
    const path = parseApiListRunsPath({ task_id: taskId }).task_id
    return `/api/v1/tasks/${encodedSegment(path)}/runs`
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "Runs 请求不符合 generated path contract。", { contractId: "api.list-runs.path", cause: error })
    }
    throw error
  }
}

export function buildRunLogRequest(runId: string): string {
  try {
    validateTaskSelector(runId)
    const path = parseApiGetRunLogPath({ run_id: runId }).run_id
    return `/api/v1/runs/${encodedSegment(path)}/log`
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "Run log 请求不符合 generated path contract。", { contractId: "api.get-run-log.path", cause: error })
    }
    throw error
  }
}

function validateRunScope(
  runs: readonly ApiListRunsResponseContract["data"][number][],
  taskId: string,
): void {
  for (const run of runs) {
    if (run.task_id !== taskId) throw new ExplorerReadError("anomaly", "Runs 响应越过了当前 task scope。")
  }
}

export async function loadTaskRuns(
  runtime: WebRuntimeConfig,
  taskId: string,
  options: ExplorerReadOptions = {},
): Promise<TaskRunsReadModel> {
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: HttpTransport
  try {
    validateTaskSelector(taskId)
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  try {
    const runs = parseContract(
      "api.list-runs.response",
      parseApiListRunsResponse,
      await getPayload(transport, buildTaskRunsRequest(taskId), options.signal, budget),
    ).data
    validateRunScope(runs, taskId)
    const selectedRun = runs.find((run) => run.has_log) ?? null
    let log: ApiGetRunLogResponseContract["data"] | null = null
    if (selectedRun) {
      log = parseContract(
        "api.get-run-log.response",
        parseApiGetRunLogResponse,
        await getPayload(transport, buildRunLogRequest(selectedRun.id), options.signal, budget),
      ).data
      if (log.run_id !== selectedRun.id) throw new ExplorerReadError("anomaly", "Run log 响应 id 与请求不一致。")
    }
    return Object.freeze({
      taskId,
      runs: Object.freeze(runs.map((run) => Object.freeze({ ...run }))),
      selectedRunId: selectedRun?.id ?? null,
      log: log ? Object.freeze({ ...log }) : null,
    })
  } catch (error) {
    return wrapTransportError(error)
  }
}

export interface TaskInspectorReadModel {
  readonly board: ExplorerBoardIdentity
  readonly task: ApiGetTaskResponseContract["data"]
  readonly neighborhood: ApiTaskNeighborhoodResponseContract["data"]
  readonly dependencies: ApiListDependenciesResponseContract["data"]
  readonly steps: ApiListStepsResponseContract["data"]
  readonly runs: ApiListRunsResponseContract["data"]
  readonly comments: ApiListCommentsResponseContract["data"]
  readonly events: ApiListEventsResponseContract["data"]
  readonly runtime: Pick<WebRuntimeConfig, "actor" | "apiBaseUrl" | "serverVersion" | "protocolVersion" | "webBuildId">
}

export interface TaskInspectorRequests {
  readonly task: string
  readonly neighborhood: string
  readonly dependencies: string
  readonly steps: string
  readonly runs: string
  readonly comments: string
  readonly events: string
}

function listEventsRequest(board: string, taskId: string): string {
  const query = parseApiListEventsQuery({ board, task_id: taskId, after: 0, limit: 50 })
  const params = new URLSearchParams()
  if (query.board !== undefined) params.set("board", query.board)
  if (query.task_id !== undefined && query.task_id !== null) params.set("task_id", query.task_id)
  if (query.after !== undefined) params.set("after", String(query.after))
  if (query.limit !== undefined) params.set("limit", String(query.limit))
  return `/api/v1/events?${params.toString()}`
}

export function buildTaskInspectorRequests(board: string, taskId: string): TaskInspectorRequests {
  try {
    const task = parseApiGetTaskPath({ task_id: taskId }).task_id
    const neighborhoodPath = parseApiTaskNeighborhoodPath({ task_id: task }).task_id
    const neighborhoodQuery = parseApiTaskNeighborhoodQuery({ depth: 1, include_archived_context: false, limit_nodes: 40 })
    const neighborhoodParams = new URLSearchParams()
    if (neighborhoodQuery.depth !== undefined) neighborhoodParams.set("depth", String(neighborhoodQuery.depth))
    if (neighborhoodQuery.include_archived_context !== undefined) neighborhoodParams.set("include_archived_context", String(neighborhoodQuery.include_archived_context))
    if (neighborhoodQuery.limit_nodes !== undefined) neighborhoodParams.set("limit_nodes", String(neighborhoodQuery.limit_nodes))
    return {
      task: `/api/v1/tasks/${encodedSegment(task)}`,
      neighborhood: `/api/v1/tasks/${encodedSegment(neighborhoodPath)}/neighborhood?${neighborhoodParams.toString()}`,
      dependencies: `/api/v1/tasks/${encodedSegment(parseApiListDependenciesPath({ task_id: task }).task_id)}/dependencies`,
      steps: `/api/v1/tasks/${encodedSegment(parseApiListStepsPath({ task_id: task }).task_id)}/steps`,
      runs: `/api/v1/tasks/${encodedSegment(parseApiListRunsPath({ task_id: task }).task_id)}/runs`,
      comments: `/api/v1/tasks/${encodedSegment(parseApiListCommentsPath({ task_id: task }).task_id)}/comments`,
      events: listEventsRequest(board, task),
    }
  } catch (error) {
    if (error instanceof ExplorerReadError) throw error
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "任务 Inspector 请求不符合 generated path/query contract。", { contractId: "api.get-task.path", cause: error })
    }
    throw error
  }
}

function validateInspectorTask(
  task: ApiGetTaskResponseContract["data"],
  board: ExplorerBoardIdentity,
  expectedId: string,
): void {
  if (task.id !== expectedId) throw new ExplorerReadError("anomaly", "任务 Inspector 响应 id 与请求不一致。")
  validateTaskBoard(task, board)
}

function validateInspectorScope(
  model: Pick<TaskInspectorReadModel, "neighborhood" | "dependencies" | "steps" | "runs" | "comments" | "events">,
  board: ExplorerBoardIdentity,
  taskId: string,
): void {
  if (model.neighborhood.center_task_id !== taskId) throw new ExplorerReadError("anomaly", "任务邻域响应中心 task id 不一致。")
  if (model.dependencies.task.id !== taskId) throw new ExplorerReadError("anomaly", "任务依赖响应 task id 不一致。")
  if (model.steps.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务步骤响应 task id 不一致。")
  for (const task of [...model.dependencies.parents, ...model.dependencies.children]) validateTaskBoard(task, board)
  for (const event of model.events) {
    if (event.board_id !== board.id || (event.task_id !== null && event.task_id !== taskId)) {
      throw new ExplorerReadError("anomaly", "任务事件响应越过了当前 board/task scope。")
    }
  }
  for (const run of model.runs) if (run.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务运行记录越过了当前 task scope。")
  for (const comment of model.comments) {
    if (comment.board_id !== board.id || comment.task_id !== taskId) throw new ExplorerReadError("anomaly", "任务评论响应越过了当前 board/task scope。")
  }
}

function linkedAbortSignal(signal: AbortSignal | undefined): { signal: AbortSignal; cleanup: () => void; abort: () => void } {
  const controller = new AbortController()
  if (signal?.aborted) controller.abort()
  const abort = () => controller.abort()
  signal?.addEventListener("abort", abort, { once: true })
  return { signal: controller.signal, abort: () => controller.abort(), cleanup: () => signal?.removeEventListener("abort", abort) }
}

export async function loadTaskInspector(
  runtime: WebRuntimeConfig,
  selector: string,
  taskId: string,
  options: ExplorerReadOptions = {},
): Promise<TaskInspectorReadModel> {
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: HttpTransport
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  const linked = linkedAbortSignal(options.signal)
  try {
    const board = await loadExplorerBoardIdentity(runtime, selector, { ...options, transport, signal: linked.signal, budget })
    const requests = buildTaskInspectorRequests(board.slug, taskId)
    let taskResponse: ApiGetTaskResponseContract
    try {
      taskResponse = parseContract(
        "api.get-task.response",
        parseApiGetTaskResponse,
        await getPayload(transport, requests.task, linked.signal, budget),
      )
    } catch (error) {
      if (error instanceof ExplorerReadError && error.status === 404) {
        throw new ExplorerReadError("http", "请求的任务不存在。", { reason: "task-not-found", status: 404, cause: error })
      }
      throw error
    }
    const [neighborhood, dependencies, steps, runs, comments, events] = await Promise.all([
      getPayload(transport, requests.neighborhood, linked.signal, budget).then((payload) => parseContract("api.task-neighborhood.response", parseApiTaskNeighborhoodResponse, payload).data),
      getPayload(transport, requests.dependencies, linked.signal, budget).then((payload) => parseContract("api.list-dependencies.response", parseApiListDependenciesResponse, payload).data),
      getPayload(transport, requests.steps, linked.signal, budget).then((payload) => parseContract("api.list-steps.response", parseApiListStepsResponse, payload).data),
      getPayload(transport, requests.runs, linked.signal, budget).then((payload) => parseContract("api.list-runs.response", parseApiListRunsResponse, payload).data),
      getPayload(transport, requests.comments, linked.signal, budget).then((payload) => parseContract("api.list-comments.response", parseApiListCommentsResponse, payload).data),
      getPayload(transport, requests.events, linked.signal, budget).then((payload) => parseContract("api.list-events.response", parseApiListEventsResponse, payload).data),
    ])
    const detail = { neighborhood, dependencies, steps, runs, comments, events }
    validateInspectorTask(taskResponse.data, board, taskId)
    validateInspectorScope(detail, board, taskId)
    return Object.freeze({
      board,
      task: Object.freeze({ ...taskResponse.data }),
      neighborhood,
      dependencies,
      steps,
      runs,
      comments,
      events,
      runtime: {
        actor: runtime.actor,
        apiBaseUrl: runtime.apiBaseUrl,
        serverVersion: runtime.serverVersion,
        protocolVersion: runtime.protocolVersion,
        webBuildId: runtime.webBuildId,
      },
    })
  } catch (error) {
    linked.abort()
    return wrapTransportError(error)
  } finally {
    linked.cleanup()
  }
}
