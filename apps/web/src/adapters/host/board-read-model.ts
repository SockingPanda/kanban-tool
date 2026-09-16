import type { Integer } from '../../domain/integer'
import type { RpcCall } from "../../application/data/rpc-transport";
import { inheritReadScope } from '../../application/query/observe-read';
import type { WebRuntimeConfig } from "../../lib/runtime";

import { asCanonicalBoardId, type CanonicalBoardId } from "../../domain/board-id";

import { parseApiListBoardsResponse } from "../../lib/api/generated/contracts/api-list-boards-response";

import { parseApiListTasksByStatusResponse } from "../../lib/api/generated/contracts/api-list-tasks-by-status-response";

import { createRpcTransport } from "./rpc-transport";
import { type RpcTransportResponse } from "../../application/data/rpc-transport";

import { type LinkedAbortSignal, type BoardReadTransport, BoardReadBudget, BoardReadError, wrapTransportError, type ResolvedBoardIdentity, emptyError, parseContract, boardQuery, validateBoardList, type BoardTaskStatus, type WireBoardTask, type BoardReadModelOptions, type BoardTask, tasksPath, DEFAULT_TASK_OFFSET, MAX_TASK_PAGES, tasksQuery, appendTasksQuery, projectTask, type BoardReadModel, validateTaskPageSize, parseColumns, columnsPath, generationAbortError } from "../../application/data/board-read-model";

export function linkAbortSignals(signals: readonly (AbortSignal | undefined)[]): LinkedAbortSignal {
  const controller = new AbortController()
  const activeSignals = signals.filter((signal): signal is AbortSignal => signal !== undefined)
  const abort = () => controller.abort()
  for (const signal of activeSignals) {
    inheritReadScope(signal, controller.signal)
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

export async function get(
  transport: BoardReadTransport,
  request: RpcCall,
  signal: AbortSignal | undefined,
  budget: BoardReadBudget,
): Promise<unknown> {
  try {
    if (signal?.aborted) throw generationAbortError()
    const response: RpcTransportResponse = await transport.call({ ...request, signal })
    if (signal?.aborted) throw generationAbortError()
    if (
      response === null
      || typeof response !== "object"
      || !("payload" in response)
      || typeof response.bytes !== "number"
      || !Number.isSafeInteger(response.bytes)
      || response.bytes < 0
    ) {
      throw new BoardReadError("anomaly", "Web API transport 返回了无效 Protobuf 字节数。")
    }
    budget.consumeBytes(response.bytes)
    return response.payload
  } catch (error) {
    return wrapTransportError(error)
  }
}

export async function resolveIdentity(
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

export function parseTasksWindow(
  payload: unknown,
  identity: ResolvedBoardIdentity,
  status: BoardTaskStatus,
  expectedOffset: number,
  expectedLimit: number,
  expectedTotal: Integer | null,
): { tasks: readonly WireBoardTask[]; total: Integer; nextOffset: number } {
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
    const labelIds = new Set<string>()
    for (const label of task.labels) {
      if (label.id.trim().length === 0 || label.name.trim().length === 0) {
        throw new BoardReadError("anomaly", `任务 ${task.id} 返回了空白标签 id 或 name。`)
      }
      if (label.board_id !== identity.canonicalBoardId) {
        throw new BoardReadError("anomaly", `任务 ${task.id} 的标签 ${label.id} 不属于当前 canonical board。`)
      }
      if (labelIds.has(label.id)) {
        throw new BoardReadError("anomaly", `任务 ${task.id} 返回了重复标签 ${label.id}。`)
      }
      labelIds.add(label.id)
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

export async function loadTasksForStatus(
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
  let total: Integer | null = null
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
        ?? createRpcTransport(runtime, options.dependencies)
    } catch (error) {
      return wrapTransportError(error)
    }
    const identity = await resolveIdentity(transport, selector, includeArchived, linked.signal, budget)
    const columns = parseColumns(
      await get(transport, columnsPath(identity.slug), linked.signal, budget),
      identity,
    )
    const statuses = options.includeTasks === false ? [] : columns.map((column) => column.status)
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
