import type { RpcCall } from "./rpc-transport";
import { type CanonicalBoardId } from "../../domain/board-id";

import { parseApiListBoardColumnsPath } from "../../lib/api/generated/contracts/api-list-board-columns-path";

import { parseApiListBoardColumnsResponse } from "../../lib/api/generated/contracts/api-list-board-columns-response";

import type { ApiListBoardColumnsResponseContract } from "../../lib/api/generated/contracts/api-list-board-columns-response";

import { parseApiListBoardsQuery } from "../../lib/api/generated/contracts/api-list-boards-query";

import type { ApiListBoardsResponseContract } from "../../lib/api/generated/contracts/api-list-boards-response";

import type { ApiErrorResponseContract } from "../../lib/api/generated/contracts/api-error-response";

import { parseApiListTasksByStatusPath } from "../../lib/api/generated/contracts/api-list-tasks-by-status-path";

import { parseApiListTasksByStatusQuery } from "../../lib/api/generated/contracts/api-list-tasks-by-status-query";

import type { ApiListTasksByStatusQueryContract } from "../../lib/api/generated/contracts/api-list-tasks-by-status-query";

import type { ApiListTasksByStatusResponseContract } from "../../lib/api/generated/contracts/api-list-tasks-by-status-response";

import { ContractValidationError } from "../../lib/api/generated/runtime";

import { RpcTransportError, type RpcTransport, type RpcTransportOptions } from "./rpc-transport";

export type BoardColumn = ApiListBoardColumnsResponseContract["data"][number]

export type WireBoardTask = ApiListTasksByStatusResponseContract["data"]["statuses"][number]["tasks"][number]

export type BoardTaskFields = Pick<
  WireBoardTask,
  | "id"
  | "seq"
  | "ref"
  | "title"
  | "description"
  | "status"
  | "priority"
  | "position"
  | "scheduled_at"
  | "due_at"
  | "lock_version"
  | "assignee"
  | "status_reason"
  | "last_heartbeat_at"
  | "dependency_blocked"
  | "unfinished_parent_count"
  | "execution_plan_state"
  | "required_step_count"
  | "completed_required_step_count"
  | "optional_step_count"
  | "labels"
>

export type BoardTask = Readonly<Omit<BoardTaskFields, "labels">> & {
  readonly labels: readonly Readonly<WireBoardTask["labels"][number]>[]
}

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
  | "invalid_headers"
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

export type BoardReadTransport = RpcTransport

export interface BoardReadDependencies extends RpcTransportOptions {
  readonly transport?: BoardReadTransport
}

export interface BoardReadModelOptions {
  readonly signal?: AbortSignal
  /** 会话只需项目身份和列；展示任务使用独立分页查询。 */
  readonly includeTasks?: boolean
  readonly includeArchived?: boolean
  readonly taskPageSize?: number
  readonly taskSort?: BoardTaskSort
  readonly dependencies?: BoardReadDependencies
}

export interface BoardReadQuery {
  observe(signal: AbortSignal, next: (model: BoardReadModel) => void, failed: (error: unknown) => void): void
  subscribeConnection(listener: (state: 'connecting' | 'live' | 'offline') => void): () => void
  /** 等待当前完整查询结果；不会创建独立缓存。 */
  load(signal?: AbortSignal): Promise<BoardReadModel>
  /** 请求 refresh，并等待对应连接的 Ready 屏障。 */
  reload(signal?: AbortSignal): Promise<BoardReadModel>
  /** Abort and discard the current generation without issuing a replacement request. */
  invalidate(): void
}

export const DEFAULT_TASK_PAGE_SIZE = 1_000

export const DEFAULT_TASK_OFFSET = 0

export const DEFAULT_TASK_SORT: BoardTaskSort = "position"

export const MAX_TOTAL_TASKS = 50_000

export const MAX_TASK_PAGES = MAX_TOTAL_TASKS

export const MAX_TOTAL_PROTOBUF_BYTES = 64 * 1024 * 1024

export const RESERVED_SLUG_PREFIXES = ["b_", "t_", "r_", "c_", "a_", "l_", "col_", "e_"] as const

export class BoardReadBudget {
  private totalBytes = 0
  private totalTasks = 0

  consumeBytes(bytes: number): void {
    if (!Number.isSafeInteger(bytes) || bytes < 0 || this.totalBytes > MAX_TOTAL_PROTOBUF_BYTES - bytes) {
      throw new BoardReadError("anomaly", `board read Protobuf 超过 ${MAX_TOTAL_PROTOBUF_BYTES} 字节预算。`)
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

export interface LinkedAbortSignal {
  readonly signal: AbortSignal
  readonly cleanup: () => void
}

export function parseContract<T>(contractId: string, parser: (value: unknown) => T, value: unknown): T {
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

export function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

export function generationAbortError(): Error {
  const error = new Error("board read generation 已被取消。")
  error.name = "AbortError"
  return error
}

export function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof BoardReadError) throw error
  if (error instanceof RpcTransportError) {
    const kind = error.kind === "invalid_bytes" ? "anomaly" : error.kind
    throw new BoardReadError(kind, error.message, {
      status: error.status ?? undefined,
      apiError: error.apiError,
      cause: error,
    })
  }
  throw error
}

export function boardQuery(includeArchived: boolean): RpcCall {
  const query = parseApiListBoardsQuery({ include_archived: includeArchived })
  return { method: "ListBoards", query }
}

export function columnsPath(selector: string): RpcCall {
  const path = parseApiListBoardColumnsPath({ board: selector })
  return { method: "ListBoardColumns", path }
}

export function tasksPath(selector: string): RpcCall {
  const path = parseApiListTasksByStatusPath({ board: selector })
  return { method: "ListTasksByStatus", path }
}

export function validateTaskPageSize(value: number | undefined): number {
  const pageSize = value ?? DEFAULT_TASK_PAGE_SIZE
  if (!Number.isSafeInteger(pageSize) || pageSize < 1 || pageSize > DEFAULT_TASK_PAGE_SIZE) {
    throw new BoardReadError("anomaly", "taskPageSize 必须是 1 到 1000 的安全整数。")
  }
  return pageSize
}

export function tasksQuery(
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

export function appendTasksQuery(request: RpcCall, query: ApiListTasksByStatusQueryContract): RpcCall {
  return { ...request, query }
}

export function emptyError(selector: string, reason: "no-boards" | "board-not-found"): BoardReadError {
  const message = reason === "no-boards"
    ? "kanban serve 未返回可用看板。"
    : "请求的看板 selector 未在看板列表中精确匹配。"
  return new BoardReadError("empty", `${message} selector=${JSON.stringify(selector)}`, { reason, selector })
}

export function isCanonicalBoardId(value: string): boolean {
  if (!value.startsWith("b_") || value.length <= 2) return false
  for (const character of value) {
    const code = character.codePointAt(0) ?? 0
    if ((code >= 0 && code <= 0x1f) || (code >= 0x7f && code <= 0x9f)) return false
  }
  return true
}

export function isCanonicalBoardSlug(value: string): boolean {
  if (value.length === 0 || value.length > 64 || !/^[a-z0-9][a-z0-9._-]*$/.test(value)) return false
  return !RESERVED_SLUG_PREFIXES.some((prefix) => value.startsWith(prefix))
}

export function validateBoardList(
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

export function parseColumns(
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

export function projectTask(task: WireBoardTask): BoardTask {
  return Object.freeze({
    id: task.id,
    seq: task.seq,
    ref: task.ref,
    title: task.title,
    description: task.description,
    status: task.status,
    assignee: task.assignee,
    priority: task.priority,
    position: task.position,
    scheduled_at: task.scheduled_at,
    due_at: task.due_at,
    lock_version: task.lock_version,
    dependency_blocked: task.dependency_blocked,
    status_reason: task.status_reason,
    last_heartbeat_at: task.last_heartbeat_at,
    unfinished_parent_count: task.unfinished_parent_count,
    execution_plan_state: task.execution_plan_state,
    required_step_count: task.required_step_count,
    completed_required_step_count: task.completed_required_step_count,
    optional_step_count: task.optional_step_count,
    labels: Object.freeze(task.labels.map((label) => Object.freeze({ ...label }))),
  })
}
