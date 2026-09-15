import type { WebRuntimeConfig } from "../../lib/runtime";

import type { CanonicalBoardSlug } from "../../domain/board-slug";

import { parseActorPreference } from "../../platform/preferences/preferences";

import { createHttpTransport } from "./http-transport";
import { type HttpTransport, type HttpRequestMethod } from "../../application/data/http-transport";

import { parseApiAddDependencyPath } from "../../lib/api/generated/contracts/api-add-dependency-path";

import { parseApiAddDependencyHeaders } from "../../lib/api/generated/contracts/api-add-dependency-headers";

import { parseApiAddDependencyRequest } from "../../lib/api/generated/contracts/api-add-dependency-request";

import { parseApiAddDependencyResponse } from "../../lib/api/generated/contracts/api-add-dependency-response";

import { parseApiAddTaskLabelPath } from "../../lib/api/generated/contracts/api-add-task-label-path";

import { parseApiAddTaskLabelHeaders } from "../../lib/api/generated/contracts/api-add-task-label-headers";

import { parseApiAddTaskLabelRequest } from "../../lib/api/generated/contracts/api-add-task-label-request";

import { parseApiAddTaskLabelResponse } from "../../lib/api/generated/contracts/api-add-task-label-response";

import { parseApiArchiveTaskHeaders } from "../../lib/api/generated/contracts/api-archive-task-headers";

import { parseApiArchiveTaskRequest } from "../../lib/api/generated/contracts/api-archive-task-request";

import { parseApiArchiveTaskResponse, type ApiArchiveTaskResponseContract } from "../../lib/api/generated/contracts/api-archive-task-response";

import { parseApiBlockTaskHeaders } from "../../lib/api/generated/contracts/api-block-task-headers";

import { parseApiBlockTaskRequest } from "../../lib/api/generated/contracts/api-block-task-request";

import { parseApiBlockTaskResponse, type ApiBlockTaskResponseContract } from "../../lib/api/generated/contracts/api-block-task-response";

import { parseApiClaimTaskHeaders } from "../../lib/api/generated/contracts/api-claim-task-headers";

import { parseApiClaimTaskRequest } from "../../lib/api/generated/contracts/api-claim-task-request";

import { parseApiClaimTaskResponse, type ApiClaimTaskResponseContract } from "../../lib/api/generated/contracts/api-claim-task-response";

import { parseApiCompleteTaskHeaders } from "../../lib/api/generated/contracts/api-complete-task-headers";

import { parseApiCompleteTaskRequest } from "../../lib/api/generated/contracts/api-complete-task-request";

import { parseApiCompleteTaskResponse, type ApiCompleteTaskResponseContract } from "../../lib/api/generated/contracts/api-complete-task-response";

import { parseApiCreateCommentPath } from "../../lib/api/generated/contracts/api-create-comment-path";

import { parseApiCreateCommentHeaders } from "../../lib/api/generated/contracts/api-create-comment-headers";

import { parseApiCreateCommentRequest } from "../../lib/api/generated/contracts/api-create-comment-request";

import { parseApiCreateCommentResponse } from "../../lib/api/generated/contracts/api-create-comment-response";

import { parseApiCreateAttachmentPath } from "../../lib/api/generated/contracts/api-create-attachment-path";

import { parseApiCreateAttachmentHeaders } from "../../lib/api/generated/contracts/api-create-attachment-headers";

import { parseApiCreateAttachmentRequest } from "../../lib/api/generated/contracts/api-create-attachment-request";

import { parseApiCreateAttachmentResponse } from "../../lib/api/generated/contracts/api-create-attachment-response";

import { parseApiCreateStepPath } from "../../lib/api/generated/contracts/api-create-step-path";

import { parseApiCreateStepHeaders } from "../../lib/api/generated/contracts/api-create-step-headers";

import { parseApiCreateStepRequest } from "../../lib/api/generated/contracts/api-create-step-request";

import { parseApiCreateStepResponse } from "../../lib/api/generated/contracts/api-create-step-response";

import { parseApiCreateTaskHeaders } from "../../lib/api/generated/contracts/api-create-task-headers";

import { parseApiCreateTaskRequest } from "../../lib/api/generated/contracts/api-create-task-request";

import { parseApiCreateTaskResponse } from "../../lib/api/generated/contracts/api-create-task-response";

import { parseApiDeleteAttachmentPath } from "../../lib/api/generated/contracts/api-delete-attachment-path";

import { parseApiDeleteAttachmentHeaders } from "../../lib/api/generated/contracts/api-delete-attachment-headers";

import { parseApiDeleteAttachmentResponse } from "../../lib/api/generated/contracts/api-delete-attachment-response";

import { parseApiHeartbeatTaskHeaders } from "../../lib/api/generated/contracts/api-heartbeat-task-headers";

import { parseApiHeartbeatTaskRequest } from "../../lib/api/generated/contracts/api-heartbeat-task-request";

import { parseApiHeartbeatTaskResponse, type ApiHeartbeatTaskResponseContract } from "../../lib/api/generated/contracts/api-heartbeat-task-response";

import { parseApiListCommentsPath } from "../../lib/api/generated/contracts/api-list-comments-path";

import { parseApiListCommentsHeaders } from "../../lib/api/generated/contracts/api-list-comments-headers";

import { parseApiListCommentsResponse } from "../../lib/api/generated/contracts/api-list-comments-response";

import { parseApiListDependenciesPath } from "../../lib/api/generated/contracts/api-list-dependencies-path";

import { parseApiListDependenciesHeaders } from "../../lib/api/generated/contracts/api-list-dependencies-headers";

import { parseApiListDependenciesResponse } from "../../lib/api/generated/contracts/api-list-dependencies-response";

import { parseApiListAttachmentsPath } from "../../lib/api/generated/contracts/api-list-attachments-path";

import { parseApiListAttachmentsHeaders } from "../../lib/api/generated/contracts/api-list-attachments-headers";

import { parseApiListAttachmentsResponse } from "../../lib/api/generated/contracts/api-list-attachments-response";

import { parseApiListStepsPath } from "../../lib/api/generated/contracts/api-list-steps-path";

import { parseApiListStepsHeaders } from "../../lib/api/generated/contracts/api-list-steps-headers";

import { parseApiListStepsResponse } from "../../lib/api/generated/contracts/api-list-steps-response";

import { parseApiMarkExecutionPlanNotRequiredPath } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-path";

import { parseApiMarkExecutionPlanNotRequiredHeaders } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-headers";

import { parseApiMarkExecutionPlanNotRequiredRequest } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-request";

import { parseApiMarkExecutionPlanNotRequiredResponse } from "../../lib/api/generated/contracts/api-mark-execution-plan-not-required-response";

import { parseApiPromoteTaskHeaders } from "../../lib/api/generated/contracts/api-promote-task-headers";

import { parseApiPromoteTaskRequest } from "../../lib/api/generated/contracts/api-promote-task-request";

import { parseApiPromoteTaskResponse, type ApiPromoteTaskResponseContract } from "../../lib/api/generated/contracts/api-promote-task-response";

import { parseApiRemoveDependencyPath } from "../../lib/api/generated/contracts/api-remove-dependency-path";

import { parseApiRemoveDependencyHeaders } from "../../lib/api/generated/contracts/api-remove-dependency-headers";

import { parseApiRemoveDependencyResponse } from "../../lib/api/generated/contracts/api-remove-dependency-response";

import { parseApiRemoveTaskLabelPath } from "../../lib/api/generated/contracts/api-remove-task-label-path";

import { parseApiRemoveTaskLabelHeaders } from "../../lib/api/generated/contracts/api-remove-task-label-headers";

import { parseApiRemoveTaskLabelResponse } from "../../lib/api/generated/contracts/api-remove-task-label-response";

import { parseApiSpecifyTaskHeaders } from "../../lib/api/generated/contracts/api-specify-task-headers";

import { parseApiSpecifyTaskRequest } from "../../lib/api/generated/contracts/api-specify-task-request";

import { parseApiSpecifyTaskResponse, type ApiSpecifyTaskResponseContract } from "../../lib/api/generated/contracts/api-specify-task-response";

import { parseApiSubmitReviewTaskHeaders } from "../../lib/api/generated/contracts/api-submit-review-task-headers";

import { parseApiSubmitReviewTaskRequest } from "../../lib/api/generated/contracts/api-submit-review-task-request";

import { parseApiSubmitReviewTaskResponse, type ApiSubmitReviewTaskResponseContract } from "../../lib/api/generated/contracts/api-submit-review-task-response";

import { parseApiSuggestTaskLabelsHeaders } from "../../lib/api/generated/contracts/api-suggest-task-labels-headers";

import { parseApiSuggestTaskLabelsResponse } from "../../lib/api/generated/contracts/api-suggest-task-labels-response";

import { parseApiUnblockTaskHeaders } from "../../lib/api/generated/contracts/api-unblock-task-headers";

import { parseApiUnblockTaskRequest } from "../../lib/api/generated/contracts/api-unblock-task-request";

import { parseApiUnblockTaskResponse, type ApiUnblockTaskResponseContract } from "../../lib/api/generated/contracts/api-unblock-task-response";

import { parseApiUpdateTaskHeaders } from "../../lib/api/generated/contracts/api-update-task-headers";

import { parseApiUpdateTaskRequest } from "../../lib/api/generated/contracts/api-update-task-request";

import { parseApiUpdateTaskResponse } from "../../lib/api/generated/contracts/api-update-task-response";

import { type RequestHeaders, type TaskMutationDependencies, type TaskMutationClient, type CreateTaskIntent, type MutationRequestOptions, mergeActor, createTaskPath, jsonHeaders, type UpdateTaskIntent, updateTaskPath, type AddTaskLabelIntent, taskPath, encodedSegment, actorHeaders, type SuggestTaskLabelsQuery, suggestTaskLabelsRequestPath, readHeaders, type SpecifyTaskIntent, type PromoteTaskIntent, type ClaimTaskIntent, type HeartbeatTaskIntent, type CompleteTaskIntent, type SubmitReviewTaskIntent, type BlockTaskIntent, type UnblockTaskIntent, type ArchiveTaskIntent, type TaskTransitionAction, type TaskTransitionResponse, transitionPath, type CreateStepIntent, type MarkExecutionPlanNotRequiredIntent, type CreateCommentIntent, type CreateAttachmentIntent } from "../../application/data/task-mutations";

export async function requestContract<T>(
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

export function createClient(
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
