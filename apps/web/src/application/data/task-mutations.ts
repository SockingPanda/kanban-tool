import { type HttpTransport, type HttpTransportOptions } from "./http-transport";

import { type ApiAddDependencyResponseContract } from "../../lib/api/generated/contracts/api-add-dependency-response";

import { type ApiAddTaskLabelResponseContract } from "../../lib/api/generated/contracts/api-add-task-label-response";

import { parseApiLabelSuggestionQuery, type ApiLabelSuggestionQueryContract } from "../../lib/api/generated/contracts/api-label-suggestion-query";

import { parseApiArchiveTaskPath } from "../../lib/api/generated/contracts/api-archive-task-path";

import { type ApiArchiveTaskResponseContract } from "../../lib/api/generated/contracts/api-archive-task-response";

import { parseApiBlockTaskPath } from "../../lib/api/generated/contracts/api-block-task-path";

import { type ApiBlockTaskResponseContract } from "../../lib/api/generated/contracts/api-block-task-response";

import { parseApiClaimTaskPath } from "../../lib/api/generated/contracts/api-claim-task-path";

import { type ApiClaimTaskResponseContract } from "../../lib/api/generated/contracts/api-claim-task-response";

import { parseApiCompleteTaskPath } from "../../lib/api/generated/contracts/api-complete-task-path";

import { type ApiCompleteTaskResponseContract } from "../../lib/api/generated/contracts/api-complete-task-response";

import { type ApiCreateCommentResponseContract } from "../../lib/api/generated/contracts/api-create-comment-response";

import { type ApiCreateAttachmentResponseContract } from "../../lib/api/generated/contracts/api-create-attachment-response";

import { type ApiCreateStepResponseContract } from "../../lib/api/generated/contracts/api-create-step-response";

import { parseApiCreateTaskPath } from "../../lib/api/generated/contracts/api-create-task-path";

import { type ApiCreateTaskResponseContract } from "../../lib/api/generated/contracts/api-create-task-response";

import { type ApiDeleteAttachmentResponseContract } from "../../lib/api/generated/contracts/api-delete-attachment-response";

import { parseApiHeartbeatTaskPath } from "../../lib/api/generated/contracts/api-heartbeat-task-path";

import { type ApiHeartbeatTaskResponseContract } from "../../lib/api/generated/contracts/api-heartbeat-task-response";

import { type ApiListCommentsResponseContract } from "../../lib/api/generated/contracts/api-list-comments-response";

import { type ApiListDependenciesResponseContract } from "../../lib/api/generated/contracts/api-list-dependencies-response";

import { type ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response";

import { type ApiListStepsResponseContract } from "../../lib/api/generated/contracts/api-list-steps-response";

import { type ApiMarkExecutionPlanNotRequiredResponseContract } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-response";

import { parseApiPromoteTaskPath } from "../../lib/api/generated/contracts/api-promote-task-path";

import { type ApiPromoteTaskResponseContract } from "../../lib/api/generated/contracts/api-promote-task-response";

import { type ApiRemoveDependencyResponseContract } from "../../lib/api/generated/contracts/api-remove-dependency-response";

import { type ApiRemoveTaskLabelResponseContract } from "../../lib/api/generated/contracts/api-remove-task-label-response";

import { parseApiSpecifyTaskPath } from "../../lib/api/generated/contracts/api-specify-task-path";

import { type ApiSpecifyTaskResponseContract } from "../../lib/api/generated/contracts/api-specify-task-response";

import { parseApiSubmitReviewTaskPath } from "../../lib/api/generated/contracts/api-submit-review-task-path";

import { type ApiSubmitReviewTaskResponseContract } from "../../lib/api/generated/contracts/api-submit-review-task-response";

import { parseApiSuggestTaskLabelsPath } from "../../lib/api/generated/contracts/api-suggest-task-labels-path";

import { type ApiSuggestTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-suggest-task-labels-response";

import { parseApiUnblockTaskPath } from "../../lib/api/generated/contracts/api-unblock-task-path";

import { type ApiUnblockTaskResponseContract } from "../../lib/api/generated/contracts/api-unblock-task-response";

import { parseApiUpdateTaskPath } from "../../lib/api/generated/contracts/api-update-task-path";

import { type ApiUpdateTaskResponseContract } from "../../lib/api/generated/contracts/api-update-task-response";

export type MutationRequestOptions = Readonly<{ signal?: AbortSignal }>

export type CreateTaskIntent = Pick<import("../../lib/api/generated/contracts/api-create-task-request").ApiCreateTaskRequestContract, "title">
  & Partial<Omit<import("../../lib/api/generated/contracts/api-create-task-request").ApiCreateTaskRequestContract, "actor" | "title">>

export type UpdateTaskIntent = Omit<import("../../lib/api/generated/contracts/api-update-task-request").ApiUpdateTaskRequestContract, "actor" | "expected_lock_version"> & {
  readonly expected_lock_version: number
}

export type AddTaskLabelIntent = Omit<import("../../lib/api/generated/contracts/api-add-task-label-request").ApiAddTaskLabelRequestContract, "actor">

export type SuggestTaskLabelsQuery = Partial<ApiLabelSuggestionQueryContract>

export type SpecifyTaskIntent = Omit<import("../../lib/api/generated/contracts/api-specify-task-request").ApiSpecifyTaskRequestContract, "actor">

export type PromoteTaskIntent = Omit<import("../../lib/api/generated/contracts/api-promote-task-request").ApiPromoteTaskRequestContract, "actor">

export type ClaimTaskIntent = Omit<import("../../lib/api/generated/contracts/api-claim-task-request").ApiClaimTaskRequestContract, "actor" | "ttl_ms">
  & Partial<Pick<import("../../lib/api/generated/contracts/api-claim-task-request").ApiClaimTaskRequestContract, "ttl_ms">>

export type HeartbeatTaskIntent = Omit<import("../../lib/api/generated/contracts/api-heartbeat-task-request").ApiHeartbeatTaskRequestContract, "actor" | "ttl_ms">
  & Partial<Pick<import("../../lib/api/generated/contracts/api-heartbeat-task-request").ApiHeartbeatTaskRequestContract, "ttl_ms">>

export type CompleteTaskIntent = Omit<import("../../lib/api/generated/contracts/api-complete-task-request").ApiCompleteTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("../../lib/api/generated/contracts/api-complete-task-request").ApiCompleteTaskRequestContract, "force">>

export type SubmitReviewTaskIntent = Omit<import("../../lib/api/generated/contracts/api-submit-review-task-request").ApiSubmitReviewTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("../../lib/api/generated/contracts/api-submit-review-task-request").ApiSubmitReviewTaskRequestContract, "force">>

export type BlockTaskIntent = Omit<import("../../lib/api/generated/contracts/api-block-task-request").ApiBlockTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("../../lib/api/generated/contracts/api-block-task-request").ApiBlockTaskRequestContract, "force">>

export type UnblockTaskIntent = Omit<import("../../lib/api/generated/contracts/api-unblock-task-request").ApiUnblockTaskRequestContract, "actor">

export type ArchiveTaskIntent = Omit<import("../../lib/api/generated/contracts/api-archive-task-request").ApiArchiveTaskRequestContract, "actor" | "force">
  & Partial<Pick<import("../../lib/api/generated/contracts/api-archive-task-request").ApiArchiveTaskRequestContract, "force">>

export type CreateStepIntent = Pick<import("../../lib/api/generated/contracts/api-create-step-request").ApiCreateStepRequestContract, "title">
  & Partial<Omit<import("../../lib/api/generated/contracts/api-create-step-request").ApiCreateStepRequestContract, "actor" | "title">>

export type MarkExecutionPlanNotRequiredIntent = Omit<import("../../lib/api/generated/contracts/api-mark-execution-plan-not-required-request").ApiMarkExecutionPlanNotRequiredRequestContract, "actor">

export type CreateCommentIntent = Omit<import("../../lib/api/generated/contracts/api-create-comment-request").ApiCreateCommentRequestContract, "author">

export type CreateAttachmentIntent = Pick<import("../../lib/api/generated/contracts/api-create-attachment-request").ApiCreateAttachmentRequestContract, "filename">
  & Partial<Omit<import("../../lib/api/generated/contracts/api-create-attachment-request").ApiCreateAttachmentRequestContract, "actor" | "filename">>

export type TaskTransitionAction =
  | "specify"
  | "promote"
  | "claim"
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
  mutateStep(taskId: string, stepId: string, command: StepMutationIntent, options?: MutationRequestOptions): Promise<StepMutationResponse>
  createTask(input: CreateTaskIntent, options?: MutationRequestOptions): Promise<ApiCreateTaskResponseContract>
  updateTask(taskId: string, input: UpdateTaskIntent, options?: MutationRequestOptions): Promise<ApiUpdateTaskResponseContract>
  addTaskLabel(taskId: string, input: AddTaskLabelIntent, options?: MutationRequestOptions): Promise<ApiAddTaskLabelResponseContract>
  removeTaskLabel(taskId: string, labelId: string, options?: MutationRequestOptions): Promise<ApiRemoveTaskLabelResponseContract>
  suggestTaskLabels(taskId: string, query?: SuggestTaskLabelsQuery, options?: MutationRequestOptions): Promise<ApiSuggestTaskLabelsResponseContract>
  transitionTask(taskId: string, action: "specify", input?: SpecifyTaskIntent, options?: MutationRequestOptions): Promise<ApiSpecifyTaskResponseContract>
  transitionTask(taskId: string, action: "promote", input?: PromoteTaskIntent, options?: MutationRequestOptions): Promise<ApiPromoteTaskResponseContract>
  transitionTask(taskId: string, action: "claim", input?: ClaimTaskIntent, options?: MutationRequestOptions): Promise<ApiClaimTaskResponseContract>
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

export function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

export function taskPath(taskId: string): string {
  return encodedSegment(taskId)
}

export function mergeActor<T extends object>(actor: string, input: T): T & { readonly actor: string } {
  return { ...input, actor }
}

export type HeaderContractParser = (value: unknown) => object

export type RequestHeaders = Readonly<Record<string, string | null>>

export function parseHeaders(parser: HeaderContractParser, value: RequestHeaders): RequestHeaders {
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

export function jsonHeaders(parser: HeaderContractParser, actor: string): RequestHeaders {
  return parseHeaders(parser, { "Content-Type": "application/json", "X-KB-Actor": actor })
}

export function actorHeaders(parser: HeaderContractParser, actor: string): RequestHeaders {
  return parseHeaders(parser, { "X-KB-Actor": actor })
}

export function readHeaders(parser: HeaderContractParser): RequestHeaders {
  return parseHeaders(parser, {})
}

export function createTaskPath(board: string): string {
  const parsed = parseApiCreateTaskPath({ board })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/tasks`
}

export function updateTaskPath(taskId: string): string {
  const parsed = parseApiUpdateTaskPath({ task_id: taskId })
  return `/api/v1/tasks/${taskPath(parsed.task_id)}`
}

export function suggestTaskLabelsPath(taskId: string): string {
  const parsed = parseApiSuggestTaskLabelsPath({ task_id: taskId })
  return `/api/v1/tasks/${taskPath(parsed.task_id)}/labels/suggestions`
}

export function suggestTaskLabelsRequestPath(taskId: string, input: SuggestTaskLabelsQuery = {}): string {
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

export function transitionPath(taskId: string, action: TaskTransitionAction): string {
  const pathByAction: Record<TaskTransitionAction, (value: unknown) => { task_id: string }> = {
    specify: (value) => parseApiSpecifyTaskPath(value),
    promote: (value) => parseApiPromoteTaskPath(value),
    claim: (value) => parseApiClaimTaskPath(value),
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

export type StepMutationIntent =
  | { readonly action: 'update'; readonly input: Omit<import('../../lib/api/generated/contracts/api-update-step-request').ApiUpdateStepRequestContract, 'actor'> }
  | { readonly action: 'remove' }
  | { readonly action: 'complete'; readonly input: Omit<import('../../lib/api/generated/contracts/api-complete-step-request').ApiCompleteStepRequestContract, 'actor'> }
  | { readonly action: 'skip'; readonly input: Omit<import('../../lib/api/generated/contracts/api-skip-step-request').ApiSkipStepRequestContract, 'actor'> }
  | { readonly action: 'reopen'; readonly input: Omit<import('../../lib/api/generated/contracts/api-reopen-step-request').ApiReopenStepRequestContract, 'actor'> };
export type StepMutationResponse =
  | import('../../lib/api/generated/contracts/api-update-step-response').ApiUpdateStepResponseContract
  | import('../../lib/api/generated/contracts/api-remove-step-response').ApiRemoveStepResponseContract
  | import('../../lib/api/generated/contracts/api-complete-step-response').ApiCompleteStepResponseContract
  | import('../../lib/api/generated/contracts/api-skip-step-response').ApiSkipStepResponseContract
  | import('../../lib/api/generated/contracts/api-reopen-step-response').ApiReopenStepResponseContract;
