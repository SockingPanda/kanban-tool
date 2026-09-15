import type { StepMutationIntent } from '../../application/data/task-mutations';
import { parseApiCompleteStepPath } from '../../lib/api/generated/contracts/api-complete-step-path';
import { parseApiCompleteStepRequest } from '../../lib/api/generated/contracts/api-complete-step-request';
import { parseApiCompleteStepResponse } from '../../lib/api/generated/contracts/api-complete-step-response';
import { parseApiRemoveStepPath } from '../../lib/api/generated/contracts/api-remove-step-path';
import { parseApiRemoveStepResponse } from '../../lib/api/generated/contracts/api-remove-step-response';
import { parseApiReopenStepPath } from '../../lib/api/generated/contracts/api-reopen-step-path';
import { parseApiReopenStepRequest } from '../../lib/api/generated/contracts/api-reopen-step-request';
import { parseApiReopenStepResponse } from '../../lib/api/generated/contracts/api-reopen-step-response';
import { parseApiSkipStepPath } from '../../lib/api/generated/contracts/api-skip-step-path';
import { parseApiSkipStepRequest } from '../../lib/api/generated/contracts/api-skip-step-request';
import { parseApiSkipStepResponse } from '../../lib/api/generated/contracts/api-skip-step-response';
import { parseApiUpdateStepPath } from '../../lib/api/generated/contracts/api-update-step-path';
import { parseApiUpdateStepRequest } from '../../lib/api/generated/contracts/api-update-step-request';
import { parseApiUpdateStepResponse } from '../../lib/api/generated/contracts/api-update-step-response';
import type { WebRuntimeConfig } from "../../lib/runtime";

import type { CanonicalBoardSlug } from "../../domain/board-slug";

import { parseActorPreference } from "../../platform/preferences/preferences";

import type { RpcCall,RpcMethod,RpcTransport } from '../../application/data/rpc-transport';
import { parseApiCreateTaskPath } from '../../lib/api/generated/contracts/api-create-task-path';
import { parseApiLabelSuggestionQuery } from '../../lib/api/generated/contracts/api-label-suggestion-query';
import { parseApiSuggestTaskLabelsPath } from '../../lib/api/generated/contracts/api-suggest-task-labels-path';
import { parseApiUpdateTaskPath } from '../../lib/api/generated/contracts/api-update-task-path';
import { createRpcTransport } from './rpc-transport';

import { parseApiAddDependencyPath } from "../../lib/api/generated/contracts/api-add-dependency-path";


import { parseApiAddDependencyRequest } from "../../lib/api/generated/contracts/api-add-dependency-request";

import { parseApiAddDependencyResponse } from "../../lib/api/generated/contracts/api-add-dependency-response";

import { parseApiAddTaskLabelPath } from "../../lib/api/generated/contracts/api-add-task-label-path";


import { parseApiAddTaskLabelRequest } from "../../lib/api/generated/contracts/api-add-task-label-request";

import { parseApiAddTaskLabelResponse } from "../../lib/api/generated/contracts/api-add-task-label-response";


import { parseApiArchiveTaskRequest } from "../../lib/api/generated/contracts/api-archive-task-request";

import { parseApiArchiveTaskResponse,type ApiArchiveTaskResponseContract } from "../../lib/api/generated/contracts/api-archive-task-response";


import { parseApiBlockTaskRequest } from "../../lib/api/generated/contracts/api-block-task-request";

import { parseApiBlockTaskResponse,type ApiBlockTaskResponseContract } from "../../lib/api/generated/contracts/api-block-task-response";


import { parseApiClaimTaskRequest } from "../../lib/api/generated/contracts/api-claim-task-request";

import { parseApiClaimTaskResponse,type ApiClaimTaskResponseContract } from "../../lib/api/generated/contracts/api-claim-task-response";


import { parseApiCompleteTaskRequest } from "../../lib/api/generated/contracts/api-complete-task-request";

import { parseApiCompleteTaskResponse,type ApiCompleteTaskResponseContract } from "../../lib/api/generated/contracts/api-complete-task-response";

import { parseApiCreateCommentPath } from "../../lib/api/generated/contracts/api-create-comment-path";


import { parseApiCreateCommentRequest } from "../../lib/api/generated/contracts/api-create-comment-request";

import { parseApiCreateCommentResponse } from "../../lib/api/generated/contracts/api-create-comment-response";

import { parseApiCreateAttachmentPath } from "../../lib/api/generated/contracts/api-create-attachment-path";


import { parseApiCreateAttachmentRequest } from "../../lib/api/generated/contracts/api-create-attachment-request";

import { parseApiCreateAttachmentResponse } from "../../lib/api/generated/contracts/api-create-attachment-response";

import { parseApiCreateStepPath } from "../../lib/api/generated/contracts/api-create-step-path";


import { parseApiCreateStepRequest } from "../../lib/api/generated/contracts/api-create-step-request";

import { parseApiCreateStepResponse } from "../../lib/api/generated/contracts/api-create-step-response";


import { parseApiCreateTaskRequest } from "../../lib/api/generated/contracts/api-create-task-request";

import { parseApiCreateTaskResponse } from "../../lib/api/generated/contracts/api-create-task-response";

import { parseApiDeleteAttachmentPath } from "../../lib/api/generated/contracts/api-delete-attachment-path";


import { parseApiDeleteAttachmentResponse } from "../../lib/api/generated/contracts/api-delete-attachment-response";


import { parseApiHeartbeatTaskRequest } from "../../lib/api/generated/contracts/api-heartbeat-task-request";

import { parseApiHeartbeatTaskResponse,type ApiHeartbeatTaskResponseContract } from "../../lib/api/generated/contracts/api-heartbeat-task-response";

import { parseApiListCommentsPath } from "../../lib/api/generated/contracts/api-list-comments-path";


import { parseApiListCommentsResponse } from "../../lib/api/generated/contracts/api-list-comments-response";

import { parseApiListDependenciesPath } from "../../lib/api/generated/contracts/api-list-dependencies-path";


import { parseApiListDependenciesResponse } from "../../lib/api/generated/contracts/api-list-dependencies-response";

import { parseApiListAttachmentsPath } from "../../lib/api/generated/contracts/api-list-attachments-path";


import { parseApiListAttachmentsResponse } from "../../lib/api/generated/contracts/api-list-attachments-response";

import { parseApiListStepsPath } from "../../lib/api/generated/contracts/api-list-steps-path";


import { parseApiListStepsResponse } from "../../lib/api/generated/contracts/api-list-steps-response";

import { parseApiMarkExecutionPlanNotRequiredPath } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-path";


import { parseApiMarkExecutionPlanNotRequiredRequest } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-request";

import { parseApiMarkExecutionPlanNotRequiredResponse } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-response";


import { parseApiPromoteTaskRequest } from "../../lib/api/generated/contracts/api-promote-task-request";

import { parseApiPromoteTaskResponse,type ApiPromoteTaskResponseContract } from "../../lib/api/generated/contracts/api-promote-task-response";

import { parseApiRemoveDependencyPath } from "../../lib/api/generated/contracts/api-remove-dependency-path";


import { parseApiRemoveDependencyResponse } from "../../lib/api/generated/contracts/api-remove-dependency-response";

import { parseApiRemoveTaskLabelPath } from "../../lib/api/generated/contracts/api-remove-task-label-path";


import { parseApiRemoveTaskLabelResponse } from "../../lib/api/generated/contracts/api-remove-task-label-response";


import { parseApiSpecifyTaskRequest } from "../../lib/api/generated/contracts/api-specify-task-request";

import { parseApiSpecifyTaskResponse,type ApiSpecifyTaskResponseContract } from "../../lib/api/generated/contracts/api-specify-task-response";


import { parseApiSubmitReviewTaskRequest } from "../../lib/api/generated/contracts/api-submit-review-task-request";

import { parseApiSubmitReviewTaskResponse,type ApiSubmitReviewTaskResponseContract } from "../../lib/api/generated/contracts/api-submit-review-task-response";


import { parseApiSuggestTaskLabelsResponse } from "../../lib/api/generated/contracts/api-suggest-task-labels-response";


import { parseApiUnblockTaskRequest } from "../../lib/api/generated/contracts/api-unblock-task-request";

import { parseApiUnblockTaskResponse,type ApiUnblockTaskResponseContract } from "../../lib/api/generated/contracts/api-unblock-task-response";


import { parseApiUpdateTaskRequest } from "../../lib/api/generated/contracts/api-update-task-request";

import { parseApiUpdateTaskResponse } from "../../lib/api/generated/contracts/api-update-task-response";

import { mergeActor,type AddTaskLabelIntent,type ArchiveTaskIntent,type BlockTaskIntent,type ClaimTaskIntent,type CompleteTaskIntent,type CreateAttachmentIntent,type CreateCommentIntent,type CreateStepIntent,type CreateTaskIntent,type HeartbeatTaskIntent,type MarkExecutionPlanNotRequiredIntent,type MutationRequestOptions,type PromoteTaskIntent,type SpecifyTaskIntent,type SubmitReviewTaskIntent,type SuggestTaskLabelsQuery,type TaskMutationClient,type TaskMutationDependencies,type TaskTransitionAction,type TaskTransitionResponse,type UnblockTaskIntent,type UpdateTaskIntent } from "../../application/data/task-mutations";

export async function requestContract<T>(
  transport: RpcTransport,
  method: RpcMethod,
  parts: Pick<RpcCall, 'path' | 'query' | 'input'>,
  parse: (value: unknown) => T,
  actor?: string,
  signal?: AbortSignal,
): Promise<T> {
  const response = await transport.call({ method, ...parts, ...(actor === undefined ? {} : { actor }), ...(signal === undefined ? {} : { signal }) })
  return parse(response.payload)
}

export function createClient(
  runtime: WebRuntimeConfig,
  activeBoard: CanonicalBoardSlug,
  dependencies: TaskMutationDependencies,
): TaskMutationClient {
  const transport = dependencies.transport ?? createRpcTransport(runtime, dependencies)
  const actor = parseActorPreference(dependencies.actor) ?? runtime.actor
  const task = (taskId: string) => parseApiUpdateTaskPath({ task_id: taskId })
  const options = (signal?: AbortSignal) => [actor, signal] as const

  const createTask = (input: CreateTaskIntent, request: MutationRequestOptions = {}) => requestContract(
    transport, 'CreateTask', { path: parseApiCreateTaskPath({ board: activeBoard }), input: parseApiCreateTaskRequest(mergeActor(actor, input)) }, parseApiCreateTaskResponse, ...options(request.signal),
  )
  const updateTask = (taskId: string, input: UpdateTaskIntent, request: MutationRequestOptions = {}) => requestContract(
    transport, 'UpdateTask', { path: task(taskId), input: parseApiUpdateTaskRequest(mergeActor(actor, input)) }, parseApiUpdateTaskResponse, ...options(request.signal),
  )
  const addTaskLabel = (taskId: string, input: AddTaskLabelIntent, request: MutationRequestOptions = {}) => requestContract(
    transport, 'AddTaskLabel', { path: parseApiAddTaskLabelPath({ task_id: taskId }), input: parseApiAddTaskLabelRequest(mergeActor(actor, input)) }, parseApiAddTaskLabelResponse, ...options(request.signal),
  )
  const removeTaskLabel = (taskId: string, labelId: string, request: MutationRequestOptions = {}) => requestContract(
    transport, 'RemoveTaskLabel', { path: parseApiRemoveTaskLabelPath({ task_id: taskId, label_id: labelId }) }, parseApiRemoveTaskLabelResponse, ...options(request.signal),
  )
  const suggestTaskLabels = (taskId: string, query: SuggestTaskLabelsQuery = {}, request: MutationRequestOptions = {}) => requestContract(
    transport, 'SuggestTaskLabels', { path: parseApiSuggestTaskLabelsPath({ task_id: taskId }), query: parseApiLabelSuggestionQuery(query) }, parseApiSuggestTaskLabelsResponse, undefined, request.signal,
  )

  function transitionTask(taskId: string, action: 'specify', input?: SpecifyTaskIntent, options?: MutationRequestOptions): Promise<ApiSpecifyTaskResponseContract>
  function transitionTask(taskId: string, action: 'promote', input?: PromoteTaskIntent, options?: MutationRequestOptions): Promise<ApiPromoteTaskResponseContract>
  function transitionTask(taskId: string, action: 'claim', input?: ClaimTaskIntent, options?: MutationRequestOptions): Promise<ApiClaimTaskResponseContract>
  function transitionTask(taskId: string, action: 'heartbeat', input: HeartbeatTaskIntent, options?: MutationRequestOptions): Promise<ApiHeartbeatTaskResponseContract>
  function transitionTask(taskId: string, action: 'complete', input?: CompleteTaskIntent, options?: MutationRequestOptions): Promise<ApiCompleteTaskResponseContract>
  function transitionTask(taskId: string, action: 'submit-review', input?: SubmitReviewTaskIntent, options?: MutationRequestOptions): Promise<ApiSubmitReviewTaskResponseContract>
  function transitionTask(taskId: string, action: 'block', input: BlockTaskIntent, options?: MutationRequestOptions): Promise<ApiBlockTaskResponseContract>
  function transitionTask(taskId: string, action: 'unblock', input?: UnblockTaskIntent, options?: MutationRequestOptions): Promise<ApiUnblockTaskResponseContract>
  function transitionTask(taskId: string, action: 'archive', input?: ArchiveTaskIntent, options?: MutationRequestOptions): Promise<ApiArchiveTaskResponseContract>
  function transitionTask(taskId: string, action: TaskTransitionAction, input: Readonly<Record<string, unknown>> = {}, request: MutationRequestOptions = {}): Promise<TaskTransitionResponse> {
    const body = mergeActor(actor, input)
    const path = task(taskId)
    switch (action) {
      case 'specify': return requestContract(transport, 'SpecifyTask', { path, input: parseApiSpecifyTaskRequest(body) }, parseApiSpecifyTaskResponse, ...options(request.signal))
      case 'promote': return requestContract(transport, 'PromoteTask', { path, input: parseApiPromoteTaskRequest(body) }, parseApiPromoteTaskResponse, ...options(request.signal))
      case 'claim': return requestContract(transport, 'ClaimTask', { path, input: parseApiClaimTaskRequest(body) }, parseApiClaimTaskResponse, ...options(request.signal))
      case 'heartbeat': return requestContract(transport, 'HeartbeatTask', { path, input: parseApiHeartbeatTaskRequest(body) }, parseApiHeartbeatTaskResponse, ...options(request.signal))
      case 'complete': return requestContract(transport, 'CompleteTask', { path, input: parseApiCompleteTaskRequest(body) }, parseApiCompleteTaskResponse, ...options(request.signal))
      case 'submit-review': return requestContract(transport, 'SubmitReviewTask', { path, input: parseApiSubmitReviewTaskRequest(body) }, parseApiSubmitReviewTaskResponse, ...options(request.signal))
      case 'block': return requestContract(transport, 'BlockTask', { path, input: parseApiBlockTaskRequest(body) }, parseApiBlockTaskResponse, ...options(request.signal))
      case 'unblock': return requestContract(transport, 'UnblockTask', { path, input: parseApiUnblockTaskRequest(body) }, parseApiUnblockTaskResponse, ...options(request.signal))
      case 'archive': return requestContract(transport, 'ArchiveTask', { path, input: parseApiArchiveTaskRequest(body) }, parseApiArchiveTaskResponse, ...options(request.signal))
    }
  }

  const listDependencies = (taskId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'ListDependencies', { path: parseApiListDependenciesPath({ task_id: taskId }) }, parseApiListDependenciesResponse, undefined, request.signal)
  const addDependency = (taskId: string, parentTaskId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'AddDependency', { path: parseApiAddDependencyPath({ task_id: taskId }), input: parseApiAddDependencyRequest({ actor, parent_task_id: parentTaskId }) }, parseApiAddDependencyResponse, ...options(request.signal))
  const removeDependency = (taskId: string, parentTaskId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'RemoveDependency', { path: parseApiRemoveDependencyPath({ child_task_id: taskId, parent_task_id: parentTaskId }) }, parseApiRemoveDependencyResponse, ...options(request.signal))
  const listSteps = (taskId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'ListSteps', { path: parseApiListStepsPath({ task_id: taskId }) }, parseApiListStepsResponse, undefined, request.signal)
  const mutateStep = (taskId: string, stepId: string, command: StepMutationIntent, request: MutationRequestOptions = {}) => {
    const path = { task_id: taskId, step_id: stepId }
    switch (command.action) {
      case 'update': return requestContract(transport, 'UpdateStep', { path: parseApiUpdateStepPath(path), input: parseApiUpdateStepRequest(mergeActor(actor, command.input)) }, parseApiUpdateStepResponse, ...options(request.signal))
      case 'remove': return requestContract(transport, 'RemoveStep', { path: parseApiRemoveStepPath(path) }, parseApiRemoveStepResponse, ...options(request.signal))
      case 'complete': return requestContract(transport, 'CompleteStep', { path: parseApiCompleteStepPath(path), input: parseApiCompleteStepRequest(mergeActor(actor, command.input)) }, parseApiCompleteStepResponse, ...options(request.signal))
      case 'skip': return requestContract(transport, 'SkipStep', { path: parseApiSkipStepPath(path), input: parseApiSkipStepRequest(mergeActor(actor, command.input)) }, parseApiSkipStepResponse, ...options(request.signal))
      case 'reopen': return requestContract(transport, 'ReopenStep', { path: parseApiReopenStepPath(path), input: parseApiReopenStepRequest(mergeActor(actor, command.input)) }, parseApiReopenStepResponse, ...options(request.signal))
    }
  }
  const createStep = (taskId: string, input: CreateStepIntent, request: MutationRequestOptions = {}) => requestContract(transport, 'CreateStep', { path: parseApiCreateStepPath({ task_id: taskId }), input: parseApiCreateStepRequest(mergeActor(actor, input)) }, parseApiCreateStepResponse, ...options(request.signal))
  const markExecutionPlanNotRequired = (taskId: string, input: MarkExecutionPlanNotRequiredIntent, request: MutationRequestOptions = {}) => requestContract(transport, 'MarkExecutionPlanNotRequired', { path: parseApiMarkExecutionPlanNotRequiredPath({ task_id: taskId }), input: parseApiMarkExecutionPlanNotRequiredRequest(mergeActor(actor, input)) }, parseApiMarkExecutionPlanNotRequiredResponse, ...options(request.signal))
  const listComments = (taskId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'ListComments', { path: parseApiListCommentsPath({ task_id: taskId }) }, parseApiListCommentsResponse, undefined, request.signal)
  const createComment = (taskId: string, input: CreateCommentIntent, request: MutationRequestOptions = {}) => requestContract(transport, 'CreateComment', { path: parseApiCreateCommentPath({ task_id: taskId }), input: parseApiCreateCommentRequest({ ...input, author: actor }) }, parseApiCreateCommentResponse, ...options(request.signal))
  const listAttachments = (taskId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'ListAttachments', { path: parseApiListAttachmentsPath({ task_id: taskId }) }, parseApiListAttachmentsResponse, undefined, request.signal)
  const createAttachment = (taskId: string, input: CreateAttachmentIntent, request: MutationRequestOptions = {}) => requestContract(transport, 'CreateAttachment', { path: parseApiCreateAttachmentPath({ task_id: taskId }), input: attachmentInput(actor, input) }, parseApiCreateAttachmentResponse, ...options(request.signal))
  const deleteAttachment = (taskId: string, attachmentId: string, request: MutationRequestOptions = {}) => requestContract(transport, 'DeleteAttachment', { path: parseApiDeleteAttachmentPath({ task_id: taskId, attachment_id: attachmentId }) }, parseApiDeleteAttachmentResponse, ...options(request.signal))
  return { createTask, updateTask, addTaskLabel, removeTaskLabel, suggestTaskLabels, transitionTask, listDependencies, addDependency, removeDependency, listSteps, mutateStep, createStep, markExecutionPlanNotRequired, listComments, createComment, listAttachments, createAttachment, deleteAttachment }
}

export function createTaskMutationClient(runtime: WebRuntimeConfig, activeBoard: CanonicalBoardSlug, dependencies: TaskMutationDependencies = {}): TaskMutationClient {
  return createClient(runtime, activeBoard, dependencies)
}

/** 二进制内容独立校验，避免将大文件展开为 JSON number[]。 */
function attachmentInput(actor: string, input: CreateAttachmentIntent) {
  const content = input.content ?? new Uint8Array()
  if (content.length > 256 * 1024 * 1024) throw new Error('附件超过 256 MiB 上传上限。')
  const parsed = parseApiCreateAttachmentRequest(mergeActor(actor, { ...input, content: content instanceof Uint8Array ? [] : content }))
  return { ...parsed, content: content instanceof Uint8Array ? content : new Uint8Array(content) }
}
