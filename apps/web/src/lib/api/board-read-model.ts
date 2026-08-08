import type { WebRuntimeConfig } from "../runtime"
import { asCanonicalBoardId, type CanonicalBoardId } from "../sync/contracts"
import {
  parseApiListBoardColumnsPath,
} from "./generated/contracts/api-list-board-columns-path"
import {
  parseApiListBoardColumnsResponse,
} from "./generated/contracts/api-list-board-columns-response"
import type { ApiListBoardColumnsResponseContract } from "./generated/contracts/api-list-board-columns-response"
import { parseApiListBoardsQuery } from "./generated/contracts/api-list-boards-query"
import { parseApiListBoardsResponse } from "./generated/contracts/api-list-boards-response"
import { parseApiListTasksByStatusPath } from "./generated/contracts/api-list-tasks-by-status-path"
import {
  parseApiListTasksByStatusQuery,
} from "./generated/contracts/api-list-tasks-by-status-query"
import type { ApiListTasksByStatusQueryContract } from "./generated/contracts/api-list-tasks-by-status-query"
import {
  parseApiListTasksByStatusResponse,
} from "./generated/contracts/api-list-tasks-by-status-response"
import type { ApiListTasksByStatusResponseContract } from "./generated/contracts/api-list-tasks-by-status-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpTransport,
  type HttpTransportOptions,
} from "./http-transport"

export type BoardColumn = ApiListBoardColumnsResponseContract["data"][number]
export type BoardTask = ApiListTasksByStatusResponseContract["data"]["statuses"][number]["tasks"][number]
export type BoardTaskStatus = BoardColumn["status"]
export type BoardTaskSort = NonNullable<ApiListTasksByStatusQueryContract["sort"]>

export interface ResolvedBoardIdentity {
  readonly selector: string
  readonly canonicalBoardId: CanonicalBoardId
  readonly slug: string
  readonly name: string
}

export interface BoardReadModel {
  readonly identity: ResolvedBoardIdentity
  readonly columns: readonly BoardColumn[]
  readonly tasksByStatus: ReadonlyMap<BoardTaskStatus, readonly BoardTask[]>
}

export type BoardReadErrorKind =
  | "empty"
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_contract"
  | "anomaly"
  | "cross_origin"

export class BoardReadError extends Error {
  readonly kind: BoardReadErrorKind
  readonly reason: "no-boards" | "board-not-found" | null
  readonly status: number | null
  readonly contractId: string | null

  constructor(
    kind: BoardReadErrorKind,
    message: string,
    options: {
      reason?: BoardReadError["reason"]
      status?: number
      contractId?: string
      cause?: unknown
    } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "BoardReadError"
    this.kind = kind
    this.reason = options.reason ?? null
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
  }
}

export type BoardReadTransport = HttpTransport

export interface BoardReadModelOptions extends HttpTransportOptions {
  readonly transport?: BoardReadTransport
  readonly signal?: AbortSignal
  readonly includeArchived?: boolean
  readonly taskLimit?: number
  readonly taskSort?: BoardTaskSort
}

export interface BoardReadQuery {
  load(signal?: AbortSignal): Promise<BoardReadModel>
  reload(signal?: AbortSignal): Promise<BoardReadModel>
  invalidate(): void
}

const DEFAULT_TASK_LIMIT = 1_000
const DEFAULT_TASK_OFFSET = 0
const DEFAULT_TASK_SORT: BoardTaskSort = "position"
const MAX_TASK_PAGES = 1_024

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

function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof BoardReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new BoardReadError(error.kind, error.message, {
      status: error.status ?? undefined,
      cause: error,
    })
  }
  throw error
}

async function get(transport: BoardReadTransport, path: string, signal?: AbortSignal): Promise<unknown> {
  try {
    return await transport.get(path, signal)
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

function tasksQuery(
  status: BoardTaskStatus,
  options: BoardReadModelOptions,
  offset: number,
): ApiListTasksByStatusQueryContract {
  const limit = options.taskLimit ?? DEFAULT_TASK_LIMIT
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
      limit,
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
    : "runtime.defaultBoard 未在看板列表中精确匹配。"
  return new BoardReadError("empty", `${message} selector=${JSON.stringify(selector)}`, { reason })
}

async function resolveIdentity(
  transport: BoardReadTransport,
  selector: string,
  includeArchived: boolean,
  signal?: AbortSignal,
): Promise<ResolvedBoardIdentity> {
  if (selector.length === 0) throw emptyError(selector, "board-not-found")
  const payload = parseContract(
    "api.list-boards.response",
    parseApiListBoardsResponse,
    await get(transport, boardQuery(includeArchived), signal),
  )
  if (payload.data.length === 0) throw emptyError(selector, "no-boards")
  const matches = payload.data.filter((board) => board.id === selector || board.slug === selector)
  if (matches.length === 0) throw emptyError(selector, "board-not-found")
  if (matches.length > 1) throw new BoardReadError("anomaly", "selector 同时匹配多个看板。")
  const board = matches[0]
  if (board === undefined) throw emptyError(selector, "board-not-found")
  if (board.id.length === 0 || board.slug.length === 0 || board.name.length === 0) {
    throw new BoardReadError("anomaly", "看板响应缺少 canonical identity 字段。")
  }
  let canonicalBoardId: CanonicalBoardId
  try {
    canonicalBoardId = asCanonicalBoardId(board.id)
  } catch (error) {
    throw new BoardReadError("anomaly", "看板响应的 canonical id 为空。", { cause: error })
  }
  return { selector, canonicalBoardId, slug: board.slug, name: board.name }
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
    if (column.id.length === 0 || column.title.length === 0) {
      throw new BoardReadError("anomaly", "看板列响应缺少 id 或 title。")
    }
    if (column.board_id !== identity.canonicalBoardId) {
      throw new BoardReadError(
        "anomaly",
        `看板列 ${column.id} 返回了错误的 board_id。`,
      )
    }
    if (columnIds.has(column.id) || statuses.has(column.status) || positions.has(column.position)) {
      throw new BoardReadError("anomaly", "看板列违反了 board 内唯一 id/status/position 约束。")
    }
    columnIds.add(column.id)
    statuses.add(column.status)
    positions.add(column.position)
  }
  return Object.freeze([...response.data])
}

function parseTasksWindow(
  payload: unknown,
  identity: ResolvedBoardIdentity,
  status: BoardTaskStatus,
  expectedOffset: number,
  expectedLimit: number,
  expectedTotal: number | null,
): { tasks: readonly BoardTask[]; total: number; nextOffset: number } {
  const response = parseContract(
    "api.list-tasks-by-status.response",
    parseApiListTasksByStatusResponse,
    payload,
  )
  if (response.meta.limit !== expectedLimit || response.meta.offset !== expectedOffset) {
    throw new BoardReadError("anomaly", `tasks-by-status 的 meta 与请求 offset/limit 不一致。`)
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
    if (task.id.length === 0 || task.title.length === 0) throw new BoardReadError("anomaly", "任务响应缺少 id 或 title。")
    if (task.status !== status) throw new BoardReadError("anomaly", `任务 ${task.id} 的 status 与请求窗口不一致。`)
    if (task.board_id !== identity.canonicalBoardId || task.board_slug !== identity.slug) {
      throw new BoardReadError("anomaly", `任务 ${task.id} 不属于当前 canonical board。`)
    }
  }
  if (window.page.total > expectedOffset && expectedLimit <= 0) {
    throw new BoardReadError("anomaly", "tasks-by-status 分页没有前进。")
  }
  if (window.page.total > expectedOffset && window.tasks.length === 0) {
    throw new BoardReadError("anomaly", "tasks-by-status 返回空页但 page.total 仍要求继续分页。")
  }
  const nextOffset = expectedLimit === 0 ? expectedOffset : expectedOffset + expectedLimit
  if (!Number.isSafeInteger(nextOffset) || (window.page.total > expectedOffset && nextOffset <= expectedOffset)) {
    throw new BoardReadError("anomaly", "tasks-by-status offset 超出安全分页范围。")
  }
  return { tasks: Object.freeze([...window.tasks]), total: window.page.total, nextOffset }
}

async function loadTasksForStatus(
  transport: BoardReadTransport,
  identity: ResolvedBoardIdentity,
  status: BoardTaskStatus,
  options: BoardReadModelOptions,
  signal?: AbortSignal,
): Promise<readonly BoardTask[]> {
  const basePath = tasksPath(identity.canonicalBoardId)
  let offset = DEFAULT_TASK_OFFSET
  const limit = options.taskLimit ?? DEFAULT_TASK_LIMIT
  let total: number | null = null
  let pages = 0
  const tasks: BoardTask[] = []
  const taskIds = new Set<string>()
  while (true) {
    if (pages >= MAX_TASK_PAGES) throw new BoardReadError("anomaly", "tasks-by-status 分页超过安全页数上限。")
    const query = tasksQuery(status, options, offset)
    const parsed = parseTasksWindow(
      await get(transport, appendTasksQuery(basePath, query), signal),
      identity,
      status,
      offset,
      limit,
      total,
    )
    total ??= parsed.total
    for (const task of parsed.tasks) {
      if (taskIds.has(task.id)) throw new BoardReadError("anomaly", `tasks-by-status 返回重复任务 ${task.id}。`)
      taskIds.add(task.id)
      tasks.push(task)
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

export async function loadBoardReadModel(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: BoardReadModelOptions = {},
): Promise<BoardReadModel> {
  const includeArchived = options.includeArchived ?? false
  let transport: BoardReadTransport
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }
  const identity = await resolveIdentity(transport, selector, includeArchived, options.signal)
  const columns = parseColumns(
    await get(transport, columnsPath(identity.canonicalBoardId), options.signal),
    identity,
  )
  const statuses = [...new Set(columns.map((column) => column.status))]
  const windows = await Promise.all(
    statuses.map(async (status) => ({ status, tasks: await loadTasksForStatus(transport, identity, status, options, options.signal) })),
  )
  const tasksByStatus = new Map<BoardTaskStatus, readonly BoardTask[]>()
  const taskIds = new Set<string>()
  for (const window of windows) {
    for (const task of window.tasks) {
      if (taskIds.has(task.id)) throw new BoardReadError("anomaly", `tasks-by-status 返回跨 status 重复任务 ${task.id}。`)
      taskIds.add(task.id)
    }
    tasksByStatus.set(window.status, window.tasks)
  }
  return {
    identity,
    columns,
    tasksByStatus,
  }
}

export function createBoardReadQuery(
  runtime: WebRuntimeConfig,
  selector = runtime.defaultBoard,
  options: BoardReadModelOptions = {},
): BoardReadQuery {
  let generation = 0
  let cached: BoardReadModel | null = null
  let pending: { readonly generation: number; readonly promise: Promise<BoardReadModel> } | null = null

  const load = (signal?: AbortSignal): Promise<BoardReadModel> => {
    if (cached !== null) return Promise.resolve(cached)
    if (pending !== null && pending.generation === generation) return pending.promise
    const requestGeneration = generation
    const loadOptions = signal === undefined ? options : { ...options, signal }
    const promise = loadBoardReadModel(runtime, selector, loadOptions).then(
      (model) => {
        if (requestGeneration === generation) {
          cached = model
          pending = null
        }
        return model
      },
      (error: unknown) => {
        if (requestGeneration === generation) pending = null
        throw error
      },
    )
    pending = { generation: requestGeneration, promise }
    return promise
  }

  return {
    load,
    reload(signal) {
      generation += 1
      cached = null
      pending = null
      return load(signal)
    },
    invalidate() {
      generation += 1
      cached = null
      pending = null
    },
  }
}
