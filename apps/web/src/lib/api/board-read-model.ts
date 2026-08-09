import type { WebRuntimeConfig } from "../runtime"
import { asCanonicalBoardId, type CanonicalBoardId } from "../sync/contracts"
import { parseApiListBoardColumnsPath } from "./generated/contracts/api-list-board-columns-path"
import {
  parseApiListBoardColumnsResponse,
} from "./generated/contracts/api-list-board-columns-response"
import type { ApiListBoardColumnsResponseContract } from "./generated/contracts/api-list-board-columns-response"
import { parseApiListBoardsQuery } from "./generated/contracts/api-list-boards-query"
import { parseApiListBoardsResponse } from "./generated/contracts/api-list-boards-response"
import type { ApiListBoardsResponseContract } from "./generated/contracts/api-list-boards-response"
import type { ApiErrorResponseContract } from "./generated/contracts/api-error-response"
import { parseApiListTasksByStatusPath } from "./generated/contracts/api-list-tasks-by-status-path"
import { parseApiListTasksByStatusQuery } from "./generated/contracts/api-list-tasks-by-status-query"
import type { ApiListTasksByStatusQueryContract } from "./generated/contracts/api-list-tasks-by-status-query"
import { parseApiListTasksByStatusResponse } from "./generated/contracts/api-list-tasks-by-status-response"
import type { ApiListTasksByStatusResponseContract } from "./generated/contracts/api-list-tasks-by-status-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpTransport,
  type HttpTransportOptions,
  type HttpTransportResponse,
} from "./http-transport"

export type BoardColumn = ApiListBoardColumnsResponseContract["data"][number]
type WireBoardTask = ApiListTasksByStatusResponseContract["data"]["statuses"][number]["tasks"][number]

/** The task fields needed by board cards and sync projections; large payload fields are discarded. */
export type BoardTask = Readonly<Pick<
  WireBoardTask,
  | "id"
  | "ref"
  | "title"
  | "status"
  | "priority"
  | "position"
  | "assignee"
  | "dependency_blocked"
  | "unfinished_parent_count"
  | "execution_plan_state"
  | "required_step_count"
  | "completed_required_step_count"
  | "optional_step_count"
>>

export type BoardTaskStatus = BoardColumn["status"]
export type BoardTaskSort = NonNullable<ApiListTasksByStatusQueryContract["sort"]>
export type BoardTaskGroups = Readonly<Partial<Record<BoardTaskStatus, readonly BoardTask[]>>>

export interface ResolvedBoardIdentity {
  readonly selector: string
  readonly canonicalBoardId: CanonicalBoardId
  readonly slug: string
  readonly name: string
}

export interface BoardReadModel {
  readonly identity: ResolvedBoardIdentity
  readonly columns: readonly BoardColumn[]
  readonly tasksByStatus: BoardTaskGroups
}

export type BoardReadErrorKind =
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

export class BoardReadError extends Error {
  readonly kind: BoardReadErrorKind
  readonly reason: "no-boards" | "board-not-found" | null
  readonly status: number | null
  readonly contractId: string | null
  readonly apiError: ApiErrorResponseContract["error"] | null
  readonly selector: string | null

  constructor(
    kind: BoardReadErrorKind,
    message: string,
    options: {
      reason?: BoardReadError["reason"]
      status?: number
      contractId?: string
      apiError?: ApiErrorResponseContract["error"] | null
      selector?: string
      cause?: unknown
    } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "BoardReadError"
    this.kind = kind
    this.reason = options.reason ?? null
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
    this.apiError = options.apiError ?? null
    this.selector = options.selector ?? null
  }

  /** Structured server error code, without exposing the raw response body. */
  get code(): ApiErrorResponseContract["error"]["code"] | null {
    return this.apiError?.code ?? null
  }
}

export type BoardReadTransport = HttpTransport

export interface BoardReadDependencies extends HttpTransportOptions {
  readonly transport?: BoardReadTransport
}

export interface BoardReadModelOptions {
  readonly signal?: AbortSignal
  readonly includeArchived?: boolean
  readonly taskPageSize?: number
  readonly taskSort?: BoardTaskSort
  readonly dependencies?: BoardReadDependencies
}

export interface BoardReadQuery {
  /** Load the cached snapshot or the current generation once. */
  load(signal?: AbortSignal): Promise<BoardReadModel>
  /** Abort the current generation, invalidate it, and load a fresh snapshot. */
  reload(signal?: AbortSignal): Promise<BoardReadModel>
  /** Abort and discard the current generation without issuing a replacement request. */
  invalidate(): void
}

const DEFAULT_TASK_PAGE_SIZE = 1_000
const DEFAULT_TASK_OFFSET = 0
const DEFAULT_TASK_SORT: BoardTaskSort = "position"
const MAX_TOTAL_TASKS = 50_000
const MAX_TASK_PAGES = MAX_TOTAL_TASKS
const MAX_TOTAL_JSON_BYTES = 64 * 1024 * 1024
const RESERVED_SLUG_PREFIXES = ["b_", "t_", "r_", "c_", "a_", "l_", "col_", "e_"] as const

class BoardReadBudget {
  private totalBytes = 0
  private totalTasks = 0

  consumeBytes(bytes: number): void {
    if (!Number.isSafeInteger(bytes) || bytes < 0 || this.totalBytes > MAX_TOTAL_JSON_BYTES - bytes) {
      throw new BoardReadError("anomaly", `board read raw JSON 超过 ${MAX_TOTAL_JSON_BYTES} 字节预算。`)
    }
    this.totalBytes += bytes
  }

  consumeTasks(count: number): void {
    if (!Number.isSafeInteger(count) || count < 0 || this.totalTasks > MAX_TOTAL_TASKS - count) {
      throw new BoardReadError("anomaly", `board read 任务数超过 ${MAX_TOTAL_TASKS} 条预算。`)
    }
    this.totalTasks += count
  }
}

interface LinkedAbortSignal {
  readonly signal: AbortSignal
  readonly cleanup: () => void
}

function linkAbortSignals(signals: readonly (AbortSignal | undefined)[]): LinkedAbortSignal {
  const controller = new AbortController()
  const activeSignals = signals.filter((signal): signal is AbortSignal => signal !== undefined)
  const abort = () => controller.abort()
  for (const signal of activeSignals) {
    if (signal.aborted) controller.abort()
    else signal.addEventListener("abort", abort, { once: true })
  }
  return {
    signal: controller.signal,
    cleanup: () => {
      for (const signal of activeSignals) signal.removeEventListener("abort", abort)
    },
  }
}

function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

function parseContract<T>(contractId: string, parser: (value: unknown) => T, value: unknown): T {
  try {
    return parser(value)
  } catch (error) {
    if (error instanceof ContractValidationError) {
      throw new BoardReadError(
        "invalid_contract",
        `Web API 响应不符合 ${contractId} contract。`,
        { contractId, cause: error },
      )
    }
    throw error
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function generationAbortError(): Error {
  const error = new Error("board read generation 已被取消。")
  error.name = "AbortError"
  return error
}

function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof BoardReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new BoardReadError(error.kind, error.message, {
      status: error.status ?? undefined,
      apiError: error.apiError,
      cause: error,
    })
  }
  throw error
}

async function get(
  transport: BoardReadTransport,
  path: string,
  signal: AbortSignal | undefined,
  budget: BoardReadBudget,
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
      throw new BoardReadError("anomaly", "Web API transport 返回了无效 raw JSON 字节数。")
    }
    budget.consumeBytes(response.bytes)
    return response.payload
  } catch (error) {
    return wrapTransportError(error)
  }
}

function boardQuery(includeArchived: boolean): string {
  const query = parseApiListBoardsQuery({ include_archived: includeArchived })
  const params = new URLSearchParams()
  params.set("include_archived", String(query.include_archived))
  return `/api/v1/boards?${params.toString()}`
}

function columnsPath(selector: string): string {
  const path = parseApiListBoardColumnsPath({ board: selector })
  return `/api/v1/boards/${encodedSegment(path.board)}/columns`
}

function tasksPath(selector: string): string {
  const path = parseApiListTasksByStatusPath({ board: selector })
  return `/api/v1/boards/${encodedSegment(path.board)}/tasks/by-status`
}

function validateTaskPageSize(value: number | undefined): number {
  const pageSize = value ?? DEFAULT_TASK_PAGE_SIZE
  if (!Number.isSafeInteger(pageSize) || pageSize < 1 || pageSize > DEFAULT_TASK_PAGE_SIZE) {
    throw new BoardReadError("anomaly", "taskPageSize 必须是 1 到 1000 的安全整数。")
  }
  return pageSize
}

function tasksQuery(
  status: BoardTaskStatus,
  options: BoardReadModelOptions,
  offset: number,
  pageSize: number,
): ApiListTasksByStatusQueryContract {
  const includeArchived = options.includeArchived ?? false
  const sort = options.taskSort ?? DEFAULT_TASK_SORT
  return parseContract(
    "api.list-tasks-by-status.query",
    parseApiListTasksByStatusQuery,
    {
      status: [status],
      priority: [],
      label: [],
      plan_filter: [],
      assignee: null,
      q: null,
      include_archived: includeArchived,
      limit: pageSize,
      offset,
      sort,
    },
  )
}

function appendTasksQuery(path: string, query: ApiListTasksByStatusQueryContract): string {
  const params = new URLSearchParams()
  for (const status of query.status ?? []) params.append("status", status)
  for (const priority of query.priority ?? []) params.append("priority", String(priority))
  for (const label of query.label ?? []) params.append("label", label)
  for (const planFilter of query.plan_filter ?? []) params.append("plan_filter", planFilter)
  if (query.assignee !== undefined && query.assignee !== null) params.set("assignee", query.assignee)
  if (query.q !== undefined && query.q !== null) params.set("q", query.q)
  if (query.include_archived !== undefined) params.set("include_archived", String(query.include_archived))
  if (query.limit !== undefined) params.set("limit", String(query.limit))
  if (query.offset !== undefined) params.set("offset", String(query.offset))
  if (query.sort !== undefined) params.set("sort", query.sort)
  return `${path}?${params.toString()}`
}

function emptyError(selector: string, reason: "no-boards" | "board-not-found"): BoardReadError {
  const message = reason === "no-boards"
    ? "kanban serve 未返回可用看板。"
    : "请求的看板 selector 未在看板列表中精确匹配。"
  return new BoardReadError("empty", `${message} selector=${JSON.stringify(selector)}`, { reason, selector })
}

function isCanonicalBoardId(value: string): boolean {
  if (!value.startsWith("b_") || value.length <= 2) return false
  for (const character of value) {
    const code = character.codePointAt(0) ?? 0
    if ((code >= 0 && code <= 0x1f) || (code >= 0x7f && code <= 0x9f)) return false
  }
  return true
}

function isCanonicalBoardSlug(value: string): boolean {
  if (value.length === 0 || value.length > 64 || !/^[a-z0-9][a-z0-9._-]*$/.test(value)) return false
  return !RESERVED_SLUG_PREFIXES.some((prefix) => value.startsWith(prefix))
}

function validateBoardList(
  boards: ApiListBoardsResponseContract["data"],
  selector: string,
): void {
  const ids = new Set<string>()
  const slugs = new Set<string>()
  for (const board of boards) {
    if (
      board.id.length === 0
      || board.slug.length === 0
      || board.name.trim().length === 0
      || !isCanonicalBoardId(board.id)
      || !isCanonicalBoardSlug(board.slug)
    ) {
      throw new BoardReadError(
        "anomaly",
        `看板列表包含无效 canonical identity：selector=${JSON.stringify(selector)}。`,
        { selector },
      )
    }
    if (ids.has(board.id) || slugs.has(board.slug)) {
      throw new BoardReadError(
        "anomaly",
        `看板列表包含重复 id 或 slug：selector=${JSON.stringify(selector)}。`,
        { selector },
      )
    }
    ids.add(board.id)
    slugs.add(board.slug)
  }
}

async function resolveIdentity(
  transport: BoardReadTransport,
  selector: string,
  includeArchived: boolean,
  signal: AbortSignal,
  budget: BoardReadBudget,
): Promise<ResolvedBoardIdentity> {
  const requestedSelector = selector.trim()
  if (requestedSelector.length === 0) throw emptyError(requestedSelector, "board-not-found")
  const payload = parseContract(
    "api.list-boards.response",
    parseApiListBoardsResponse,
    await get(transport, boardQuery(includeArchived), signal, budget),
  )
  if (payload.data.length === 0) throw emptyError(requestedSelector, "no-boards")
  validateBoardList(payload.data, requestedSelector)
  const matches = payload.data.filter((board) => board.id === requestedSelector || board.slug === requestedSelector)
  if (matches.length === 0) throw emptyError(requestedSelector, "board-not-found")
  if (matches.length > 1) {
    throw new BoardReadError("anomaly", `selector 同时匹配多个看板：${JSON.stringify(requestedSelector)}。`, { selector: requestedSelector })
  }
  const board = matches[0]
  if (board === undefined) throw emptyError(requestedSelector, "board-not-found")
  let canonicalBoardId: CanonicalBoardId
  try {
    canonicalBoardId = asCanonicalBoardId(board.id)
  } catch (error) {
    throw new BoardReadError("anomaly", "看板响应的 canonical id 为空。", { selector: requestedSelector, cause: error })
  }
  return Object.freeze({ selector: requestedSelector, canonicalBoardId, slug: board.slug, name: board.name.trim() })
}

function parseColumns(
  payload: unknown,
  identity: ResolvedBoardIdentity,
): readonly BoardColumn[] {
  const response = parseContract(
    "api.list-board-columns.response",
    parseApiListBoardColumnsResponse,
    payload,
  )
  const columnIds = new Set<string>()
  const statuses = new Set<BoardTaskStatus>()
  const positions = new Set<number>()
  for (const column of response.data) {
    if (column.id.trim().length === 0 || column.title.trim().length === 0) {
      throw new BoardReadError("anomaly", "看板列响应缺少 id 或 title。")
    }
    if (column.board_id !== identity.canonicalBoardId) {
      throw new BoardReadError("anomaly", `看板列 ${column.id} 返回了错误的 board_id。`)
    }
    if (columnIds.has(column.id) || statuses.has(column.status) || positions.has(column.position)) {
      throw new BoardReadError("anomaly", "看板列违反了 board 内唯一 id/status/position 约束。")
    }
    columnIds.add(column.id)
    statuses.add(column.status)
    positions.add(column.position)
  }
  return Object.freeze(response.data.map((column) => Object.freeze({ ...column })))
}

function parseTasksWindow(
  payload: unknown,
  identity: ResolvedBoardIdentity,
  status: BoardTaskStatus,
  expectedOffset: number,
  expectedLimit: number,
  expectedTotal: number | null,
): { tasks: readonly WireBoardTask[]; total: number; nextOffset: number } {
  const response = parseContract(
    "api.list-tasks-by-status.response",
    parseApiListTasksByStatusResponse,
    payload,
  )
  if (response.meta.limit !== expectedLimit || response.meta.offset !== expectedOffset) {
    throw new BoardReadError("anomaly", "tasks-by-status 的 meta 与请求 offset/limit 不一致。")
  }
  if (response.data.statuses.length !== 1) {
    throw new BoardReadError("anomaly", "tasks-by-status 返回了超出请求 status 的窗口。")
  }
  const window = response.data.statuses[0]
  if (window === undefined || window.status !== status) {
    throw new BoardReadError("anomaly", `tasks-by-status 返回了错误的 status window：${status}。`)
  }
  if (window.page.offset !== expectedOffset || window.page.limit !== expectedLimit) {
    throw new BoardReadError("anomaly", "tasks-by-status 的 page 与请求 offset/limit 不一致。")
  }
  if (expectedTotal !== null && window.page.total !== expectedTotal) {
    throw new BoardReadError("anomaly", "tasks-by-status 的 page.total 在分页过程中发生漂移。")
  }
  if (window.tasks.length > expectedLimit) {
    throw new BoardReadError("anomaly", "tasks-by-status 返回的任务数超过 page.limit。")
  }
  for (const task of window.tasks) {
    if (task.id.trim().length === 0 || task.title.trim().length === 0) throw new BoardReadError("anomaly", "任务响应缺少 id 或 title。")
    if (task.status !== status) throw new BoardReadError("anomaly", `任务 ${task.id} 的 status 与请求窗口不一致。`)
    if (task.board_id !== identity.canonicalBoardId || task.board_slug !== identity.slug) {
      throw new BoardReadError("anomaly", `任务 ${task.id} 不属于当前 canonical board。`)
    }
  }
  if (window.page.total > expectedOffset && window.tasks.length === 0) {
    throw new BoardReadError("anomaly", "tasks-by-status 返回空页但 page.total 仍要求继续分页。")
  }
  const nextOffset = expectedOffset + expectedLimit
  if (!Number.isSafeInteger(nextOffset) || (window.page.total > expectedOffset && nextOffset <= expectedOffset)) {
    throw new BoardReadError("anomaly", "tasks-by-status offset 超出安全分页范围。")
  }
  return { tasks: window.tasks, total: window.page.total, nextOffset }
}

function projectTask(task: WireBoardTask): BoardTask {
  return Object.freeze({
    id: task.id,
    ref: task.ref,
    title: task.title,
    status: task.status,
    assignee: task.assignee,
    priority: task.priority,
    position: task.position,
    dependency_blocked: task.dependency_blocked,
    unfinished_parent_count: task.unfinished_parent_count,
    execution_plan_state: task.execution_plan_state,
    required_step_count: task.required_step_count,
    completed_required_step_count: task.completed_required_step_count,
    optional_step_count: task.optional_step_count,
  })
}

/** Traverse every server task page for one column status, preserving order and completeness. */
async function loadTasksForStatus(
  transport: BoardReadTransport,
  identity: ResolvedBoardIdentity,
  status: BoardTaskStatus,
  options: BoardReadModelOptions,
  pageSize: number,
  signal: AbortSignal,
  budget: BoardReadBudget,
): Promise<readonly BoardTask[]> {
  const basePath = tasksPath(identity.slug)
  let offset = DEFAULT_TASK_OFFSET
  let total: number | null = null
  let pages = 0
  const tasks: BoardTask[] = []
  const taskIds = new Set<string>()
  while (true) {
    if (pages >= MAX_TASK_PAGES) throw new BoardReadError("anomaly", "tasks-by-status 分页超过安全页数上限。")
    const query = tasksQuery(status, options, offset, pageSize)
    const parsed = parseTasksWindow(
      await get(transport, appendTasksQuery(basePath, query), signal, budget),
      identity,
      status,
      offset,
      pageSize,
      total,
    )
    total ??= parsed.total
    budget.consumeTasks(parsed.tasks.length)
    for (const task of parsed.tasks) {
      if (taskIds.has(task.id)) throw new BoardReadError("anomaly", `tasks-by-status 返回重复任务 ${task.id}。`)
      taskIds.add(task.id)
      tasks.push(projectTask(task))
    }
    pages += 1
    if (parsed.nextOffset >= parsed.total) break
    if (parsed.nextOffset <= offset) throw new BoardReadError("anomaly", "tasks-by-status 分页没有前进。")
    offset = parsed.nextOffset
  }
  if (total === null || tasks.length !== total) {
    throw new BoardReadError("anomaly", "tasks-by-status 返回的任务总数未完整覆盖 page.total。")
  }
  return Object.freeze(tasks)
}

/** Load the canonical board, server columns, and all status windows into one immutable read model. */
export async function loadBoardReadModel(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: BoardReadModelOptions = {},
): Promise<BoardReadModel> {
  const includeArchived = options.includeArchived ?? false
  const pageSize = validateTaskPageSize(options.taskPageSize)
  const statusController = new AbortController()
  const linked = linkAbortSignals([options.signal, statusController.signal])
  const budget = new BoardReadBudget()
  try {
    let transport: BoardReadTransport
    try {
      transport = options.dependencies?.transport
        ?? createHttpTransport(runtime, options.dependencies)
    } catch (error) {
      return wrapTransportError(error)
    }
    const identity = await resolveIdentity(transport, selector, includeArchived, linked.signal, budget)
    const columns = parseColumns(
      await get(transport, columnsPath(identity.slug), linked.signal, budget),
      identity,
    )
    const statuses = columns.map((column) => column.status)
    const windows = await Promise.all(
      statuses.map(async (status) => ({
        status,
        tasks: await loadTasksForStatus(transport, identity, status, options, pageSize, linked.signal, budget),
      })),
    )
    const grouped: Partial<Record<BoardTaskStatus, readonly BoardTask[]>> = {}
    const taskIds = new Set<string>()
    for (const window of windows) {
      for (const task of window.tasks) {
        if (taskIds.has(task.id)) throw new BoardReadError("anomaly", `tasks-by-status 返回跨 status 重复任务 ${task.id}。`)
        taskIds.add(task.id)
      }
      grouped[window.status] = window.tasks
    }
    const tasksByStatus = Object.freeze(grouped)
    return Object.freeze({ identity, columns, tasksByStatus })
  } catch (error) {
    statusController.abort()
    return wrapTransportError(error)
  } finally {
    linked.cleanup()
  }
}

export function createBoardReadQuery(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: BoardReadModelOptions = {},
): BoardReadQuery {
  let generation = 0
  let cached: BoardReadModel | null = null
  let generationController: AbortController | null = null
  let pending: { readonly generation: number; readonly promise: Promise<BoardReadModel> } | null = null

  const load = (signal?: AbortSignal): Promise<BoardReadModel> => {
    if (cached !== null) return Promise.resolve(cached)
    if (pending !== null && pending.generation === generation) return pending.promise

    const requestGeneration = generation
    const controller = new AbortController()
    const linked = linkAbortSignals([options.signal, signal, controller.signal])
    generationController = controller
    const loadOptions = { ...options, signal: linked.signal }
    const promise = loadBoardReadModel(runtime, selector, loadOptions).then(
      (model) => {
        if (requestGeneration !== generation) throw generationAbortError()
        cached = model
        pending = null
        generationController = null
        return model
      },
      (error: unknown) => {
        if (requestGeneration === generation) {
          pending = null
          generationController = null
        }
        throw error
      },
    ).finally(() => linked.cleanup())
    pending = { generation: requestGeneration, promise }
    return promise
  }

  const invalidate = (): void => {
    generationController?.abort()
    generationController = null
    generation += 1
    cached = null
    pending = null
  }

  return {
    load,
    reload(signal) {
      invalidate()
      return load(signal)
    },
    invalidate,
  }
}
