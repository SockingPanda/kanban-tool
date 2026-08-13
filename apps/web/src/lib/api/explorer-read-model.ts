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
import { parseApiListTaskLabelsPath } from "./generated/contracts/api-list-task-labels-path"
import { parseApiListTaskLabelsResponse, type ApiListTaskLabelsResponseContract } from "./generated/contracts/api-list-task-labels-response"
import { parseApiListAttachmentsPath } from "./generated/contracts/api-list-attachments-path"
import { parseApiListAttachmentsResponse, type ApiListAttachmentsResponseContract } from "./generated/contracts/api-list-attachments-response"
import { parseApiListEventsQuery } from "./generated/contracts/api-list-events-query"
import { parseApiListEventsResponse, type ApiListEventsResponseContract } from "./generated/contracts/api-list-events-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpTransportErrorKind,
  type HttpReadTransport,
  type HttpTransportOptions,
  type HttpTransportResponse,
} from "./http-transport"
import { parseTaskSelector } from "../tasks-url"

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
  /** Read models only need the GET half of the shared browser transport. */
  readonly transport?: Pick<HttpReadTransport, "get">
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
  /** Incremental event cursor; zero is reserved for initial/recovery reads. */
  readonly after?: number
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
/** 防止恶意或无界事件历史把一次读取变成无休止的游标遍历。 */
export const MAX_BOARD_EVENTS_PAGES = 1_024

export type ExplorerEvent = ApiListEventsResponseContract["data"][number]

/** Re-validate the already generated SSE event before handing it to EventsView. */
export function parseBoardEvent(value: unknown): ExplorerEvent | null {
  try {
    const id = typeof value === "object" && value !== null && "id" in value && typeof value.id === "number" ? value.id : -1
    return parseApiListEventsResponse({ data: [value] as ApiListEventsResponseContract["data"], meta: { next_after: id } }).data[0] ?? null
  } catch {
    return null
  }
}

/** 由现有 sync owner 交给 UI 的已校验、看板隔离事件 batch。 */
export interface BoardEventsBatch {
  readonly boardId: CanonicalBoardId
  readonly events: readonly ExplorerEvent[]
  readonly nextAfter: number
}

export interface BoardEventsReadModel {
  readonly board: ExplorerBoardIdentity
  readonly taskId: string | null
  readonly events: readonly ExplorerEvent[]
  readonly meta: {
    readonly count: number
    readonly nextAfter: number
    readonly limit: number
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function throwIfAborted(signal: AbortSignal | undefined): void {
  if (!signal?.aborted) return
  const error = new Error("Explorer 事件读取已取消。")
  error.name = "AbortError"
  throw error
}

function explorerErrorKind(kind: HttpTransportErrorKind): ExplorerReadErrorKind {
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

function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof ExplorerReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new ExplorerReadError(explorerErrorKind(error.kind), error.message, {
      status: error.status ?? undefined,
      apiError: error.apiError,
      cause: error,
    })
  }
  throw error
}

async function getPayload(
  transport: Pick<HttpReadTransport, "get">,
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
  if (value.trim() !== value || !value.startsWith("b_") || value.length <= 2 || hasUnsafeIdentityCharacters(value)) return null
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
    if (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) return true
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

export async function loadExplorerBoardIdentity(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: ExplorerReadOptions = {},
): Promise<ExplorerBoardIdentity> {
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: Pick<HttpReadTransport, "get">
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

function safeEventCursor(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

function safeEventId(value: unknown): value is number {
  return safeEventCursor(value) && value > 0
}

function stableEventValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableEventValue)
  if (typeof value !== "object" || value === null) return value
  const record = value as Record<string, unknown>
  return Object.fromEntries(Object.keys(record).sort().map((key) => [key, stableEventValue(record[key])]))
}

function eventFingerprint(event: ExplorerEvent): string {
  return JSON.stringify(stableEventValue(event))
}

function validateEventBatch(
  events: readonly ExplorerEvent[],
  board: ExplorerBoardIdentity,
  taskId: string | null,
  after: number,
  nextAfter: number,
  limit: number,
): void {
  if (!safeEventCursor(after) || !safeEventCursor(nextAfter)) {
    throw new ExplorerReadError("anomaly", "事件响应的 cursor 不是非负安全整数。")
  }
  if (!Number.isSafeInteger(limit) || limit < 0 || events.length > limit) {
    throw new ExplorerReadError("anomaly", "事件响应超过请求的 page limit。")
  }
  let previousId = after
  const ids = new Set<number>()
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

/**
 * 按 canonical numeric id 与 event_id 合并看板隔离事件 batch。
 * 返回窗口按 ASC 排序，只保留最新 150 个 id。
 */
export function mergeBoardEvents(
  existing: readonly ExplorerEvent[],
  incoming: readonly ExplorerEvent[],
  boardId: CanonicalBoardId,
): readonly ExplorerEvent[] {
  const byId = new Map<number, ExplorerEvent>()
  const byEventId = new Map<string, ExplorerEvent>()

  const add = (event: ExplorerEvent): void => {
    if (!safeEventId(event.id) || event.event_id.trim().length === 0) {
      throw new ExplorerReadError("anomaly", "事件 batch 含有无效 canonical id。")
    }
    if (event.board_id !== boardId) {
      throw new ExplorerReadError("anomaly", `事件 ${String(event.id)} 越过当前 board scope。`)
    }
    const byIdMatch = byId.get(event.id)
    const byEventIdMatch = byEventId.get(event.event_id)
    if (byIdMatch !== undefined && byIdMatch.event_id !== event.event_id) {
      throw new ExplorerReadError("anomaly", `事件 id ${String(event.id)} 对应多个 event_id。`)
    }
    if (byEventIdMatch !== undefined && byEventIdMatch.id !== event.id) {
      throw new ExplorerReadError("anomaly", `事件 event_id ${event.event_id} 对应多个数字 id。`)
    }
    if (byIdMatch !== undefined || byEventIdMatch !== undefined) {
      const previous = byIdMatch ?? byEventIdMatch
      if (previous && eventFingerprint(previous) !== eventFingerprint(event)) {
        throw new ExplorerReadError("anomaly", `事件 ${String(event.id)} 的重复内容不一致。`)
      }
      return
    }
    byId.set(event.id, event)
    byEventId.set(event.event_id, event)
  }

  for (const event of existing) add(event)
  let previousIncomingId = -1
  for (const event of incoming) {
    if (!safeEventId(event.id) || event.id <= previousIncomingId) {
      throw new ExplorerReadError("anomaly", "事件 batch 的 id 必须严格递增。")
    }
    add(event)
    previousIncomingId = event.id
  }
  const sorted = [...byId.values()].sort((left, right) => left.id - right.id)
  return Object.freeze(sorted.slice(-BOARD_EVENTS_PAGE_LIMIT).map((event) => Object.isFrozen(event) ? event : Object.freeze({ ...event })))
}

export function buildBoardEventsRequest(board: string, taskId: string | null = null, after = 0): string {
  try {
    if (taskId !== null) validateCanonicalTaskSelector(taskId)
    const parsed = parseApiListEventsQuery({
      board,
      task_id: taskId,
      after,
      limit: BOARD_EVENTS_PAGE_LIMIT,
    })
    const params = new URLSearchParams()
    if (parsed.board !== undefined) params.set("board", parsed.board)
    if (parsed.task_id !== undefined && parsed.task_id !== null) params.set("task_id", parsed.task_id)
    if (parsed.after !== undefined) params.set("after", String(parsed.after))
    if (parsed.limit !== undefined) params.set("limit", String(parsed.limit))
    return `/api/v1/events?${params.toString()}`
  } catch (error) {
    if (error instanceof ContractValidationError) {
      throw new ExplorerReadError("invalid_contract", "Board Events 请求不符合 generated query contract。", {
        contractId: "api.list-events.query",
        cause: error,
      })
    }
    throw error
  }
}

export async function loadBoardEvents(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: ExplorerReadOptions & { readonly taskId?: string | null } = {},
): Promise<BoardEventsReadModel> {
  const taskId = options.taskId?.trim() || null
  if (taskId !== null) validateCanonicalTaskSelector(taskId)
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: Pick<HttpReadTransport, "get">
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  try {
    const board = await loadExplorerBoardIdentity(runtime, selector, { ...options, transport, budget })
    const afterOption = options.after ?? 0
    if (!Number.isSafeInteger(afterOption) || afterOption < 0) throw new ExplorerReadError("anomaly", "事件读取 after 必须是非负安全整数。")
    let after = afterOption
    let pageCount = 0
    let events: readonly ExplorerEvent[] = []
    while (true) {
      throwIfAborted(options.signal)
      if (pageCount >= MAX_BOARD_EVENTS_PAGES) {
        throw new ExplorerReadError("anomaly", `事件读取超过 ${MAX_BOARD_EVENTS_PAGES} 页预算。`)
      }
      pageCount += 1
      throwIfAborted(options.signal)
      const payload = await getPayload(transport, buildBoardEventsRequest(board.slug, taskId, after), options.signal, budget)
      throwIfAborted(options.signal)
      const response = parseContract(
        "api.list-events.response",
        parseApiListEventsResponse,
        payload,
      )
      validateEventBatch(response.data, board, taskId, after, response.meta.next_after, BOARD_EVENTS_PAGE_LIMIT)
      events = mergeBoardEvents(events, response.data, board.id)
      if (response.data.length < BOARD_EVENTS_PAGE_LIMIT) {
        after = response.meta.next_after
        break
      }
      if (response.meta.next_after <= after) {
        throw new ExplorerReadError("anomaly", "事件响应的 next_after 必须在完整 page 后前进。")
      }
      after = response.meta.next_after
    }
    return Object.freeze({
      board,
      taskId,
      events,
      meta: Object.freeze({ count: events.length, nextAfter: after, limit: BOARD_EVENTS_PAGE_LIMIT }),
    })
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
  let transport: Pick<HttpReadTransport, "get">
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

function validateTaskMapOptions(options: TaskMapQueryOptions): void {
  if (!Number.isSafeInteger(options.contextDepth) || options.contextDepth < 0 || options.contextDepth > 8) {
    throw new ExplorerReadError("anomaly", "任务关系图 context depth 超出服务端安全范围。")
  }
  if (!Number.isSafeInteger(options.limitNodes) || options.limitNodes < 1 || options.limitNodes > 1000) {
    throw new ExplorerReadError("anomaly", "任务关系图节点上限必须是 1 到 1000 的安全整数。")
  }
}

function validateProvidedBoardIdentity(identity: ExplorerBoardIdentity, selector: string): ExplorerBoardIdentity {
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

export function buildTaskMapRequest(board: string, options: TaskMapQueryOptions = defaultTaskMapQuery): string {
  try {
    validateTaskMapOptions(options)
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

function validateMapBoard(map: ExplorerTaskMap, board: ExplorerBoardIdentity, options: TaskMapQueryOptions): void {
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

export async function loadTaskMap(
  runtime: WebRuntimeConfig,
  selector: string,
  options: ExplorerReadOptions & Partial<TaskMapQueryOptions> & { readonly boardIdentity?: ExplorerBoardIdentity } = {},
): Promise<ExplorerTaskMapReadModel> {
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: Pick<HttpReadTransport, "get">
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  try {
    const queryOptions: TaskMapQueryOptions = {
      activeOnly: options.activeOnly ?? defaultTaskMapQuery.activeOnly,
      contextDepth: options.contextDepth ?? defaultTaskMapQuery.contextDepth,
      includeDoneContext: options.includeDoneContext ?? defaultTaskMapQuery.includeDoneContext,
      includeArchivedContext: options.includeArchivedContext ?? defaultTaskMapQuery.includeArchivedContext,
      hideIsolated: options.hideIsolated ?? defaultTaskMapQuery.hideIsolated,
      limitNodes: options.limitNodes ?? defaultTaskMapQuery.limitNodes,
    }
    validateTaskMapOptions(queryOptions)
    const board = options.boardIdentity
      ? validateProvidedBoardIdentity(options.boardIdentity, selector)
      : await loadExplorerBoardIdentity(runtime, selector, { ...options, transport, budget })
    const map = parseContract(
      "api.board-task-map.response",
      parseApiBoardTaskMapResponse,
      await getPayload(transport, buildTaskMapRequest(board.slug, queryOptions), options.signal, budget),
    ).data
    validateMapBoard(map, board, queryOptions)
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

function validateCanonicalTaskSelector(value: string): void {
  if (parseTaskSelector(value).kind !== "valid") {
    throw new ExplorerReadError("anomaly", "请求的 task selector 必须是 canonical t_ identity。", { reason: "task-not-found" })
  }
}

function hasUnsafeSelector(value: string): boolean {
  if (value.trim() !== value || value.length === 0 || /[\\/?#]/u.test(value)) return true
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) return true
  }
  return false
}

function validateRunSelector(value: string): void {
  if (hasUnsafeSelector(value) || !value.startsWith("r_") || value.length <= 2) {
    throw new ExplorerReadError("anomaly", "Run selector 必须是 canonical r_ identity。")
  }
}

export function buildTaskRunsRequest(taskId: string): string {
  try {
    validateCanonicalTaskSelector(taskId)
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
    validateRunSelector(runId)
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
  let transport: Pick<HttpReadTransport, "get">
  try {
    validateCanonicalTaskSelector(taskId)
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

type TaskInspectorTask = Omit<ApiGetTaskResponseContract["data"], "labels"> & {
  readonly labels: ApiListTaskLabelsResponseContract["data"]
}

export interface TaskInspectorRequests {
  readonly task: string
  readonly labels: string
  readonly neighborhood: string
  readonly dependencies: string
  readonly steps: string
  readonly runs: string
  readonly comments: string
  readonly attachments: string
  readonly events: string
}

function listEventsRequest(board: string, taskId: string): string {
  validateCanonicalTaskSelector(taskId)
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
    validateCanonicalTaskSelector(taskId)
    const task = parseApiGetTaskPath({ task_id: taskId }).task_id
    const neighborhoodPath = parseApiTaskNeighborhoodPath({ task_id: task }).task_id
    const neighborhoodQuery = parseApiTaskNeighborhoodQuery({ depth: 1, include_archived_context: false, limit_nodes: 40 })
    const neighborhoodParams = new URLSearchParams()
    if (neighborhoodQuery.depth !== undefined) neighborhoodParams.set("depth", String(neighborhoodQuery.depth))
    if (neighborhoodQuery.include_archived_context !== undefined) neighborhoodParams.set("include_archived_context", String(neighborhoodQuery.include_archived_context))
    if (neighborhoodQuery.limit_nodes !== undefined) neighborhoodParams.set("limit_nodes", String(neighborhoodQuery.limit_nodes))
    return {
      task: `/api/v1/tasks/${encodedSegment(task)}`,
      labels: `/api/v1/tasks/${encodedSegment(parseApiListTaskLabelsPath({ task_id: task }).task_id)}/labels`,
      neighborhood: `/api/v1/tasks/${encodedSegment(neighborhoodPath)}/neighborhood?${neighborhoodParams.toString()}`,
      dependencies: `/api/v1/tasks/${encodedSegment(parseApiListDependenciesPath({ task_id: task }).task_id)}/dependencies`,
      steps: `/api/v1/tasks/${encodedSegment(parseApiListStepsPath({ task_id: task }).task_id)}/steps`,
      runs: `/api/v1/tasks/${encodedSegment(parseApiListRunsPath({ task_id: task }).task_id)}/runs`,
      comments: `/api/v1/tasks/${encodedSegment(parseApiListCommentsPath({ task_id: task }).task_id)}/comments`,
      attachments: `/api/v1/tasks/${encodedSegment(parseApiListAttachmentsPath({ task_id: task }).task_id)}/attachments`,
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
  if (task.board_id !== board.id || task.board_slug !== board.slug) {
    throw new ExplorerReadError("anomaly", `任务 ${task.id} 不属于当前 canonical board。`, { reason: "task-not-found" })
  }
}

function validateInspectorLabels(
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

function validateNeighborhoodScope(
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

function validateInspectorScope(
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
  options: TaskInspectorReadOptions = {},
): Promise<TaskInspectorReadModel> {
  // Reject a copied/typed task deep link before resolving board identity or
  // constructing a transport request. Invalid selectors are local errors.
  validateCanonicalTaskSelector(taskId)
  const budget = options.budget ?? new ExplorerReadBudget()
  let transport: Pick<HttpReadTransport, "get">
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
    validateInspectorTask(taskResponse.data, board, taskId)
    const labelsPromise = getPayload(transport, requests.labels, linked.signal, budget).then((payload) => parseContract("api.list-task-labels.response", parseApiListTaskLabelsResponse, payload).data)
    const neighborhoodPromise: Promise<ApiTaskNeighborhoodResponseContract["data"] | null> = options.includeNeighborhood === false
      ? Promise.resolve(null)
      : getPayload(transport, requests.neighborhood, linked.signal, budget).then((payload) => parseContract("api.task-neighborhood.response", parseApiTaskNeighborhoodResponse, payload).data)
    const runsPromise: Promise<ApiListRunsResponseContract["data"]> = options.includeRuns === false
      ? Promise.resolve([])
      : getPayload(transport, requests.runs, linked.signal, budget).then((payload) => parseContract("api.list-runs.response", parseApiListRunsResponse, payload).data)
    const eventsPromise: Promise<ApiListEventsResponseContract["data"]> = options.includeEvents === false
      ? Promise.resolve([])
      : getPayload(transport, requests.events, linked.signal, budget).then((payload) => parseContract("api.list-events.response", parseApiListEventsResponse, payload).data)
    const attachmentsPromise: Promise<ApiListAttachmentsResponseContract["data"]> = options.includeAttachments === false
      ? Promise.resolve([])
      : getPayload(transport, requests.attachments, linked.signal, budget).then((payload) => parseContract("api.list-attachments.response", parseApiListAttachmentsResponse, payload).data)
    const [labels, neighborhood, dependencies, steps, runs, comments, attachments, events] = await Promise.all([
      labelsPromise,
      neighborhoodPromise,
      getPayload(transport, requests.dependencies, linked.signal, budget).then((payload) => parseContract("api.list-dependencies.response", parseApiListDependenciesResponse, payload).data),
      getPayload(transport, requests.steps, linked.signal, budget).then((payload) => parseContract("api.list-steps.response", parseApiListStepsResponse, payload).data),
      runsPromise,
      getPayload(transport, requests.comments, linked.signal, budget).then((payload) => parseContract("api.list-comments.response", parseApiListCommentsResponse, payload).data),
      attachmentsPromise,
      eventsPromise,
    ])
    const detail = { neighborhood, dependencies, steps, runs, comments, attachments, events }
    const task = { ...taskResponse.data, labels }
    validateInspectorScope({ ...detail, task }, board, taskId)
    return Object.freeze({
      board,
      task: Object.freeze(task),
      neighborhood,
      dependencies,
      steps,
      runs,
      comments,
      attachments,
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

async function loadInspectorSectionContext(
  runtime: WebRuntimeConfig,
  selector: string,
  taskId: string,
  options: ExplorerReadOptions,
): Promise<{ readonly board: ExplorerBoardIdentity; readonly transport: Pick<HttpReadTransport, "get">; readonly budget: ExplorerReadBudget }> {
  validateCanonicalTaskSelector(taskId)
  const budget = options.budget ?? new ExplorerReadBudget()
  const transport = options.transport ?? createHttpTransport(runtime, options)
  const board = await loadExplorerBoardIdentity(runtime, selector, { ...options, transport, budget })
  return { board, transport, budget }
}

export async function loadTaskInspectorNeighborhood(
  runtime: WebRuntimeConfig,
  selector: string,
  taskId: string,
  options: ExplorerReadOptions = {},
): Promise<ApiTaskNeighborhoodResponseContract["data"]> {
  try {
    const { board, transport, budget } = await loadInspectorSectionContext(runtime, selector, taskId, options)
    const requests = buildTaskInspectorRequests(board.slug, taskId)
    const neighborhood = parseContract(
      "api.task-neighborhood.response",
      parseApiTaskNeighborhoodResponse,
      await getPayload(transport, requests.neighborhood, options.signal, budget),
    ).data
    validateNeighborhoodScope(neighborhood, board, taskId)
    return neighborhood
  } catch (error) {
    return wrapTransportError(error)
  }
}

export async function loadTaskInspectorRuns(
  runtime: WebRuntimeConfig,
  selector: string,
  taskId: string,
  options: ExplorerReadOptions = {},
): Promise<ApiListRunsResponseContract["data"]> {
  try {
    const { board, transport, budget } = await loadInspectorSectionContext(runtime, selector, taskId, options)
    const requests = buildTaskInspectorRequests(board.slug, taskId)
    const runs = parseContract(
      "api.list-runs.response",
      parseApiListRunsResponse,
      await getPayload(transport, requests.runs, options.signal, budget),
    ).data
    validateRunScope(runs, taskId)
    return runs
  } catch (error) {
    return wrapTransportError(error)
  }
}

export async function loadTaskInspectorEvents(
  runtime: WebRuntimeConfig,
  selector: string,
  taskId: string,
  options: ExplorerReadOptions = {},
): Promise<ApiListEventsResponseContract["data"]> {
  try {
    const { board, transport, budget } = await loadInspectorSectionContext(runtime, selector, taskId, options)
    const requests = buildTaskInspectorRequests(board.slug, taskId)
    const response = parseContract(
      "api.list-events.response",
      parseApiListEventsResponse,
      await getPayload(transport, requests.events, options.signal, budget),
    )
    validateEventBatch(response.data, board, taskId, 0, response.meta.next_after, 50)
    return response.data
  } catch (error) {
    return wrapTransportError(error)
  }
}

export async function loadTaskInspectorAttachments(
  runtime: WebRuntimeConfig,
  selector: string,
  taskId: string,
  options: ExplorerReadOptions = {},
): Promise<ApiListAttachmentsResponseContract["data"]> {
  try {
    const { board, transport, budget } = await loadInspectorSectionContext(runtime, selector, taskId, options)
    const requests = buildTaskInspectorRequests(board.slug, taskId)
    const attachments = parseContract(
      "api.list-attachments.response",
      parseApiListAttachmentsResponse,
      await getPayload(transport, requests.attachments, options.signal, budget),
    ).data
    for (const attachment of attachments) {
      if (attachment.board_id !== board.id || attachment.task_id !== taskId) {
        throw new ExplorerReadError("anomaly", "任务附件响应越过了当前 board/task scope。")
      }
    }
    return attachments
  } catch (error) {
    return wrapTransportError(error)
  }
}
