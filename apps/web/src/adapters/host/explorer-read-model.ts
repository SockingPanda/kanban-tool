import { mergeBoardEvents } from "../../application/data/explorer-read-model";
import type { WebRuntimeConfig } from "../../lib/runtime";



import { parseApiListBoardsResponse } from "../../lib/api/generated/contracts/api-list-boards-response";

import { parseApiListTasksResponse } from "../../lib/api/generated/contracts/api-list-tasks-response";

import { parseApiBoardTaskMapResponse } from "../../lib/api/generated/contracts/api-board-task-map-response";

import { parseApiGetTaskResponse, type ApiGetTaskResponseContract } from "../../lib/api/generated/contracts/api-get-task-response";

import { parseApiTaskNeighborhoodResponse, type ApiTaskNeighborhoodResponseContract } from "../../lib/api/generated/contracts/api-task-neighborhood-response";

import { parseApiListDependenciesResponse } from "../../lib/api/generated/contracts/api-list-dependencies-response";

import { parseApiListStepsResponse } from "../../lib/api/generated/contracts/api-list-steps-response";

import { parseApiListRunsResponse, type ApiListRunsResponseContract } from "../../lib/api/generated/contracts/api-list-runs-response";

import { parseApiGetRunLogResponse, type ApiGetRunLogResponseContract } from "../../lib/api/generated/contracts/api-get-run-log-response";

import { parseApiListCommentsResponse } from "../../lib/api/generated/contracts/api-list-comments-response";

import { parseApiListTaskLabelsResponse } from "../../lib/api/generated/contracts/api-list-task-labels-response";

import { parseApiListAttachmentsResponse, type ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response";

import { parseApiListEventsResponse, type ApiListEventsResponseContract } from "../../lib/api/generated/contracts/api-list-events-response";

import { createHttpTransport } from "./http-transport";
import { type HttpReadTransport, type HttpTransportResponse } from "../../application/data/http-transport";

import { type TaskListQueryState, ExplorerReadBudget, ExplorerReadError, wrapTransportError, type ExplorerReadOptions, type ExplorerBoardIdentity, parseContract, boardListPath, resolveBoard, type ExplorerEvent, BOARD_EVENTS_PAGE_LIMIT, type BoardEventsReadModel, validateCanonicalTaskSelector, throwIfAborted, MAX_BOARD_EVENTS_PAGES, buildBoardEventsRequest, validateEventBatch, type ExplorerTaskListPage, buildTaskListRequest, validateTaskBoard, type TaskMapQueryOptions, type ExplorerTaskMapReadModel, defaultTaskMapQuery, validateTaskMapOptions, validateProvidedBoardIdentity, buildTaskMapRequest, validateMapBoard, type TaskRunsReadModel, buildTaskRunsRequest, validateRunScope, buildRunLogRequest, type TaskInspectorReadOptions, type TaskInspectorReadModel, buildTaskInspectorRequests, validateInspectorTask, validateInspectorScope, validateNeighborhoodScope } from "../../application/data/explorer-read-model";

export async function getPayload(
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
      const nextAfter = response.meta.next_after
      validateEventBatch(response.data, board, taskId, after, nextAfter, BOARD_EVENTS_PAGE_LIMIT)
      events = mergeBoardEvents(events, response.data, board.id)
      if (response.data.length < BOARD_EVENTS_PAGE_LIMIT) {
        after = nextAfter
        break
      }
      if (nextAfter <= after) {
        throw new ExplorerReadError("anomaly", "事件响应的 next_after 必须在完整 page 后前进。")
      }
      after = nextAfter
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

export function linkedAbortSignal(signal: AbortSignal | undefined): { signal: AbortSignal; cleanup: () => void; abort: () => void } {
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

export async function loadInspectorSectionContext(
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
