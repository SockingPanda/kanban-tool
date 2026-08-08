import type { WebRuntimeConfig } from "../runtime"
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

export interface ExplorerReadOptions extends ExplorerReadDependencies {
  readonly signal?: AbortSignal
  readonly includeArchived?: boolean
}

export interface ExplorerBoardIdentity {
  readonly selector: string
  readonly id: CanonicalBoardId
  readonly slug: string
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

async function getPayload(transport: HttpTransport, path: string, signal: AbortSignal | undefined): Promise<unknown> {
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
  try {
    return asCanonicalBoardId(value)
  } catch {
    return null
  }
}

function resolveBoard(boards: ApiListBoardsResponseContract["data"], selector: string): ExplorerBoardIdentity {
  const requested = selector.trim()
  const board = boards.find((candidate) => candidate.id === requested || candidate.slug === requested)
  if (!board) {
    throw new ExplorerReadError("empty", "请求的看板 selector 未在看板列表中精确匹配。", {
      reason: "board-not-found",
    })
  }
  const id = validBoardId(board.id)
  if (!id || board.slug.trim().length === 0 || board.name.trim().length === 0) {
    throw new ExplorerReadError("anomaly", "看板列表包含无效 canonical identity。")
  }
  return Object.freeze({ selector: requested, id, slug: board.slug, name: board.name.trim() })
}

export async function loadExplorerBoardIdentity(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: ExplorerReadOptions = {},
): Promise<ExplorerBoardIdentity> {
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
      await getPayload(transport, boardListPath(options.includeArchived ?? false), options.signal),
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
  const board = await loadExplorerBoardIdentity(runtime, selector, options)
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
      await getPayload(transport, buildTaskListRequest(board.slug, query), options.signal),
    )
    for (const task of response.data) validateTaskBoard(task, board)
    return Object.freeze({ board, tasks: Object.freeze(response.data.map((task) => Object.freeze({ ...task }))), meta: response.meta })
  } catch (error) {
    return wrapTransportError(error)
  }
}
