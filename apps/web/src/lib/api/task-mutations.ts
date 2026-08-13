import type { WebRuntimeConfig } from "../runtime"
import type { CanonicalBoardSlug } from "../board-slug"
import { parseActorPreference } from "../preferences"
import {
  createHttpTransport,
  type HttpTransport,
  type HttpRequestMethod,
  type HttpTransportOptions,
} from "./http-transport"
import { parseApiAddDependencyPath } from "./generated/contracts/api-add-dependency-path"
import { parseApiAddDependencyHeaders } from "./generated/contracts/api-add-dependency-headers"
import { parseApiAddDependencyRequest } from "./generated/contracts/api-add-dependency-request"
import { parseApiAddDependencyResponse, type ApiAddDependencyResponseContract } from "./generated/contracts/api-add-dependency-response"
import { parseApiAddTaskLabelPath } from "./generated/contracts/api-add-task-label-path"
import { parseApiAddTaskLabelHeaders } from "./generated/contracts/api-add-task-label-headers"
import { parseApiAddTaskLabelRequest } from "./generated/contracts/api-add-task-label-request"
import { parseApiAddTaskLabelResponse, type ApiAddTaskLabelResponseContract } from "./generated/contracts/api-add-task-label-response"
import { parseApiLabelSuggestionQuery, type ApiLabelSuggestionQueryContract } from "./generated/contracts/api-label-suggestion-query"
import { parseApiArchiveTaskPath } from "./generated/contracts/api-archive-task-path"
import { parseApiArchiveTaskHeaders } from "./generated/contracts/api-archive-task-headers"
import { parseApiArchiveTaskRequest } from "./generated/contracts/api-archive-task-request"
import { parseApiArchiveTaskResponse, type ApiArchiveTaskResponseContract } from "./generated/contracts/api-archive-task-response"
import { parseApiBlockTaskPath } from "./generated/contracts/api-block-task-path"
import { parseApiBlockTaskHeaders } from "./generated/contracts/api-block-task-headers"
import { parseApiBlockTaskRequest } from "./generated/contracts/api-block-task-request"
import { parseApiBlockTaskResponse, type ApiBlockTaskResponseContract } from "./generated/contracts/api-block-task-response"
import { parseApiClaimTaskPath } from "./generated/contracts/api-claim-task-path"
import { parseApiClaimTaskHeaders } from "./generated/contracts/api-claim-task-headers"
import { parseApiClaimTaskRequest } from "./generated/contracts/api-claim-task-request"
import { parseApiClaimTaskResponse, type ApiClaimTaskResponseContract } from "./generated/contracts/api-claim-task-response"
import { parseApiCompleteTaskPath } from "./generated/contracts/api-complete-task-path"
import { parseApiCompleteTaskHeaders } from "./generated/contracts/api-complete-task-headers"
import { parseApiCompleteTaskRequest } from "./generated/contracts/api-complete-task-request"
import { parseApiCompleteTaskResponse, type ApiCompleteTaskResponseContract } from "./generated/contracts/api-complete-task-response"
import { parseApiCreateCommentPath } from "./generated/contracts/api-create-comment-path"
import { parseApiCreateCommentHeaders } from "./generated/contracts/api-create-comment-headers"
import { parseApiCreateCommentRequest } from "./generated/contracts/api-create-comment-request"
import { parseApiCreateCommentResponse, type ApiCreateCommentResponseContract } from "./generated/contracts/api-create-comment-response"
import { parseApiCreateAttachmentPath } from "./generated/contracts/api-create-attachment-path"
import { parseApiCreateAttachmentHeaders } from "./generated/contracts/api-create-attachment-headers"
import { parseApiCreateAttachmentRequest } from "./generated/contracts/api-create-attachment-request"
import { parseApiCreateAttachmentResponse, type ApiCreateAttachmentResponseContract } from "./generated/contracts/api-create-attachment-response"
import { parseApiCreateStepPath } from "./generated/contracts/api-create-step-path"
import { parseApiCreateStepHeaders } from "./generated/contracts/api-create-step-headers"
import { parseApiCreateStepRequest } from "./generated/contracts/api-create-step-request"
import { parseApiCreateStepResponse, type ApiCreateStepResponseContract } from "./generated/contracts/api-create-step-response"
import { parseApiCreateTaskPath } from "./generated/contracts/api-create-task-path"
import { parseApiCreateTaskHeaders } from "./generated/contracts/api-create-task-headers"
import { parseApiCreateTaskRequest } from "./generated/contracts/api-create-task-request"
import { parseApiCreateTaskResponse, type ApiCreateTaskResponseContract } from "./generated/contracts/api-create-task-response"
import { parseApiDeleteAttachmentPath } from "./generated/contracts/api-delete-attachment-path"
import { parseApiDeleteAttachmentHeaders } from "./generated/contracts/api-delete-attachment-headers"
import { parseApiDeleteAttachmentResponse, type ApiDeleteAttachmentResponseContract } from "./generated/contracts/api-delete-attachment-response"
import { parseApiHeartbeatTaskPath } from "./generated/contracts/api-heartbeat-task-path"
import { parseApiHeartbeatTaskHeaders } from "./generated/contracts/api-heartbeat-task-headers"
import { parseApiHeartbeatTaskRequest } from "./generated/contracts/api-heartbeat-task-request"
import { parseApiHeartbeatTaskResponse, type ApiHeartbeatTaskResponseContract } from "./generated/contracts/api-heartbeat-task-response"
import { parseApiListCommentsPath } from "./generated/contracts/api-list-comments-path"
import { parseApiListCommentsHeaders } from "./generated/contracts/api-list-comments-headers"
import { parseApiListCommentsResponse, type ApiListCommentsResponseContract } from "./generated/contracts/api-list-comments-response"
import { parseApiListDependenciesPath } from "./generated/contracts/api-list-dependencies-path"
import { parseApiListDependenciesHeaders } from "./generated/contracts/api-list-dependencies-headers"
import { parseApiListDependenciesResponse, type ApiListDependenciesResponseContract } from "./generated/contracts/api-list-dependencies-response"
import { parseApiListAttachmentsPath } from "./generated/contracts/api-list-attachments-path"
import { parseApiListAttachmentsHeaders } from "./generated/contracts/api-list-attachments-headers"
import { parseApiListAttachmentsResponse, type ApiListAttachmentsResponseContract } from "./generated/contracts/api-list-attachments-response"
import { parseApiListStepsPath } from "./generated/contracts/api-list-steps-path"
import { parseApiListStepsHeaders } from "./generated/contracts/api-list-steps-headers"
import { parseApiListStepsResponse, type ApiListStepsResponseContract } from "./generated/contracts/api-list-steps-response"
import { parseApiMarkExecutionPlanNotRequiredPath } from "./generated/contracts/api-mark-execution-plan-not-required-path"
import { parseApiMarkExecutionPlanNotRequiredHeaders } from "./generated/contracts/api-mark-execution-plan-not-required-headers"
import { parseApiMarkExecutionPlanNotRequiredRequest } from "./generated/contracts/api-mark-execution-plan-not-required-request"
import { parseApiMarkExecutionPlanNotRequiredResponse, type ApiMarkExecutionPlanNotRequiredResponseContract } from "./generated/contracts/api-mark-execution-plan-not-required-response"
import { parseApiPromoteTaskPath } from "./generated/contracts/api-promote-task-path"
import { parseApiPromoteTaskHeaders } from "./generated/contracts/api-promote-task-headers"
import { parseApiPromoteTaskRequest } from "./generated/contracts/api-promote-task-request"
import { parseApiPromoteTaskResponse, type ApiPromoteTaskResponseContract } from "./generated/contracts/api-promote-task-response"
import { parseApiReleaseTaskPath } from "./generated/contracts/api-release-task-path"
import { parseApiReleaseTaskHeaders } from "./generated/contracts/api-release-task-headers"
import { parseApiReleaseTaskRequest } from "./generated/contracts/api-release-task-request"
import { parseApiReleaseTaskResponse, type ApiReleaseTaskResponseContract } from "./generated/contracts/api-release-task-response"
import { parseApiRemoveDependencyPath } from "./generated/contracts/api-remove-dependency-path"
import { parseApiRemoveDependencyHeaders } from "./generated/contracts/api-remove-dependency-headers"
import { parseApiRemoveDependencyResponse, type ApiRemoveDependencyResponseContract } from "./generated/contracts/api-remove-dependency-response"
import { parseApiRemoveTaskLabelPath } from "./generated/contracts/api-remove-task-label-path"
import { parseApiRemoveTaskLabelHeaders } from "./generated/contracts/api-remove-task-label-headers"
import { parseApiRemoveTaskLabelResponse, type ApiRemoveTaskLabelResponseContract } from "./generated/contracts/api-remove-task-label-response"
import { parseApiSpecifyTaskPath } from "./generated/contracts/api-specify-task-path"
import { parseApiSpecifyTaskHeaders } from "./generated/contracts/api-specify-task-headers"
import { parseApiSpecifyTaskRequest } from "./generated/contracts/api-specify-task-request"
import { parseApiSpecifyTaskResponse, type ApiSpecifyTaskResponseContract } from "./generated/contracts/api-specify-task-response"
import { parseApiSubmitReviewTaskPath } from "./generated/contracts/api-submit-review-task-path"
import { parseApiSubmitReviewTaskHeaders } from "./generated/contracts/api-submit-review-task-headers"
import { parseApiSubmitReviewTaskRequest } from "./generated/contracts/api-submit-review-task-request"
import { parseApiSubmitReviewTaskResponse, type ApiSubmitReviewTaskResponseContract } from "./generated/contracts/api-submit-review-task-response"
import { parseApiSuggestTaskLabelsHeaders } from "./generated/contracts/api-suggest-task-labels-headers"
import { parseApiSuggestTaskLabelsPath } from "./generated/contracts/api-suggest-task-labels-path"
import { parseApiSuggestTaskLabelsResponse, type ApiSuggestTaskLabelsResponseContract } from "./generated/contracts/api-suggest-task-labels-response"
import { parseApiUnblockTaskPath } from "./generated/contracts/api-unblock-task-path"
import { parseApiUnblockTaskHeaders } from "./generated/contracts/api-unblock-task-headers"
import { parseApiUnblockTaskRequest } from "./generated/contracts/api-unblock-task-request"
import { parseApiUnblockTaskResponse, type ApiUnblockTaskResponseContract } from "./generated/contracts/api-unblock-task-response"
import { parseApiUpdateTaskPath } from "./generated/contracts/api-update-task-path"
import { parseApiUpdateTaskHeaders } from "./generated/contracts/api-update-task-headers"
import { parseApiUpdateTaskRequest } from "./generated/contracts/api-update-task-request"
import { parseApiUpdateTaskResponse, type ApiUpdateTaskResponseContract } from "./generated/contracts/api-update-task-response"

export type MutationRequestOptions = Readonly<{ signal?: AbortSignal }>

export type CreateTaskIntent = Pick<import("./generated/contracts/api-create-task-request").ApiCreateTaskRequestContract, "title">
  & Partial<Omit<import("./generated/contracts/api-create-task-request").ApiCreateTaskRequestContract, "actor" | "title">>
export type UpdateTaskIntent = Omit<import("./generated/contracts/api-update-task-request").ApiUpdateTaskRequestContract, "actor" | "expected_lock_version"> & {
  readonly expected_lock_version: number
}
export type AddTaskLabelIntent = Omit<import("./generated/contracts/api-add-task-label-request").ApiAddTaskLabelRequestContract, "actor">
export type SuggestTaskLabelsQuery = Partial<ApiLabelSuggestionQueryContract>
export type SpecifyTaskIntent = Omit<import("./generated/contracts/api-specify-task-request").ApiSpecifyTaskRequestContract, "actor">
export type PromoteTaskIntent = Omit<import("./generated/contracts/api-promote-task-request").ApiPromoteTaskRequestContract, "actor">
export type ClaimTaskIntent = Omit<import("./generated/contracts/api-claim-task-request").ApiClaimTaskRequestContract, "actor" | "ttl_ms">
  & Partial<Pick<import("./generated/contracts/api-claim-task-request").ApiClaimTaskRequestContract, "ttl_ms">>
export type ReleaseTaskIntent = Omit<import("./generated/contracts/api-release-task-request").ApiReleaseTaskRequestContract, "actor">
export type HeartbeatTaskIntent = Omit<import("./generated/contracts/api-heartbeat-task-request").ApiHeartbeatTaskRequestContract, "actor" | "ttl_ms">
  & Partial<Pick<import("./generated/contracts/api-heartbeat-task-request").ApiHeartbeatTaskRequestContract, "ttl_ms">>
export type CompleteTaskIntent = Omit<import("./generated/contracts/api-complete-task-request").ApiCompleteTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("./generated/contracts/api-complete-task-request").ApiCompleteTaskRequestContract, "force">>
export type SubmitReviewTaskIntent = Omit<import("./generated/contracts/api-submit-review-task-request").ApiSubmitReviewTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("./generated/contracts/api-submit-review-task-request").ApiSubmitReviewTaskRequestContract, "force">>
export type BlockTaskIntent = Omit<import("./generated/contracts/api-block-task-request").ApiBlockTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("./generated/contracts/api-block-task-request").ApiBlockTaskRequestContract, "force">>
export type UnblockTaskIntent = Omit<import("./generated/contracts/api-unblock-task-request").ApiUnblockTaskRequestContract, "actor">
export type ArchiveTaskIntent = Omit<import("./generated/contracts/api-archive-task-request").ApiArchiveTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("./generated/contracts/api-archive-task-request").ApiArchiveTaskRequestContract, "force">>
export type CreateStepIntent = Pick<import("./generated/contracts/api-create-step-request").ApiCreateStepRequestContract, "title">
  & Partial<Omit<import("./generated/contracts/api-create-step-request").ApiCreateStepRequestContract, "actor" | "title">>
export type MarkExecutionPlanNotRequiredIntent = Omit<import("./generated/contracts/api-mark-execution-plan-not-required-request").ApiMarkExecutionPlanNotRequiredRequestContract, "actor">
export type CreateCommentIntent = Omit<import("./generated/contracts/api-create-comment-request").ApiCreateCommentRequestContract, "author">
export type CreateAttachmentIntent = Pick<import("./generated/contracts/api-create-attachment-request").ApiCreateAttachmentRequestContract, "filename">
  & Partial<Omit<import("./generated/contracts/api-create-attachment-request").ApiCreateAttachmentRequestContract, "actor" | "filename">>

/** Transition operations currently rendered by the desktop product surface. */
export type TaskTransitionAction =
  | "specify"
  | "promote"
  | "claim"
  | "release"
  | "heartbeat"
  | "complete"
  | "submit-review"
  | "block"
  | "unblock"
  | "archive"

export type TaskTransitionIntent =
  | { readonly action: "specify"; readonly input: SpecifyTaskIntent }
  | { readonly action: "promote"; readonly input: PromoteTaskIntent }
  | { readonly action: "claim"; readonly input: ClaimTaskIntent }
  | { readonly action: "release"; readonly input: ReleaseTaskIntent }
  | { readonly action: "heartbeat"; readonly input: HeartbeatTaskIntent }
  | { readonly action: "complete"; readonly input: CompleteTaskIntent }
  | { readonly action: "submit-review"; readonly input: SubmitReviewTaskIntent }
  | { readonly action: "block"; readonly input: BlockTaskIntent }
  | { readonly action: "unblock"; readonly input: UnblockTaskIntent }
  | { readonly action: "archive"; readonly input: ArchiveTaskIntent }

export type TaskTransitionResponse =
  | ApiSpecifyTaskResponseContract
  | ApiPromoteTaskResponseContract
  | ApiClaimTaskResponseContract
  | ApiReleaseTaskResponseContract
  | ApiHeartbeatTaskResponseContract
  | ApiCompleteTaskResponseContract
  | ApiSubmitReviewTaskResponseContract
  | ApiBlockTaskResponseContract
  | ApiUnblockTaskResponseContract
  | ApiArchiveTaskResponseContract

export interface TaskMutationDependencies extends HttpTransportOptions {
  readonly transport?: Pick<HttpTransport, "request">
  /** Optional Web actor override; empty values must be normalized by callers. */
  readonly actor?: string
}

export interface TaskMutationClient {
  createTask(input: CreateTaskIntent, options?: MutationRequestOptions): Promise<ApiCreateTaskResponseContract>
  updateTask(taskId: string, input: UpdateTaskIntent, options?: MutationRequestOptions): Promise<ApiUpdateTaskResponseContract>
  addTaskLabel(taskId: string, input: AddTaskLabelIntent, options?: MutationRequestOptions): Promise<ApiAddTaskLabelResponseContract>
  removeTaskLabel(taskId: string, labelId: string, options?: MutationRequestOptions): Promise<ApiRemoveTaskLabelResponseContract>
  suggestTaskLabels(taskId: string, query?: SuggestTaskLabelsQuery, options?: MutationRequestOptions): Promise<ApiSuggestTaskLabelsResponseContract>
  transitionTask(taskId: string, action: "specify", input?: SpecifyTaskIntent, options?: MutationRequestOptions): Promise<ApiSpecifyTaskResponseContract>
  transitionTask(taskId: string, action: "promote", input?: PromoteTaskIntent, options?: MutationRequestOptions): Promise<ApiPromoteTaskResponseContract>
  transitionTask(taskId: string, action: "claim", input?: ClaimTaskIntent, options?: MutationRequestOptions): Promise<ApiClaimTaskResponseContract>
  transitionTask(taskId: string, action: "release", input: ReleaseTaskIntent, options?: MutationRequestOptions): Promise<ApiReleaseTaskResponseContract>
  transitionTask(taskId: string, action: "heartbeat", input: HeartbeatTaskIntent, options?: MutationRequestOptions): Promise<ApiHeartbeatTaskResponseContract>
  transitionTask(taskId: string, action: "complete", input?: CompleteTaskIntent, options?: MutationRequestOptions): Promise<ApiCompleteTaskResponseContract>
  transitionTask(taskId: string, action: "submit-review", input?: SubmitReviewTaskIntent, options?: MutationRequestOptions): Promise<ApiSubmitReviewTaskResponseContract>
  transitionTask(taskId: string, action: "block", input: BlockTaskIntent, options?: MutationRequestOptions): Promise<ApiBlockTaskResponseContract>
  transitionTask(taskId: string, action: "unblock", input?: UnblockTaskIntent, options?: MutationRequestOptions): Promise<ApiUnblockTaskResponseContract>
  transitionTask(taskId: string, action: "archive", input?: ArchiveTaskIntent, options?: MutationRequestOptions): Promise<ApiArchiveTaskResponseContract>
  listDependencies(taskId: string, options?: MutationRequestOptions): Promise<ApiListDependenciesResponseContract>
  addDependency(taskId: string, parentTaskId: string, options?: MutationRequestOptions): Promise<ApiAddDependencyResponseContract>
  removeDependency(taskId: string, parentTaskId: string, options?: MutationRequestOptions): Promise<ApiRemoveDependencyResponseContract>
  listSteps(taskId: string, options?: MutationRequestOptions): Promise<ApiListStepsResponseContract>
  createStep(taskId: string, input: CreateStepIntent, options?: MutationRequestOptions): Promise<ApiCreateStepResponseContract>
  markExecutionPlanNotRequired(taskId: string, input: MarkExecutionPlanNotRequiredIntent, options?: MutationRequestOptions): Promise<ApiMarkExecutionPlanNotRequiredResponseContract>
  listComments(taskId: string, options?: MutationRequestOptions): Promise<ApiListCommentsResponseContract>
  createComment(taskId: string, input: CreateCommentIntent, options?: MutationRequestOptions): Promise<ApiCreateCommentResponseContract>
  listAttachments(taskId: string, options?: MutationRequestOptions): Promise<ApiListAttachmentsResponseContract>
  createAttachment(taskId: string, input: CreateAttachmentIntent, options?: MutationRequestOptions): Promise<ApiCreateAttachmentResponseContract>
  deleteAttachment(taskId: string, attachmentId: string, options?: MutationRequestOptions): Promise<ApiDeleteAttachmentResponseContract>
}

function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

function taskPath(taskId: string): string {
  return encodedSegment(taskId)
}

function mergeActor<T extends object>(actor: string, input: T): T & { readonly actor: string } {
  return { ...input, actor }
}

type HeaderContractParser = (value: unknown) => object
type RequestHeaders = Readonly<Record<string, string | null>>

function parseHeaders(parser: HeaderContractParser, value: RequestHeaders): RequestHeaders {
  const parsed = parser(value)
  const headers: Record<string, string | null> = {}
  for (const [name, entry] of Object.entries(parsed)) {
    if (entry === undefined) continue
    if (entry !== null && typeof entry !== "string") {
      throw new TypeError(`generated header contract returned a non-string value for ${name}`)
    }
    headers[name] = entry
  }
  return headers
}

function jsonHeaders(parser: HeaderContractParser, actor: string): RequestHeaders {
  return parseHeaders(parser, { "Content-Type": "application/json", "X-KB-Actor": actor })
}

function actorHeaders(parser: HeaderContractParser, actor: string): RequestHeaders {
  return parseHeaders(parser, { "X-KB-Actor": actor })
}

function readHeaders(parser: HeaderContractParser): RequestHeaders {
  return parseHeaders(parser, {})
}

async function requestContract<T>(
  transport: Pick<HttpTransport, "request">,
  method: HttpRequestMethod,
  path: string,
  body: unknown,
  headers: RequestHeaders,
  parse: (value: unknown) => T,
  signal?: AbortSignal,
): Promise<T> {
  const response = await transport.request({ method, path, body, headers, signal })
  return parse(response.payload)
}

function createTaskPath(board: string): string {
  const parsed = parseApiCreateTaskPath({ board })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/tasks`
}

function updateTaskPath(taskId: string): string {
  const parsed = parseApiUpdateTaskPath({ task_id: taskId })
  return `/api/v1/tasks/${taskPath(parsed.task_id)}`
}

function suggestTaskLabelsPath(taskId: string): string {
  const parsed = parseApiSuggestTaskLabelsPath({ task_id: taskId })
  return `/api/v1/tasks/${taskPath(parsed.task_id)}/labels/suggestions`
}

function suggestTaskLabelsRequestPath(taskId: string, input: SuggestTaskLabelsQuery = {}): string {
  const path = suggestTaskLabelsPath(taskId)
  const query = parseApiLabelSuggestionQuery(input)
  const params = new URLSearchParams()
  if (query.limit !== undefined) params.set("limit", String(query.limit))
  if (query.candidate_limit !== undefined) params.set("candidate_limit", String(query.candidate_limit))
  if (query.atom_limit !== undefined) params.set("atom_limit", String(query.atom_limit))
  if (query.max_selected_labels !== undefined) params.set("max_selected_labels", String(query.max_selected_labels))
  if (query.min_score !== undefined) params.set("min_score", String(query.min_score))
  const encodedQuery = params.toString()
  return encodedQuery.length === 0 ? path : `${path}?${encodedQuery}`
}

function transitionPath(taskId: string, action: TaskTransitionAction): string {
  const pathByAction: Record<TaskTransitionAction, (value: unknown) => { task_id: string }> = {
    specify: (value) => parseApiSpecifyTaskPath(value),
    promote: (value) => parseApiPromoteTaskPath(value),
    claim: (value) => parseApiClaimTaskPath(value),
    release: (value) => parseApiReleaseTaskPath(value),
    heartbeat: (value) => parseApiHeartbeatTaskPath(value),
    complete: (value) => parseApiCompleteTaskPath(value),
    "submit-review": (value) => parseApiSubmitReviewTaskPath(value),
    block: (value) => parseApiBlockTaskPath(value),
    unblock: (value) => parseApiUnblockTaskPath(value),
    archive: (value) => parseApiArchiveTaskPath(value),
  }
  const parsed = pathByAction[action]({ task_id: taskId })
  return `/api/v1/tasks/${taskPath(parsed.task_id)}/transitions/${action}`
}

function createClient(
  runtime: WebRuntimeConfig,
  activeBoard: CanonicalBoardSlug,
  dependencies: TaskMutationDependencies,
): TaskMutationClient {
  const transport = dependencies.transport ?? createHttpTransport(runtime, dependencies)
  const actor = parseActorPreference(dependencies.actor) ?? runtime.actor

  const createTask = (input: CreateTaskIntent, options: MutationRequestOptions = {}) => {
    const body = parseApiCreateTaskRequest(mergeActor(actor, input))
    return requestContract(transport, "POST", createTaskPath(activeBoard), body, jsonHeaders(parseApiCreateTaskHeaders, actor), parseApiCreateTaskResponse, options.signal)
  }

  const updateTask = (taskId: string, input: UpdateTaskIntent, options: MutationRequestOptions = {}) => {
    const body = parseApiUpdateTaskRequest(mergeActor(actor, input))
    return requestContract(transport, "PATCH", updateTaskPath(taskId), body, jsonHeaders(parseApiUpdateTaskHeaders, actor), parseApiUpdateTaskResponse, options.signal)
  }

  const addTaskLabel = (taskId: string, input: AddTaskLabelIntent, options: MutationRequestOptions = {}) => {
    const path = parseApiAddTaskLabelPath({ task_id: taskId })
    const body = parseApiAddTaskLabelRequest(mergeActor(actor, input))
    return requestContract(transport, "POST", `/api/v1/tasks/${taskPath(path.task_id)}/labels`, body, jsonHeaders(parseApiAddTaskLabelHeaders, actor), parseApiAddTaskLabelResponse, options.signal)
  }

  const removeTaskLabel = (taskId: string, labelId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiRemoveTaskLabelPath({ task_id: taskId, label_id: labelId })
    return requestContract(transport, "DELETE", `/api/v1/tasks/${taskPath(path.task_id)}/labels/${encodedSegment(path.label_id)}`, undefined, actorHeaders(parseApiRemoveTaskLabelHeaders, actor), parseApiRemoveTaskLabelResponse, options.signal)
  }

  const suggestTaskLabels = (taskId: string, query: SuggestTaskLabelsQuery = {}, options: MutationRequestOptions = {}) => {
    return requestContract(
      transport,
      "GET",
      suggestTaskLabelsRequestPath(taskId, query),
      undefined,
      readHeaders(parseApiSuggestTaskLabelsHeaders),
      parseApiSuggestTaskLabelsResponse,
      options.signal,
    )
  }

  function transitionTask(taskId: string, action: "specify", input?: SpecifyTaskIntent, options?: MutationRequestOptions): Promise<ApiSpecifyTaskResponseContract>
  function transitionTask(taskId: string, action: "promote", input?: PromoteTaskIntent, options?: MutationRequestOptions): Promise<ApiPromoteTaskResponseContract>
  function transitionTask(taskId: string, action: "claim", input?: ClaimTaskIntent, options?: MutationRequestOptions): Promise<ApiClaimTaskResponseContract>
  function transitionTask(taskId: string, action: "release", input: ReleaseTaskIntent, options?: MutationRequestOptions): Promise<ApiReleaseTaskResponseContract>
  function transitionTask(taskId: string, action: "heartbeat", input: HeartbeatTaskIntent, options?: MutationRequestOptions): Promise<ApiHeartbeatTaskResponseContract>
  function transitionTask(taskId: string, action: "complete", input?: CompleteTaskIntent, options?: MutationRequestOptions): Promise<ApiCompleteTaskResponseContract>
  function transitionTask(taskId: string, action: "submit-review", input?: SubmitReviewTaskIntent, options?: MutationRequestOptions): Promise<ApiSubmitReviewTaskResponseContract>
  function transitionTask(taskId: string, action: "block", input: BlockTaskIntent, options?: MutationRequestOptions): Promise<ApiBlockTaskResponseContract>
  function transitionTask(taskId: string, action: "unblock", input?: UnblockTaskIntent, options?: MutationRequestOptions): Promise<ApiUnblockTaskResponseContract>
  function transitionTask(taskId: string, action: "archive", input?: ArchiveTaskIntent, options?: MutationRequestOptions): Promise<ApiArchiveTaskResponseContract>
  function transitionTask(taskId: string, action: TaskTransitionAction, input: Readonly<Record<string, unknown>> = {}, options: MutationRequestOptions = {}): Promise<TaskTransitionResponse> {
    const body = mergeActor(actor, input)
    const path = transitionPath(taskId, action)
    switch (action) {
      case "specify": {
        const parsed = parseApiSpecifyTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiSpecifyTaskHeaders, actor), parseApiSpecifyTaskResponse, options.signal)
      }
      case "promote": {
        const parsed = parseApiPromoteTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiPromoteTaskHeaders, actor), parseApiPromoteTaskResponse, options.signal)
      }
      case "claim": {
        const parsed = parseApiClaimTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiClaimTaskHeaders, actor), parseApiClaimTaskResponse, options.signal)
      }
      case "release": {
        const parsed = parseApiReleaseTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiReleaseTaskHeaders, actor), parseApiReleaseTaskResponse, options.signal)
      }
      case "heartbeat": {
        const parsed = parseApiHeartbeatTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiHeartbeatTaskHeaders, actor), parseApiHeartbeatTaskResponse, options.signal)
      }
      case "complete": {
        const parsed = parseApiCompleteTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiCompleteTaskHeaders, actor), parseApiCompleteTaskResponse, options.signal)
      }
      case "submit-review": {
        const parsed = parseApiSubmitReviewTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiSubmitReviewTaskHeaders, actor), parseApiSubmitReviewTaskResponse, options.signal)
      }
      case "block": {
        const parsed = parseApiBlockTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiBlockTaskHeaders, actor), parseApiBlockTaskResponse, options.signal)
      }
      case "unblock": {
        const parsed = parseApiUnblockTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiUnblockTaskHeaders, actor), parseApiUnblockTaskResponse, options.signal)
      }
      case "archive": {
        const parsed = parseApiArchiveTaskRequest(body)
        return requestContract(transport, "POST", path, parsed, jsonHeaders(parseApiArchiveTaskHeaders, actor), parseApiArchiveTaskResponse, options.signal)
      }
    }
  }

  const listDependencies = (taskId: string, options: MutationRequestOptions = {}) => {
    const parsed = parseApiListDependenciesPath({ task_id: taskId })
    return requestContract(transport, "GET", `/api/v1/tasks/${taskPath(parsed.task_id)}/dependencies`, undefined, readHeaders(parseApiListDependenciesHeaders), parseApiListDependenciesResponse, options.signal)
  }

  const addDependency = (taskId: string, parentTaskId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiAddDependencyPath({ task_id: taskId })
    const body = parseApiAddDependencyRequest({ actor, parent_task_id: parentTaskId })
    return requestContract(transport, "POST", `/api/v1/tasks/${taskPath(path.task_id)}/dependencies`, body, jsonHeaders(parseApiAddDependencyHeaders, actor), parseApiAddDependencyResponse, options.signal)
  }

  const removeDependency = (taskId: string, parentTaskId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiRemoveDependencyPath({ child_task_id: taskId, parent_task_id: parentTaskId })
    return requestContract(transport, "DELETE", `/api/v1/tasks/${taskPath(path.child_task_id)}/dependencies/${taskPath(path.parent_task_id)}`, undefined, actorHeaders(parseApiRemoveDependencyHeaders, actor), parseApiRemoveDependencyResponse, options.signal)
  }

  const listSteps = (taskId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiListStepsPath({ task_id: taskId })
    return requestContract(transport, "GET", `/api/v1/tasks/${taskPath(path.task_id)}/steps`, undefined, readHeaders(parseApiListStepsHeaders), parseApiListStepsResponse, options.signal)
  }

  const createStep = (taskId: string, input: CreateStepIntent, options: MutationRequestOptions = {}) => {
    const path = parseApiCreateStepPath({ task_id: taskId })
    const body = parseApiCreateStepRequest(mergeActor(actor, input))
    return requestContract(transport, "POST", `/api/v1/tasks/${taskPath(path.task_id)}/steps`, body, jsonHeaders(parseApiCreateStepHeaders, actor), parseApiCreateStepResponse, options.signal)
  }

  const markExecutionPlanNotRequired = (taskId: string, input: MarkExecutionPlanNotRequiredIntent, options: MutationRequestOptions = {}) => {
    const path = parseApiMarkExecutionPlanNotRequiredPath({ task_id: taskId })
    const body = parseApiMarkExecutionPlanNotRequiredRequest(mergeActor(actor, input))
    return requestContract(transport, "POST", `/api/v1/tasks/${taskPath(path.task_id)}/execution-plan/not-required`, body, jsonHeaders(parseApiMarkExecutionPlanNotRequiredHeaders, actor), parseApiMarkExecutionPlanNotRequiredResponse, options.signal)
  }

  const listComments = (taskId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiListCommentsPath({ task_id: taskId })
    return requestContract(transport, "GET", `/api/v1/tasks/${taskPath(path.task_id)}/comments`, undefined, readHeaders(parseApiListCommentsHeaders), parseApiListCommentsResponse, options.signal)
  }

  const createComment = (taskId: string, input: CreateCommentIntent, options: MutationRequestOptions = {}) => {
    const path = parseApiCreateCommentPath({ task_id: taskId })
    const body = parseApiCreateCommentRequest({ ...input, author: actor })
    return requestContract(transport, "POST", `/api/v1/tasks/${taskPath(path.task_id)}/comments`, body, jsonHeaders(parseApiCreateCommentHeaders, actor), parseApiCreateCommentResponse, options.signal)
  }

  const listAttachments = (taskId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiListAttachmentsPath({ task_id: taskId })
    return requestContract(transport, "GET", `/api/v1/tasks/${taskPath(path.task_id)}/attachments`, undefined, readHeaders(parseApiListAttachmentsHeaders), parseApiListAttachmentsResponse, options.signal)
  }

  const createAttachment = (taskId: string, input: CreateAttachmentIntent, options: MutationRequestOptions = {}) => {
    const path = parseApiCreateAttachmentPath({ task_id: taskId })
    const body = parseApiCreateAttachmentRequest(mergeActor(actor, input))
    return requestContract(transport, "POST", `/api/v1/tasks/${taskPath(path.task_id)}/attachments`, body, jsonHeaders(parseApiCreateAttachmentHeaders, actor), parseApiCreateAttachmentResponse, options.signal)
  }

  const deleteAttachment = (taskId: string, attachmentId: string, options: MutationRequestOptions = {}) => {
    const path = parseApiDeleteAttachmentPath({ task_id: taskId, attachment_id: attachmentId })
    return requestContract(transport, "DELETE", `/api/v1/tasks/${taskPath(path.task_id)}/attachments/${encodedSegment(path.attachment_id)}`, undefined, actorHeaders(parseApiDeleteAttachmentHeaders, actor), parseApiDeleteAttachmentResponse, options.signal)
  }

  return {
    createTask,
    updateTask,
    addTaskLabel,
    removeTaskLabel,
    suggestTaskLabels,
    transitionTask,
    listDependencies,
    addDependency,
    removeDependency,
    listSteps,
    createStep,
    markExecutionPlanNotRequired,
    listComments,
    createComment,
    listAttachments,
    createAttachment,
    deleteAttachment,
  }
}

export function createTaskMutationClient(
  runtime: WebRuntimeConfig,
  activeBoard: CanonicalBoardSlug,
  dependencies: TaskMutationDependencies = {},
): TaskMutationClient {
  return createClient(runtime, activeBoard, dependencies)
}
