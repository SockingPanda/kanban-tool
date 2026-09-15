// @generated: xtask rpc-codegen；显式字段 codec 与具名 Connect 调用。
import { create, fromBinary, toBinary } from "@bufbuild/protobuf"
import type { Client, CallOptions } from "@connectrpc/connect"
import type { RpcCall, RpcMethod } from "../../application/data/rpc-transport"
import * as s from "../../generated/rpc/kanban/v1/kanban_pb"
import * as d from "../../generated/rpc/kanban/v1/dto_pb"
import { EmptySchema } from "../../generated/rpc/kanban/v1/common_pb"
import * as c from "./value-codec"
import { eventCase, validateEventCase, validateStreamEvent } from "./event-shape"

export function encodeGetHealthRequest(call: RpcCall): s.GetHealthRequest {
  void call
  return create(s.GetHealthRequestSchema, {
  })
}

export function encodeListBoardsRequest(call: RpcCall): s.ListBoardsRequest {
  const query = c.record(call.query ?? {}, ["include_archived"])
  return create(s.ListBoardsRequestSchema, {
    includeArchived: c.present(query["include_archived"], (value) => c.bool(value)),
  })
}

export function encodeCreateBoardRequest(call: RpcCall): s.CreateBoardRequest {
  const input = c.record(call.input ?? {}, ["slug","name","description","actor"])
  return create(s.CreateBoardRequestSchema, {
    slug: c.text(input["slug"]),
    name: c.text(input["name"]),
    description: c.optional(input["description"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeGetBoardRequest(call: RpcCall): s.GetBoardRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.GetBoardRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeArchiveBoardRequest(call: RpcCall): s.ArchiveBoardRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["actor"])
  return create(s.ArchiveBoardRequestSchema, {
    board: c.text(path["board"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeListBoardColumnsRequest(call: RpcCall): s.ListBoardColumnsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.ListBoardColumnsRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeListTasksRequest(call: RpcCall): s.ListTasksRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["status","priority","label","plan_filter","assignee","q","include_archived","limit","offset","sort"])
  return create(s.ListTasksRequestSchema, {
    board: c.text(path["board"]),
    status: c.array((query["status"] ?? []), (value) => encodeDtoApiTaskStatus(value)),
    priority: c.array((query["priority"] ?? []), (value) => encodeDtoApiTaskPriority(value)),
    label: c.array((query["label"] ?? []), (value) => encodeDtoTaskReadLabel(value)),
    planFilter: c.array((query["plan_filter"] ?? []), (value) => encodeDtoTaskReadPlanFilter(value)),
    assignee: c.optional(query["assignee"], (value) => c.text(value)),
    q: c.optional(query["q"], (value) => c.text(value)),
    includeArchived: c.present(query["include_archived"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    offset: c.present(query["offset"], (value) => c.int64(value, true)),
    sort: c.present(query["sort"], (value) => encodeDtoTaskReadSort(value)),
  })
}

export function encodeListTasksByStatusRequest(call: RpcCall): s.ListTasksByStatusRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["status","priority","label","plan_filter","assignee","q","include_archived","limit","offset","sort"])
  return create(s.ListTasksByStatusRequestSchema, {
    board: c.text(path["board"]),
    status: c.array((query["status"] ?? []), (value) => encodeDtoApiTaskStatus(value)),
    priority: c.array((query["priority"] ?? []), (value) => encodeDtoApiTaskPriority(value)),
    label: c.array((query["label"] ?? []), (value) => encodeDtoTaskReadLabel(value)),
    planFilter: c.array((query["plan_filter"] ?? []), (value) => encodeDtoTaskReadPlanFilter(value)),
    assignee: c.optional(query["assignee"], (value) => c.text(value)),
    q: c.optional(query["q"], (value) => c.text(value)),
    includeArchived: c.present(query["include_archived"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    offset: c.present(query["offset"], (value) => c.int64(value, true)),
    sort: c.present(query["sort"], (value) => encodeDtoTaskReadSort(value)),
  })
}

export function encodeCreateTaskRequest(call: RpcCall): s.CreateTaskRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["task_id","idempotency_key","title","description","status","assignee","priority","scheduled_at","due_at","max_retries","metadata","labels","depends_on","actor"])
  return create(s.CreateTaskRequestSchema, {
    board: c.text(path["board"]),
    taskId: c.optional(input["task_id"], (value) => c.text(value)),
    idempotencyKey: c.optional(input["idempotency_key"], (value) => c.text(value)),
    title: c.text(input["title"]),
    description: c.optional(input["description"], (value) => c.text(value)),
    status: c.optional(input["status"], (value) => encodeDtoApiCreateTaskStatus(value)),
    assignee: c.optional(input["assignee"], (value) => c.text(value)),
    priority: c.present(input["priority"], (value) => c.int64(value)),
    scheduledAt: c.optional(input["scheduled_at"], (value) => c.int64(value)),
    dueAt: c.optional(input["due_at"], (value) => c.int64(value)),
    maxRetries: c.optional(input["max_retries"], (value) => c.int64(value)),
    metadata: c.optional(input["metadata"], (value) => encodeMapOfJsonValue(value)),
    labels: c.array((input["labels"] ?? []), (value) => c.text(value)),
    dependsOn: c.array((input["depends_on"] ?? []), (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeGetTaskRequest(call: RpcCall): s.GetTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["include"])
  return create(s.GetTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    include: c.optional(query["include"], (value) => c.text(value)),
  })
}

export function encodeUpdateTaskRequest(call: RpcCall): s.UpdateTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["title","description","assignee","priority","scheduled_at","due_at","max_retries","metadata","actor","expected_lock_version"])
  return create(s.UpdateTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    title: c.optional(input["title"], (value) => c.text(value)),
    description: c.present(input["description"], (value) => create(d.PatchStringSchema, { change: value === null ? { case: "clear", value: create(EmptySchema) } : { case: "value", value: c.text(value) } })),
    assignee: c.present(input["assignee"], (value) => create(d.PatchStringSchema, { change: value === null ? { case: "clear", value: create(EmptySchema) } : { case: "value", value: c.text(value) } })),
    priority: c.optional(input["priority"], (value) => c.int64(value)),
    scheduledAt: c.present(input["scheduled_at"], (value) => create(d.PatchI64Schema, { change: value === null ? { case: "clear", value: create(EmptySchema) } : { case: "value", value: c.int64(value) } })),
    dueAt: c.present(input["due_at"], (value) => create(d.PatchI64Schema, { change: value === null ? { case: "clear", value: create(EmptySchema) } : { case: "value", value: c.int64(value) } })),
    maxRetries: c.present(input["max_retries"], (value) => create(d.PatchI64Schema, { change: value === null ? { case: "clear", value: create(EmptySchema) } : { case: "value", value: c.int64(value) } })),
    metadata: c.present(input["metadata"], (value) => create(d.PatchJsonValueSchema, { change: value === null ? { case: "clear", value: create(EmptySchema) } : { case: "value", value: c.encodeJson(value) } })),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    expectedLockVersion: c.optional(input["expected_lock_version"], (value) => c.int64(value)),
  })
}

export function encodeSpecifyTaskRequest(call: RpcCall): s.SpecifyTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","description","scheduled_at"])
  return create(s.SpecifyTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    description: c.optional(input["description"], (value) => c.text(value)),
    scheduledAt: c.optional(input["scheduled_at"], (value) => c.int64(value)),
  })
}

export function encodePromoteTaskRequest(call: RpcCall): s.PromoteTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor"])
  return create(s.PromoteTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeClaimTaskRequest(call: RpcCall): s.ClaimTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","ttl_ms","worker_profile","metadata"])
  return create(s.ClaimTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    ttlMs: c.present(input["ttl_ms"], (value) => c.int64(value)),
    workerProfile: c.optional(input["worker_profile"], (value) => c.text(value)),
    metadata: c.optional(input["metadata"], (value) => c.encodeJson(value)),
  })
}

export function encodeReopenTaskRequest(call: RpcCall): s.ReopenTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","reason"])
  return create(s.ReopenTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
  })
}

export function encodeReclaimTaskRequest(call: RpcCall): s.ReclaimTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","force","to_status","reason"])
  return create(s.ReclaimTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    force: c.present(input["force"], (value) => c.bool(value)),
    toStatus: c.optional(input["to_status"], (value) => encodeDtoReclaimTargetStatus(value)),
    reason: c.optional(input["reason"], (value) => c.text(value)),
  })
}

export function encodeHeartbeatTaskRequest(call: RpcCall): s.HeartbeatTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","claim_token","ttl_ms","note"])
  return create(s.HeartbeatTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    claimToken: c.text(input["claim_token"]),
    ttlMs: c.present(input["ttl_ms"], (value) => c.int64(value)),
    note: c.optional(input["note"], (value) => c.text(value)),
  })
}

export function encodeReleaseTaskRequest(call: RpcCall): s.ReleaseTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","claim_token"])
  return create(s.ReleaseTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    claimToken: c.text(input["claim_token"]),
  })
}

export function encodeCompleteTaskRequest(call: RpcCall): s.CompleteTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","claim_token","force","summary","result"])
  return create(s.CompleteTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    claimToken: c.optional(input["claim_token"], (value) => c.text(value)),
    force: c.present(input["force"], (value) => c.bool(value)),
    summary: c.optional(input["summary"], (value) => c.text(value)),
    result: c.optional(input["result"], (value) => c.encodeJson(value)),
  })
}

export function encodeSubmitReviewTaskRequest(call: RpcCall): s.SubmitReviewTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","claim_token","force","summary"])
  return create(s.SubmitReviewTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    claimToken: c.optional(input["claim_token"], (value) => c.text(value)),
    force: c.present(input["force"], (value) => c.bool(value)),
    summary: c.optional(input["summary"], (value) => c.text(value)),
  })
}

export function encodeBlockTaskRequest(call: RpcCall): s.BlockTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","reason","claim_token","force"])
  return create(s.BlockTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
    claimToken: c.optional(input["claim_token"], (value) => c.text(value)),
    force: c.present(input["force"], (value) => c.bool(value)),
  })
}

export function encodeUnblockTaskRequest(call: RpcCall): s.UnblockTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor"])
  return create(s.UnblockTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeArchiveTaskRequest(call: RpcCall): s.ArchiveTaskRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["actor","force"])
  return create(s.ArchiveTaskRequestSchema, {
    taskId: c.text(path["task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    force: c.present(input["force"], (value) => c.bool(value)),
  })
}

export function encodeListStepsRequest(call: RpcCall): s.ListStepsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.ListStepsRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeCreateStepRequest(call: RpcCall): s.CreateStepRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["idempotency_key","title","body","linked_task_ref","position","required","actor"])
  return create(s.CreateStepRequestSchema, {
    taskId: c.text(path["task_id"]),
    idempotencyKey: c.optional(input["idempotency_key"], (value) => c.text(value)),
    title: c.text(input["title"]),
    body: c.optional(input["body"], (value) => c.text(value)),
    linkedTaskRef: c.optional(input["linked_task_ref"], (value) => c.text(value)),
    position: c.optional(input["position"], (value) => c.int64(value)),
    required: c.present(input["required"], (value) => c.bool(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeUpdateStepRequest(call: RpcCall): s.UpdateStepRequest {
  const path = c.record(call.path ?? {}, ["task_id","step_id"])
  const input = c.record(call.input ?? {}, ["title","body","linked_task_ref","unlink_task","position","required","actor"])
  return create(s.UpdateStepRequestSchema, {
    taskId: c.text(path["task_id"]),
    stepId: c.text(path["step_id"]),
    title: c.optional(input["title"], (value) => c.text(value)),
    body: c.optional(input["body"], (value) => c.text(value)),
    linkedTaskRef: c.optional(input["linked_task_ref"], (value) => c.text(value)),
    unlinkTask: c.present(input["unlink_task"], (value) => c.bool(value)),
    position: c.optional(input["position"], (value) => c.int64(value)),
    required: c.optional(input["required"], (value) => c.bool(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeRemoveStepRequest(call: RpcCall): s.RemoveStepRequest {
  const path = c.record(call.path ?? {}, ["task_id","step_id"])
  return create(s.RemoveStepRequestSchema, {
    taskId: c.text(path["task_id"]),
    stepId: c.text(path["step_id"]),
  })
}

export function encodeCompleteStepRequest(call: RpcCall): s.CompleteStepRequest {
  const path = c.record(call.path ?? {}, ["task_id","step_id"])
  const input = c.record(call.input ?? {}, ["note","actor"])
  return create(s.CompleteStepRequestSchema, {
    taskId: c.text(path["task_id"]),
    stepId: c.text(path["step_id"]),
    note: c.text(input["note"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeSkipStepRequest(call: RpcCall): s.SkipStepRequest {
  const path = c.record(call.path ?? {}, ["task_id","step_id"])
  const input = c.record(call.input ?? {}, ["reason","actor"])
  return create(s.SkipStepRequestSchema, {
    taskId: c.text(path["task_id"]),
    stepId: c.text(path["step_id"]),
    reason: c.text(input["reason"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeReopenStepRequest(call: RpcCall): s.ReopenStepRequest {
  const path = c.record(call.path ?? {}, ["task_id","step_id"])
  const input = c.record(call.input ?? {}, ["reason","actor"])
  return create(s.ReopenStepRequestSchema, {
    taskId: c.text(path["task_id"]),
    stepId: c.text(path["step_id"]),
    reason: c.text(input["reason"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeMarkExecutionPlanNotRequiredRequest(call: RpcCall): s.MarkExecutionPlanNotRequiredRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["reason","actor"])
  return create(s.MarkExecutionPlanNotRequiredRequestSchema, {
    taskId: c.text(path["task_id"]),
    reason: c.text(input["reason"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeListDependenciesRequest(call: RpcCall): s.ListDependenciesRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.ListDependenciesRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeAddDependencyRequest(call: RpcCall): s.AddDependencyRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["parent_task_id","actor"])
  return create(s.AddDependencyRequestSchema, {
    taskId: c.text(path["task_id"]),
    parentTaskId: c.text(input["parent_task_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeRemoveDependencyRequest(call: RpcCall): s.RemoveDependencyRequest {
  const path = c.record(call.path ?? {}, ["child_task_id","parent_task_id"])
  return create(s.RemoveDependencyRequestSchema, {
    childTaskId: c.text(path["child_task_id"]),
    parentTaskId: c.text(path["parent_task_id"]),
  })
}

export function encodeListRunsRequest(call: RpcCall): s.ListRunsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.ListRunsRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeGetRunRequest(call: RpcCall): s.GetRunRequest {
  const path = c.record(call.path ?? {}, ["run_id"])
  return create(s.GetRunRequestSchema, {
    runId: c.text(path["run_id"]),
  })
}

export function encodeGetRunLogRequest(call: RpcCall): s.GetRunLogRequest {
  const path = c.record(call.path ?? {}, ["run_id"])
  return create(s.GetRunLogRequestSchema, {
    runId: c.text(path["run_id"]),
  })
}

export function encodeListCommentsRequest(call: RpcCall): s.ListCommentsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.ListCommentsRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeCreateCommentRequest(call: RpcCall): s.CreateCommentRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["idempotency_key","author","body","kind","author_type","agent_type","metadata"])
  return create(s.CreateCommentRequestSchema, {
    taskId: c.text(path["task_id"]),
    idempotencyKey: c.optional(input["idempotency_key"], (value) => c.text(value)),
    author: c.optional(input["author"], (value) => c.text(value)),
    body: c.text(input["body"]),
    kind: c.optional(input["kind"], (value) => encodeDtoCommentsCommentKind(value)),
    authorType: c.optional(input["author_type"], (value) => encodeDtoCommentsCommentAuthorType(value)),
    agentType: c.optional(input["agent_type"], (value) => c.text(value)),
    metadata: c.optional(input["metadata"], (value) => c.encodeJson(value)),
  })
}

export function encodeListAttachmentsRequest(call: RpcCall): s.ListAttachmentsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.ListAttachmentsRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeCreateAttachmentRequest(call: RpcCall): s.CreateAttachmentRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["id","filename","content","content_type","rel_path","sha256","actor"])
  return create(s.CreateAttachmentRequestSchema, {
    taskId: c.text(path["task_id"]),
    id: c.optional(input["id"], (value) => c.text(value)),
    filename: c.text(input["filename"]),
    content: c.bytes(input["content"] ?? new Uint8Array(0)),
    contentType: c.optional(input["content_type"], (value) => c.text(value)),
    relPath: c.optional(input["rel_path"], (value) => c.text(value)),
    sha256: c.optional(input["sha256"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeDownloadAttachmentRequest(call: RpcCall): s.DownloadAttachmentRequest {
  const path = c.record(call.path ?? {}, ["task_id","attachment_id"])
  return create(s.DownloadAttachmentRequestSchema, {
    taskId: c.text(path["task_id"]),
    attachmentId: c.text(path["attachment_id"]),
  })
}

export function encodeDeleteAttachmentRequest(call: RpcCall): s.DeleteAttachmentRequest {
  const path = c.record(call.path ?? {}, ["task_id","attachment_id"])
  return create(s.DeleteAttachmentRequestSchema, {
    taskId: c.text(path["task_id"]),
    attachmentId: c.text(path["attachment_id"]),
  })
}

export function encodeListEventsRequest(call: RpcCall): s.ListEventsRequest {
  const query = c.record(call.query ?? {}, ["board","task_id","after","limit"])
  return create(s.ListEventsRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    taskId: c.optional(query["task_id"], (value) => c.text(value)),
    after: c.present(query["after"], (value) => c.int64(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeListTaskLabelsRequest(call: RpcCall): s.ListTaskLabelsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.ListTaskLabelsRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeAddTaskLabelRequest(call: RpcCall): s.AddTaskLabelRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["name","names","create_missing","actor"])
  return create(s.AddTaskLabelRequestSchema, {
    taskId: c.text(path["task_id"]),
    name: c.optional(input["name"], (value) => c.text(value)),
    names: c.optional(input["names"], (value) => encodeListOfString(value)),
    createMissing: c.present(input["create_missing"], (value) => c.bool(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeBootstrapTaskLabelRequest(call: RpcCall): s.BootstrapTaskLabelRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const input = c.record(call.input ?? {}, ["name","description","applies_when","excludes_when","positive_examples","negative_examples","verify","min_verify_score","vector_config","actor"])
  return create(s.BootstrapTaskLabelRequestSchema, {
    taskId: c.text(path["task_id"]),
    name: c.text(input["name"]),
    description: c.optional(input["description"], (value) => c.text(value)),
    appliesWhen: c.array((input["applies_when"] ?? []), (value) => c.text(value)),
    excludesWhen: c.array((input["excludes_when"] ?? []), (value) => c.text(value)),
    positiveExamples: c.array((input["positive_examples"] ?? []), (value) => c.text(value)),
    negativeExamples: c.array((input["negative_examples"] ?? []), (value) => c.text(value)),
    verify: c.present(input["verify"], (value) => c.bool(value)),
    minVerifyScore: c.present(input["min_verify_score"], (value) => c.float(value, true)),
    vectorConfig: c.optional(input["vector_config"], (value) => encodeDtoVectorConfigureRequest(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
  })
}

export function encodeRemoveTaskLabelRequest(call: RpcCall): s.RemoveTaskLabelRequest {
  const path = c.record(call.path ?? {}, ["task_id","label_id"])
  return create(s.RemoveTaskLabelRequestSchema, {
    taskId: c.text(path["task_id"]),
    labelId: c.text(path["label_id"]),
  })
}

export function encodeListBoardLabelsRequest(call: RpcCall): s.ListBoardLabelsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.ListBoardLabelsRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeListBoardLabelProposalsRequest(call: RpcCall): s.ListBoardLabelProposalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["status"])
  return create(s.ListBoardLabelProposalsRequestSchema, {
    board: c.text(path["board"]),
    status: c.optional(query["status"], (value) => encodeDtoLabelProposalStatusWire(value)),
  })
}

export function encodeCreateBoardLabelRequest(call: RpcCall): s.CreateBoardLabelRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["name","color"])
  return create(s.CreateBoardLabelRequestSchema, {
    board: c.text(path["board"]),
    name: c.text(input["name"]),
    color: c.optional(input["color"], (value) => c.text(value)),
  })
}

export function encodeDeleteBoardLabelRequest(call: RpcCall): s.DeleteBoardLabelRequest {
  const path = c.record(call.path ?? {}, ["board","label_id"])
  const query = c.record(call.query ?? {}, ["force"])
  return create(s.DeleteBoardLabelRequestSchema, {
    board: c.text(path["board"]),
    labelId: c.text(path["label_id"]),
    force: c.present(query["force"], (value) => c.bool(value)),
  })
}

export function encodeListLabelSemanticsRequest(call: RpcCall): s.ListLabelSemanticsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.ListLabelSemanticsRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeGetLabelSemanticsRequest(call: RpcCall): s.GetLabelSemanticsRequest {
  const path = c.record(call.path ?? {}, ["board","label_id"])
  return create(s.GetLabelSemanticsRequestSchema, {
    board: c.text(path["board"]),
    labelId: c.text(path["label_id"]),
  })
}

export function encodeUpsertLabelSemanticsRequest(call: RpcCall): s.UpsertLabelSemanticsRequest {
  const path = c.record(call.path ?? {}, ["board","label_id"])
  const input = c.record(call.input ?? {}, ["actor","expected_semantics_hash","replace","reason","source_signal_ids","description","applies_when","excludes_when","positive_examples","negative_examples","remove_applies_when","remove_excludes_when","remove_positive_examples","remove_negative_examples"])
  return create(s.UpsertLabelSemanticsRequestSchema, {
    board: c.text(path["board"]),
    labelId: c.text(path["label_id"]),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    expectedSemanticsHash: c.optional(input["expected_semantics_hash"], (value) => c.text(value)),
    replace: c.present(input["replace"], (value) => c.bool(value)),
    reason: c.optional(input["reason"], (value) => c.text(value)),
    sourceSignalIds: c.array((input["source_signal_ids"] ?? []), (value) => c.text(value)),
    description: c.optional(input["description"], (value) => c.text(value)),
    appliesWhen: c.optional(input["applies_when"], (value) => encodeListOfString(value)),
    excludesWhen: c.optional(input["excludes_when"], (value) => encodeListOfString(value)),
    positiveExamples: c.optional(input["positive_examples"], (value) => encodeListOfString(value)),
    negativeExamples: c.optional(input["negative_examples"], (value) => encodeListOfString(value)),
    removeAppliesWhen: c.array((input["remove_applies_when"] ?? []), (value) => c.text(value)),
    removeExcludesWhen: c.array((input["remove_excludes_when"] ?? []), (value) => c.text(value)),
    removePositiveExamples: c.array((input["remove_positive_examples"] ?? []), (value) => c.text(value)),
    removeNegativeExamples: c.array((input["remove_negative_examples"] ?? []), (value) => c.text(value)),
  })
}

export function encodeDeleteLabelSemanticsRequest(call: RpcCall): s.DeleteLabelSemanticsRequest {
  const path = c.record(call.path ?? {}, ["board","label_id"])
  const query = c.record(call.query ?? {}, ["expected_semantics_hash","reason"])
  return create(s.DeleteLabelSemanticsRequestSchema, {
    board: c.text(path["board"]),
    labelId: c.text(path["label_id"]),
    expectedSemanticsHash: c.text(query["expected_semantics_hash"]),
    reason: c.text(query["reason"]),
  })
}

export function encodeListLabelAtomsRequest(call: RpcCall): s.ListLabelAtomsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.ListLabelAtomsRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeExplainLabelAtomRequest(call: RpcCall): s.ExplainLabelAtomRequest {
  const path = c.record(call.path ?? {}, ["board","atom_ref"])
  return create(s.ExplainLabelAtomRequestSchema, {
    board: c.text(path["board"]),
    atomRef: c.text(path["atom_ref"]),
  })
}

export function encodeLabelAtomIndexStatusRequest(call: RpcCall): s.LabelAtomIndexStatusRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.LabelAtomIndexStatusRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeRebuildLabelAtomIndexRequest(call: RpcCall): s.RebuildLabelAtomIndexRequest {
  const path = c.record(call.path ?? {}, ["board"])
  return create(s.RebuildLabelAtomIndexRequestSchema, {
    board: c.text(path["board"]),
  })
}

export function encodeQueryLabelAtomIndexRequest(call: RpcCall): s.QueryLabelAtomIndexRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["q","vector_json","embedding_model","include_vector","polarity","limit"])
  return create(s.QueryLabelAtomIndexRequestSchema, {
    board: c.text(path["board"]),
    q: c.optional(query["q"], (value) => c.text(value)),
    vectorJson: c.optional(query["vector_json"], (value) => c.text(value)),
    embeddingModel: c.optional(query["embedding_model"], (value) => c.text(value)),
    includeVector: c.present(query["include_vector"], (value) => c.bool(value)),
    polarity: c.optional(query["polarity"], (value) => c.text(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeListSignalsRequest(call: RpcCall): s.ListSignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["status","kind","task_ref","include_all","limit"])
  return create(s.ListSignalsRequestSchema, {
    board: c.text(path["board"]),
    status: c.array((query["status"] ?? []), (value) => c.text(value)),
    kind: c.array((query["kind"] ?? []), (value) => c.text(value)),
    taskRef: c.optional(query["task_ref"], (value) => c.text(value)),
    includeAll: c.present(query["include_all"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeReviewSignalsRequest(call: RpcCall): s.ReviewSignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["status","kind","task_ref","include_all","limit"])
  return create(s.ReviewSignalsRequestSchema, {
    board: c.text(path["board"]),
    status: c.array((query["status"] ?? []), (value) => c.text(value)),
    kind: c.array((query["kind"] ?? []), (value) => c.text(value)),
    taskRef: c.optional(query["task_ref"], (value) => c.text(value)),
    includeAll: c.present(query["include_all"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeGetSignalRequest(call: RpcCall): s.GetSignalRequest {
  const path = c.record(call.path ?? {}, ["signal_id"])
  return create(s.GetSignalRequestSchema, {
    signalId: c.text(path["signal_id"]),
  })
}

export function encodeRecordSignalRequest(call: RpcCall): s.RecordSignalRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["kind","title","summary","severity","task_ref","task_id","run_id","comment_id","actor","agent_type","dedupe_key","source","evidence","comment"])
  return create(s.RecordSignalRequestSchema, {
    board: c.text(path["board"]),
    kind: c.text(input["kind"]),
    title: c.text(input["title"]),
    summary: c.text(input["summary"]),
    severity: c.optional(input["severity"], (value) => c.text(value)),
    taskRef: c.optional(input["task_ref"], (value) => c.text(value)),
    taskId: c.optional(input["task_id"], (value) => c.text(value)),
    runId: c.optional(input["run_id"], (value) => c.text(value)),
    commentId: c.optional(input["comment_id"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    agentType: c.optional(input["agent_type"], (value) => c.text(value)),
    dedupeKey: c.optional(input["dedupe_key"], (value) => c.text(value)),
    source: c.optional(input["source"], (value) => c.text(value)),
    evidence: c.optional(input["evidence"], (value) => encodeDtoStructuredMetadataJsonObject(value)),
    comment: c.optional(input["comment"], (value) => encodeDtoSignalCommentRequest(value)),
  })
}

export function encodeConfirmSignalsRequest(call: RpcCall): s.ConfirmSignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["signal_ids","reason","replacement_signal_id","actor","expected_updated_at"])
  return create(s.ConfirmSignalsRequestSchema, {
    board: c.text(path["board"]),
    signalIds: c.array(input["signal_ids"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
    replacementSignalId: c.optional(input["replacement_signal_id"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    expectedUpdatedAt: c.optional(input["expected_updated_at"], (value) => c.int64(value)),
  })
}

export function encodeRejectSignalsRequest(call: RpcCall): s.RejectSignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["signal_ids","reason","replacement_signal_id","actor","expected_updated_at"])
  return create(s.RejectSignalsRequestSchema, {
    board: c.text(path["board"]),
    signalIds: c.array(input["signal_ids"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
    replacementSignalId: c.optional(input["replacement_signal_id"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    expectedUpdatedAt: c.optional(input["expected_updated_at"], (value) => c.int64(value)),
  })
}

export function encodeResolveSignalsRequest(call: RpcCall): s.ResolveSignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["signal_ids","reason","replacement_signal_id","actor","expected_updated_at"])
  return create(s.ResolveSignalsRequestSchema, {
    board: c.text(path["board"]),
    signalIds: c.array(input["signal_ids"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
    replacementSignalId: c.optional(input["replacement_signal_id"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    expectedUpdatedAt: c.optional(input["expected_updated_at"], (value) => c.int64(value)),
  })
}

export function encodeSupersedeSignalsRequest(call: RpcCall): s.SupersedeSignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["signal_ids","reason","replacement_signal_id","actor","expected_updated_at"])
  return create(s.SupersedeSignalsRequestSchema, {
    board: c.text(path["board"]),
    signalIds: c.array(input["signal_ids"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
    replacementSignalId: c.optional(input["replacement_signal_id"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    expectedUpdatedAt: c.optional(input["expected_updated_at"], (value) => c.int64(value)),
  })
}

export function encodeSuggestTaskLabelsRequest(call: RpcCall): s.SuggestTaskLabelsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["board","limit","candidate_limit","atom_limit","max_selected_labels","min_score"])
  return create(s.SuggestTaskLabelsRequestSchema, {
    taskId: c.text(path["task_id"]),
    board: c.optional(query["board"], (value) => c.text(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    candidateLimit: c.present(query["candidate_limit"], (value) => c.int64(value, true)),
    atomLimit: c.present(query["atom_limit"], (value) => c.int64(value, true)),
    maxSelectedLabels: c.present(query["max_selected_labels"], (value) => c.int64(value, true)),
    minScore: c.present(query["min_score"], (value) => c.float(value, true)),
  })
}

export function encodeListTaskLabelProposalsRequest(call: RpcCall): s.ListTaskLabelProposalsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["board","status"])
  return create(s.ListTaskLabelProposalsRequestSchema, {
    taskId: c.text(path["task_id"]),
    board: c.optional(query["board"], (value) => c.text(value)),
    status: c.optional(query["status"], (value) => c.text(value)),
  })
}

export function encodeProposeTaskLabelRequest(call: RpcCall): s.ProposeTaskLabelRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["board","limit","candidate_limit","atom_limit","max_selected_labels","min_score"])
  const input = c.record(call.input ?? {}, ["proposal","actor","source_signal_ids","ontology_actor","allow_retarget","retarget_reason"])
  return create(s.ProposeTaskLabelRequestSchema, {
    taskId: c.text(path["task_id"]),
    board: c.optional(query["board"], (value) => c.text(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    candidateLimit: c.present(query["candidate_limit"], (value) => c.int64(value, true)),
    atomLimit: c.present(query["atom_limit"], (value) => c.int64(value, true)),
    maxSelectedLabels: c.present(query["max_selected_labels"], (value) => c.int64(value, true)),
    minScore: c.present(query["min_score"], (value) => c.float(value, true)),
    proposal: c.optional(input["proposal"], (value) => encodeDtoLabelProposalCandidateWire(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    sourceSignalIds: c.array((input["source_signal_ids"] ?? []), (value) => c.text(value)),
    ontologyActor: c.optional(input["ontology_actor"], (value) => encodeDtoLabelOntologyActorWire(value)),
    allowRetarget: c.present(input["allow_retarget"], (value) => c.bool(value)),
    retargetReason: c.optional(input["retarget_reason"], (value) => c.text(value)),
  })
}

export function encodeRecordLabelOntologyObservationRequest(call: RpcCall): s.RecordLabelOntologyObservationRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["actor","agent_candidates","suggestion_snapshot","final_decision","suggest_coverage","suggest_coverage_cosine","suggest_residual_norm","suggest_needs_new_label","suggest_degraded","diagnostics","capture_fingerprint","signals"])
  return create(s.RecordLabelOntologyObservationRequestSchema, {
    taskId: c.text(path["task_id"]),
    board: c.optional(query["board"], (value) => c.text(value)),
    actor: encodeDtoLabelOntologyActorWire(input["actor"]),
    agentCandidates: c.present(input["agent_candidates"], c.encodeJson),
    suggestionSnapshot: c.present(input["suggestion_snapshot"], c.encodeJson),
    finalDecision: c.present(input["final_decision"], c.encodeJson),
    suggestCoverage: c.optional(input["suggest_coverage"], (value) => c.float(value)),
    suggestCoverageCosine: c.optional(input["suggest_coverage_cosine"], (value) => c.float(value)),
    suggestResidualNorm: c.optional(input["suggest_residual_norm"], (value) => c.float(value)),
    suggestNeedsNewLabel: c.optional(input["suggest_needs_new_label"], (value) => c.bool(value)),
    suggestDegraded: c.optional(input["suggest_degraded"], (value) => c.bool(value)),
    diagnostics: c.present(input["diagnostics"], c.encodeJson),
    captureFingerprint: c.optional(input["capture_fingerprint"], (value) => c.text(value)),
    signals: c.array(input["signals"], (value) => encodeDtoLabelOntologySignalRequest(value)),
  })
}

export function encodeListLabelOntologySignalsRequest(call: RpcCall): s.ListLabelOntologySignalsRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["status","kind","task_ref","target_label_ref","proposed_label_name","include_all","limit"])
  return create(s.ListLabelOntologySignalsRequestSchema, {
    board: c.text(path["board"]),
    status: c.array((query["status"] ?? []), (value) => c.text(value)),
    kind: c.array((query["kind"] ?? []), (value) => c.text(value)),
    taskRef: c.optional(query["task_ref"], (value) => c.text(value)),
    targetLabelRef: c.optional(query["target_label_ref"], (value) => c.text(value)),
    proposedLabelName: c.optional(query["proposed_label_name"], (value) => c.text(value)),
    includeAll: c.present(query["include_all"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeReviewLabelOntologyRequest(call: RpcCall): s.ReviewLabelOntologyRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["group_by","include_all","limit"])
  return create(s.ReviewLabelOntologyRequestSchema, {
    board: c.text(path["board"]),
    groupBy: c.present(query["group_by"], (value) => encodeDtoLabelOntologyReviewGroupByWire(value)),
    includeAll: c.present(query["include_all"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeCreateLabelOntologyActionRequest(call: RpcCall): s.CreateLabelOntologyActionRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["actor","idempotency_key","action_type","signal_ids","reason","superseded_by_signal_id","parent_action_id","target_label_ref","result_label_ref","result_atom_id","result_atom_content_hash","result_proposal_id","canonical_before_hash","canonical_after_hash","change","validation_status","validation"])
  return create(s.CreateLabelOntologyActionRequestSchema, {
    board: c.text(path["board"]),
    actor: encodeDtoLabelOntologyActorWire(input["actor"]),
    idempotencyKey: c.optional(input["idempotency_key"], (value) => c.text(value)),
    actionType: encodeDtoLabelOntologyActionTypeWire(input["action_type"]),
    signalIds: c.array(input["signal_ids"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
    supersededBySignalId: c.optional(input["superseded_by_signal_id"], (value) => c.text(value)),
    parentActionId: c.optional(input["parent_action_id"], (value) => c.text(value)),
    targetLabelRef: c.optional(input["target_label_ref"], (value) => c.text(value)),
    resultLabelRef: c.optional(input["result_label_ref"], (value) => c.text(value)),
    resultAtomId: c.optional(input["result_atom_id"], (value) => c.text(value)),
    resultAtomContentHash: c.optional(input["result_atom_content_hash"], (value) => c.text(value)),
    resultProposalId: c.optional(input["result_proposal_id"], (value) => c.text(value)),
    canonicalBeforeHash: c.optional(input["canonical_before_hash"], (value) => c.text(value)),
    canonicalAfterHash: c.optional(input["canonical_after_hash"], (value) => c.text(value)),
    change: c.present(input["change"], c.encodeJson),
    validationStatus: c.optional(input["validation_status"], (value) => encodeDtoLabelOntologyValidationStatusWire(value)),
    validation: c.present(input["validation"], c.encodeJson),
  })
}

export function encodeApplyLabelOntologyAtomRequest(call: RpcCall): s.ApplyLabelOntologyAtomRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["actor","signal_ids","label_ref","kind","text","reason","allow_retarget","retarget_reason"])
  return create(s.ApplyLabelOntologyAtomRequestSchema, {
    board: c.text(path["board"]),
    actor: encodeDtoLabelOntologyActorWire(input["actor"]),
    signalIds: c.array(input["signal_ids"], (value) => c.text(value)),
    labelRef: c.text(input["label_ref"]),
    kind: c.text(input["kind"]),
    text: c.text(input["text"]),
    reason: c.text(input["reason"]),
    allowRetarget: c.present(input["allow_retarget"], (value) => c.bool(value)),
    retargetReason: c.optional(input["retarget_reason"], (value) => c.text(value)),
  })
}

export function encodeRevertLabelOntologyMutationRequest(call: RpcCall): s.RevertLabelOntologyMutationRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["actor","target_action_id","expected_current_hash","reason"])
  return create(s.RevertLabelOntologyMutationRequestSchema, {
    board: c.text(path["board"]),
    actor: encodeDtoLabelOntologyActorWire(input["actor"]),
    targetActionId: c.text(input["target_action_id"]),
    expectedCurrentHash: c.optional(input["expected_current_hash"], (value) => c.text(value)),
    reason: c.text(input["reason"]),
  })
}

export function encodeValidateLabelOntologyActionRequest(call: RpcCall): s.ValidateLabelOntologyActionRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const input = c.record(call.input ?? {}, ["actor","parent_action_id","signal_ids","reason","validation_status","validation"])
  return create(s.ValidateLabelOntologyActionRequestSchema, {
    board: c.text(path["board"]),
    actor: encodeDtoLabelOntologyActorWire(input["actor"]),
    parentActionId: c.text(input["parent_action_id"]),
    signalIds: c.array((input["signal_ids"] ?? []), (value) => c.text(value)),
    reason: c.text(input["reason"]),
    validationStatus: encodeDtoLabelOntologyValidationStatusWire(input["validation_status"]),
    validation: c.present(input["validation"], c.encodeJson),
  })
}

export function encodeGetLabelOntologySignalRequest(call: RpcCall): s.GetLabelOntologySignalRequest {
  const path = c.record(call.path ?? {}, ["signal_id"])
  return create(s.GetLabelOntologySignalRequestSchema, {
    signalId: c.text(path["signal_id"]),
  })
}

export function encodeGetLabelProposalRequest(call: RpcCall): s.GetLabelProposalRequest {
  const path = c.record(call.path ?? {}, ["proposal_id"])
  return create(s.GetLabelProposalRequestSchema, {
    proposalId: c.text(path["proposal_id"]),
  })
}

export function encodeAcceptLabelProposalRequest(call: RpcCall): s.AcceptLabelProposalRequest {
  const path = c.record(call.path ?? {}, ["proposal_id"])
  const input = c.record(call.input ?? {}, ["reason","actor","source_signal_ids","ontology_actor","allow_retarget","retarget_reason"])
  return create(s.AcceptLabelProposalRequestSchema, {
    proposalId: c.text(path["proposal_id"]),
    reason: c.optional(input["reason"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    sourceSignalIds: c.array((input["source_signal_ids"] ?? []), (value) => c.text(value)),
    ontologyActor: c.optional(input["ontology_actor"], (value) => encodeDtoLabelOntologyActorWire(value)),
    allowRetarget: c.present(input["allow_retarget"], (value) => c.bool(value)),
    retargetReason: c.optional(input["retarget_reason"], (value) => c.text(value)),
  })
}

export function encodeRejectLabelProposalRequest(call: RpcCall): s.RejectLabelProposalRequest {
  const path = c.record(call.path ?? {}, ["proposal_id"])
  const input = c.record(call.input ?? {}, ["reason","actor","source_signal_ids","ontology_actor","allow_retarget","retarget_reason"])
  return create(s.RejectLabelProposalRequestSchema, {
    proposalId: c.text(path["proposal_id"]),
    reason: c.optional(input["reason"], (value) => c.text(value)),
    actor: c.optional(input["actor"], (value) => c.text(value)),
    sourceSignalIds: c.array((input["source_signal_ids"] ?? []), (value) => c.text(value)),
    ontologyActor: c.optional(input["ontology_actor"], (value) => encodeDtoLabelOntologyActorWire(value)),
    allowRetarget: c.present(input["allow_retarget"], (value) => c.bool(value)),
    retargetReason: c.optional(input["retarget_reason"], (value) => c.text(value)),
  })
}

export function encodeBoardTaskMapRequest(call: RpcCall): s.BoardTaskMapRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["active_only","context_depth","limit_nodes","include_done_context","include_archived_context","hide_isolated"])
  return create(s.BoardTaskMapRequestSchema, {
    board: c.text(path["board"]),
    activeOnly: c.present(query["active_only"], (value) => c.bool(value)),
    contextDepth: c.present(query["context_depth"], (value) => c.int64(value, true)),
    limitNodes: c.present(query["limit_nodes"], (value) => c.int64(value, true)),
    includeDoneContext: c.present(query["include_done_context"], (value) => c.bool(value)),
    includeArchivedContext: c.present(query["include_archived_context"], (value) => c.bool(value)),
    hideIsolated: c.present(query["hide_isolated"], (value) => c.bool(value)),
  })
}

export function encodeTaskNeighborhoodRequest(call: RpcCall): s.TaskNeighborhoodRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["depth","limit_nodes","include_archived_context"])
  return create(s.TaskNeighborhoodRequestSchema, {
    taskId: c.text(path["task_id"]),
    depth: c.present(query["depth"], (value) => c.int64(value, true)),
    limitNodes: c.present(query["limit_nodes"], (value) => c.int64(value, true)),
    includeArchivedContext: c.present(query["include_archived_context"], (value) => c.bool(value)),
  })
}

export function encodeSearchTasksRequest(call: RpcCall): s.SearchTasksRequest {
  const query = c.record(call.query ?? {}, ["board","q","status","label","include_archived","limit","offset","assignee"])
  return create(s.SearchTasksRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    q: c.optional(query["q"], (value) => c.text(value)),
    status: c.array((query["status"] ?? []), (value) => encodeDtoApiTaskStatus(value)),
    label: c.array((query["label"] ?? []), (value) => c.text(value)),
    includeArchived: c.present(query["include_archived"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    offset: c.present(query["offset"], (value) => c.int64(value, true)),
    assignee: c.optional(query["assignee"], (value) => c.text(value)),
  })
}

export function encodeSearchTasksByStatusRequest(call: RpcCall): s.SearchTasksByStatusRequest {
  const query = c.record(call.query ?? {}, ["board","q","status","label","include_archived","limit","offset","assignee"])
  return create(s.SearchTasksByStatusRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    q: c.optional(query["q"], (value) => c.text(value)),
    status: c.array((query["status"] ?? []), (value) => encodeDtoApiTaskStatus(value)),
    label: c.array((query["label"] ?? []), (value) => c.text(value)),
    includeArchived: c.present(query["include_archived"], (value) => c.bool(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    offset: c.present(query["offset"], (value) => c.int64(value, true)),
    assignee: c.optional(query["assignee"], (value) => c.text(value)),
  })
}

export function encodeSearchStatusRequest(call: RpcCall): s.SearchStatusRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.SearchStatusRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeRebuildSearchIndexRequest(call: RpcCall): s.RebuildSearchIndexRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.RebuildSearchIndexRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeSyncSearchIndexRequest(call: RpcCall): s.SyncSearchIndexRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.SyncSearchIndexRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeBuildContextRequest(call: RpcCall): s.BuildContextRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  const query = c.record(call.query ?? {}, ["board","lexical_limit","graph_limit","vector_limit","max_items","task","reference","query","depth","budget"])
  return create(s.BuildContextRequestSchema, {
    taskId: c.text(path["task_id"]),
    board: c.present(query["board"], (value) => c.text(value)),
    lexicalLimit: c.present(query["lexical_limit"], (value) => c.int64(value, true)),
    graphLimit: c.present(query["graph_limit"], (value) => c.int64(value, true)),
    vectorLimit: c.present(query["vector_limit"], (value) => c.int64(value, true)),
    maxItems: c.present(query["max_items"], (value) => c.int64(value, true)),
    task: c.optional(query["task"], (value) => c.text(value)),
    reference: c.optional(query["reference"], (value) => c.text(value)),
    query: c.optional(query["query"], (value) => c.text(value)),
    depth: c.present(query["depth"], (value) => c.int64(value, true)),
    budget: c.optional(query["budget"], (value) => c.int64(value, true)),
  })
}

export function encodeGraphStatusRequest(call: RpcCall): s.GraphStatusRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.GraphStatusRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeGraphNeighborsRequest(call: RpcCall): s.GraphNeighborsRequest {
  const query = c.record(call.query ?? {}, ["board","entity_uri","predicate","limit"])
  return create(s.GraphNeighborsRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    entityUri: c.text(query["entity_uri"]),
    predicate: c.optional(query["predicate"], (value) => c.text(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeGraphQueryRequest(call: RpcCall): s.GraphQueryRequest {
  const query = c.record(call.query ?? {}, ["board","query","limit"])
  return create(s.GraphQueryRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    query: c.present(query["query"], (value) => c.text(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeGraphRebuildRequest(call: RpcCall): s.GraphRebuildRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.GraphRebuildRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeGraphSyncRequest(call: RpcCall): s.GraphSyncRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.GraphSyncRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeListEntitiesRequest(call: RpcCall): s.ListEntitiesRequest {
  const query = c.record(call.query ?? {}, ["board","kind","limit"])
  return create(s.ListEntitiesRequestSchema, {
    board: c.optional(query["board"], (value) => c.text(value)),
    kind: c.optional(query["kind"], (value) => c.text(value)),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
  })
}

export function encodeUpsertEntityRequest(call: RpcCall): s.UpsertEntityRequest {
  const input = c.record(call.input ?? {}, ["uri","kind","source_table","source_id","board","task_id","title","summary","content_hash","archived_at"])
  return create(s.UpsertEntityRequestSchema, {
    uri: c.text(input["uri"]),
    kind: c.text(input["kind"]),
    sourceTable: c.text(input["source_table"]),
    sourceId: c.text(input["source_id"]),
    board: c.optional(input["board"], (value) => c.text(value)),
    taskId: c.optional(input["task_id"], (value) => c.text(value)),
    title: c.optional(input["title"], (value) => c.text(value)),
    summary: c.optional(input["summary"], (value) => c.text(value)),
    contentHash: c.optional(input["content_hash"], (value) => c.text(value)),
    archivedAt: c.optional(input["archived_at"], (value) => c.int64(value)),
  })
}

export function encodeGetEntityRequest(call: RpcCall): s.GetEntityRequest {
  const path = c.record(call.path ?? {}, ["uri"])
  return create(s.GetEntityRequestSchema, {
    uri: c.text(path["uri"]),
  })
}

export function encodeVectorStatusRequest(call: RpcCall): s.VectorStatusRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.VectorStatusRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeVectorConfigureRequest(call: RpcCall): s.VectorConfigureRequest {
  const input = c.record(call.input ?? {}, ["provider","endpoint","model","dimensions"])
  return create(s.VectorConfigureRequestSchema, {
    provider: c.text(input["provider"]),
    endpoint: c.text(input["endpoint"]),
    model: c.text(input["model"]),
    dimensions: c.int64(input["dimensions"], true),
  })
}

export function encodeVectorRebuildRequest(call: RpcCall): s.VectorRebuildRequest {
  const input = c.record(call.input ?? {}, ["board"])
  return create(s.VectorRebuildRequestSchema, {
    board: c.present(input["board"], (value) => c.text(value)),
  })
}

export function encodeVectorSyncRequest(call: RpcCall): s.VectorSyncRequest {
  const input = c.record(call.input ?? {}, ["board"])
  return create(s.VectorSyncRequestSchema, {
    board: c.present(input["board"], (value) => c.text(value)),
  })
}

export function encodeVectorQueryChunksRequest(call: RpcCall): s.VectorQueryChunksRequest {
  const query = c.record(call.query ?? {}, ["board","q","limit","embedding_model","polarity","include_vector"])
  return create(s.VectorQueryChunksRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    q: c.text(query["q"]),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    embeddingModel: c.optional(query["embedding_model"], (value) => c.text(value)),
    polarity: c.optional(query["polarity"], (value) => c.text(value)),
    includeVector: c.present(query["include_vector"], (value) => c.bool(value)),
  })
}

export function encodeVectorQueryLabelAtomsRequest(call: RpcCall): s.VectorQueryLabelAtomsRequest {
  const query = c.record(call.query ?? {}, ["board","q","limit","embedding_model","polarity","include_vector"])
  return create(s.VectorQueryLabelAtomsRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
    q: c.text(query["q"]),
    limit: c.present(query["limit"], (value) => c.int64(value, true)),
    embeddingModel: c.optional(query["embedding_model"], (value) => c.text(value)),
    polarity: c.optional(query["polarity"], (value) => c.text(value)),
    includeVector: c.present(query["include_vector"], (value) => c.bool(value)),
  })
}

export function encodeGetStatsRequest(call: RpcCall): s.GetStatsRequest {
  const query = c.record(call.query ?? {}, ["board"])
  return create(s.GetStatsRequestSchema, {
    board: c.present(query["board"], (value) => c.text(value)),
  })
}

export function encodeDoctorRequest(call: RpcCall): s.DoctorRequest {
  void call
  return create(s.DoctorRequestSchema, {
  })
}

export function encodeCheckpointRequest(call: RpcCall): s.CheckpointRequest {
  void call
  return create(s.CheckpointRequestSchema, {
  })
}

export function encodeMaintenanceBackupRequest(call: RpcCall): s.MaintenanceBackupRequest {
  const input = c.record(call.input ?? {}, ["path"])
  return create(s.MaintenanceBackupRequestSchema, {
    path: c.text(input["path"]),
  })
}

export function encodeMaintenanceExportRequest(call: RpcCall): s.MaintenanceExportRequest {
  const input = c.record(call.input ?? {}, ["path"])
  return create(s.MaintenanceExportRequestSchema, {
    path: c.text(input["path"]),
  })
}

export function encodeMaintenanceImportRequest(call: RpcCall): s.MaintenanceImportRequest {
  const input = c.record(call.input ?? {}, ["path","replace"])
  return create(s.MaintenanceImportRequestSchema, {
    path: c.text(input["path"]),
    replace: c.present(input["replace"], (value) => c.bool(value)),
  })
}

export function encodeMaintenanceVacuumRequest(call: RpcCall): s.MaintenanceVacuumRequest {
  void call
  return create(s.MaintenanceVacuumRequestSchema, {
  })
}

export function encodeMaintenanceStatusRequest(call: RpcCall): s.MaintenanceStatusRequest {
  void call
  return create(s.MaintenanceStatusRequestSchema, {
  })
}

export function encodeMaintenanceRunRequest(call: RpcCall): s.MaintenanceRunRequest {
  const input = c.record(call.input ?? {}, ["owner","action"])
  return create(s.MaintenanceRunRequestSchema, {
    owner: c.optional(input["owner"], (value) => c.text(value)),
    action: c.optional(input["action"], (value) => c.text(value)),
  })
}

export function encodeMaintenanceRebuildRequest(call: RpcCall): s.MaintenanceRebuildRequest {
  const input = c.record(call.input ?? {}, ["owner","action"])
  return create(s.MaintenanceRebuildRequestSchema, {
    owner: c.optional(input["owner"], (value) => c.text(value)),
    action: c.optional(input["action"], (value) => c.text(value)),
  })
}

export function encodeMaintenanceCleanupRequest(call: RpcCall): s.MaintenanceCleanupRequest {
  const input = c.record(call.input ?? {}, ["owner","action"])
  return create(s.MaintenanceCleanupRequestSchema, {
    owner: c.optional(input["owner"], (value) => c.text(value)),
    action: c.optional(input["action"], (value) => c.text(value)),
  })
}

export function encodeMaintenanceImportV30Request(call: RpcCall): s.MaintenanceImportV30Request {
  const input = c.record(call.input ?? {}, ["path","canonical_attachment_root"])
  return create(s.MaintenanceImportV30RequestSchema, {
    path: c.text(input["path"]),
    canonicalAttachmentRoot: c.optional(input["canonical_attachment_root"], (value) => c.text(value)),
  })
}

export function encodeGetTaskDetailsRequest(call: RpcCall): s.GetTaskDetailsRequest {
  const path = c.record(call.path ?? {}, ["task_id"])
  return create(s.GetTaskDetailsRequestSchema, {
    taskId: c.text(path["task_id"]),
  })
}

export function encodeGetLabelOntologyQualityRequest(call: RpcCall): s.GetLabelOntologyQualityRequest {
  const path = c.record(call.path ?? {}, ["board"])
  const query = c.record(call.query ?? {}, ["sample_limit"])
  return create(s.GetLabelOntologyQualityRequestSchema, {
    board: c.text(path["board"]),
    sampleLimit: c.present(query["sample_limit"], (value) => c.int64(value, true)),
  })
}

export function encodeAcceptLabelProposalResponse(value: unknown): d.AcceptLabelProposalResponse {
  const dto = c.record(value, ["data"])
  return create(d.AcceptLabelProposalResponseSchema, {
    data: encodeDtoLabelSemanticProposalWire(dto["data"]),
  })
}
export function decodeAcceptLabelProposalResponse(wire: d.AcceptLabelProposalResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelSemanticProposalWire(c.required(wire.data, "data")),
  })
}

export function encodeAddDependencyResponse(value: unknown): d.AddDependencyResponse {
  const dto = c.record(value, ["data"])
  return create(d.AddDependencyResponseSchema, {
    data: encodeDtoApiDependencies(dto["data"]),
  })
}
export function decodeAddDependencyResponse(wire: d.AddDependencyResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiDependencies(c.required(wire.data, "data")),
  })
}

export function encodeAddTaskLabelResponse(value: unknown): d.AddTaskLabelResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.AddTaskLabelResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
    meta: c.optional(dto["meta"], (value) => encodeDtoCreatedLabelsMetaOfDtoApiLabel(value)),
  })
}
export function decodeAddTaskLabelResponse(wire: d.AddTaskLabelResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
    "meta": wire.meta === undefined ? undefined : ((value) => decodeDtoCreatedLabelsMetaOfDtoApiLabel(value))(wire.meta),
  })
}

export function encodeApplyLabelOntologyAtomResponse(value: unknown): d.ApplyLabelOntologyAtomResponse {
  const dto = c.record(value, ["data"])
  return create(d.ApplyLabelOntologyAtomResponseSchema, {
    data: encodeDtoLabelOntologyActionWire(dto["data"]),
  })
}
export function decodeApplyLabelOntologyAtomResponse(wire: d.ApplyLabelOntologyAtomResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelOntologyActionWire(c.required(wire.data, "data")),
  })
}

export function encodeArchiveBoardResponse(value: unknown): d.ArchiveBoardResponse {
  const dto = c.record(value, ["data"])
  return create(d.ArchiveBoardResponseSchema, {
    data: encodeDtoApiBoard(dto["data"]),
  })
}
export function decodeArchiveBoardResponse(wire: d.ArchiveBoardResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiBoard(c.required(wire.data, "data")),
  })
}

export function encodeArchiveTaskResponse(value: unknown): d.ArchiveTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.ArchiveTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeArchiveTaskResponse(wire: d.ArchiveTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeBlockTaskResponse(value: unknown): d.BlockTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.BlockTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeBlockTaskResponse(wire: d.BlockTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeBoardTaskMapResponse(value: unknown): d.BoardTaskMapResponse {
  const dto = c.record(value, ["data"])
  return create(d.BoardTaskMapResponseSchema, {
    data: encodeDtoBoardTaskMap(dto["data"]),
  })
}
export function decodeBoardTaskMapResponse(wire: d.BoardTaskMapResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoBoardTaskMap(c.required(wire.data, "data")),
  })
}

export function encodeBootstrapTaskLabelResponse(value: unknown): d.BootstrapTaskLabelResponse {
  const dto = c.record(value, ["data"])
  return create(d.BootstrapTaskLabelResponseSchema, {
    data: encodeDtoBootstrapTaskLabelData(dto["data"]),
  })
}
export function decodeBootstrapTaskLabelResponse(wire: d.BootstrapTaskLabelResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoBootstrapTaskLabelData(c.required(wire.data, "data")),
  })
}

export function encodeBuildContextResponse(value: unknown): d.BuildContextResponse {
  const dto = c.record(value, ["data"])
  return create(d.BuildContextResponseSchema, {
    data: encodeDtoContextPack(dto["data"]),
  })
}
export function decodeBuildContextResponse(wire: d.BuildContextResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoContextPack(c.required(wire.data, "data")),
  })
}

export function encodeCheckpointResponse(value: unknown): d.CheckpointResponse {
  const dto = c.record(value, ["data"])
  return create(d.CheckpointResponseSchema, {
    data: encodeDtoCheckpointReport(dto["data"]),
  })
}
export function decodeCheckpointResponse(wire: d.CheckpointResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoCheckpointReport(c.required(wire.data, "data")),
  })
}

export function encodeClaimTaskResponse(value: unknown): d.ClaimTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.ClaimTaskResponseSchema, {
    data: encodeDtoApiClaim(dto["data"]),
  })
}
export function decodeClaimTaskResponse(wire: d.ClaimTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiClaim(c.required(wire.data, "data")),
  })
}

export function encodeCompleteStepResponse(value: unknown): d.CompleteStepResponse {
  const dto = c.record(value, ["data"])
  return create(d.CompleteStepResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeCompleteStepResponse(wire: d.CompleteStepResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeCompleteTaskResponse(value: unknown): d.CompleteTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.CompleteTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeCompleteTaskResponse(wire: d.CompleteTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeConfirmSignalsResponse(value: unknown): d.ConfirmSignalsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ConfirmSignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoSignalWire(value)),
  })
}
export function decodeConfirmSignalsResponse(wire: d.ConfirmSignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoSignalWire(value)),
  })
}

export function encodeCreateAttachmentResponse(value: unknown): d.CreateAttachmentResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateAttachmentResponseSchema, {
    data: encodeDtoApiAttachment(dto["data"]),
  })
}
export function decodeCreateAttachmentResponse(wire: d.CreateAttachmentResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiAttachment(c.required(wire.data, "data")),
  })
}

export function encodeCreateBoardLabelResponse(value: unknown): d.CreateBoardLabelResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateBoardLabelResponseSchema, {
    data: encodeDtoApiLabel(dto["data"]),
  })
}
export function decodeCreateBoardLabelResponse(wire: d.CreateBoardLabelResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiLabel(c.required(wire.data, "data")),
  })
}

export function encodeCreateBoardResponse(value: unknown): d.CreateBoardResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateBoardResponseSchema, {
    data: encodeDtoApiBoard(dto["data"]),
  })
}
export function decodeCreateBoardResponse(wire: d.CreateBoardResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiBoard(c.required(wire.data, "data")),
  })
}

export function encodeCreateCommentResponse(value: unknown): d.CreateCommentResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateCommentResponseSchema, {
    data: encodeDtoApiComment(dto["data"]),
  })
}
export function decodeCreateCommentResponse(wire: d.CreateCommentResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiComment(c.required(wire.data, "data")),
  })
}

export function encodeCreateLabelOntologyActionResponse(value: unknown): d.CreateLabelOntologyActionResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateLabelOntologyActionResponseSchema, {
    data: encodeDtoLabelOntologyActionWire(dto["data"]),
  })
}
export function decodeCreateLabelOntologyActionResponse(wire: d.CreateLabelOntologyActionResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelOntologyActionWire(c.required(wire.data, "data")),
  })
}

export function encodeCreateStepResponse(value: unknown): d.CreateStepResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateStepResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeCreateStepResponse(wire: d.CreateStepResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeCreateTaskResponse(value: unknown): d.CreateTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.CreateTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeCreateTaskResponse(wire: d.CreateTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeDeleteAttachmentResponse(value: unknown): d.DeleteAttachmentResponse {
  const dto = c.record(value, ["data"])
  return create(d.DeleteAttachmentResponseSchema, {
    data: encodeDtoDeleteResult(dto["data"]),
  })
}
export function decodeDeleteAttachmentResponse(wire: d.DeleteAttachmentResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoDeleteResult(c.required(wire.data, "data")),
  })
}

export function encodeDeleteBoardLabelResponse(value: unknown): d.DeleteBoardLabelResponse {
  const dto = c.record(value, ["data"])
  return create(d.DeleteBoardLabelResponseSchema, {
    data: encodeDtoDeleteBoardLabelResult(dto["data"]),
  })
}
export function decodeDeleteBoardLabelResponse(wire: d.DeleteBoardLabelResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoDeleteBoardLabelResult(c.required(wire.data, "data")),
  })
}

export function encodeDeleteLabelSemanticsResponse(value: unknown): d.DeleteLabelSemanticsResponse {
  const dto = c.record(value, ["data"])
  return create(d.DeleteLabelSemanticsResponseSchema, {
    data: encodeDtoDeleteResult(dto["data"]),
  })
}
export function decodeDeleteLabelSemanticsResponse(wire: d.DeleteLabelSemanticsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoDeleteResult(c.required(wire.data, "data")),
  })
}

export function encodeDoctorResponse(value: unknown): d.DoctorResponse {
  const dto = c.record(value, ["data"])
  return create(d.DoctorResponseSchema, {
    data: encodeDtoDoctorReport(dto["data"]),
  })
}
export function decodeDoctorResponse(wire: d.DoctorResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoDoctorReport(c.required(wire.data, "data")),
  })
}

export function encodeDownloadAttachmentResponse(value: unknown): d.DownloadAttachmentResponse {
  const dto = c.record(value, ["attachment","content"])
  return create(d.DownloadAttachmentResponseSchema, {
    attachment: encodeDtoApiAttachment(dto["attachment"]),
    content: c.bytes(dto["content"]),
  })
}
export function decodeDownloadAttachmentResponse(wire: d.DownloadAttachmentResponse): Record<string, unknown> {
  return c.omitUndefined({
    "attachment": decodeDtoApiAttachment(c.required(wire.attachment, "attachment")),
    "content": wire.content,
  })
}

export function encodeDtoApiAttachment(value: unknown): d.DtoApiAttachment {
  const dto = c.record(value, ["id","board_id","task_id","filename","rel_path","content_type","size_bytes","sha256","created_by","created_at"])
  return create(d.DtoApiAttachmentSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    taskId: c.text(dto["task_id"]),
    filename: c.text(dto["filename"]),
    relPath: c.text(dto["rel_path"]),
    contentType: c.optional(dto["content_type"], (value) => c.text(value)),
    sizeBytes: c.int64(dto["size_bytes"]),
    sha256: c.optional(dto["sha256"], (value) => c.text(value)),
    createdBy: c.text(dto["created_by"]),
    createdAt: c.int64(dto["created_at"]),
  })
}
export function decodeDtoApiAttachment(wire: d.DtoApiAttachment): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": c.required(wire.taskId, "task_id"),
    "filename": c.required(wire.filename, "filename"),
    "rel_path": c.required(wire.relPath, "rel_path"),
    "content_type": wire.contentType === undefined ? null : ((value) => value)(wire.contentType),
    "size_bytes": c.safeNumber(c.required(wire.sizeBytes, "size_bytes")),
    "sha256": wire.sha256 === undefined ? null : ((value) => value)(wire.sha256),
    "created_by": c.required(wire.createdBy, "created_by"),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
  })
}

export function encodeDtoApiBoard(value: unknown): d.DtoApiBoard {
  const dto = c.record(value, ["id","slug","name","description","created_at","updated_at","archived_at"])
  return create(d.DtoApiBoardSchema, {
    id: c.text(dto["id"]),
    slug: c.text(dto["slug"]),
    name: c.text(dto["name"]),
    description: c.optional(dto["description"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    archivedAt: c.optional(dto["archived_at"], (value) => c.int64(value)),
  })
}
export function decodeDtoApiBoard(wire: d.DtoApiBoard): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "slug": c.required(wire.slug, "slug"),
    "name": c.required(wire.name, "name"),
    "description": wire.description === undefined ? null : ((value) => value)(wire.description),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "archived_at": wire.archivedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.archivedAt),
  })
}

export function encodeDtoApiBoardColumn(value: unknown): d.DtoApiBoardColumn {
  const dto = c.record(value, ["id","board_id","status","title","position","hidden","wip_limit","created_at","updated_at"])
  return create(d.DtoApiBoardColumnSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    status: encodeDtoApiTaskStatus(dto["status"]),
    title: c.text(dto["title"]),
    position: c.int64(dto["position"]),
    hidden: c.bool(dto["hidden"]),
    wipLimit: c.optional(dto["wip_limit"], (value) => c.int64(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoApiBoardColumn(wire: d.DtoApiBoardColumn): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "status": decodeDtoApiTaskStatus(c.required(wire.status, "status")),
    "title": c.required(wire.title, "title"),
    "position": c.safeNumber(c.required(wire.position, "position")),
    "hidden": c.required(wire.hidden, "hidden"),
    "wip_limit": wire.wipLimit === undefined ? null : ((value) => c.safeNumber(value))(wire.wipLimit),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

export function encodeDtoApiClaim(value: unknown): d.DtoApiClaim {
  const dto = c.record(value, ["task","run","claim_token","claim_expires_at"])
  return create(d.DtoApiClaimSchema, {
    task: encodeDtoApiTask(dto["task"]),
    run: encodeDtoApiRun(dto["run"]),
    claimToken: c.text(dto["claim_token"]),
    claimExpiresAt: c.optional(dto["claim_expires_at"], (value) => c.int64(value)),
  })
}
export function decodeDtoApiClaim(wire: d.DtoApiClaim): Record<string, unknown> {
  return c.omitUndefined({
    "task": decodeDtoApiTask(c.required(wire.task, "task")),
    "run": decodeDtoApiRun(c.required(wire.run, "run")),
    "claim_token": c.required(wire.claimToken, "claim_token"),
    "claim_expires_at": wire.claimExpiresAt === undefined ? null : ((value) => c.safeNumber(value))(wire.claimExpiresAt),
  })
}

export function encodeDtoApiComment(value: unknown): d.DtoApiComment {
  const dto = c.record(value, ["id","board_id","task_id","author","author_type","agent_type","body","kind","metadata","created_at"])
  return create(d.DtoApiCommentSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    taskId: c.text(dto["task_id"]),
    author: c.text(dto["author"]),
    authorType: encodeDtoCommentsCommentAuthorType(dto["author_type"]),
    agentType: c.optional(dto["agent_type"], (value) => c.text(value)),
    body: c.text(dto["body"]),
    kind: encodeDtoCommentsCommentKind(dto["kind"]),
    metadata: encodeDtoStructuredMetadataJsonObject(dto["metadata"]),
    createdAt: c.int64(dto["created_at"]),
  })
}
export function decodeDtoApiComment(wire: d.DtoApiComment): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": c.required(wire.taskId, "task_id"),
    "author": c.required(wire.author, "author"),
    "author_type": decodeDtoCommentsCommentAuthorType(c.required(wire.authorType, "author_type")),
    "agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),
    "body": c.required(wire.body, "body"),
    "kind": decodeDtoCommentsCommentKind(c.required(wire.kind, "kind")),
    "metadata": decodeDtoStructuredMetadataJsonObject(c.required(wire.metadata, "metadata")),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
  })
}

const DtoApiCreateTaskStatusNames = {
  "triage": d.DtoApiCreateTaskStatus.TRIAGE,
  "todo": d.DtoApiCreateTaskStatus.TODO,
  "scheduled": d.DtoApiCreateTaskStatus.SCHEDULED,
  "ready": d.DtoApiCreateTaskStatus.READY,
} as const
export function encodeDtoApiCreateTaskStatus(value: unknown): d.DtoApiCreateTaskStatus { return c.enumValue(value, DtoApiCreateTaskStatusNames) }
export function decodeDtoApiCreateTaskStatus(value: d.DtoApiCreateTaskStatus): string { return c.enumName(value, DtoApiCreateTaskStatusNames) }

export function encodeDtoApiDependencies(value: unknown): d.DtoApiDependencies {
  const dto = c.record(value, ["task","parents","children","edges"])
  return create(d.DtoApiDependenciesSchema, {
    task: encodeDtoApiDependencyTask(dto["task"]),
    parents: c.array(dto["parents"], (value) => encodeDtoApiTask(value)),
    children: c.array(dto["children"], (value) => encodeDtoApiTask(value)),
    edges: c.array(dto["edges"], (value) => encodeDtoApiDependencyEdge(value)),
  })
}
export function decodeDtoApiDependencies(wire: d.DtoApiDependencies): Record<string, unknown> {
  return c.omitUndefined({
    "task": decodeDtoApiDependencyTask(c.required(wire.task, "task")),
    "parents": wire.parents.map((value) => decodeDtoApiTask(value)),
    "children": wire.children.map((value) => decodeDtoApiTask(value)),
    "edges": wire.edges.map((value) => decodeDtoApiDependencyEdge(value)),
  })
}

export function encodeDtoApiDependencyEdge(value: unknown): d.DtoApiDependencyEdge {
  const dto = c.record(value, ["parent","child"])
  return create(d.DtoApiDependencyEdgeSchema, {
    parent: encodeDtoApiDependencyTask(dto["parent"]),
    child: encodeDtoApiDependencyTask(dto["child"]),
  })
}
export function decodeDtoApiDependencyEdge(wire: d.DtoApiDependencyEdge): Record<string, unknown> {
  return c.omitUndefined({
    "parent": decodeDtoApiDependencyTask(c.required(wire.parent, "parent")),
    "child": decodeDtoApiDependencyTask(c.required(wire.child, "child")),
  })
}

export function encodeDtoApiDependencyTask(value: unknown): d.DtoApiDependencyTask {
  const dto = c.record(value, ["id","board_id","board_slug","ref","title","status"])
  return create(d.DtoApiDependencyTaskSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    boardSlug: c.text(dto["board_slug"]),
    taskRef: c.text(dto["ref"]),
    title: c.text(dto["title"]),
    status: encodeDtoApiTaskStatus(dto["status"]),
  })
}
export function decodeDtoApiDependencyTask(wire: d.DtoApiDependencyTask): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "board_slug": c.required(wire.boardSlug, "board_slug"),
    "ref": c.required(wire.taskRef, "task_ref"),
    "title": c.required(wire.title, "title"),
    "status": decodeDtoApiTaskStatus(c.required(wire.status, "status")),
  })
}

const DtoApiErrorCodeNames = {
  "not_found": d.DtoApiErrorCode.NOT_FOUND,
  "conflict": d.DtoApiErrorCode.CONFLICT,
  "idempotency_conflict": d.DtoApiErrorCode.IDEMPOTENCY_CONFLICT,
  "dependency_cycle": d.DtoApiErrorCode.DEPENDENCY_CYCLE,
  "invalid_input": d.DtoApiErrorCode.INVALID_INPUT,
  "feature_not_available": d.DtoApiErrorCode.FEATURE_NOT_AVAILABLE,
  "server_unavailable": d.DtoApiErrorCode.SERVER_UNAVAILABLE,
  "execution_plan_required": d.DtoApiErrorCode.EXECUTION_PLAN_REQUIRED,
  "steps_incomplete": d.DtoApiErrorCode.STEPS_INCOMPLETE,
  "claim_token_mismatch": d.DtoApiErrorCode.CLAIM_TOKEN_MISMATCH,
  "dependency_blocked": d.DtoApiErrorCode.DEPENDENCY_BLOCKED,
  "claim_conflict": d.DtoApiErrorCode.CLAIM_CONFLICT,
  "invalid_transition": d.DtoApiErrorCode.INVALID_TRANSITION,
  "internal": d.DtoApiErrorCode.INTERNAL,
} as const
export function encodeDtoApiErrorCode(value: unknown): d.DtoApiErrorCode { return c.enumValue(value, DtoApiErrorCodeNames) }
export function decodeDtoApiErrorCode(value: d.DtoApiErrorCode): string { return c.enumName(value, DtoApiErrorCodeNames) }

export function encodeDtoApiExecutionPlan(value: unknown): d.DtoApiExecutionPlan {
  const dto = c.record(value, ["board_id","task_id","state","reason","updated_by","updated_at"])
  return create(d.DtoApiExecutionPlanSchema, {
    boardId: c.text(dto["board_id"]),
    taskId: c.text(dto["task_id"]),
    state: encodeDtoApiExecutionPlanState(dto["state"]),
    reason: c.optional(dto["reason"], (value) => c.text(value)),
    updatedBy: c.text(dto["updated_by"]),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoApiExecutionPlan(wire: d.DtoApiExecutionPlan): Record<string, unknown> {
  return c.omitUndefined({
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": c.required(wire.taskId, "task_id"),
    "state": decodeDtoApiExecutionPlanState(c.required(wire.state, "state")),
    "reason": wire.reason === undefined ? null : ((value) => value)(wire.reason),
    "updated_by": c.required(wire.updatedBy, "updated_by"),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

const DtoApiExecutionPlanStateNames = {
  "unplanned": d.DtoApiExecutionPlanState.UNPLANNED,
  "planned": d.DtoApiExecutionPlanState.PLANNED,
  "not_required": d.DtoApiExecutionPlanState.NOT_REQUIRED,
} as const
export function encodeDtoApiExecutionPlanState(value: unknown): d.DtoApiExecutionPlanState { return c.enumValue(value, DtoApiExecutionPlanStateNames) }
export function decodeDtoApiExecutionPlanState(value: d.DtoApiExecutionPlanState): string { return c.enumName(value, DtoApiExecutionPlanStateNames) }

export function encodeDtoApiLabel(value: unknown): d.DtoApiLabel {
  const dto = c.record(value, ["id","board_id","name","color","created_at","updated_at"])
  return create(d.DtoApiLabelSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    name: c.text(dto["name"]),
    color: c.optional(dto["color"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoApiLabel(wire: d.DtoApiLabel): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "name": c.required(wire.name, "name"),
    "color": wire.color === undefined ? null : ((value) => value)(wire.color),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

export function encodeDtoApiRelation(value: unknown): d.DtoApiRelation {
  const dto = c.record(value, ["subject_uri","predicate","object_uri","graph_uri","provenance","metadata","created_at","updated_at"])
  return create(d.DtoApiRelationSchema, {
    subjectUri: c.text(dto["subject_uri"]),
    predicate: c.text(dto["predicate"]),
    objectUri: c.text(dto["object_uri"]),
    graphUri: c.text(dto["graph_uri"]),
    provenance: encodeDtoApiRelationProvenance(dto["provenance"]),
    metadata: c.encodeJson(dto["metadata"]),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoApiRelation(wire: d.DtoApiRelation): Record<string, unknown> {
  return c.omitUndefined({
    "subject_uri": c.required(wire.subjectUri, "subject_uri"),
    "predicate": c.required(wire.predicate, "predicate"),
    "object_uri": c.required(wire.objectUri, "object_uri"),
    "graph_uri": c.required(wire.graphUri, "graph_uri"),
    "provenance": decodeDtoApiRelationProvenance(c.required(wire.provenance, "provenance")),
    "metadata": c.decodeJson(c.required(wire.metadata, "metadata")),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

export function encodeDtoApiRelationProvenance(value: unknown): d.DtoApiRelationProvenance {
  const dto = c.record(value, ["source_table","source_id","source_event_id","authoritative_store"])
  return create(d.DtoApiRelationProvenanceSchema, {
    sourceTable: c.optional(dto["source_table"], (value) => c.text(value)),
    sourceId: c.optional(dto["source_id"], (value) => c.text(value)),
    sourceEventId: c.optional(dto["source_event_id"], (value) => c.int64(value)),
    authoritativeStore: c.text(dto["authoritative_store"]),
  })
}
export function decodeDtoApiRelationProvenance(wire: d.DtoApiRelationProvenance): Record<string, unknown> {
  return c.omitUndefined({
    "source_table": wire.sourceTable === undefined ? null : ((value) => value)(wire.sourceTable),
    "source_id": wire.sourceId === undefined ? null : ((value) => value)(wire.sourceId),
    "source_event_id": wire.sourceEventId === undefined ? null : ((value) => c.safeNumber(value))(wire.sourceEventId),
    "authoritative_store": c.required(wire.authoritativeStore, "authoritative_store"),
  })
}

export function encodeDtoApiRun(value: unknown): d.DtoApiRun {
  const dto = c.record(value, ["id","task_id","status","worker_profile","worker_pid","claim_owner","started_at","finished_at","exit_code","summary","error","has_log","metadata"])
  return create(d.DtoApiRunSchema, {
    id: c.text(dto["id"]),
    taskId: c.text(dto["task_id"]),
    status: encodeDtoApiRunStatus(dto["status"]),
    workerProfile: c.optional(dto["worker_profile"], (value) => c.text(value)),
    workerPid: c.optional(dto["worker_pid"], (value) => c.int64(value)),
    claimOwner: c.text(dto["claim_owner"]),
    startedAt: c.int64(dto["started_at"]),
    finishedAt: c.optional(dto["finished_at"], (value) => c.int64(value)),
    exitCode: c.optional(dto["exit_code"], (value) => c.int64(value)),
    summary: c.optional(dto["summary"], (value) => c.text(value)),
    error: c.optional(dto["error"], (value) => c.text(value)),
    hasLog: c.bool(dto["has_log"]),
    metadata: c.encodeJson(dto["metadata"]),
  })
}
export function decodeDtoApiRun(wire: d.DtoApiRun): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "task_id": c.required(wire.taskId, "task_id"),
    "status": decodeDtoApiRunStatus(c.required(wire.status, "status")),
    "worker_profile": wire.workerProfile === undefined ? null : ((value) => value)(wire.workerProfile),
    "worker_pid": wire.workerPid === undefined ? null : ((value) => c.safeNumber(value))(wire.workerPid),
    "claim_owner": c.required(wire.claimOwner, "claim_owner"),
    "started_at": c.safeNumber(c.required(wire.startedAt, "started_at")),
    "finished_at": wire.finishedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.finishedAt),
    "exit_code": wire.exitCode === undefined ? null : ((value) => c.safeNumber(value))(wire.exitCode),
    "summary": wire.summary === undefined ? null : ((value) => value)(wire.summary),
    "error": wire.error === undefined ? null : ((value) => value)(wire.error),
    "has_log": c.required(wire.hasLog, "has_log"),
    "metadata": c.decodeJson(c.required(wire.metadata, "metadata")),
  })
}

export function encodeDtoApiRunLog(value: unknown): d.DtoApiRunLog {
  const dto = c.record(value, ["run_id","content","truncated"])
  return create(d.DtoApiRunLogSchema, {
    runId: c.text(dto["run_id"]),
    content: c.text(dto["content"]),
    truncated: c.bool(dto["truncated"]),
  })
}
export function decodeDtoApiRunLog(wire: d.DtoApiRunLog): Record<string, unknown> {
  return c.omitUndefined({
    "run_id": c.required(wire.runId, "run_id"),
    "content": c.required(wire.content, "content"),
    "truncated": c.required(wire.truncated, "truncated"),
  })
}

const DtoApiRunStatusNames = {
  "running": d.DtoApiRunStatus.RUNNING,
  "succeeded": d.DtoApiRunStatus.SUCCEEDED,
  "failed": d.DtoApiRunStatus.FAILED,
  "canceled": d.DtoApiRunStatus.CANCELED,
  "expired": d.DtoApiRunStatus.EXPIRED,
} as const
export function encodeDtoApiRunStatus(value: unknown): d.DtoApiRunStatus { return c.enumValue(value, DtoApiRunStatusNames) }
export function decodeDtoApiRunStatus(value: d.DtoApiRunStatus): string { return c.enumName(value, DtoApiRunStatusNames) }

const DtoApiStepStatusNames = {
  "todo": d.DtoApiStepStatus.TODO,
  "done": d.DtoApiStepStatus.DONE,
  "skipped": d.DtoApiStepStatus.SKIPPED,
} as const
export function encodeDtoApiStepStatus(value: unknown): d.DtoApiStepStatus { return c.enumValue(value, DtoApiStepStatusNames) }
export function decodeDtoApiStepStatus(value: d.DtoApiStepStatus): string { return c.enumName(value, DtoApiStepStatusNames) }

export function encodeDtoApiTask(value: unknown): d.DtoApiTask {
  const dto = c.record(value, ["id","board_id","board_slug","ref","seq","title","description","status","status_reason","assignee","priority","position","scheduled_at","due_at","created_by","created_at","updated_at","started_at","completed_at","archived_at","claim_owner","claim_expires_at","last_heartbeat_at","current_run_id","retry_count","max_retries","result_summary","result","metadata","lock_version","dependency_blocked","unfinished_parent_count","execution_plan_state","required_step_count","completed_required_step_count","optional_step_count","labels"])
  return create(d.DtoApiTaskSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    boardSlug: c.text(dto["board_slug"]),
    taskRef: c.text(dto["ref"]),
    seq: c.int64(dto["seq"]),
    title: c.text(dto["title"]),
    description: c.optional(dto["description"], (value) => c.text(value)),
    status: encodeDtoApiTaskStatus(dto["status"]),
    statusReason: c.optional(dto["status_reason"], (value) => c.text(value)),
    assignee: c.optional(dto["assignee"], (value) => c.text(value)),
    priority: encodeDtoApiTaskPriority(dto["priority"]),
    position: c.int64(dto["position"]),
    scheduledAt: c.optional(dto["scheduled_at"], (value) => c.int64(value)),
    dueAt: c.optional(dto["due_at"], (value) => c.int64(value)),
    createdBy: c.text(dto["created_by"]),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    startedAt: c.optional(dto["started_at"], (value) => c.int64(value)),
    completedAt: c.optional(dto["completed_at"], (value) => c.int64(value)),
    archivedAt: c.optional(dto["archived_at"], (value) => c.int64(value)),
    claimOwner: c.optional(dto["claim_owner"], (value) => c.text(value)),
    claimExpiresAt: c.optional(dto["claim_expires_at"], (value) => c.int64(value)),
    lastHeartbeatAt: c.optional(dto["last_heartbeat_at"], (value) => c.int64(value)),
    currentRunId: c.optional(dto["current_run_id"], (value) => c.text(value)),
    retryCount: c.int64(dto["retry_count"]),
    maxRetries: c.optional(dto["max_retries"], (value) => c.int64(value)),
    resultSummary: c.optional(dto["result_summary"], (value) => c.text(value)),
    result: c.optional(dto["result"], (value) => c.encodeJson(value)),
    metadata: c.encodeJson(dto["metadata"]),
    lockVersion: c.int64(dto["lock_version"]),
    dependencyBlocked: c.bool(dto["dependency_blocked"]),
    unfinishedParentCount: c.int64(dto["unfinished_parent_count"]),
    executionPlanState: encodeDtoApiExecutionPlanState(dto["execution_plan_state"]),
    requiredStepCount: c.int64(dto["required_step_count"]),
    completedRequiredStepCount: c.int64(dto["completed_required_step_count"]),
    optionalStepCount: c.int64(dto["optional_step_count"]),
    labels: c.array(dto["labels"], (value) => encodeDtoApiLabel(value)),
  })
}
export function decodeDtoApiTask(wire: d.DtoApiTask): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "board_slug": c.required(wire.boardSlug, "board_slug"),
    "ref": c.required(wire.taskRef, "task_ref"),
    "seq": c.safeNumber(c.required(wire.seq, "seq")),
    "title": c.required(wire.title, "title"),
    "description": wire.description === undefined ? null : ((value) => value)(wire.description),
    "status": decodeDtoApiTaskStatus(c.required(wire.status, "status")),
    "status_reason": wire.statusReason === undefined ? null : ((value) => value)(wire.statusReason),
    "assignee": wire.assignee === undefined ? null : ((value) => value)(wire.assignee),
    "priority": decodeDtoApiTaskPriority(c.required(wire.priority, "priority")),
    "position": c.safeNumber(c.required(wire.position, "position")),
    "scheduled_at": wire.scheduledAt === undefined ? null : ((value) => c.safeNumber(value))(wire.scheduledAt),
    "due_at": wire.dueAt === undefined ? null : ((value) => c.safeNumber(value))(wire.dueAt),
    "created_by": c.required(wire.createdBy, "created_by"),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "started_at": wire.startedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.startedAt),
    "completed_at": wire.completedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.completedAt),
    "archived_at": wire.archivedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.archivedAt),
    "claim_owner": wire.claimOwner === undefined ? null : ((value) => value)(wire.claimOwner),
    "claim_expires_at": wire.claimExpiresAt === undefined ? null : ((value) => c.safeNumber(value))(wire.claimExpiresAt),
    "last_heartbeat_at": wire.lastHeartbeatAt === undefined ? null : ((value) => c.safeNumber(value))(wire.lastHeartbeatAt),
    "current_run_id": wire.currentRunId === undefined ? null : ((value) => value)(wire.currentRunId),
    "retry_count": c.safeNumber(c.required(wire.retryCount, "retry_count")),
    "max_retries": wire.maxRetries === undefined ? null : ((value) => c.safeNumber(value))(wire.maxRetries),
    "result_summary": wire.resultSummary === undefined ? null : ((value) => value)(wire.resultSummary),
    "result": wire.result === undefined ? null : ((value) => c.decodeJson(value))(wire.result),
    "metadata": c.decodeJson(c.required(wire.metadata, "metadata")),
    "lock_version": c.safeNumber(c.required(wire.lockVersion, "lock_version")),
    "dependency_blocked": c.required(wire.dependencyBlocked, "dependency_blocked"),
    "unfinished_parent_count": c.safeNumber(c.required(wire.unfinishedParentCount, "unfinished_parent_count")),
    "execution_plan_state": decodeDtoApiExecutionPlanState(c.required(wire.executionPlanState, "execution_plan_state")),
    "required_step_count": c.safeNumber(c.required(wire.requiredStepCount, "required_step_count")),
    "completed_required_step_count": c.safeNumber(c.required(wire.completedRequiredStepCount, "completed_required_step_count")),
    "optional_step_count": c.safeNumber(c.required(wire.optionalStepCount, "optional_step_count")),
    "labels": wire.labels.map((value) => decodeDtoApiLabel(value)),
  })
}

const DtoApiTaskGraphEdgeKindNames = {
  "dependency": d.DtoApiTaskGraphEdgeKind.DEPENDENCY,
  "step": d.DtoApiTaskGraphEdgeKind.STEP,
} as const
export function encodeDtoApiTaskGraphEdgeKind(value: unknown): d.DtoApiTaskGraphEdgeKind { return c.enumValue(value, DtoApiTaskGraphEdgeKindNames) }
export function decodeDtoApiTaskGraphEdgeKind(value: d.DtoApiTaskGraphEdgeKind): string { return c.enumName(value, DtoApiTaskGraphEdgeKindNames) }

const DtoApiTaskGraphNodeRoleNames = {
  "center": d.DtoApiTaskGraphNodeRole.CENTER,
  "dependency_parent": d.DtoApiTaskGraphNodeRole.DEPENDENCY_PARENT,
  "dependency_child": d.DtoApiTaskGraphNodeRole.DEPENDENCY_CHILD,
  "step_parent": d.DtoApiTaskGraphNodeRole.STEP_PARENT,
  "step_child": d.DtoApiTaskGraphNodeRole.STEP_CHILD,
  "active": d.DtoApiTaskGraphNodeRole.ACTIVE,
  "context": d.DtoApiTaskGraphNodeRole.CONTEXT,
} as const
export function encodeDtoApiTaskGraphNodeRole(value: unknown): d.DtoApiTaskGraphNodeRole { return c.enumValue(value, DtoApiTaskGraphNodeRoleNames) }
export function decodeDtoApiTaskGraphNodeRole(value: d.DtoApiTaskGraphNodeRole): string { return c.enumName(value, DtoApiTaskGraphNodeRoleNames) }

export function encodeDtoApiTaskPriority(value: unknown): d.DtoApiTaskPriority {
  value = c.taskPriority(value)
  return create(d.DtoApiTaskPrioritySchema, { value: c.int32(value, true, 255) })
}
export function decodeDtoApiTaskPriority(wire: d.DtoApiTaskPriority): unknown {
  return c.taskPriority(c.required(wire.value, "0"))
}

const DtoApiTaskStatusNames = {
  "triage": d.DtoApiTaskStatus.TRIAGE,
  "todo": d.DtoApiTaskStatus.TODO,
  "scheduled": d.DtoApiTaskStatus.SCHEDULED,
  "ready": d.DtoApiTaskStatus.READY,
  "running": d.DtoApiTaskStatus.RUNNING,
  "blocked": d.DtoApiTaskStatus.BLOCKED,
  "review": d.DtoApiTaskStatus.REVIEW,
  "done": d.DtoApiTaskStatus.DONE,
  "archived": d.DtoApiTaskStatus.ARCHIVED,
} as const
export function encodeDtoApiTaskStatus(value: unknown): d.DtoApiTaskStatus { return c.enumValue(value, DtoApiTaskStatusNames) }
export function decodeDtoApiTaskStatus(value: d.DtoApiTaskStatus): string { return c.enumName(value, DtoApiTaskStatusNames) }

export function encodeDtoApiTaskStep(value: unknown): d.DtoApiTaskStep {
  const dto = c.record(value, ["id","parent_task_id","title","body","linked_task","position","required","status","resolution_note","resolved_by","resolved_at","created_by","created_at","updated_by","updated_at"])
  return create(d.DtoApiTaskStepSchema, {
    id: c.text(dto["id"]),
    parentTaskId: c.text(dto["parent_task_id"]),
    title: c.text(dto["title"]),
    body: c.optional(dto["body"], (value) => c.text(value)),
    linkedTask: c.optional(dto["linked_task"], (value) => encodeDtoApiTask(value)),
    position: c.int64(dto["position"]),
    required: c.bool(dto["required"]),
    status: encodeDtoApiStepStatus(dto["status"]),
    resolutionNote: c.optional(dto["resolution_note"], (value) => c.text(value)),
    resolvedBy: c.optional(dto["resolved_by"], (value) => c.text(value)),
    resolvedAt: c.optional(dto["resolved_at"], (value) => c.int64(value)),
    createdBy: c.text(dto["created_by"]),
    createdAt: c.int64(dto["created_at"]),
    updatedBy: c.text(dto["updated_by"]),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoApiTaskStep(wire: d.DtoApiTaskStep): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "parent_task_id": c.required(wire.parentTaskId, "parent_task_id"),
    "title": c.required(wire.title, "title"),
    "body": wire.body === undefined ? null : ((value) => value)(wire.body),
    "linked_task": wire.linkedTask === undefined ? null : ((value) => decodeDtoApiTask(value))(wire.linkedTask),
    "position": c.safeNumber(c.required(wire.position, "position")),
    "required": c.required(wire.required, "required"),
    "status": decodeDtoApiStepStatus(c.required(wire.status, "status")),
    "resolution_note": wire.resolutionNote === undefined ? null : ((value) => value)(wire.resolutionNote),
    "resolved_by": wire.resolvedBy === undefined ? null : ((value) => value)(wire.resolvedBy),
    "resolved_at": wire.resolvedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.resolvedAt),
    "created_by": c.required(wire.createdBy, "created_by"),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_by": c.required(wire.updatedBy, "updated_by"),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

export function encodeDtoApiTaskSteps(value: unknown): d.DtoApiTaskSteps {
  const dto = c.record(value, ["task_id","steps","execution_plan"])
  return create(d.DtoApiTaskStepsSchema, {
    taskId: c.text(dto["task_id"]),
    steps: c.array(dto["steps"], (value) => encodeDtoApiTaskStep(value)),
    executionPlan: encodeDtoApiExecutionPlan(dto["execution_plan"]),
  })
}
export function decodeDtoApiTaskSteps(wire: d.DtoApiTaskSteps): Record<string, unknown> {
  return c.omitUndefined({
    "task_id": c.required(wire.taskId, "task_id"),
    "steps": wire.steps.map((value) => decodeDtoApiTaskStep(value)),
    "execution_plan": decodeDtoApiExecutionPlan(c.required(wire.executionPlan, "execution_plan")),
  })
}

export function encodeDtoBackupReport(value: unknown): d.DtoBackupReport {
  const dto = c.record(value, ["out_path","checksum_sha256","bytes","source_fingerprint"])
  return create(d.DtoBackupReportSchema, {
    outPath: c.text(dto["out_path"]),
    checksumSha256: c.text(dto["checksum_sha256"]),
    bytes: c.int64(dto["bytes"], true),
    sourceFingerprint: c.text(dto["source_fingerprint"]),
  })
}
export function decodeDtoBackupReport(wire: d.DtoBackupReport): Record<string, unknown> {
  return c.omitUndefined({
    "out_path": c.required(wire.outPath, "out_path"),
    "checksum_sha256": c.required(wire.checksumSha256, "checksum_sha256"),
    "bytes": c.safeNumber(c.required(wire.bytes, "bytes")),
    "source_fingerprint": c.required(wire.sourceFingerprint, "source_fingerprint"),
  })
}

export function encodeDtoBlockedReasonCount(value: unknown): d.DtoBlockedReasonCount {
  const dto = c.record(value, ["reason","count"])
  return create(d.DtoBlockedReasonCountSchema, {
    reason: c.text(dto["reason"]),
    count: c.int64(dto["count"]),
  })
}
export function decodeDtoBlockedReasonCount(wire: d.DtoBlockedReasonCount): Record<string, unknown> {
  return c.omitUndefined({
    "reason": c.required(wire.reason, "reason"),
    "count": c.safeNumber(c.required(wire.count, "count")),
  })
}

export function encodeDtoBoardCreatedPayload(value: unknown): d.DtoBoardCreatedPayload {
  const dto = c.record(value, ["slug"])
  return create(d.DtoBoardCreatedPayloadSchema, {
    slug: c.text(dto["slug"]),
  })
}
export function decodeDtoBoardCreatedPayload(wire: d.DtoBoardCreatedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "slug": c.required(wire.slug, "slug"),
  })
}

export function encodeDtoBoardTaskMap(value: unknown): d.DtoBoardTaskMap {
  const dto = c.record(value, ["nodes","edges","meta"])
  return create(d.DtoBoardTaskMapSchema, {
    nodes: c.array(dto["nodes"], (value) => encodeDtoTaskGraphNode(value)),
    edges: c.array(dto["edges"], (value) => encodeDtoTaskGraphEdge(value)),
    meta: encodeDtoTaskGraphMeta(dto["meta"]),
  })
}
export function decodeDtoBoardTaskMap(wire: d.DtoBoardTaskMap): Record<string, unknown> {
  return c.omitUndefined({
    "nodes": wire.nodes.map((value) => decodeDtoTaskGraphNode(value)),
    "edges": wire.edges.map((value) => decodeDtoTaskGraphEdge(value)),
    "meta": decodeDtoTaskGraphMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeDtoBootstrapTaskLabelData(value: unknown): d.DtoBootstrapTaskLabelData {
  const dto = c.record(value, ["task","semantics","verification"])
  return create(d.DtoBootstrapTaskLabelDataSchema, {
    task: encodeDtoApiTask(dto["task"]),
    semantics: encodeDtoLabelSemanticsWire(dto["semantics"]),
    verification: c.optional(dto["verification"], (value) => encodeDtoBootstrapTaskLabelVerification(value)),
  })
}
export function decodeDtoBootstrapTaskLabelData(wire: d.DtoBootstrapTaskLabelData): Record<string, unknown> {
  return c.omitUndefined({
    "task": decodeDtoApiTask(c.required(wire.task, "task")),
    "semantics": decodeDtoLabelSemanticsWire(c.required(wire.semantics, "semantics")),
    "verification": wire.verification === undefined ? undefined : ((value) => decodeDtoBootstrapTaskLabelVerification(value))(wire.verification),
  })
}

export function encodeDtoBootstrapTaskLabelVerification(value: unknown): d.DtoBootstrapTaskLabelVerification {
  const dto = c.record(value, ["label_name","score","source","min_score","degraded","diagnostics"])
  return create(d.DtoBootstrapTaskLabelVerificationSchema, {
    labelName: c.text(dto["label_name"]),
    score: c.float(dto["score"], true),
    source: c.text(dto["source"]),
    minScore: c.float(dto["min_score"], true),
    degraded: c.bool(dto["degraded"]),
    diagnostics: c.array(dto["diagnostics"], (value) => c.text(value)),
  })
}
export function decodeDtoBootstrapTaskLabelVerification(wire: d.DtoBootstrapTaskLabelVerification): Record<string, unknown> {
  return c.omitUndefined({
    "label_name": c.required(wire.labelName, "label_name"),
    "score": c.float(c.required(wire.score, "score")),
    "source": c.required(wire.source, "source"),
    "min_score": c.float(c.required(wire.minScore, "min_score")),
    "degraded": c.required(wire.degraded, "degraded"),
    "diagnostics": wire.diagnostics.map((value) => value),
  })
}

export function encodeDtoCheckpointReport(value: unknown): d.DtoCheckpointReport {
  const dto = c.record(value, ["busy","log_frames","checkpointed_frames"])
  return create(d.DtoCheckpointReportSchema, {
    busy: c.int64(dto["busy"]),
    logFrames: c.int64(dto["log_frames"]),
    checkpointedFrames: c.int64(dto["checkpointed_frames"]),
  })
}
export function decodeDtoCheckpointReport(wire: d.DtoCheckpointReport): Record<string, unknown> {
  return c.omitUndefined({
    "busy": c.safeNumber(c.required(wire.busy, "busy")),
    "log_frames": c.safeNumber(c.required(wire.logFrames, "log_frames")),
    "checkpointed_frames": c.safeNumber(c.required(wire.checkpointedFrames, "checkpointed_frames")),
  })
}

export function encodeDtoCliEntity(value: unknown): d.DtoCliEntity {
  const dto = c.record(value, ["uri","kind","source_table","source_id","board_id","task_id","title","summary","content_hash","created_at","updated_at","archived_at"])
  return create(d.DtoCliEntitySchema, {
    uri: c.text(dto["uri"]),
    kind: c.text(dto["kind"]),
    sourceTable: c.text(dto["source_table"]),
    sourceId: c.text(dto["source_id"]),
    boardId: c.optional(dto["board_id"], (value) => c.text(value)),
    taskId: c.optional(dto["task_id"], (value) => c.text(value)),
    title: c.optional(dto["title"], (value) => c.text(value)),
    summary: c.optional(dto["summary"], (value) => c.text(value)),
    contentHash: c.optional(dto["content_hash"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    archivedAt: c.optional(dto["archived_at"], (value) => c.int64(value)),
  })
}
export function decodeDtoCliEntity(wire: d.DtoCliEntity): Record<string, unknown> {
  return c.omitUndefined({
    "uri": c.required(wire.uri, "uri"),
    "kind": c.required(wire.kind, "kind"),
    "source_table": c.required(wire.sourceTable, "source_table"),
    "source_id": c.required(wire.sourceId, "source_id"),
    "board_id": wire.boardId === undefined ? null : ((value) => value)(wire.boardId),
    "task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),
    "title": wire.title === undefined ? null : ((value) => value)(wire.title),
    "summary": wire.summary === undefined ? null : ((value) => value)(wire.summary),
    "content_hash": wire.contentHash === undefined ? null : ((value) => value)(wire.contentHash),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "archived_at": wire.archivedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.archivedAt),
  })
}

export function encodeDtoCliGraphQueryBinding(value: unknown): d.DtoCliGraphQueryBinding {
  const dto = c.record(value, ["name","value"])
  return create(d.DtoCliGraphQueryBindingSchema, {
    name: c.text(dto["name"]),
    value: c.text(dto["value"]),
  })
}
export function decodeDtoCliGraphQueryBinding(wire: d.DtoCliGraphQueryBinding): Record<string, unknown> {
  return c.omitUndefined({
    "name": c.required(wire.name, "name"),
    "value": c.required(wire.value, "value"),
  })
}

export function encodeDtoCliGraphQueryRow(value: unknown): d.DtoCliGraphQueryRow {
  const dto = c.record(value, ["bindings"])
  return create(d.DtoCliGraphQueryRowSchema, {
    bindings: c.array(dto["bindings"], (value) => encodeDtoCliGraphQueryBinding(value)),
  })
}
export function decodeDtoCliGraphQueryRow(wire: d.DtoCliGraphQueryRow): Record<string, unknown> {
  return c.omitUndefined({
    "bindings": wire.bindings.map((value) => decodeDtoCliGraphQueryBinding(value)),
  })
}

export function encodeDtoCliLabelOntologyPrecisionRecall(value: unknown): d.DtoCliLabelOntologyPrecisionRecall {
  const dto = c.record(value, ["available","reason"])
  return create(d.DtoCliLabelOntologyPrecisionRecallSchema, {
    available: c.bool(dto["available"]),
    reason: c.text(dto["reason"]),
  })
}
export function decodeDtoCliLabelOntologyPrecisionRecall(wire: d.DtoCliLabelOntologyPrecisionRecall): Record<string, unknown> {
  return c.omitUndefined({
    "available": c.required(wire.available, "available"),
    "reason": c.required(wire.reason, "reason"),
  })
}

export function encodeDtoCliLabelOntologyQuality(value: unknown): d.DtoCliLabelOntologyQuality {
  const dto = c.record(value, ["board_id","denominator","disagreement","rates","precision_recall","warnings"])
  return create(d.DtoCliLabelOntologyQualitySchema, {
    boardId: c.text(dto["board_id"]),
    denominator: encodeDtoCliLabelOntologyQualityDenominator(dto["denominator"]),
    disagreement: encodeDtoCliLabelOntologyQualityDisagreement(dto["disagreement"]),
    rates: encodeDtoCliLabelOntologyQualityRates(dto["rates"]),
    precisionRecall: encodeDtoCliLabelOntologyPrecisionRecall(dto["precision_recall"]),
    warnings: c.array(dto["warnings"], (value) => c.text(value)),
  })
}
export function decodeDtoCliLabelOntologyQuality(wire: d.DtoCliLabelOntologyQuality): Record<string, unknown> {
  return c.omitUndefined({
    "board_id": c.required(wire.boardId, "board_id"),
    "denominator": decodeDtoCliLabelOntologyQualityDenominator(c.required(wire.denominator, "denominator")),
    "disagreement": decodeDtoCliLabelOntologyQualityDisagreement(c.required(wire.disagreement, "disagreement")),
    "rates": decodeDtoCliLabelOntologyQualityRates(c.required(wire.rates, "rates")),
    "precision_recall": decodeDtoCliLabelOntologyPrecisionRecall(c.required(wire.precisionRecall, "precision_recall")),
    "warnings": wire.warnings.map((value) => value),
  })
}

export function encodeDtoCliLabelOntologyQualityDenominator(value: unknown): d.DtoCliLabelOntologyQualityDenominator {
  const dto = c.record(value, ["source","description","observation_count","distinct_task_count","agreement_observation_count","agreement_task_count","degraded_observation_count","first_observed_at","latest_observed_at","sample_task_refs"])
  return create(d.DtoCliLabelOntologyQualityDenominatorSchema, {
    source: c.text(dto["source"]),
    description: c.text(dto["description"]),
    observationCount: c.int64(dto["observation_count"]),
    distinctTaskCount: c.int64(dto["distinct_task_count"]),
    agreementObservationCount: c.int64(dto["agreement_observation_count"]),
    agreementTaskCount: c.int64(dto["agreement_task_count"]),
    degradedObservationCount: c.int64(dto["degraded_observation_count"]),
    firstObservedAt: c.optional(dto["first_observed_at"], (value) => c.int64(value)),
    latestObservedAt: c.optional(dto["latest_observed_at"], (value) => c.int64(value)),
    sampleTaskRefs: c.array(dto["sample_task_refs"], (value) => c.text(value)),
  })
}
export function decodeDtoCliLabelOntologyQualityDenominator(wire: d.DtoCliLabelOntologyQualityDenominator): Record<string, unknown> {
  return c.omitUndefined({
    "source": c.required(wire.source, "source"),
    "description": c.required(wire.description, "description"),
    "observation_count": c.safeNumber(c.required(wire.observationCount, "observation_count")),
    "distinct_task_count": c.safeNumber(c.required(wire.distinctTaskCount, "distinct_task_count")),
    "agreement_observation_count": c.safeNumber(c.required(wire.agreementObservationCount, "agreement_observation_count")),
    "agreement_task_count": c.safeNumber(c.required(wire.agreementTaskCount, "agreement_task_count")),
    "degraded_observation_count": c.safeNumber(c.required(wire.degradedObservationCount, "degraded_observation_count")),
    "first_observed_at": wire.firstObservedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.firstObservedAt),
    "latest_observed_at": wire.latestObservedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.latestObservedAt),
    "sample_task_refs": wire.sampleTaskRefs.map((value) => value),
  })
}

export function encodeDtoCliLabelOntologyQualityDisagreement(value: unknown): d.DtoCliLabelOntologyQualityDisagreement {
  const dto = c.record(value, ["signal_count","distinct_task_count","by_kind","by_status"])
  return create(d.DtoCliLabelOntologyQualityDisagreementSchema, {
    signalCount: c.int64(dto["signal_count"]),
    distinctTaskCount: c.int64(dto["distinct_task_count"]),
    byKind: c.dictionary(dto["by_kind"], (value) => c.int64(value)),
    byStatus: c.dictionary(dto["by_status"], (value) => c.int64(value)),
  })
}
export function decodeDtoCliLabelOntologyQualityDisagreement(wire: d.DtoCliLabelOntologyQualityDisagreement): Record<string, unknown> {
  return c.omitUndefined({
    "signal_count": c.safeNumber(c.required(wire.signalCount, "signal_count")),
    "distinct_task_count": c.safeNumber(c.required(wire.distinctTaskCount, "distinct_task_count")),
    "by_kind": Object.fromEntries(Object.entries(wire.byKind).map(([key, value]) => [key, c.safeNumber(value)])),
    "by_status": Object.fromEntries(Object.entries(wire.byStatus).map(([key, value]) => [key, c.safeNumber(value)])),
  })
}

export function encodeDtoCliLabelOntologyQualityRates(value: unknown): d.DtoCliLabelOntologyQualityRates {
  const dto = c.record(value, ["disagreement_task_rate","disagreement_task_rate_basis"])
  return create(d.DtoCliLabelOntologyQualityRatesSchema, {
    disagreementTaskRate: c.optional(dto["disagreement_task_rate"], (value) => c.float(value)),
    disagreementTaskRateBasis: c.text(dto["disagreement_task_rate_basis"]),
  })
}
export function decodeDtoCliLabelOntologyQualityRates(wire: d.DtoCliLabelOntologyQualityRates): Record<string, unknown> {
  return c.omitUndefined({
    "disagreement_task_rate": wire.disagreementTaskRate === undefined ? null : ((value) => c.float(value))(wire.disagreementTaskRate),
    "disagreement_task_rate_basis": c.required(wire.disagreementTaskRateBasis, "disagreement_task_rate_basis"),
  })
}

const DtoCommentsCommentAuthorTypeNames = {
  "user": d.DtoCommentsCommentAuthorType.USER,
  "agent": d.DtoCommentsCommentAuthorType.AGENT,
} as const
export function encodeDtoCommentsCommentAuthorType(value: unknown): d.DtoCommentsCommentAuthorType { return c.enumValue(value, DtoCommentsCommentAuthorTypeNames) }
export function decodeDtoCommentsCommentAuthorType(value: d.DtoCommentsCommentAuthorType): string { return c.enumName(value, DtoCommentsCommentAuthorTypeNames) }

const DtoCommentsCommentKindNames = {
  "note": d.DtoCommentsCommentKind.NOTE,
  "decision": d.DtoCommentsCommentKind.DECISION,
  "signal": d.DtoCommentsCommentKind.SIGNAL,
} as const
export function encodeDtoCommentsCommentKind(value: unknown): d.DtoCommentsCommentKind { return c.enumValue(value, DtoCommentsCommentKindNames) }
export function decodeDtoCommentsCommentKind(value: d.DtoCommentsCommentKind): string { return c.enumName(value, DtoCommentsCommentKindNames) }

export function encodeDtoContextDiagnostic(value: unknown): d.DtoContextDiagnostic {
  const dto = c.record(value, ["source","code","message"])
  return create(d.DtoContextDiagnosticSchema, {
    source: c.text(dto["source"]),
    code: c.text(dto["code"]),
    message: c.text(dto["message"]),
  })
}
export function decodeDtoContextDiagnostic(wire: d.DtoContextDiagnostic): Record<string, unknown> {
  return c.omitUndefined({
    "source": c.required(wire.source, "source"),
    "code": c.required(wire.code, "code"),
    "message": c.required(wire.message, "message"),
  })
}

export function encodeDtoContextEvidence(value: unknown): d.DtoContextEvidence {
  const dto = c.record(value, ["kind","entity_uri","task_id","relation_id","predicate","summary"])
  return create(d.DtoContextEvidenceSchema, {
    kind: c.text(dto["kind"]),
    entityUri: c.optional(dto["entity_uri"], (value) => c.text(value)),
    taskId: c.optional(dto["task_id"], (value) => c.text(value)),
    relationId: c.optional(dto["relation_id"], (value) => c.text(value)),
    predicate: c.optional(dto["predicate"], (value) => c.text(value)),
    summary: c.optional(dto["summary"], (value) => c.text(value)),
  })
}
export function decodeDtoContextEvidence(wire: d.DtoContextEvidence): Record<string, unknown> {
  return c.omitUndefined({
    "kind": c.required(wire.kind, "kind"),
    "entity_uri": wire.entityUri === undefined ? undefined : ((value) => value)(wire.entityUri),
    "task_id": wire.taskId === undefined ? undefined : ((value) => value)(wire.taskId),
    "relation_id": wire.relationId === undefined ? undefined : ((value) => value)(wire.relationId),
    "predicate": wire.predicate === undefined ? undefined : ((value) => value)(wire.predicate),
    "summary": wire.summary === undefined ? undefined : ((value) => value)(wire.summary),
  })
}

export function encodeDtoContextItem(value: unknown): d.DtoContextItem {
  const dto = c.record(value, ["entity_uri","source","provenance","score","title","snippet","rank","reason","evidence"])
  return create(d.DtoContextItemSchema, {
    entityUri: c.text(dto["entity_uri"]),
    source: c.text(dto["source"]),
    provenance: c.array((dto["provenance"] ?? []), (value) => c.text(value)),
    score: c.optional(dto["score"], (value) => c.float(value)),
    title: c.optional(dto["title"], (value) => c.text(value)),
    snippet: c.optional(dto["snippet"], (value) => c.text(value)),
    rank: c.present(dto["rank"], (value) => c.int64(value, true)),
    reason: c.present(dto["reason"], (value) => c.text(value)),
    evidence: c.array((dto["evidence"] ?? []), (value) => encodeDtoContextEvidence(value)),
  })
}
export function decodeDtoContextItem(wire: d.DtoContextItem): Record<string, unknown> {
  return c.omitUndefined({
    "entity_uri": c.required(wire.entityUri, "entity_uri"),
    "source": c.required(wire.source, "source"),
    "provenance": wire.provenance.map((value) => value),
    "score": wire.score === undefined ? null : ((value) => c.float(value))(wire.score),
    "title": wire.title === undefined ? null : ((value) => value)(wire.title),
    "snippet": wire.snippet === undefined ? null : ((value) => value)(wire.snippet),
    "rank": c.safeNumber(c.required(wire.rank, "rank")),
    "reason": c.required(wire.reason, "reason"),
    "evidence": wire.evidence.map((value) => decodeDtoContextEvidence(value)),
  })
}

export function encodeDtoContextPack(value: unknown): d.DtoContextPack {
  const dto = c.record(value, ["subject","policy","items","degraded","diagnostics","providers","truncated","truncation_reason"])
  return create(d.DtoContextPackSchema, {
    subject: c.text(dto["subject"]),
    policy: encodeDtoContextPolicy(dto["policy"]),
    items: c.array(dto["items"], (value) => encodeDtoContextItem(value)),
    degraded: c.array(dto["degraded"], (value) => c.text(value)),
    diagnostics: c.array((dto["diagnostics"] ?? []), (value) => encodeDtoContextDiagnostic(value)),
    providers: c.array((dto["providers"] ?? []), (value) => encodeDtoContextProviderStatus(value)),
    truncated: c.present(dto["truncated"], (value) => c.bool(value)),
    truncationReason: c.optional(dto["truncation_reason"], (value) => c.text(value)),
  })
}
export function decodeDtoContextPack(wire: d.DtoContextPack): Record<string, unknown> {
  return c.omitUndefined({
    "subject": c.required(wire.subject, "subject"),
    "policy": decodeDtoContextPolicy(c.required(wire.policy, "policy")),
    "items": wire.items.map((value) => decodeDtoContextItem(value)),
    "degraded": wire.degraded.map((value) => value),
    "diagnostics": wire.diagnostics.map((value) => decodeDtoContextDiagnostic(value)),
    "providers": wire.providers.map((value) => decodeDtoContextProviderStatus(value)),
    "truncated": c.required(wire.truncated, "truncated"),
    "truncation_reason": wire.truncationReason === undefined ? undefined : ((value) => value)(wire.truncationReason),
  })
}

export function encodeDtoContextPolicy(value: unknown): d.DtoContextPolicy {
  const dto = c.record(value, ["depth","lexical_limit","graph_limit","vector_limit","max_items","budget"])
  return create(d.DtoContextPolicySchema, {
    depth: c.present(dto["depth"], (value) => c.int64(value, true)),
    lexicalLimit: c.int64(dto["lexical_limit"], true),
    graphLimit: c.int64(dto["graph_limit"], true),
    vectorLimit: c.int64(dto["vector_limit"], true),
    maxItems: c.int64(dto["max_items"], true),
    budget: c.optional(dto["budget"], (value) => c.int64(value, true)),
  })
}
export function decodeDtoContextPolicy(wire: d.DtoContextPolicy): Record<string, unknown> {
  return c.omitUndefined({
    "depth": c.safeNumber(c.required(wire.depth, "depth")),
    "lexical_limit": c.safeNumber(c.required(wire.lexicalLimit, "lexical_limit")),
    "graph_limit": c.safeNumber(c.required(wire.graphLimit, "graph_limit")),
    "vector_limit": c.safeNumber(c.required(wire.vectorLimit, "vector_limit")),
    "max_items": c.safeNumber(c.required(wire.maxItems, "max_items")),
    "budget": wire.budget === undefined ? undefined : ((value) => c.safeNumber(value))(wire.budget),
  })
}

export function encodeDtoContextProviderStatus(value: unknown): d.DtoContextProviderStatus {
  const dto = c.record(value, ["provider","capability","available","degraded","reason"])
  return create(d.DtoContextProviderStatusSchema, {
    provider: c.text(dto["provider"]),
    capability: c.text(dto["capability"]),
    available: c.bool(dto["available"]),
    degraded: c.bool(dto["degraded"]),
    reason: c.optional(dto["reason"], (value) => c.text(value)),
  })
}
export function decodeDtoContextProviderStatus(wire: d.DtoContextProviderStatus): Record<string, unknown> {
  return c.omitUndefined({
    "provider": c.required(wire.provider, "provider"),
    "capability": c.required(wire.capability, "capability"),
    "available": c.required(wire.available, "available"),
    "degraded": c.required(wire.degraded, "degraded"),
    "reason": wire.reason === undefined ? undefined : ((value) => value)(wire.reason),
  })
}

export function encodeDtoCreatedLabelsMetaOfDtoApiLabel(value: unknown): d.DtoCreatedLabelsMetaOfDtoApiLabel {
  const dto = c.record(value, ["created_labels"])
  return create(d.DtoCreatedLabelsMetaOfDtoApiLabelSchema, {
    createdLabels: c.array(dto["created_labels"], (value) => encodeDtoApiLabel(value)),
  })
}
export function decodeDtoCreatedLabelsMetaOfDtoApiLabel(wire: d.DtoCreatedLabelsMetaOfDtoApiLabel): Record<string, unknown> {
  return c.omitUndefined({
    "created_labels": wire.createdLabels.map((value) => decodeDtoApiLabel(value)),
  })
}

export function encodeDtoDeleteBoardLabelResult(value: unknown): d.DtoDeleteBoardLabelResult {
  const dto = c.record(value, ["label","forced","removed_task_bindings","removed_semantics","removed_atoms"])
  return create(d.DtoDeleteBoardLabelResultSchema, {
    label: encodeDtoApiLabel(dto["label"]),
    forced: c.bool(dto["forced"]),
    removedTaskBindings: c.int64(dto["removed_task_bindings"]),
    removedSemantics: c.bool(dto["removed_semantics"]),
    removedAtoms: c.int64(dto["removed_atoms"]),
  })
}
export function decodeDtoDeleteBoardLabelResult(wire: d.DtoDeleteBoardLabelResult): Record<string, unknown> {
  return c.omitUndefined({
    "label": decodeDtoApiLabel(c.required(wire.label, "label")),
    "forced": c.required(wire.forced, "forced"),
    "removed_task_bindings": c.safeNumber(c.required(wire.removedTaskBindings, "removed_task_bindings")),
    "removed_semantics": c.required(wire.removedSemantics, "removed_semantics"),
    "removed_atoms": c.safeNumber(c.required(wire.removedAtoms, "removed_atoms")),
  })
}

export function encodeDtoDeleteResult(value: unknown): d.DtoDeleteResult {
  const dto = c.record(value, ["deleted"])
  return create(d.DtoDeleteResultSchema, {
    deleted: c.bool(dto["deleted"]),
  })
}
export function decodeDtoDeleteResult(wire: d.DtoDeleteResult): Record<string, unknown> {
  return c.omitUndefined({
    "deleted": c.required(wire.deleted, "deleted"),
  })
}

export function encodeDtoDependencyPayload(value: unknown): d.DtoDependencyPayload {
  const dto = c.record(value, ["parent_task_id"])
  return create(d.DtoDependencyPayloadSchema, {
    parentTaskId: c.text(dto["parent_task_id"]),
  })
}
export function decodeDtoDependencyPayload(wire: d.DtoDependencyPayload): Record<string, unknown> {
  return c.omitUndefined({
    "parent_task_id": c.required(wire.parentTaskId, "parent_task_id"),
  })
}

export function encodeDtoDoctorDerivedStore(value: unknown): d.DtoDoctorDerivedStore {
  const dto = c.record(value, ["store_name","schema_version","last_event_id","dirty","last_error","pending_outbox","running_outbox","failed_outbox"])
  return create(d.DtoDoctorDerivedStoreSchema, {
    storeName: c.text(dto["store_name"]),
    schemaVersion: c.int64(dto["schema_version"]),
    lastEventId: c.int64(dto["last_event_id"]),
    dirty: c.bool(dto["dirty"]),
    lastError: c.optional(dto["last_error"], (value) => c.text(value)),
    pendingOutbox: c.int64(dto["pending_outbox"]),
    runningOutbox: c.int64(dto["running_outbox"]),
    failedOutbox: c.int64(dto["failed_outbox"]),
  })
}
export function decodeDtoDoctorDerivedStore(wire: d.DtoDoctorDerivedStore): Record<string, unknown> {
  return c.omitUndefined({
    "store_name": c.required(wire.storeName, "store_name"),
    "schema_version": c.safeNumber(c.required(wire.schemaVersion, "schema_version")),
    "last_event_id": c.safeNumber(c.required(wire.lastEventId, "last_event_id")),
    "dirty": c.required(wire.dirty, "dirty"),
    "last_error": wire.lastError === undefined ? null : ((value) => value)(wire.lastError),
    "pending_outbox": c.safeNumber(c.required(wire.pendingOutbox, "pending_outbox")),
    "running_outbox": c.safeNumber(c.required(wire.runningOutbox, "running_outbox")),
    "failed_outbox": c.safeNumber(c.required(wire.failedOutbox, "failed_outbox")),
  })
}

export function encodeDtoDoctorIssue(value: unknown): d.DtoDoctorIssue {
  const dto = c.record(value, ["severity","code","message","record_ids"])
  return create(d.DtoDoctorIssueSchema, {
    severity: c.text(dto["severity"]),
    code: c.text(dto["code"]),
    message: c.text(dto["message"]),
    recordIds: c.array(dto["record_ids"], (value) => c.text(value)),
  })
}
export function decodeDtoDoctorIssue(wire: d.DtoDoctorIssue): Record<string, unknown> {
  return c.omitUndefined({
    "severity": c.required(wire.severity, "severity"),
    "code": c.required(wire.code, "code"),
    "message": c.required(wire.message, "message"),
    "record_ids": wire.recordIds.map((value) => value),
  })
}

export function encodeDtoDoctorReport(value: unknown): d.DtoDoctorReport {
  const dto = c.record(value, ["ok","integrity_check","migration_version","user_version","expired_running_tasks","running_tasks_without_active_run","orphan_running_runs","dependency_cycles","archived_dependency_edges","missing_run_logs","suspicious_run_log_paths","executable_dependency_violations","executable_spec_violations","executable_schedule_violations","unplanned_active_tasks","active_parents_with_incomplete_required_steps","outbox_pending","outbox_running","outbox_failed","derived_dirty_stores","derived_error_stores","derived_stores","consistency_errors","consistency_warnings","consistency_issues","ontology_ledger_errors","ontology_ledger_warnings","ontology_ledger_issues"])
  return create(d.DtoDoctorReportSchema, {
    ok: c.bool(dto["ok"]),
    integrityCheck: c.text(dto["integrity_check"]),
    migrationVersion: c.optional(dto["migration_version"], (value) => c.int64(value)),
    userVersion: c.int64(dto["user_version"]),
    expiredRunningTasks: c.int64(dto["expired_running_tasks"]),
    runningTasksWithoutActiveRun: c.int64(dto["running_tasks_without_active_run"]),
    orphanRunningRuns: c.int64(dto["orphan_running_runs"]),
    dependencyCycles: c.int64(dto["dependency_cycles"]),
    archivedDependencyEdges: c.int64(dto["archived_dependency_edges"]),
    missingRunLogs: c.int64(dto["missing_run_logs"]),
    suspiciousRunLogPaths: c.int64(dto["suspicious_run_log_paths"]),
    executableDependencyViolations: c.int64(dto["executable_dependency_violations"]),
    executableSpecViolations: c.int64(dto["executable_spec_violations"]),
    executableScheduleViolations: c.int64(dto["executable_schedule_violations"]),
    unplannedActiveTasks: c.int64(dto["unplanned_active_tasks"]),
    activeParentsWithIncompleteRequiredSteps: c.int64(dto["active_parents_with_incomplete_required_steps"]),
    outboxPending: c.int64(dto["outbox_pending"]),
    outboxRunning: c.int64(dto["outbox_running"]),
    outboxFailed: c.int64(dto["outbox_failed"]),
    derivedDirtyStores: c.int64(dto["derived_dirty_stores"]),
    derivedErrorStores: c.int64(dto["derived_error_stores"]),
    derivedStores: c.array(dto["derived_stores"], (value) => encodeDtoDoctorDerivedStore(value)),
    consistencyErrors: c.int64(dto["consistency_errors"]),
    consistencyWarnings: c.int64(dto["consistency_warnings"]),
    consistencyIssues: c.array(dto["consistency_issues"], (value) => encodeDtoDoctorIssue(value)),
    ontologyLedgerErrors: c.int64(dto["ontology_ledger_errors"]),
    ontologyLedgerWarnings: c.int64(dto["ontology_ledger_warnings"]),
    ontologyLedgerIssues: c.array(dto["ontology_ledger_issues"], (value) => encodeDtoDoctorIssue(value)),
  })
}
export function decodeDtoDoctorReport(wire: d.DtoDoctorReport): Record<string, unknown> {
  return c.omitUndefined({
    "ok": c.required(wire.ok, "ok"),
    "integrity_check": c.required(wire.integrityCheck, "integrity_check"),
    "migration_version": wire.migrationVersion === undefined ? null : ((value) => c.safeNumber(value))(wire.migrationVersion),
    "user_version": c.safeNumber(c.required(wire.userVersion, "user_version")),
    "expired_running_tasks": c.safeNumber(c.required(wire.expiredRunningTasks, "expired_running_tasks")),
    "running_tasks_without_active_run": c.safeNumber(c.required(wire.runningTasksWithoutActiveRun, "running_tasks_without_active_run")),
    "orphan_running_runs": c.safeNumber(c.required(wire.orphanRunningRuns, "orphan_running_runs")),
    "dependency_cycles": c.safeNumber(c.required(wire.dependencyCycles, "dependency_cycles")),
    "archived_dependency_edges": c.safeNumber(c.required(wire.archivedDependencyEdges, "archived_dependency_edges")),
    "missing_run_logs": c.safeNumber(c.required(wire.missingRunLogs, "missing_run_logs")),
    "suspicious_run_log_paths": c.safeNumber(c.required(wire.suspiciousRunLogPaths, "suspicious_run_log_paths")),
    "executable_dependency_violations": c.safeNumber(c.required(wire.executableDependencyViolations, "executable_dependency_violations")),
    "executable_spec_violations": c.safeNumber(c.required(wire.executableSpecViolations, "executable_spec_violations")),
    "executable_schedule_violations": c.safeNumber(c.required(wire.executableScheduleViolations, "executable_schedule_violations")),
    "unplanned_active_tasks": c.safeNumber(c.required(wire.unplannedActiveTasks, "unplanned_active_tasks")),
    "active_parents_with_incomplete_required_steps": c.safeNumber(c.required(wire.activeParentsWithIncompleteRequiredSteps, "active_parents_with_incomplete_required_steps")),
    "outbox_pending": c.safeNumber(c.required(wire.outboxPending, "outbox_pending")),
    "outbox_running": c.safeNumber(c.required(wire.outboxRunning, "outbox_running")),
    "outbox_failed": c.safeNumber(c.required(wire.outboxFailed, "outbox_failed")),
    "derived_dirty_stores": c.safeNumber(c.required(wire.derivedDirtyStores, "derived_dirty_stores")),
    "derived_error_stores": c.safeNumber(c.required(wire.derivedErrorStores, "derived_error_stores")),
    "derived_stores": wire.derivedStores.map((value) => decodeDtoDoctorDerivedStore(value)),
    "consistency_errors": c.safeNumber(c.required(wire.consistencyErrors, "consistency_errors")),
    "consistency_warnings": c.safeNumber(c.required(wire.consistencyWarnings, "consistency_warnings")),
    "consistency_issues": wire.consistencyIssues.map((value) => decodeDtoDoctorIssue(value)),
    "ontology_ledger_errors": c.safeNumber(c.required(wire.ontologyLedgerErrors, "ontology_ledger_errors")),
    "ontology_ledger_warnings": c.safeNumber(c.required(wire.ontologyLedgerWarnings, "ontology_ledger_warnings")),
    "ontology_ledger_issues": wire.ontologyLedgerIssues.map((value) => decodeDtoDoctorIssue(value)),
  })
}

export function encodeDtoEmptyPayload(value: unknown): d.DtoEmptyPayload {
  c.record(value, [])
  return create(d.DtoEmptyPayloadSchema, {
  })
}
export function decodeDtoEmptyPayload(_wire: d.DtoEmptyPayload): Record<string, unknown> {
  void _wire
  return c.omitUndefined({
  })
}

export function encodeDtoEventPayload(value: unknown, kind: string): d.DtoEventPayload {
  switch (eventCase(kind, value)) {
    case "empty": return create(d.DtoEventPayloadSchema, { value: { case: "empty", value: encodeDtoEmptyPayload(value) } })
    case "boardCreated": return create(d.DtoEventPayloadSchema, { value: { case: "boardCreated", value: encodeDtoBoardCreatedPayload(value) } })
    case "dependency": return create(d.DtoEventPayloadSchema, { value: { case: "dependency", value: encodeDtoDependencyPayload(value) } })
    case "labelCreated": return create(d.DtoEventPayloadSchema, { value: { case: "labelCreated", value: encodeDtoLabelCreatedPayload(value) } })
    case "labelDeleted": return create(d.DtoEventPayloadSchema, { value: { case: "labelDeleted", value: encodeDtoLabelDeletedPayload(value) } })
    case "labelOntologyObservationRecorded": return create(d.DtoEventPayloadSchema, { value: { case: "labelOntologyObservationRecorded", value: encodeDtoLabelOntologyObservationRecordedPayload(value) } })
    case "labelOntologyActionCreated": return create(d.DtoEventPayloadSchema, { value: { case: "labelOntologyActionCreated", value: encodeDtoLabelOntologyActionCreatedPayload(value) } })
    case "labelOntologySignalReviewed": return create(d.DtoEventPayloadSchema, { value: { case: "labelOntologySignalReviewed", value: encodeDtoLabelOntologySignalReviewedPayload(value) } })
    case "signalRecorded": return create(d.DtoEventPayloadSchema, { value: { case: "signalRecorded", value: encodeDtoSignalRecordedPayload(value) } })
    case "signalReviewed": return create(d.DtoEventPayloadSchema, { value: { case: "signalReviewed", value: encodeDtoSignalReviewedPayload(value) } })
    case "taskReason": return create(d.DtoEventPayloadSchema, { value: { case: "taskReason", value: encodeDtoTaskReasonPayload(value) } })
    case "taskClaimed": return create(d.DtoEventPayloadSchema, { value: { case: "taskClaimed", value: encodeDtoTaskClaimedPayload(value) } })
    case "taskCommentCreated": return create(d.DtoEventPayloadSchema, { value: { case: "taskCommentCreated", value: encodeDtoTaskCommentCreatedPayload(value) } })
    case "taskResult": return create(d.DtoEventPayloadSchema, { value: { case: "taskResult", value: encodeDtoTaskResultPayload(value) } })
    case "taskStatus": return create(d.DtoEventPayloadSchema, { value: { case: "taskStatus", value: encodeDtoTaskStatusPayload(value) } })
    case "taskToStatus": return create(d.DtoEventPayloadSchema, { value: { case: "taskToStatus", value: encodeDtoTaskToStatusPayload(value) } })
    case "executionPlan": return create(d.DtoEventPayloadSchema, { value: { case: "executionPlan", value: encodeDtoExecutionPlanPayload(value) } })
    case "heartbeat": return create(d.DtoEventPayloadSchema, { value: { case: "heartbeat", value: encodeDtoHeartbeatPayload(value) } })
    case "taskLabel": return create(d.DtoEventPayloadSchema, { value: { case: "taskLabel", value: encodeDtoTaskLabelPayload(value) } })
    case "labelProposal": return create(d.DtoEventPayloadSchema, { value: { case: "labelProposal", value: encodeDtoLabelProposalPayload(value) } })
    case "taskReclaimed": return create(d.DtoEventPayloadSchema, { value: { case: "taskReclaimed", value: encodeDtoTaskReclaimedPayload(value) } })
    case "taskRetry": return create(d.DtoEventPayloadSchema, { value: { case: "taskRetry", value: encodeDtoTaskRetryPayload(value) } })
    case "taskReopened": return create(d.DtoEventPayloadSchema, { value: { case: "taskReopened", value: encodeDtoTaskReopenedPayload(value) } })
    case "retryPolicy": return create(d.DtoEventPayloadSchema, { value: { case: "retryPolicy", value: encodeDtoRetryPolicyPayload(value) } })
    case "taskStep": return create(d.DtoEventPayloadSchema, { value: { case: "taskStep", value: encodeDtoTaskStepPayload(value) } })
    case "taskExportSanitized": return create(d.DtoEventPayloadSchema, { value: { case: "taskExportSanitized", value: encodeDtoTaskExportSanitizedPayload(value) } })
    case "unknown": return create(d.DtoEventPayloadSchema, { value: { case: "unknown", value: c.encodeJson(value) } })
  }
}
export function decodeDtoEventPayload(wire: d.DtoEventPayload, kind: string): unknown {
  let value: unknown
  switch (wire.value.case) {
    case "empty": value = decodeDtoEmptyPayload(wire.value.value); break
    case "boardCreated": value = decodeDtoBoardCreatedPayload(wire.value.value); break
    case "dependency": value = decodeDtoDependencyPayload(wire.value.value); break
    case "labelCreated": value = decodeDtoLabelCreatedPayload(wire.value.value); break
    case "labelDeleted": value = decodeDtoLabelDeletedPayload(wire.value.value); break
    case "labelOntologyObservationRecorded": value = decodeDtoLabelOntologyObservationRecordedPayload(wire.value.value); break
    case "labelOntologyActionCreated": value = decodeDtoLabelOntologyActionCreatedPayload(wire.value.value); break
    case "labelOntologySignalReviewed": value = decodeDtoLabelOntologySignalReviewedPayload(wire.value.value); break
    case "signalRecorded": value = decodeDtoSignalRecordedPayload(wire.value.value); break
    case "signalReviewed": value = decodeDtoSignalReviewedPayload(wire.value.value); break
    case "taskReason": value = decodeDtoTaskReasonPayload(wire.value.value); break
    case "taskClaimed": value = decodeDtoTaskClaimedPayload(wire.value.value); break
    case "taskCommentCreated": value = decodeDtoTaskCommentCreatedPayload(wire.value.value); break
    case "taskResult": value = decodeDtoTaskResultPayload(wire.value.value); break
    case "taskStatus": value = decodeDtoTaskStatusPayload(wire.value.value); break
    case "taskToStatus": value = decodeDtoTaskToStatusPayload(wire.value.value); break
    case "executionPlan": value = decodeDtoExecutionPlanPayload(wire.value.value); break
    case "heartbeat": value = decodeDtoHeartbeatPayload(wire.value.value); break
    case "taskLabel": value = decodeDtoTaskLabelPayload(wire.value.value); break
    case "labelProposal": value = decodeDtoLabelProposalPayload(wire.value.value); break
    case "taskReclaimed": value = decodeDtoTaskReclaimedPayload(wire.value.value); break
    case "taskRetry": value = decodeDtoTaskRetryPayload(wire.value.value); break
    case "taskReopened": value = decodeDtoTaskReopenedPayload(wire.value.value); break
    case "retryPolicy": value = decodeDtoRetryPolicyPayload(wire.value.value); break
    case "taskStep": value = decodeDtoTaskStepPayload(wire.value.value); break
    case "taskExportSanitized": value = decodeDtoTaskExportSanitizedPayload(wire.value.value); break
    case "unknown": value = c.decodeJson(wire.value.value); break
    default: throw new c.RpcCodecError("RPC event payload 缺少分支。")
  }
  validateEventCase(kind, wire.value.case, value)
  return value
}

const DtoEventPayloadCommentAuthorTypeNames = {
  "user": d.DtoEventPayloadCommentAuthorType.USER,
  "agent": d.DtoEventPayloadCommentAuthorType.AGENT,
} as const
export function encodeDtoEventPayloadCommentAuthorType(value: unknown): d.DtoEventPayloadCommentAuthorType { return c.enumValue(value, DtoEventPayloadCommentAuthorTypeNames) }
export function decodeDtoEventPayloadCommentAuthorType(value: d.DtoEventPayloadCommentAuthorType): string { return c.enumName(value, DtoEventPayloadCommentAuthorTypeNames) }

const DtoEventPayloadCommentKindNames = {
  "note": d.DtoEventPayloadCommentKind.NOTE,
  "decision": d.DtoEventPayloadCommentKind.DECISION,
  "signal": d.DtoEventPayloadCommentKind.SIGNAL,
} as const
export function encodeDtoEventPayloadCommentKind(value: unknown): d.DtoEventPayloadCommentKind { return c.enumValue(value, DtoEventPayloadCommentKindNames) }
export function decodeDtoEventPayloadCommentKind(value: d.DtoEventPayloadCommentKind): string { return c.enumName(value, DtoEventPayloadCommentKindNames) }

export function encodeDtoExecutionPlanPayload(value: unknown): d.DtoExecutionPlanPayload {
  const dto = c.record(value, ["state"])
  return create(d.DtoExecutionPlanPayloadSchema, {
    state: encodeDtoExecutionPlanState(dto["state"]),
  })
}
export function decodeDtoExecutionPlanPayload(wire: d.DtoExecutionPlanPayload): Record<string, unknown> {
  return c.omitUndefined({
    "state": decodeDtoExecutionPlanState(c.required(wire.state, "state")),
  })
}

const DtoExecutionPlanStateNames = {
  "planned": d.DtoExecutionPlanState.PLANNED,
  "not_required": d.DtoExecutionPlanState.NOT_REQUIRED,
  "unplanned": d.DtoExecutionPlanState.UNPLANNED,
} as const
export function encodeDtoExecutionPlanState(value: unknown): d.DtoExecutionPlanState { return c.enumValue(value, DtoExecutionPlanStateNames) }
export function decodeDtoExecutionPlanState(value: d.DtoExecutionPlanState): string { return c.enumName(value, DtoExecutionPlanStateNames) }

export function encodeDtoExportReport(value: unknown): d.DtoExportReport {
  const dto = c.record(value, ["out_path","checksum_sha256","bytes","record_count","source_fingerprint"])
  return create(d.DtoExportReportSchema, {
    outPath: c.text(dto["out_path"]),
    checksumSha256: c.text(dto["checksum_sha256"]),
    bytes: c.int64(dto["bytes"], true),
    recordCount: c.int64(dto["record_count"], true),
    sourceFingerprint: c.text(dto["source_fingerprint"]),
  })
}
export function decodeDtoExportReport(wire: d.DtoExportReport): Record<string, unknown> {
  return c.omitUndefined({
    "out_path": c.required(wire.outPath, "out_path"),
    "checksum_sha256": c.required(wire.checksumSha256, "checksum_sha256"),
    "bytes": c.safeNumber(c.required(wire.bytes, "bytes")),
    "record_count": c.safeNumber(c.required(wire.recordCount, "record_count")),
    "source_fingerprint": c.required(wire.sourceFingerprint, "source_fingerprint"),
  })
}

export function encodeDtoGraphMaintenance(value: unknown): d.DtoGraphMaintenance {
  const dto = c.record(value, ["mode","board_id","generation","fingerprint","validated_tasks","validated_entities","validated_relations","pending_jobs","consumed_jobs","updated_at","message"])
  return create(d.DtoGraphMaintenanceSchema, {
    mode: c.text(dto["mode"]),
    boardId: c.text(dto["board_id"]),
    generation: c.text(dto["generation"]),
    fingerprint: c.text(dto["fingerprint"]),
    validatedTasks: c.int64(dto["validated_tasks"]),
    validatedEntities: c.int64(dto["validated_entities"]),
    validatedRelations: c.int64(dto["validated_relations"]),
    pendingJobs: c.int64(dto["pending_jobs"]),
    consumedJobs: c.int64(dto["consumed_jobs"]),
    updatedAt: c.int64(dto["updated_at"]),
    message: c.text(dto["message"]),
  })
}
export function decodeDtoGraphMaintenance(wire: d.DtoGraphMaintenance): Record<string, unknown> {
  return c.omitUndefined({
    "mode": c.required(wire.mode, "mode"),
    "board_id": c.required(wire.boardId, "board_id"),
    "generation": c.required(wire.generation, "generation"),
    "fingerprint": c.required(wire.fingerprint, "fingerprint"),
    "validated_tasks": c.safeNumber(c.required(wire.validatedTasks, "validated_tasks")),
    "validated_entities": c.safeNumber(c.required(wire.validatedEntities, "validated_entities")),
    "validated_relations": c.safeNumber(c.required(wire.validatedRelations, "validated_relations")),
    "pending_jobs": c.safeNumber(c.required(wire.pendingJobs, "pending_jobs")),
    "consumed_jobs": c.safeNumber(c.required(wire.consumedJobs, "consumed_jobs")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "message": c.required(wire.message, "message"),
  })
}

export function encodeDtoGraphStatus(value: unknown): d.DtoGraphStatus {
  const dto = c.record(value, ["backend","enabled","message"])
  return create(d.DtoGraphStatusSchema, {
    backend: c.text(dto["backend"]),
    enabled: c.bool(dto["enabled"]),
    message: c.text(dto["message"]),
  })
}
export function decodeDtoGraphStatus(wire: d.DtoGraphStatus): Record<string, unknown> {
  return c.omitUndefined({
    "backend": c.required(wire.backend, "backend"),
    "enabled": c.required(wire.enabled, "enabled"),
    "message": c.required(wire.message, "message"),
  })
}

export function encodeDtoHealthReport(value: unknown): d.DtoHealthReport {
  const dto = c.record(value, ["ok","db","version","db_path","db_fingerprint"])
  return create(d.DtoHealthReportSchema, {
    ok: c.bool(dto["ok"]),
    db: c.text(dto["db"]),
    version: c.text(dto["version"]),
    dbPath: c.text(dto["db_path"]),
    dbFingerprint: c.text(dto["db_fingerprint"]),
  })
}
export function decodeDtoHealthReport(wire: d.DtoHealthReport): Record<string, unknown> {
  return c.omitUndefined({
    "ok": c.required(wire.ok, "ok"),
    "db": c.required(wire.db, "db"),
    "version": c.required(wire.version, "version"),
    "db_path": c.required(wire.dbPath, "db_path"),
    "db_fingerprint": c.required(wire.dbFingerprint, "db_fingerprint"),
  })
}

export function encodeDtoHeartbeatPayload(value: unknown): d.DtoHeartbeatPayload {
  const dto = c.record(value, ["note"])
  return create(d.DtoHeartbeatPayloadSchema, {
    note: c.optional(dto["note"], (value) => c.text(value)),
  })
}
export function decodeDtoHeartbeatPayload(wire: d.DtoHeartbeatPayload): Record<string, unknown> {
  return c.omitUndefined({
    "note": wire.note === undefined ? null : ((value) => value)(wire.note),
  })
}

export function encodeDtoImportReport(value: unknown): d.DtoImportReport {
  const dto = c.record(value, ["in_path","source_fingerprint","imported_records","skipped_records","rebuild_jobs_enqueued","journal_id","phase","restart_required","staged_database_path","target_fingerprint_before","staged_fingerprint","publish_preconditions"])
  return create(d.DtoImportReportSchema, {
    inPath: c.text(dto["in_path"]),
    sourceFingerprint: c.text(dto["source_fingerprint"]),
    importedRecords: c.int64(dto["imported_records"], true),
    skippedRecords: c.int64(dto["skipped_records"], true),
    rebuildJobsEnqueued: c.int64(dto["rebuild_jobs_enqueued"], true),
    journalId: c.text(dto["journal_id"]),
    phase: c.text(dto["phase"]),
    restartRequired: c.bool(dto["restart_required"]),
    stagedDatabasePath: c.optional(dto["staged_database_path"], (value) => c.text(value)),
    targetFingerprintBefore: c.optional(dto["target_fingerprint_before"], (value) => c.text(value)),
    stagedFingerprint: c.optional(dto["staged_fingerprint"], (value) => c.text(value)),
    publishPreconditions: c.array(dto["publish_preconditions"], (value) => c.text(value)),
  })
}
export function decodeDtoImportReport(wire: d.DtoImportReport): Record<string, unknown> {
  return c.omitUndefined({
    "in_path": c.required(wire.inPath, "in_path"),
    "source_fingerprint": c.required(wire.sourceFingerprint, "source_fingerprint"),
    "imported_records": c.safeNumber(c.required(wire.importedRecords, "imported_records")),
    "skipped_records": c.safeNumber(c.required(wire.skippedRecords, "skipped_records")),
    "rebuild_jobs_enqueued": c.safeNumber(c.required(wire.rebuildJobsEnqueued, "rebuild_jobs_enqueued")),
    "journal_id": c.required(wire.journalId, "journal_id"),
    "phase": c.required(wire.phase, "phase"),
    "restart_required": c.required(wire.restartRequired, "restart_required"),
    "staged_database_path": wire.stagedDatabasePath === undefined ? null : ((value) => value)(wire.stagedDatabasePath),
    "target_fingerprint_before": wire.targetFingerprintBefore === undefined ? null : ((value) => value)(wire.targetFingerprintBefore),
    "staged_fingerprint": wire.stagedFingerprint === undefined ? null : ((value) => value)(wire.stagedFingerprint),
    "publish_preconditions": wire.publishPreconditions.map((value) => value),
  })
}

export function encodeDtoJsonArray(value: unknown): d.DtoJsonArray {
  return create(d.DtoJsonArraySchema, { value: c.array(value, (value) => c.encodeJson(value)) })
}
export function decodeDtoJsonArray(wire: d.DtoJsonArray): unknown {
  return wire.value.map((value) => c.decodeJson(value))
}

export function encodeDtoLabelAtomExplainActionWire(value: unknown): d.DtoLabelAtomExplainActionWire {
  const dto = c.record(value, ["action","matched_by"])
  return create(d.DtoLabelAtomExplainActionWireSchema, {
    action: encodeDtoLabelOntologyActionWire(dto["action"]),
    matchedBy: c.text(dto["matched_by"]),
  })
}
export function decodeDtoLabelAtomExplainActionWire(wire: d.DtoLabelAtomExplainActionWire): Record<string, unknown> {
  return c.omitUndefined({
    "action": decodeDtoLabelOntologyActionWire(c.required(wire.action, "action")),
    "matched_by": c.required(wire.matchedBy, "matched_by"),
  })
}

export function encodeDtoLabelAtomExplainSignalWire(value: unknown): d.DtoLabelAtomExplainSignalWire {
  const dto = c.record(value, ["signal","observation","source_task","task_ref_snapshot","suggest_input_stale","suggest_degraded","warnings"])
  return create(d.DtoLabelAtomExplainSignalWireSchema, {
    signal: encodeDtoLabelOntologySignalWire(dto["signal"]),
    observation: encodeDtoLabelOntologyObservationWire(dto["observation"]),
    sourceTask: encodeDtoApiTask(dto["source_task"]),
    taskRefSnapshot: c.text(dto["task_ref_snapshot"]),
    suggestInputStale: c.bool(dto["suggest_input_stale"]),
    suggestDegraded: c.bool(dto["suggest_degraded"]),
    warnings: c.array(dto["warnings"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelAtomExplainSignalWire(wire: d.DtoLabelAtomExplainSignalWire): Record<string, unknown> {
  return c.omitUndefined({
    "signal": decodeDtoLabelOntologySignalWire(c.required(wire.signal, "signal")),
    "observation": decodeDtoLabelOntologyObservationWire(c.required(wire.observation, "observation")),
    "source_task": decodeDtoApiTask(c.required(wire.sourceTask, "source_task")),
    "task_ref_snapshot": c.required(wire.taskRefSnapshot, "task_ref_snapshot"),
    "suggest_input_stale": c.required(wire.suggestInputStale, "suggest_input_stale"),
    "suggest_degraded": c.required(wire.suggestDegraded, "suggest_degraded"),
    "warnings": wire.warnings.map((value) => value),
  })
}

export function encodeDtoLabelAtomExplainValidationWire(value: unknown): d.DtoLabelAtomExplainValidationWire {
  const dto = c.record(value, ["action","parent_action_id","validation_status","manual","summary","cases","warnings"])
  return create(d.DtoLabelAtomExplainValidationWireSchema, {
    action: encodeDtoLabelOntologyActionWire(dto["action"]),
    parentActionId: c.text(dto["parent_action_id"]),
    validationStatus: encodeDtoLabelOntologyValidationStatusWire(dto["validation_status"]),
    manual: c.encodeJson(dto["manual"]),
    summary: c.encodeJson(dto["summary"]),
    cases: c.encodeJson(dto["cases"]),
    warnings: c.array(dto["warnings"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelAtomExplainValidationWire(wire: d.DtoLabelAtomExplainValidationWire): Record<string, unknown> {
  return c.omitUndefined({
    "action": decodeDtoLabelOntologyActionWire(c.required(wire.action, "action")),
    "parent_action_id": c.required(wire.parentActionId, "parent_action_id"),
    "validation_status": decodeDtoLabelOntologyValidationStatusWire(c.required(wire.validationStatus, "validation_status")),
    "manual": c.decodeJson(c.required(wire.manual, "manual")),
    "summary": c.decodeJson(c.required(wire.summary, "summary")),
    "cases": c.decodeJson(c.required(wire.cases, "cases")),
    "warnings": wire.warnings.map((value) => value),
  })
}

export function encodeDtoLabelAtomExplainWire(value: unknown): d.DtoLabelAtomExplainWire {
  const dto = c.record(value, ["query","atom","current_semantics","provenance_actions","supporting_signals","validation_history","legacy_untracked","legacy_reason"])
  return create(d.DtoLabelAtomExplainWireSchema, {
    query: c.text(dto["query"]),
    atom: c.optional(dto["atom"], (value) => encodeDtoLabelAtomWire(value)),
    currentSemantics: c.optional(dto["current_semantics"], (value) => encodeDtoLabelSemanticsWire(value)),
    provenanceActions: c.array(dto["provenance_actions"], (value) => encodeDtoLabelAtomExplainActionWire(value)),
    supportingSignals: c.array(dto["supporting_signals"], (value) => encodeDtoLabelAtomExplainSignalWire(value)),
    validationHistory: c.array(dto["validation_history"], (value) => encodeDtoLabelAtomExplainValidationWire(value)),
    legacyUntracked: c.bool(dto["legacy_untracked"]),
    legacyReason: c.optional(dto["legacy_reason"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelAtomExplainWire(wire: d.DtoLabelAtomExplainWire): Record<string, unknown> {
  return c.omitUndefined({
    "query": c.required(wire.query, "query"),
    "atom": wire.atom === undefined ? null : ((value) => decodeDtoLabelAtomWire(value))(wire.atom),
    "current_semantics": wire.currentSemantics === undefined ? null : ((value) => decodeDtoLabelSemanticsWire(value))(wire.currentSemantics),
    "provenance_actions": wire.provenanceActions.map((value) => decodeDtoLabelAtomExplainActionWire(value)),
    "supporting_signals": wire.supportingSignals.map((value) => decodeDtoLabelAtomExplainSignalWire(value)),
    "validation_history": wire.validationHistory.map((value) => decodeDtoLabelAtomExplainValidationWire(value)),
    "legacy_untracked": c.required(wire.legacyUntracked, "legacy_untracked"),
    "legacy_reason": wire.legacyReason === undefined ? null : ((value) => value)(wire.legacyReason),
  })
}

export function encodeDtoLabelAtomIndexHit(value: unknown): d.DtoLabelAtomIndexHit {
  const dto = c.record(value, ["atom_id","label_id","label_name","board_id","polarity","kind","text","ordinal","content_hash","embedding_model","distance"])
  return create(d.DtoLabelAtomIndexHitSchema, {
    atomId: c.text(dto["atom_id"]),
    labelId: c.text(dto["label_id"]),
    labelName: c.text(dto["label_name"]),
    boardId: c.text(dto["board_id"]),
    polarity: c.text(dto["polarity"]),
    kind: c.text(dto["kind"]),
    text: c.text(dto["text"]),
    ordinal: c.int64(dto["ordinal"]),
    contentHash: c.text(dto["content_hash"]),
    embeddingModel: c.text(dto["embedding_model"]),
    distance: c.float(dto["distance"]),
  })
}
export function decodeDtoLabelAtomIndexHit(wire: d.DtoLabelAtomIndexHit): Record<string, unknown> {
  return c.omitUndefined({
    "atom_id": c.required(wire.atomId, "atom_id"),
    "label_id": c.required(wire.labelId, "label_id"),
    "label_name": c.required(wire.labelName, "label_name"),
    "board_id": c.required(wire.boardId, "board_id"),
    "polarity": c.required(wire.polarity, "polarity"),
    "kind": c.required(wire.kind, "kind"),
    "text": c.required(wire.text, "text"),
    "ordinal": c.safeNumber(c.required(wire.ordinal, "ordinal")),
    "content_hash": c.required(wire.contentHash, "content_hash"),
    "embedding_model": c.required(wire.embeddingModel, "embedding_model"),
    "distance": c.float(c.required(wire.distance, "distance")),
  })
}

export function encodeDtoLabelAtomIndexQueryData(value: unknown): d.DtoLabelAtomIndexQueryData {
  const dto = c.record(value, ["data","degraded","diagnostics"])
  return create(d.DtoLabelAtomIndexQueryDataSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelAtomIndexHit(value)),
    degraded: c.bool(dto["degraded"]),
    diagnostics: c.array(dto["diagnostics"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelAtomIndexQueryData(wire: d.DtoLabelAtomIndexQueryData): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelAtomIndexHit(value)),
    "degraded": c.required(wire.degraded, "degraded"),
    "diagnostics": wire.diagnostics.map((value) => value),
  })
}

export function encodeDtoLabelAtomWire(value: unknown): d.DtoLabelAtomWire {
  const dto = c.record(value, ["id","label_id","board_id","label_name","polarity","kind","text","ordinal","content_hash","created_at","updated_at"])
  return create(d.DtoLabelAtomWireSchema, {
    id: c.text(dto["id"]),
    labelId: c.text(dto["label_id"]),
    boardId: c.text(dto["board_id"]),
    labelName: c.text(dto["label_name"]),
    polarity: c.text(dto["polarity"]),
    kind: c.text(dto["kind"]),
    text: c.text(dto["text"]),
    ordinal: c.int64(dto["ordinal"]),
    contentHash: c.text(dto["content_hash"]),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoLabelAtomWire(wire: d.DtoLabelAtomWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "label_id": c.required(wire.labelId, "label_id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "label_name": c.required(wire.labelName, "label_name"),
    "polarity": c.required(wire.polarity, "polarity"),
    "kind": c.required(wire.kind, "kind"),
    "text": c.required(wire.text, "text"),
    "ordinal": c.safeNumber(c.required(wire.ordinal, "ordinal")),
    "content_hash": c.required(wire.contentHash, "content_hash"),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

export function encodeDtoLabelCreatedPayload(value: unknown): d.DtoLabelCreatedPayload {
  const dto = c.record(value, ["label_id","label","color"])
  return create(d.DtoLabelCreatedPayloadSchema, {
    labelId: c.text(dto["label_id"]),
    label: c.text(dto["label"]),
    color: c.optional(dto["color"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelCreatedPayload(wire: d.DtoLabelCreatedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "label_id": c.required(wire.labelId, "label_id"),
    "label": c.required(wire.label, "label"),
    "color": wire.color === undefined ? null : ((value) => value)(wire.color),
  })
}

export function encodeDtoLabelDeletedPayload(value: unknown): d.DtoLabelDeletedPayload {
  const dto = c.record(value, ["label_id","label","forced","removed_task_bindings","removed_semantics","removed_atoms"])
  return create(d.DtoLabelDeletedPayloadSchema, {
    labelId: c.text(dto["label_id"]),
    label: c.text(dto["label"]),
    forced: c.bool(dto["forced"]),
    removedTaskBindings: c.int64(dto["removed_task_bindings"], true),
    removedSemantics: c.bool(dto["removed_semantics"]),
    removedAtoms: c.int64(dto["removed_atoms"], true),
  })
}
export function decodeDtoLabelDeletedPayload(wire: d.DtoLabelDeletedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "label_id": c.required(wire.labelId, "label_id"),
    "label": c.required(wire.label, "label"),
    "forced": c.required(wire.forced, "forced"),
    "removed_task_bindings": c.safeNumber(c.required(wire.removedTaskBindings, "removed_task_bindings")),
    "removed_semantics": c.required(wire.removedSemantics, "removed_semantics"),
    "removed_atoms": c.safeNumber(c.required(wire.removedAtoms, "removed_atoms")),
  })
}

export function encodeDtoLabelOntologyActionCreatedPayload(value: unknown): d.DtoLabelOntologyActionCreatedPayload {
  const dto = c.record(value, ["action_id","action_type","signal_ids"])
  return create(d.DtoLabelOntologyActionCreatedPayloadSchema, {
    actionId: c.text(dto["action_id"]),
    actionType: c.text(dto["action_type"]),
    signalIds: c.array(dto["signal_ids"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelOntologyActionCreatedPayload(wire: d.DtoLabelOntologyActionCreatedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "action_id": c.required(wire.actionId, "action_id"),
    "action_type": c.required(wire.actionType, "action_type"),
    "signal_ids": wire.signalIds.map((value) => value),
  })
}

const DtoLabelOntologyActionTypeWireNames = {
  "confirm": d.DtoLabelOntologyActionTypeWire.CONFIRM,
  "reject": d.DtoLabelOntologyActionTypeWire.REJECT,
  "supersede": d.DtoLabelOntologyActionTypeWire.SUPERSEDE,
  "resolve_no_change": d.DtoLabelOntologyActionTypeWire.RESOLVE_NO_CHANGE,
  "add_positive_atom": d.DtoLabelOntologyActionTypeWire.ADD_POSITIVE_ATOM,
  "add_negative_atom": d.DtoLabelOntologyActionTypeWire.ADD_NEGATIVE_ATOM,
  "adopt_existing_atom": d.DtoLabelOntologyActionTypeWire.ADOPT_EXISTING_ATOM,
  "update_semantics": d.DtoLabelOntologyActionTypeWire.UPDATE_SEMANTICS,
  "create_label_proposal": d.DtoLabelOntologyActionTypeWire.CREATE_LABEL_PROPOSAL,
  "bootstrap_label": d.DtoLabelOntologyActionTypeWire.BOOTSTRAP_LABEL,
  "rename_label": d.DtoLabelOntologyActionTypeWire.RENAME_LABEL,
  "split_label": d.DtoLabelOntologyActionTypeWire.SPLIT_LABEL,
  "merge_labels": d.DtoLabelOntologyActionTypeWire.MERGE_LABELS,
  "revert_ontology_mutation": d.DtoLabelOntologyActionTypeWire.REVERT_ONTOLOGY_MUTATION,
  "validate": d.DtoLabelOntologyActionTypeWire.VALIDATE,
} as const
export function encodeDtoLabelOntologyActionTypeWire(value: unknown): d.DtoLabelOntologyActionTypeWire { return c.enumValue(value, DtoLabelOntologyActionTypeWireNames) }
export function decodeDtoLabelOntologyActionTypeWire(value: d.DtoLabelOntologyActionTypeWire): string { return c.enumName(value, DtoLabelOntologyActionTypeWireNames) }

export function encodeDtoLabelOntologyActionWire(value: unknown): d.DtoLabelOntologyActionWire {
  const dto = c.record(value, ["id","board_id","parent_action_id","action_type","reason","target_label_id","result_label_id","result_atom_id","result_atom_content_hash","result_proposal_id","canonical_before_hash","canonical_after_hash","change","validation_requirement","validation_status","validation_effective_outcome","validation_latest_attempt_id","validation","created_by","created_by_type","agent_type","created_at","signal_ids"])
  return create(d.DtoLabelOntologyActionWireSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    parentActionId: c.optional(dto["parent_action_id"], (value) => c.text(value)),
    actionType: encodeDtoLabelOntologyActionTypeWire(dto["action_type"]),
    reason: c.text(dto["reason"]),
    targetLabelId: c.optional(dto["target_label_id"], (value) => c.text(value)),
    resultLabelId: c.optional(dto["result_label_id"], (value) => c.text(value)),
    resultAtomId: c.optional(dto["result_atom_id"], (value) => c.text(value)),
    resultAtomContentHash: c.optional(dto["result_atom_content_hash"], (value) => c.text(value)),
    resultProposalId: c.optional(dto["result_proposal_id"], (value) => c.text(value)),
    canonicalBeforeHash: c.optional(dto["canonical_before_hash"], (value) => c.text(value)),
    canonicalAfterHash: c.optional(dto["canonical_after_hash"], (value) => c.text(value)),
    change: encodeDtoStructuredMetadataJsonObject(dto["change"]),
    validationRequirement: encodeDtoLabelOntologyValidationRequirementWire(dto["validation_requirement"]),
    validationStatus: encodeDtoLabelOntologyValidationStatusWire(dto["validation_status"]),
    validationEffectiveOutcome: encodeDtoLabelOntologyValidationEffectiveOutcomeWire(dto["validation_effective_outcome"]),
    validationLatestAttemptId: c.optional(dto["validation_latest_attempt_id"], (value) => c.text(value)),
    validation: encodeDtoStructuredMetadataJsonObject(dto["validation"]),
    createdBy: c.text(dto["created_by"]),
    createdByType: c.text(dto["created_by_type"]),
    agentType: c.optional(dto["agent_type"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    signalIds: c.array(dto["signal_ids"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelOntologyActionWire(wire: d.DtoLabelOntologyActionWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "parent_action_id": wire.parentActionId === undefined ? null : ((value) => value)(wire.parentActionId),
    "action_type": decodeDtoLabelOntologyActionTypeWire(c.required(wire.actionType, "action_type")),
    "reason": c.required(wire.reason, "reason"),
    "target_label_id": wire.targetLabelId === undefined ? null : ((value) => value)(wire.targetLabelId),
    "result_label_id": wire.resultLabelId === undefined ? null : ((value) => value)(wire.resultLabelId),
    "result_atom_id": wire.resultAtomId === undefined ? null : ((value) => value)(wire.resultAtomId),
    "result_atom_content_hash": wire.resultAtomContentHash === undefined ? null : ((value) => value)(wire.resultAtomContentHash),
    "result_proposal_id": wire.resultProposalId === undefined ? null : ((value) => value)(wire.resultProposalId),
    "canonical_before_hash": wire.canonicalBeforeHash === undefined ? null : ((value) => value)(wire.canonicalBeforeHash),
    "canonical_after_hash": wire.canonicalAfterHash === undefined ? null : ((value) => value)(wire.canonicalAfterHash),
    "change": decodeDtoStructuredMetadataJsonObject(c.required(wire.change, "change")),
    "validation_requirement": decodeDtoLabelOntologyValidationRequirementWire(c.required(wire.validationRequirement, "validation_requirement")),
    "validation_status": decodeDtoLabelOntologyValidationStatusWire(c.required(wire.validationStatus, "validation_status")),
    "validation_effective_outcome": decodeDtoLabelOntologyValidationEffectiveOutcomeWire(c.required(wire.validationEffectiveOutcome, "validation_effective_outcome")),
    "validation_latest_attempt_id": wire.validationLatestAttemptId === undefined ? null : ((value) => value)(wire.validationLatestAttemptId),
    "validation": decodeDtoStructuredMetadataJsonObject(c.required(wire.validation, "validation")),
    "created_by": c.required(wire.createdBy, "created_by"),
    "created_by_type": c.required(wire.createdByType, "created_by_type"),
    "agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "signal_ids": wire.signalIds.map((value) => value),
  })
}

export function encodeDtoLabelOntologyActorWire(value: unknown): d.DtoLabelOntologyActorWire {
  const dto = c.record(value, ["name","type","agent_type"])
  return create(d.DtoLabelOntologyActorWireSchema, {
    name: c.text(dto["name"]),
    actorType: c.text(dto["type"]),
    agentType: c.optional(dto["agent_type"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelOntologyActorWire(wire: d.DtoLabelOntologyActorWire): Record<string, unknown> {
  return c.omitUndefined({
    "name": c.required(wire.name, "name"),
    "type": c.required(wire.actorType, "actor_type"),
    "agent_type": wire.agentType === undefined ? undefined : ((value) => value)(wire.agentType),
  })
}

export function encodeDtoLabelOntologyCandidateAtomRequest(value: unknown): d.DtoLabelOntologyCandidateAtomRequest {
  const dto = c.record(value, ["polarity","kind","text"])
  return create(d.DtoLabelOntologyCandidateAtomRequestSchema, {
    polarity: c.text(dto["polarity"]),
    kind: c.text(dto["kind"]),
    text: c.text(dto["text"]),
  })
}
export function decodeDtoLabelOntologyCandidateAtomRequest(wire: d.DtoLabelOntologyCandidateAtomRequest): Record<string, unknown> {
  return c.omitUndefined({
    "polarity": c.required(wire.polarity, "polarity"),
    "kind": c.required(wire.kind, "kind"),
    "text": c.required(wire.text, "text"),
  })
}

export function encodeDtoLabelOntologyObservationRecordedPayload(value: unknown): d.DtoLabelOntologyObservationRecordedPayload {
  const dto = c.record(value, ["observation_id","signal_ids"])
  return create(d.DtoLabelOntologyObservationRecordedPayloadSchema, {
    observationId: c.text(dto["observation_id"]),
    signalIds: c.array(dto["signal_ids"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelOntologyObservationRecordedPayload(wire: d.DtoLabelOntologyObservationRecordedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "observation_id": c.required(wire.observationId, "observation_id"),
    "signal_ids": wire.signalIds.map((value) => value),
  })
}

export function encodeDtoLabelOntologyObservationWire(value: unknown): d.DtoLabelOntologyObservationWire {
  const dto = c.record(value, ["id","board_id","task_id","task_ref_snapshot","task_snapshot","suggest_input_hash","agent_candidates","suggestion_snapshot","final_decision","suggest_coverage","suggest_coverage_cosine","suggest_residual_norm","suggest_needs_new_label","suggest_degraded","diagnostics","capture_fingerprint","created_by","created_by_type","agent_type","created_at","signals"])
  return create(d.DtoLabelOntologyObservationWireSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    taskId: c.text(dto["task_id"]),
    taskRefSnapshot: c.text(dto["task_ref_snapshot"]),
    taskSnapshot: encodeDtoStructuredMetadataJsonObject(dto["task_snapshot"]),
    suggestInputHash: c.optional(dto["suggest_input_hash"], (value) => c.text(value)),
    agentCandidates: encodeDtoJsonArray(dto["agent_candidates"]),
    suggestionSnapshot: encodeDtoStructuredMetadataJsonObject(dto["suggestion_snapshot"]),
    finalDecision: encodeDtoStructuredMetadataJsonObject(dto["final_decision"]),
    suggestCoverage: c.optional(dto["suggest_coverage"], (value) => c.float(value)),
    suggestCoverageCosine: c.optional(dto["suggest_coverage_cosine"], (value) => c.float(value)),
    suggestResidualNorm: c.optional(dto["suggest_residual_norm"], (value) => c.float(value)),
    suggestNeedsNewLabel: c.bool(dto["suggest_needs_new_label"]),
    suggestDegraded: c.bool(dto["suggest_degraded"]),
    diagnostics: encodeDtoJsonArray(dto["diagnostics"]),
    captureFingerprint: c.text(dto["capture_fingerprint"]),
    createdBy: c.text(dto["created_by"]),
    createdByType: c.text(dto["created_by_type"]),
    agentType: c.optional(dto["agent_type"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    signals: c.array(dto["signals"], (value) => encodeDtoLabelOntologySignalWire(value)),
  })
}
export function decodeDtoLabelOntologyObservationWire(wire: d.DtoLabelOntologyObservationWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": c.required(wire.taskId, "task_id"),
    "task_ref_snapshot": c.required(wire.taskRefSnapshot, "task_ref_snapshot"),
    "task_snapshot": decodeDtoStructuredMetadataJsonObject(c.required(wire.taskSnapshot, "task_snapshot")),
    "suggest_input_hash": wire.suggestInputHash === undefined ? null : ((value) => value)(wire.suggestInputHash),
    "agent_candidates": decodeDtoJsonArray(c.required(wire.agentCandidates, "agent_candidates")),
    "suggestion_snapshot": decodeDtoStructuredMetadataJsonObject(c.required(wire.suggestionSnapshot, "suggestion_snapshot")),
    "final_decision": decodeDtoStructuredMetadataJsonObject(c.required(wire.finalDecision, "final_decision")),
    "suggest_coverage": wire.suggestCoverage === undefined ? null : ((value) => c.float(value))(wire.suggestCoverage),
    "suggest_coverage_cosine": wire.suggestCoverageCosine === undefined ? null : ((value) => c.float(value))(wire.suggestCoverageCosine),
    "suggest_residual_norm": wire.suggestResidualNorm === undefined ? null : ((value) => c.float(value))(wire.suggestResidualNorm),
    "suggest_needs_new_label": c.required(wire.suggestNeedsNewLabel, "suggest_needs_new_label"),
    "suggest_degraded": c.required(wire.suggestDegraded, "suggest_degraded"),
    "diagnostics": decodeDtoJsonArray(c.required(wire.diagnostics, "diagnostics")),
    "capture_fingerprint": c.required(wire.captureFingerprint, "capture_fingerprint"),
    "created_by": c.required(wire.createdBy, "created_by"),
    "created_by_type": c.required(wire.createdByType, "created_by_type"),
    "agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "signals": wire.signals.map((value) => decodeDtoLabelOntologySignalWire(value)),
  })
}

const DtoLabelOntologyProposedActionWireNames = {
  "observe": d.DtoLabelOntologyProposedActionWire.OBSERVE,
  "add_positive_atom": d.DtoLabelOntologyProposedActionWire.ADD_POSITIVE_ATOM,
  "add_negative_atom": d.DtoLabelOntologyProposedActionWire.ADD_NEGATIVE_ATOM,
  "update_semantics": d.DtoLabelOntologyProposedActionWire.UPDATE_SEMANTICS,
  "bootstrap_label": d.DtoLabelOntologyProposedActionWire.BOOTSTRAP_LABEL,
  "rename_label": d.DtoLabelOntologyProposedActionWire.RENAME_LABEL,
  "split_label": d.DtoLabelOntologyProposedActionWire.SPLIT_LABEL,
  "merge_labels": d.DtoLabelOntologyProposedActionWire.MERGE_LABELS,
} as const
export function encodeDtoLabelOntologyProposedActionWire(value: unknown): d.DtoLabelOntologyProposedActionWire { return c.enumValue(value, DtoLabelOntologyProposedActionWireNames) }
export function decodeDtoLabelOntologyProposedActionWire(value: d.DtoLabelOntologyProposedActionWire): string { return c.enumName(value, DtoLabelOntologyProposedActionWireNames) }

export function encodeDtoLabelOntologyReviewAtomVariantWire(value: unknown): d.DtoLabelOntologyReviewAtomVariantWire {
  const dto = c.record(value, ["content_hash","polarity","kind","text","signal_count"])
  return create(d.DtoLabelOntologyReviewAtomVariantWireSchema, {
    contentHash: c.text(dto["content_hash"]),
    polarity: c.optional(dto["polarity"], (value) => c.text(value)),
    kind: c.optional(dto["kind"], (value) => c.text(value)),
    text: c.optional(dto["text"], (value) => c.text(value)),
    signalCount: c.int64(dto["signal_count"]),
  })
}
export function decodeDtoLabelOntologyReviewAtomVariantWire(wire: d.DtoLabelOntologyReviewAtomVariantWire): Record<string, unknown> {
  return c.omitUndefined({
    "content_hash": c.required(wire.contentHash, "content_hash"),
    "polarity": wire.polarity === undefined ? null : ((value) => value)(wire.polarity),
    "kind": wire.kind === undefined ? null : ((value) => value)(wire.kind),
    "text": wire.text === undefined ? null : ((value) => value)(wire.text),
    "signal_count": c.safeNumber(c.required(wire.signalCount, "signal_count")),
  })
}

const DtoLabelOntologyReviewGroupByWireNames = {
  "label": d.DtoLabelOntologyReviewGroupByWire.LABEL,
  "candidate_atom": d.DtoLabelOntologyReviewGroupByWire.CANDIDATE_ATOM,
  "proposed_label": d.DtoLabelOntologyReviewGroupByWire.PROPOSED_LABEL,
} as const
export function encodeDtoLabelOntologyReviewGroupByWire(value: unknown): d.DtoLabelOntologyReviewGroupByWire { return c.enumValue(value, DtoLabelOntologyReviewGroupByWireNames) }
export function decodeDtoLabelOntologyReviewGroupByWire(value: d.DtoLabelOntologyReviewGroupByWire): string { return c.enumName(value, DtoLabelOntologyReviewGroupByWireNames) }

export function encodeDtoLabelOntologyReviewGroupWire(value: unknown): d.DtoLabelOntologyReviewGroupWire {
  const dto = c.record(value, ["group_by","key","label_id","label_name","candidate_atom_polarity","candidate_atom_kind","candidate_text","candidate_content_hash","proposed_label_name","proposed_label_name_normalized","cluster_key","cluster_reason","task_count","signal_count","open_count","confirmed_count","resolved_count","rejected_count","superseded_count","degraded_count","average_score","median_score","oldest_signal_at","latest_signal_at","sample_task_refs","signal_ids","action_count","action_ids","proposal_ids","labels","candidate_atom_variants"])
  return create(d.DtoLabelOntologyReviewGroupWireSchema, {
    groupBy: encodeDtoLabelOntologyReviewGroupByWire(dto["group_by"]),
    key: c.text(dto["key"]),
    labelId: c.optional(dto["label_id"], (value) => c.text(value)),
    labelName: c.optional(dto["label_name"], (value) => c.text(value)),
    candidateAtomPolarity: c.optional(dto["candidate_atom_polarity"], (value) => c.text(value)),
    candidateAtomKind: c.optional(dto["candidate_atom_kind"], (value) => c.text(value)),
    candidateText: c.optional(dto["candidate_text"], (value) => c.text(value)),
    candidateContentHash: c.optional(dto["candidate_content_hash"], (value) => c.text(value)),
    proposedLabelName: c.optional(dto["proposed_label_name"], (value) => c.text(value)),
    proposedLabelNameNormalized: c.optional(dto["proposed_label_name_normalized"], (value) => c.text(value)),
    clusterKey: c.optional(dto["cluster_key"], (value) => c.text(value)),
    clusterReason: c.optional(dto["cluster_reason"], (value) => c.text(value)),
    taskCount: c.int64(dto["task_count"]),
    signalCount: c.int64(dto["signal_count"]),
    openCount: c.int64(dto["open_count"]),
    confirmedCount: c.int64(dto["confirmed_count"]),
    resolvedCount: c.int64(dto["resolved_count"]),
    rejectedCount: c.int64(dto["rejected_count"]),
    supersededCount: c.int64(dto["superseded_count"]),
    degradedCount: c.int64(dto["degraded_count"]),
    averageScore: c.optional(dto["average_score"], (value) => c.float(value)),
    medianScore: c.optional(dto["median_score"], (value) => c.float(value)),
    oldestSignalAt: c.int64(dto["oldest_signal_at"]),
    latestSignalAt: c.int64(dto["latest_signal_at"]),
    sampleTaskRefs: c.array(dto["sample_task_refs"], (value) => c.text(value)),
    signalIds: c.array(dto["signal_ids"], (value) => c.text(value)),
    actionCount: c.int64(dto["action_count"]),
    actionIds: c.array(dto["action_ids"], (value) => c.text(value)),
    proposalIds: c.array(dto["proposal_ids"], (value) => c.text(value)),
    labels: c.array(dto["labels"], (value) => encodeDtoLabelOntologyReviewLabelRefWire(value)),
    candidateAtomVariants: c.array(dto["candidate_atom_variants"], (value) => encodeDtoLabelOntologyReviewAtomVariantWire(value)),
  })
}
export function decodeDtoLabelOntologyReviewGroupWire(wire: d.DtoLabelOntologyReviewGroupWire): Record<string, unknown> {
  return c.omitUndefined({
    "group_by": decodeDtoLabelOntologyReviewGroupByWire(c.required(wire.groupBy, "group_by")),
    "key": c.required(wire.key, "key"),
    "label_id": wire.labelId === undefined ? null : ((value) => value)(wire.labelId),
    "label_name": wire.labelName === undefined ? null : ((value) => value)(wire.labelName),
    "candidate_atom_polarity": wire.candidateAtomPolarity === undefined ? null : ((value) => value)(wire.candidateAtomPolarity),
    "candidate_atom_kind": wire.candidateAtomKind === undefined ? null : ((value) => value)(wire.candidateAtomKind),
    "candidate_text": wire.candidateText === undefined ? null : ((value) => value)(wire.candidateText),
    "candidate_content_hash": wire.candidateContentHash === undefined ? null : ((value) => value)(wire.candidateContentHash),
    "proposed_label_name": wire.proposedLabelName === undefined ? null : ((value) => value)(wire.proposedLabelName),
    "proposed_label_name_normalized": wire.proposedLabelNameNormalized === undefined ? null : ((value) => value)(wire.proposedLabelNameNormalized),
    "cluster_key": wire.clusterKey === undefined ? null : ((value) => value)(wire.clusterKey),
    "cluster_reason": wire.clusterReason === undefined ? null : ((value) => value)(wire.clusterReason),
    "task_count": c.safeNumber(c.required(wire.taskCount, "task_count")),
    "signal_count": c.safeNumber(c.required(wire.signalCount, "signal_count")),
    "open_count": c.safeNumber(c.required(wire.openCount, "open_count")),
    "confirmed_count": c.safeNumber(c.required(wire.confirmedCount, "confirmed_count")),
    "resolved_count": c.safeNumber(c.required(wire.resolvedCount, "resolved_count")),
    "rejected_count": c.safeNumber(c.required(wire.rejectedCount, "rejected_count")),
    "superseded_count": c.safeNumber(c.required(wire.supersededCount, "superseded_count")),
    "degraded_count": c.safeNumber(c.required(wire.degradedCount, "degraded_count")),
    "average_score": wire.averageScore === undefined ? null : ((value) => c.float(value))(wire.averageScore),
    "median_score": wire.medianScore === undefined ? null : ((value) => c.float(value))(wire.medianScore),
    "oldest_signal_at": c.safeNumber(c.required(wire.oldestSignalAt, "oldest_signal_at")),
    "latest_signal_at": c.safeNumber(c.required(wire.latestSignalAt, "latest_signal_at")),
    "sample_task_refs": wire.sampleTaskRefs.map((value) => value),
    "signal_ids": wire.signalIds.map((value) => value),
    "action_count": c.safeNumber(c.required(wire.actionCount, "action_count")),
    "action_ids": wire.actionIds.map((value) => value),
    "proposal_ids": wire.proposalIds.map((value) => value),
    "labels": wire.labels.map((value) => decodeDtoLabelOntologyReviewLabelRefWire(value)),
    "candidate_atom_variants": wire.candidateAtomVariants.map((value) => decodeDtoLabelOntologyReviewAtomVariantWire(value)),
  })
}

export function encodeDtoLabelOntologyReviewLabelRefWire(value: unknown): d.DtoLabelOntologyReviewLabelRefWire {
  const dto = c.record(value, ["id","name"])
  return create(d.DtoLabelOntologyReviewLabelRefWireSchema, {
    id: c.text(dto["id"]),
    name: c.optional(dto["name"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelOntologyReviewLabelRefWire(wire: d.DtoLabelOntologyReviewLabelRefWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "name": wire.name === undefined ? null : ((value) => value)(wire.name),
  })
}

export function encodeDtoLabelOntologyReviewMeta(value: unknown): d.DtoLabelOntologyReviewMeta {
  const dto = c.record(value, ["group_by","include_all","limit"])
  return create(d.DtoLabelOntologyReviewMetaSchema, {
    groupBy: c.text(dto["group_by"]),
    includeAll: c.bool(dto["include_all"]),
    limit: c.int64(dto["limit"], true),
  })
}
export function decodeDtoLabelOntologyReviewMeta(wire: d.DtoLabelOntologyReviewMeta): Record<string, unknown> {
  return c.omitUndefined({
    "group_by": c.required(wire.groupBy, "group_by"),
    "include_all": c.required(wire.includeAll, "include_all"),
    "limit": c.safeNumber(c.required(wire.limit, "limit")),
  })
}

export function encodeDtoLabelOntologySignalDetailWire(value: unknown): d.DtoLabelOntologySignalDetailWire {
  const dto = c.record(value, ["signal","observation","actions"])
  return create(d.DtoLabelOntologySignalDetailWireSchema, {
    signal: encodeDtoLabelOntologySignalWire(dto["signal"]),
    observation: encodeDtoLabelOntologyObservationWire(dto["observation"]),
    actions: c.array(dto["actions"], (value) => encodeDtoLabelOntologyActionWire(value)),
  })
}
export function decodeDtoLabelOntologySignalDetailWire(wire: d.DtoLabelOntologySignalDetailWire): Record<string, unknown> {
  return c.omitUndefined({
    "signal": decodeDtoLabelOntologySignalWire(c.required(wire.signal, "signal")),
    "observation": decodeDtoLabelOntologyObservationWire(c.required(wire.observation, "observation")),
    "actions": wire.actions.map((value) => decodeDtoLabelOntologyActionWire(value)),
  })
}

const DtoLabelOntologySignalKindWireNames = {
  "false_negative": d.DtoLabelOntologySignalKindWire.FALSE_NEGATIVE,
  "false_positive": d.DtoLabelOntologySignalKindWire.FALSE_POSITIVE,
  "vocabulary_gap": d.DtoLabelOntologySignalKindWire.VOCABULARY_GAP,
  "name_issue": d.DtoLabelOntologySignalKindWire.NAME_ISSUE,
  "boundary_issue": d.DtoLabelOntologySignalKindWire.BOUNDARY_ISSUE,
  "structure_issue": d.DtoLabelOntologySignalKindWire.STRUCTURE_ISSUE,
} as const
export function encodeDtoLabelOntologySignalKindWire(value: unknown): d.DtoLabelOntologySignalKindWire { return c.enumValue(value, DtoLabelOntologySignalKindWireNames) }
export function decodeDtoLabelOntologySignalKindWire(value: d.DtoLabelOntologySignalKindWire): string { return c.enumName(value, DtoLabelOntologySignalKindWireNames) }

export function encodeDtoLabelOntologySignalRequest(value: unknown): d.DtoLabelOntologySignalRequest {
  const dto = c.record(value, ["kind","target_label_ref","related_labels","proposed_action","candidate_atom","proposed_label_name","proposal","agent_selected","suggest_state","suggest_score","suggest_rank","final_selected","rationale","confidence","signal_key"])
  return create(d.DtoLabelOntologySignalRequestSchema, {
    kind: encodeDtoLabelOntologySignalKindWire(dto["kind"]),
    targetLabelRef: c.optional(dto["target_label_ref"], (value) => c.text(value)),
    relatedLabels: c.present(dto["related_labels"], c.encodeJson),
    proposedAction: encodeDtoLabelOntologyProposedActionWire(dto["proposed_action"]),
    candidateAtom: c.optional(dto["candidate_atom"], (value) => encodeDtoLabelOntologyCandidateAtomRequest(value)),
    proposedLabelName: c.optional(dto["proposed_label_name"], (value) => c.text(value)),
    proposal: c.present(dto["proposal"], c.encodeJson),
    agentSelected: c.bool(dto["agent_selected"]),
    suggestState: c.optional(dto["suggest_state"], (value) => encodeDtoLabelOntologySuggestStateWire(value)),
    suggestScore: c.optional(dto["suggest_score"], (value) => c.float(value)),
    suggestRank: c.optional(dto["suggest_rank"], (value) => c.int64(value)),
    finalSelected: c.bool(dto["final_selected"]),
    rationale: c.text(dto["rationale"]),
    confidence: c.optional(dto["confidence"], (value) => c.float(value)),
    signalKey: c.optional(dto["signal_key"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelOntologySignalRequest(wire: d.DtoLabelOntologySignalRequest): Record<string, unknown> {
  return c.omitUndefined({
    "kind": decodeDtoLabelOntologySignalKindWire(c.required(wire.kind, "kind")),
    "target_label_ref": wire.targetLabelRef === undefined ? null : ((value) => value)(wire.targetLabelRef),
    "related_labels": wire.relatedLabels === undefined ? undefined : c.decodeJson(wire.relatedLabels),
    "proposed_action": decodeDtoLabelOntologyProposedActionWire(c.required(wire.proposedAction, "proposed_action")),
    "candidate_atom": wire.candidateAtom === undefined ? null : ((value) => decodeDtoLabelOntologyCandidateAtomRequest(value))(wire.candidateAtom),
    "proposed_label_name": wire.proposedLabelName === undefined ? null : ((value) => value)(wire.proposedLabelName),
    "proposal": wire.proposal === undefined ? undefined : c.decodeJson(wire.proposal),
    "agent_selected": c.required(wire.agentSelected, "agent_selected"),
    "suggest_state": wire.suggestState === undefined ? null : ((value) => decodeDtoLabelOntologySuggestStateWire(value))(wire.suggestState),
    "suggest_score": wire.suggestScore === undefined ? null : ((value) => c.float(value))(wire.suggestScore),
    "suggest_rank": wire.suggestRank === undefined ? null : ((value) => c.safeNumber(value))(wire.suggestRank),
    "final_selected": c.required(wire.finalSelected, "final_selected"),
    "rationale": c.required(wire.rationale, "rationale"),
    "confidence": wire.confidence === undefined ? null : ((value) => c.float(value))(wire.confidence),
    "signal_key": wire.signalKey === undefined ? null : ((value) => value)(wire.signalKey),
  })
}

export function encodeDtoLabelOntologySignalReviewedPayload(value: unknown): d.DtoLabelOntologySignalReviewedPayload {
  const dto = c.record(value, ["signal_id","action_id","status","reason"])
  return create(d.DtoLabelOntologySignalReviewedPayloadSchema, {
    signalId: c.text(dto["signal_id"]),
    actionId: c.text(dto["action_id"]),
    status: encodeDtoSignalStatus(dto["status"]),
    reason: c.text(dto["reason"]),
  })
}
export function decodeDtoLabelOntologySignalReviewedPayload(wire: d.DtoLabelOntologySignalReviewedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "signal_id": c.required(wire.signalId, "signal_id"),
    "action_id": c.required(wire.actionId, "action_id"),
    "status": decodeDtoSignalStatus(c.required(wire.status, "status")),
    "reason": c.required(wire.reason, "reason"),
  })
}

export function encodeDtoLabelOntologySignalWire(value: unknown): d.DtoLabelOntologySignalWire {
  const dto = c.record(value, ["id","observation_id","board_id","kind","status","target_label_id","target_label_name_snapshot","related_labels","proposed_action","candidate_atom_polarity","candidate_atom_kind","candidate_text","candidate_content_hash","proposed_label_name","proposed_label_name_normalized","proposal","agent_selected","suggest_state","suggest_score","suggest_rank","final_selected","rationale","confidence","signal_key","superseded_by_signal_id","status_reason","created_at","updated_at","reviewed_at","closed_at"])
  return create(d.DtoLabelOntologySignalWireSchema, {
    id: c.text(dto["id"]),
    observationId: c.text(dto["observation_id"]),
    boardId: c.text(dto["board_id"]),
    kind: c.text(dto["kind"]),
    status: c.text(dto["status"]),
    targetLabelId: c.optional(dto["target_label_id"], (value) => c.text(value)),
    targetLabelNameSnapshot: c.optional(dto["target_label_name_snapshot"], (value) => c.text(value)),
    relatedLabels: encodeDtoJsonArray(dto["related_labels"]),
    proposedAction: c.text(dto["proposed_action"]),
    candidateAtomPolarity: c.optional(dto["candidate_atom_polarity"], (value) => c.text(value)),
    candidateAtomKind: c.optional(dto["candidate_atom_kind"], (value) => c.text(value)),
    candidateText: c.optional(dto["candidate_text"], (value) => c.text(value)),
    candidateContentHash: c.optional(dto["candidate_content_hash"], (value) => c.text(value)),
    proposedLabelName: c.optional(dto["proposed_label_name"], (value) => c.text(value)),
    proposedLabelNameNormalized: c.optional(dto["proposed_label_name_normalized"], (value) => c.text(value)),
    proposal: encodeDtoStructuredMetadataJsonObject(dto["proposal"]),
    agentSelected: c.bool(dto["agent_selected"]),
    suggestState: c.optional(dto["suggest_state"], (value) => c.text(value)),
    suggestScore: c.optional(dto["suggest_score"], (value) => c.float(value)),
    suggestRank: c.optional(dto["suggest_rank"], (value) => c.int64(value)),
    finalSelected: c.bool(dto["final_selected"]),
    rationale: c.text(dto["rationale"]),
    confidence: c.optional(dto["confidence"], (value) => c.float(value)),
    signalKey: c.text(dto["signal_key"]),
    supersededBySignalId: c.optional(dto["superseded_by_signal_id"], (value) => c.text(value)),
    statusReason: c.optional(dto["status_reason"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    reviewedAt: c.optional(dto["reviewed_at"], (value) => c.int64(value)),
    closedAt: c.optional(dto["closed_at"], (value) => c.int64(value)),
  })
}
export function decodeDtoLabelOntologySignalWire(wire: d.DtoLabelOntologySignalWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "observation_id": c.required(wire.observationId, "observation_id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "kind": c.required(wire.kind, "kind"),
    "status": c.required(wire.status, "status"),
    "target_label_id": wire.targetLabelId === undefined ? null : ((value) => value)(wire.targetLabelId),
    "target_label_name_snapshot": wire.targetLabelNameSnapshot === undefined ? null : ((value) => value)(wire.targetLabelNameSnapshot),
    "related_labels": decodeDtoJsonArray(c.required(wire.relatedLabels, "related_labels")),
    "proposed_action": c.required(wire.proposedAction, "proposed_action"),
    "candidate_atom_polarity": wire.candidateAtomPolarity === undefined ? null : ((value) => value)(wire.candidateAtomPolarity),
    "candidate_atom_kind": wire.candidateAtomKind === undefined ? null : ((value) => value)(wire.candidateAtomKind),
    "candidate_text": wire.candidateText === undefined ? null : ((value) => value)(wire.candidateText),
    "candidate_content_hash": wire.candidateContentHash === undefined ? null : ((value) => value)(wire.candidateContentHash),
    "proposed_label_name": wire.proposedLabelName === undefined ? null : ((value) => value)(wire.proposedLabelName),
    "proposed_label_name_normalized": wire.proposedLabelNameNormalized === undefined ? null : ((value) => value)(wire.proposedLabelNameNormalized),
    "proposal": decodeDtoStructuredMetadataJsonObject(c.required(wire.proposal, "proposal")),
    "agent_selected": c.required(wire.agentSelected, "agent_selected"),
    "suggest_state": wire.suggestState === undefined ? null : ((value) => value)(wire.suggestState),
    "suggest_score": wire.suggestScore === undefined ? null : ((value) => c.float(value))(wire.suggestScore),
    "suggest_rank": wire.suggestRank === undefined ? null : ((value) => c.safeNumber(value))(wire.suggestRank),
    "final_selected": c.required(wire.finalSelected, "final_selected"),
    "rationale": c.required(wire.rationale, "rationale"),
    "confidence": wire.confidence === undefined ? null : ((value) => c.float(value))(wire.confidence),
    "signal_key": c.required(wire.signalKey, "signal_key"),
    "superseded_by_signal_id": wire.supersededBySignalId === undefined ? null : ((value) => value)(wire.supersededBySignalId),
    "status_reason": wire.statusReason === undefined ? null : ((value) => value)(wire.statusReason),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "reviewed_at": wire.reviewedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.reviewedAt),
    "closed_at": wire.closedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.closedAt),
  })
}

const DtoLabelOntologySuggestStateWireNames = {
  "selected": d.DtoLabelOntologySuggestStateWire.SELECTED,
  "candidate": d.DtoLabelOntologySuggestStateWire.CANDIDATE,
  "absent": d.DtoLabelOntologySuggestStateWire.ABSENT,
  "unavailable": d.DtoLabelOntologySuggestStateWire.UNAVAILABLE,
} as const
export function encodeDtoLabelOntologySuggestStateWire(value: unknown): d.DtoLabelOntologySuggestStateWire { return c.enumValue(value, DtoLabelOntologySuggestStateWireNames) }
export function decodeDtoLabelOntologySuggestStateWire(value: d.DtoLabelOntologySuggestStateWire): string { return c.enumName(value, DtoLabelOntologySuggestStateWireNames) }

const DtoLabelOntologyValidationEffectiveOutcomeWireNames = {
  "not_required": d.DtoLabelOntologyValidationEffectiveOutcomeWire.NOT_REQUIRED,
  "unsupported": d.DtoLabelOntologyValidationEffectiveOutcomeWire.UNSUPPORTED,
  "pending": d.DtoLabelOntologyValidationEffectiveOutcomeWire.PENDING,
  "passed": d.DtoLabelOntologyValidationEffectiveOutcomeWire.PASSED,
  "failed": d.DtoLabelOntologyValidationEffectiveOutcomeWire.FAILED,
  "partial": d.DtoLabelOntologyValidationEffectiveOutcomeWire.PARTIAL,
} as const
export function encodeDtoLabelOntologyValidationEffectiveOutcomeWire(value: unknown): d.DtoLabelOntologyValidationEffectiveOutcomeWire { return c.enumValue(value, DtoLabelOntologyValidationEffectiveOutcomeWireNames) }
export function decodeDtoLabelOntologyValidationEffectiveOutcomeWire(value: d.DtoLabelOntologyValidationEffectiveOutcomeWire): string { return c.enumName(value, DtoLabelOntologyValidationEffectiveOutcomeWireNames) }

const DtoLabelOntologyValidationRequirementWireNames = {
  "none": d.DtoLabelOntologyValidationRequirementWire.NONE,
  "required": d.DtoLabelOntologyValidationRequirementWire.REQUIRED,
  "unsupported": d.DtoLabelOntologyValidationRequirementWire.UNSUPPORTED,
} as const
export function encodeDtoLabelOntologyValidationRequirementWire(value: unknown): d.DtoLabelOntologyValidationRequirementWire { return c.enumValue(value, DtoLabelOntologyValidationRequirementWireNames) }
export function decodeDtoLabelOntologyValidationRequirementWire(value: d.DtoLabelOntologyValidationRequirementWire): string { return c.enumName(value, DtoLabelOntologyValidationRequirementWireNames) }

const DtoLabelOntologyValidationStatusWireNames = {
  "not_required": d.DtoLabelOntologyValidationStatusWire.NOT_REQUIRED,
  "pending": d.DtoLabelOntologyValidationStatusWire.PENDING,
  "passed": d.DtoLabelOntologyValidationStatusWire.PASSED,
  "failed": d.DtoLabelOntologyValidationStatusWire.FAILED,
  "partial": d.DtoLabelOntologyValidationStatusWire.PARTIAL,
} as const
export function encodeDtoLabelOntologyValidationStatusWire(value: unknown): d.DtoLabelOntologyValidationStatusWire { return c.enumValue(value, DtoLabelOntologyValidationStatusWireNames) }
export function decodeDtoLabelOntologyValidationStatusWire(value: d.DtoLabelOntologyValidationStatusWire): string { return c.enumName(value, DtoLabelOntologyValidationStatusWireNames) }

export function encodeDtoLabelProposalAttemptWire(value: unknown): d.DtoLabelProposalAttemptWire {
  const dto = c.record(value, ["task_id","board_id","proposal","degraded","diagnostics","heuristic_coverage","heuristic_coverage_cosine","heuristic_residual_norm","top1_existing_label_id","top1_existing_label_name"])
  return create(d.DtoLabelProposalAttemptWireSchema, {
    taskId: c.text(dto["task_id"]),
    boardId: c.text(dto["board_id"]),
    proposal: c.optional(dto["proposal"], (value) => encodeDtoLabelSemanticProposalWire(value)),
    degraded: c.bool(dto["degraded"]),
    diagnostics: c.array(dto["diagnostics"], (value) => c.text(value)),
    heuristicCoverage: c.float(dto["heuristic_coverage"], true),
    heuristicCoverageCosine: c.float(dto["heuristic_coverage_cosine"], true),
    heuristicResidualNorm: c.float(dto["heuristic_residual_norm"], true),
    top1ExistingLabelId: c.optional(dto["top1_existing_label_id"], (value) => c.text(value)),
    top1ExistingLabelName: c.optional(dto["top1_existing_label_name"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelProposalAttemptWire(wire: d.DtoLabelProposalAttemptWire): Record<string, unknown> {
  return c.omitUndefined({
    "task_id": c.required(wire.taskId, "task_id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "proposal": wire.proposal === undefined ? null : ((value) => decodeDtoLabelSemanticProposalWire(value))(wire.proposal),
    "degraded": c.required(wire.degraded, "degraded"),
    "diagnostics": wire.diagnostics.map((value) => value),
    "heuristic_coverage": c.float(c.required(wire.heuristicCoverage, "heuristic_coverage")),
    "heuristic_coverage_cosine": c.float(c.required(wire.heuristicCoverageCosine, "heuristic_coverage_cosine")),
    "heuristic_residual_norm": c.float(c.required(wire.heuristicResidualNorm, "heuristic_residual_norm")),
    "top1_existing_label_id": wire.top1ExistingLabelId === undefined ? null : ((value) => value)(wire.top1ExistingLabelId),
    "top1_existing_label_name": wire.top1ExistingLabelName === undefined ? null : ((value) => value)(wire.top1ExistingLabelName),
  })
}

export function encodeDtoLabelProposalCandidateWire(value: unknown): d.DtoLabelProposalCandidateWire {
  const dto = c.record(value, ["name","description","applies_when","excludes_when","positive_examples","negative_examples"])
  return create(d.DtoLabelProposalCandidateWireSchema, {
    name: c.text(dto["name"]),
    description: c.optional(dto["description"], (value) => c.text(value)),
    appliesWhen: c.array((dto["applies_when"] ?? []), (value) => c.text(value)),
    excludesWhen: c.array((dto["excludes_when"] ?? []), (value) => c.text(value)),
    positiveExamples: c.array((dto["positive_examples"] ?? []), (value) => c.text(value)),
    negativeExamples: c.array((dto["negative_examples"] ?? []), (value) => c.text(value)),
  })
}
export function decodeDtoLabelProposalCandidateWire(wire: d.DtoLabelProposalCandidateWire): Record<string, unknown> {
  return c.omitUndefined({
    "name": c.required(wire.name, "name"),
    "description": wire.description === undefined ? null : ((value) => value)(wire.description),
    "applies_when": wire.appliesWhen.map((value) => value),
    "excludes_when": wire.excludesWhen.map((value) => value),
    "positive_examples": wire.positiveExamples.map((value) => value),
    "negative_examples": wire.negativeExamples.map((value) => value),
  })
}

export function encodeDtoLabelProposalPayload(value: unknown): d.DtoLabelProposalPayload {
  const dto = c.record(value, ["proposal_id","name","status"])
  return create(d.DtoLabelProposalPayloadSchema, {
    proposalId: c.text(dto["proposal_id"]),
    name: c.text(dto["name"]),
    status: encodeDtoLabelProposalStatus(dto["status"]),
  })
}
export function decodeDtoLabelProposalPayload(wire: d.DtoLabelProposalPayload): Record<string, unknown> {
  return c.omitUndefined({
    "proposal_id": c.required(wire.proposalId, "proposal_id"),
    "name": c.required(wire.name, "name"),
    "status": decodeDtoLabelProposalStatus(c.required(wire.status, "status")),
  })
}

const DtoLabelProposalStatusNames = {
  "proposed": d.DtoLabelProposalStatus.PROPOSED,
  "accepted": d.DtoLabelProposalStatus.ACCEPTED,
  "rejected": d.DtoLabelProposalStatus.REJECTED,
} as const
export function encodeDtoLabelProposalStatus(value: unknown): d.DtoLabelProposalStatus { return c.enumValue(value, DtoLabelProposalStatusNames) }
export function decodeDtoLabelProposalStatus(value: d.DtoLabelProposalStatus): string { return c.enumName(value, DtoLabelProposalStatusNames) }

const DtoLabelProposalStatusWireNames = {
  "proposed": d.DtoLabelProposalStatusWire.PROPOSED,
  "accepted": d.DtoLabelProposalStatusWire.ACCEPTED,
  "rejected": d.DtoLabelProposalStatusWire.REJECTED,
} as const
export function encodeDtoLabelProposalStatusWire(value: unknown): d.DtoLabelProposalStatusWire { return c.enumValue(value, DtoLabelProposalStatusWireNames) }
export function decodeDtoLabelProposalStatusWire(value: d.DtoLabelProposalStatusWire): string { return c.enumName(value, DtoLabelProposalStatusWireNames) }

export function encodeDtoLabelSemanticProposalWire(value: unknown): d.DtoLabelSemanticProposalWire {
  const dto = c.record(value, ["id","board_id","task_id","status","name","description","applies_when","excludes_when","positive_examples","negative_examples","heuristic_coverage","heuristic_coverage_cosine","heuristic_residual_norm","top1_existing_label_id","top1_existing_label_name","diagnostics","created_by","decision_reason","resolved_label_id","created_at","updated_at","decided_at"])
  return create(d.DtoLabelSemanticProposalWireSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    taskId: c.text(dto["task_id"]),
    status: encodeDtoLabelProposalStatusWire(dto["status"]),
    name: c.text(dto["name"]),
    description: c.optional(dto["description"], (value) => c.text(value)),
    appliesWhen: c.array(dto["applies_when"], (value) => c.text(value)),
    excludesWhen: c.array(dto["excludes_when"], (value) => c.text(value)),
    positiveExamples: c.array(dto["positive_examples"], (value) => c.text(value)),
    negativeExamples: c.array(dto["negative_examples"], (value) => c.text(value)),
    heuristicCoverage: c.float(dto["heuristic_coverage"], true),
    heuristicCoverageCosine: c.float(dto["heuristic_coverage_cosine"], true),
    heuristicResidualNorm: c.float(dto["heuristic_residual_norm"], true),
    top1ExistingLabelId: c.optional(dto["top1_existing_label_id"], (value) => c.text(value)),
    top1ExistingLabelName: c.optional(dto["top1_existing_label_name"], (value) => c.text(value)),
    diagnostics: c.array(dto["diagnostics"], (value) => c.text(value)),
    createdBy: c.text(dto["created_by"]),
    decisionReason: c.optional(dto["decision_reason"], (value) => c.text(value)),
    resolvedLabelId: c.optional(dto["resolved_label_id"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    decidedAt: c.optional(dto["decided_at"], (value) => c.int64(value)),
  })
}
export function decodeDtoLabelSemanticProposalWire(wire: d.DtoLabelSemanticProposalWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": c.required(wire.taskId, "task_id"),
    "status": decodeDtoLabelProposalStatusWire(c.required(wire.status, "status")),
    "name": c.required(wire.name, "name"),
    "description": wire.description === undefined ? null : ((value) => value)(wire.description),
    "applies_when": wire.appliesWhen.map((value) => value),
    "excludes_when": wire.excludesWhen.map((value) => value),
    "positive_examples": wire.positiveExamples.map((value) => value),
    "negative_examples": wire.negativeExamples.map((value) => value),
    "heuristic_coverage": c.float(c.required(wire.heuristicCoverage, "heuristic_coverage")),
    "heuristic_coverage_cosine": c.float(c.required(wire.heuristicCoverageCosine, "heuristic_coverage_cosine")),
    "heuristic_residual_norm": c.float(c.required(wire.heuristicResidualNorm, "heuristic_residual_norm")),
    "top1_existing_label_id": wire.top1ExistingLabelId === undefined ? null : ((value) => value)(wire.top1ExistingLabelId),
    "top1_existing_label_name": wire.top1ExistingLabelName === undefined ? null : ((value) => value)(wire.top1ExistingLabelName),
    "diagnostics": wire.diagnostics.map((value) => value),
    "created_by": c.required(wire.createdBy, "created_by"),
    "decision_reason": wire.decisionReason === undefined ? null : ((value) => value)(wire.decisionReason),
    "resolved_label_id": wire.resolvedLabelId === undefined ? null : ((value) => value)(wire.resolvedLabelId),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "decided_at": wire.decidedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.decidedAt),
  })
}

export function encodeDtoLabelSemanticsWire(value: unknown): d.DtoLabelSemanticsWire {
  const dto = c.record(value, ["label_id","board_id","label_name","semantics_hash","description","applies_when","excludes_when","positive_examples","negative_examples","created_at","updated_at","atoms"])
  return create(d.DtoLabelSemanticsWireSchema, {
    labelId: c.text(dto["label_id"]),
    boardId: c.text(dto["board_id"]),
    labelName: c.text(dto["label_name"]),
    semanticsHash: c.text(dto["semantics_hash"]),
    description: c.optional(dto["description"], (value) => c.text(value)),
    appliesWhen: c.array(dto["applies_when"], (value) => c.text(value)),
    excludesWhen: c.array(dto["excludes_when"], (value) => c.text(value)),
    positiveExamples: c.array(dto["positive_examples"], (value) => c.text(value)),
    negativeExamples: c.array(dto["negative_examples"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    atoms: c.array(dto["atoms"], (value) => encodeDtoLabelAtomWire(value)),
  })
}
export function decodeDtoLabelSemanticsWire(wire: d.DtoLabelSemanticsWire): Record<string, unknown> {
  return c.omitUndefined({
    "label_id": c.required(wire.labelId, "label_id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "label_name": c.required(wire.labelName, "label_name"),
    "semantics_hash": c.required(wire.semanticsHash, "semantics_hash"),
    "description": wire.description === undefined ? null : ((value) => value)(wire.description),
    "applies_when": wire.appliesWhen.map((value) => value),
    "excludes_when": wire.excludesWhen.map((value) => value),
    "positive_examples": wire.positiveExamples.map((value) => value),
    "negative_examples": wire.negativeExamples.map((value) => value),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "atoms": wire.atoms.map((value) => decodeDtoLabelAtomWire(value)),
  })
}

export function encodeDtoLabelSuggestionCandidateWire(value: unknown): d.DtoLabelSuggestionCandidateWire {
  const dto = c.record(value, ["label_id","label_name","score","weight","already_applied","evidence_atoms","negative_evidence_atoms"])
  return create(d.DtoLabelSuggestionCandidateWireSchema, {
    labelId: c.text(dto["label_id"]),
    labelName: c.text(dto["label_name"]),
    score: c.float(dto["score"], true),
    weight: c.float(dto["weight"], true),
    alreadyApplied: c.bool(dto["already_applied"]),
    evidenceAtoms: c.array(dto["evidence_atoms"], (value) => encodeDtoLabelSuggestionEvidenceAtomWire(value)),
    negativeEvidenceAtoms: c.array(dto["negative_evidence_atoms"], (value) => encodeDtoLabelSuggestionEvidenceAtomWire(value)),
  })
}
export function decodeDtoLabelSuggestionCandidateWire(wire: d.DtoLabelSuggestionCandidateWire): Record<string, unknown> {
  return c.omitUndefined({
    "label_id": c.required(wire.labelId, "label_id"),
    "label_name": c.required(wire.labelName, "label_name"),
    "score": c.float(c.required(wire.score, "score")),
    "weight": c.float(c.required(wire.weight, "weight")),
    "already_applied": c.required(wire.alreadyApplied, "already_applied"),
    "evidence_atoms": wire.evidenceAtoms.map((value) => decodeDtoLabelSuggestionEvidenceAtomWire(value)),
    "negative_evidence_atoms": wire.negativeEvidenceAtoms.map((value) => decodeDtoLabelSuggestionEvidenceAtomWire(value)),
  })
}

export function encodeDtoLabelSuggestionEvidenceAtomWire(value: unknown): d.DtoLabelSuggestionEvidenceAtomWire {
  const dto = c.record(value, ["atom_id","label_id","label_name","polarity","kind","text","score"])
  return create(d.DtoLabelSuggestionEvidenceAtomWireSchema, {
    atomId: c.text(dto["atom_id"]),
    labelId: c.text(dto["label_id"]),
    labelName: c.text(dto["label_name"]),
    polarity: c.text(dto["polarity"]),
    kind: c.text(dto["kind"]),
    text: c.text(dto["text"]),
    score: c.float(dto["score"], true),
  })
}
export function decodeDtoLabelSuggestionEvidenceAtomWire(wire: d.DtoLabelSuggestionEvidenceAtomWire): Record<string, unknown> {
  return c.omitUndefined({
    "atom_id": c.required(wire.atomId, "atom_id"),
    "label_id": c.required(wire.labelId, "label_id"),
    "label_name": c.required(wire.labelName, "label_name"),
    "polarity": c.required(wire.polarity, "polarity"),
    "kind": c.required(wire.kind, "kind"),
    "text": c.required(wire.text, "text"),
    "score": c.float(c.required(wire.score, "score")),
  })
}

export function encodeDtoLabelSuggestionResultWire(value: unknown): d.DtoLabelSuggestionResultWire {
  const dto = c.record(value, ["task_id","board_id","selected_labels","candidates","coverage","coverage_cosine","residual_norm","needs_new_label","reason_codes","degraded","diagnostics"])
  return create(d.DtoLabelSuggestionResultWireSchema, {
    taskId: c.text(dto["task_id"]),
    boardId: c.text(dto["board_id"]),
    selectedLabels: c.array(dto["selected_labels"], (value) => encodeDtoLabelSuggestionCandidateWire(value)),
    candidates: c.array(dto["candidates"], (value) => encodeDtoLabelSuggestionCandidateWire(value)),
    coverage: c.float(dto["coverage"], true),
    coverageCosine: c.float(dto["coverage_cosine"], true),
    residualNorm: c.float(dto["residual_norm"], true),
    needsNewLabel: c.bool(dto["needs_new_label"]),
    reasonCodes: c.array(dto["reason_codes"], (value) => c.text(value)),
    degraded: c.bool(dto["degraded"]),
    diagnostics: c.array(dto["diagnostics"], (value) => c.text(value)),
  })
}
export function decodeDtoLabelSuggestionResultWire(wire: d.DtoLabelSuggestionResultWire): Record<string, unknown> {
  return c.omitUndefined({
    "task_id": c.required(wire.taskId, "task_id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "selected_labels": wire.selectedLabels.map((value) => decodeDtoLabelSuggestionCandidateWire(value)),
    "candidates": wire.candidates.map((value) => decodeDtoLabelSuggestionCandidateWire(value)),
    "coverage": c.float(c.required(wire.coverage, "coverage")),
    "coverage_cosine": c.float(c.required(wire.coverageCosine, "coverage_cosine")),
    "residual_norm": c.float(c.required(wire.residualNorm, "residual_norm")),
    "needs_new_label": c.required(wire.needsNewLabel, "needs_new_label"),
    "reason_codes": wire.reasonCodes.map((value) => value),
    "degraded": c.required(wire.degraded, "degraded"),
    "diagnostics": wire.diagnostics.map((value) => value),
  })
}

export function encodeDtoLegacyImportReport(value: unknown): d.DtoLegacyImportReport {
  const dto = c.record(value, ["journal_id","phase","source_path","source_fingerprint","schema_fingerprint","resumed","attachment_count","table_counts"])
  return create(d.DtoLegacyImportReportSchema, {
    journalId: c.text(dto["journal_id"]),
    phase: c.text(dto["phase"]),
    sourcePath: c.text(dto["source_path"]),
    sourceFingerprint: c.text(dto["source_fingerprint"]),
    schemaFingerprint: c.text(dto["schema_fingerprint"]),
    resumed: c.bool(dto["resumed"]),
    attachmentCount: c.int64(dto["attachment_count"], true),
    tableCounts: c.array(dto["table_counts"], (value) => encodeDtoLegacyImportTableCount(value)),
  })
}
export function decodeDtoLegacyImportReport(wire: d.DtoLegacyImportReport): Record<string, unknown> {
  return c.omitUndefined({
    "journal_id": c.required(wire.journalId, "journal_id"),
    "phase": c.required(wire.phase, "phase"),
    "source_path": c.required(wire.sourcePath, "source_path"),
    "source_fingerprint": c.required(wire.sourceFingerprint, "source_fingerprint"),
    "schema_fingerprint": c.required(wire.schemaFingerprint, "schema_fingerprint"),
    "resumed": c.required(wire.resumed, "resumed"),
    "attachment_count": c.safeNumber(c.required(wire.attachmentCount, "attachment_count")),
    "table_counts": wire.tableCounts.map((value) => decodeDtoLegacyImportTableCount(value)),
  })
}

export function encodeDtoLegacyImportTableCount(value: unknown): d.DtoLegacyImportTableCount {
  const dto = c.record(value, ["table","source_rows","target_rows"])
  return create(d.DtoLegacyImportTableCountSchema, {
    table: c.text(dto["table"]),
    sourceRows: c.int64(dto["source_rows"], true),
    targetRows: c.int64(dto["target_rows"], true),
  })
}
export function decodeDtoLegacyImportTableCount(wire: d.DtoLegacyImportTableCount): Record<string, unknown> {
  return c.omitUndefined({
    "table": c.required(wire.table, "table"),
    "source_rows": c.safeNumber(c.required(wire.sourceRows, "source_rows")),
    "target_rows": c.safeNumber(c.required(wire.targetRows, "target_rows")),
  })
}

export function encodeDtoLimitMeta(value: unknown): d.DtoLimitMeta {
  const dto = c.record(value, ["limit"])
  return create(d.DtoLimitMetaSchema, {
    limit: c.int64(dto["limit"], true),
  })
}
export function decodeDtoLimitMeta(wire: d.DtoLimitMeta): Record<string, unknown> {
  return c.omitUndefined({
    "limit": c.safeNumber(c.required(wire.limit, "limit")),
  })
}

export function encodeDtoListTasksByStatusData(value: unknown): d.DtoListTasksByStatusData {
  const dto = c.record(value, ["statuses"])
  return create(d.DtoListTasksByStatusDataSchema, {
    statuses: c.array(dto["statuses"], (value) => encodeDtoListTasksStatusWindow(value)),
  })
}
export function decodeDtoListTasksByStatusData(wire: d.DtoListTasksByStatusData): Record<string, unknown> {
  return c.omitUndefined({
    "statuses": wire.statuses.map((value) => decodeDtoListTasksStatusWindow(value)),
  })
}

export function encodeDtoListTasksStatusWindow(value: unknown): d.DtoListTasksStatusWindow {
  const dto = c.record(value, ["status","tasks","page"])
  return create(d.DtoListTasksStatusWindowSchema, {
    status: encodeDtoApiTaskStatus(dto["status"]),
    tasks: c.array(dto["tasks"], (value) => encodeDtoApiTask(value)),
    page: encodeDtoTotalPaginationMeta(dto["page"]),
  })
}
export function decodeDtoListTasksStatusWindow(wire: d.DtoListTasksStatusWindow): Record<string, unknown> {
  return c.omitUndefined({
    "status": decodeDtoApiTaskStatus(c.required(wire.status, "status")),
    "tasks": wire.tasks.map((value) => decodeDtoApiTask(value)),
    "page": decodeDtoTotalPaginationMeta(c.required(wire.page, "page")),
  })
}

export function encodeDtoMaintenanceOwnerStatus(value: unknown): d.DtoMaintenanceOwnerStatus {
  const dto = c.record(value, ["owner","mode","lease_expires_at","fence_epoch","build_identity","last_heartbeat_at","active"])
  return create(d.DtoMaintenanceOwnerStatusSchema, {
    owner: c.optional(dto["owner"], (value) => c.text(value)),
    mode: c.optional(dto["mode"], (value) => c.text(value)),
    leaseExpiresAt: c.optional(dto["lease_expires_at"], (value) => c.int64(value)),
    fenceEpoch: c.int64(dto["fence_epoch"]),
    buildIdentity: c.optional(dto["build_identity"], (value) => c.text(value)),
    lastHeartbeatAt: c.optional(dto["last_heartbeat_at"], (value) => c.int64(value)),
    active: c.bool(dto["active"]),
  })
}
export function decodeDtoMaintenanceOwnerStatus(wire: d.DtoMaintenanceOwnerStatus): Record<string, unknown> {
  return c.omitUndefined({
    "owner": wire.owner === undefined ? null : ((value) => value)(wire.owner),
    "mode": wire.mode === undefined ? null : ((value) => value)(wire.mode),
    "lease_expires_at": wire.leaseExpiresAt === undefined ? null : ((value) => c.safeNumber(value))(wire.leaseExpiresAt),
    "fence_epoch": c.safeNumber(c.required(wire.fenceEpoch, "fence_epoch")),
    "build_identity": wire.buildIdentity === undefined ? null : ((value) => value)(wire.buildIdentity),
    "last_heartbeat_at": wire.lastHeartbeatAt === undefined ? null : ((value) => c.safeNumber(value))(wire.lastHeartbeatAt),
    "active": c.required(wire.active, "active"),
  })
}

export function encodeDtoMaintenanceRunReport(value: unknown): d.DtoMaintenanceRunReport {
  const dto = c.record(value, ["database_instance_id","protocol_version","owner","mode","action","processed","phase","degraded","errors","stores"])
  return create(d.DtoMaintenanceRunReportSchema, {
    databaseInstanceId: c.text(dto["database_instance_id"]),
    protocolVersion: c.int64(dto["protocol_version"]),
    owner: c.text(dto["owner"]),
    mode: c.text(dto["mode"]),
    action: c.text(dto["action"]),
    processed: c.int64(dto["processed"], true),
    phase: c.present(dto["phase"], (value) => c.text(value)),
    degraded: c.present(dto["degraded"], (value) => c.bool(value)),
    errors: c.array((dto["errors"] ?? []), (value) => c.text(value)),
    stores: c.array(dto["stores"], (value) => encodeDtoProjectionStoreStatus(value)),
  })
}
export function decodeDtoMaintenanceRunReport(wire: d.DtoMaintenanceRunReport): Record<string, unknown> {
  return c.omitUndefined({
    "database_instance_id": c.required(wire.databaseInstanceId, "database_instance_id"),
    "protocol_version": c.safeNumber(c.required(wire.protocolVersion, "protocol_version")),
    "owner": c.required(wire.owner, "owner"),
    "mode": c.required(wire.mode, "mode"),
    "action": c.required(wire.action, "action"),
    "processed": c.safeNumber(c.required(wire.processed, "processed")),
    "phase": c.required(wire.phase, "phase"),
    "degraded": c.required(wire.degraded, "degraded"),
    "errors": wire.errors.map((value) => value),
    "stores": wire.stores.map((value) => decodeDtoProjectionStoreStatus(value)),
  })
}

export function encodeDtoMaintenanceStatusReport(value: unknown): d.DtoMaintenanceStatusReport {
  const dto = c.record(value, ["database_instance_id","protocol_version","owner","stores"])
  return create(d.DtoMaintenanceStatusReportSchema, {
    databaseInstanceId: c.text(dto["database_instance_id"]),
    protocolVersion: c.int64(dto["protocol_version"]),
    owner: encodeDtoMaintenanceOwnerStatus(dto["owner"]),
    stores: c.array(dto["stores"], (value) => encodeDtoProjectionStoreStatus(value)),
  })
}
export function decodeDtoMaintenanceStatusReport(wire: d.DtoMaintenanceStatusReport): Record<string, unknown> {
  return c.omitUndefined({
    "database_instance_id": c.required(wire.databaseInstanceId, "database_instance_id"),
    "protocol_version": c.safeNumber(c.required(wire.protocolVersion, "protocol_version")),
    "owner": decodeDtoMaintenanceOwnerStatus(c.required(wire.owner, "owner")),
    "stores": wire.stores.map((value) => decodeDtoProjectionStoreStatus(value)),
  })
}

export function encodeDtoNextAfterMeta(value: unknown): d.DtoNextAfterMeta {
  const dto = c.record(value, ["next_after"])
  return create(d.DtoNextAfterMetaSchema, {
    nextAfter: c.int64(dto["next_after"]),
  })
}
export function decodeDtoNextAfterMeta(wire: d.DtoNextAfterMeta): Record<string, unknown> {
  return c.omitUndefined({
    "next_after": c.safeNumber(c.required(wire.nextAfter, "next_after")),
  })
}

export function encodeDtoOffsetPaginationMeta(value: unknown): d.DtoOffsetPaginationMeta {
  const dto = c.record(value, ["limit","offset"])
  return create(d.DtoOffsetPaginationMetaSchema, {
    limit: c.int64(dto["limit"], true),
    offset: c.int64(dto["offset"], true),
  })
}
export function decodeDtoOffsetPaginationMeta(wire: d.DtoOffsetPaginationMeta): Record<string, unknown> {
  return c.omitUndefined({
    "limit": c.safeNumber(c.required(wire.limit, "limit")),
    "offset": c.safeNumber(c.required(wire.offset, "offset")),
  })
}

export function encodeDtoProjectionStoreStatus(value: unknown): d.DtoProjectionStoreStatus {
  const dto = c.record(value, ["store_name","active_generation","active_fingerprint","previous_generation","building_generation","lifecycle_status","fence_epoch","last_event_id","dirty","pending","running","failed","last_error","phase","degraded","errors","updated_at"])
  return create(d.DtoProjectionStoreStatusSchema, {
    storeName: c.text(dto["store_name"]),
    activeGeneration: c.optional(dto["active_generation"], (value) => c.text(value)),
    activeFingerprint: c.optional(dto["active_fingerprint"], (value) => c.text(value)),
    previousGeneration: c.optional(dto["previous_generation"], (value) => c.text(value)),
    buildingGeneration: c.optional(dto["building_generation"], (value) => c.text(value)),
    lifecycleStatus: c.text(dto["lifecycle_status"]),
    fenceEpoch: c.int64(dto["fence_epoch"]),
    lastEventId: c.int64(dto["last_event_id"]),
    dirty: c.bool(dto["dirty"]),
    pending: c.int64(dto["pending"]),
    running: c.int64(dto["running"]),
    failed: c.int64(dto["failed"]),
    lastError: c.optional(dto["last_error"], (value) => c.text(value)),
    phase: c.present(dto["phase"], (value) => c.text(value)),
    degraded: c.present(dto["degraded"], (value) => c.bool(value)),
    errors: c.array((dto["errors"] ?? []), (value) => c.text(value)),
    updatedAt: c.int64(dto["updated_at"]),
  })
}
export function decodeDtoProjectionStoreStatus(wire: d.DtoProjectionStoreStatus): Record<string, unknown> {
  return c.omitUndefined({
    "store_name": c.required(wire.storeName, "store_name"),
    "active_generation": wire.activeGeneration === undefined ? null : ((value) => value)(wire.activeGeneration),
    "active_fingerprint": wire.activeFingerprint === undefined ? null : ((value) => value)(wire.activeFingerprint),
    "previous_generation": wire.previousGeneration === undefined ? null : ((value) => value)(wire.previousGeneration),
    "building_generation": wire.buildingGeneration === undefined ? null : ((value) => value)(wire.buildingGeneration),
    "lifecycle_status": c.required(wire.lifecycleStatus, "lifecycle_status"),
    "fence_epoch": c.safeNumber(c.required(wire.fenceEpoch, "fence_epoch")),
    "last_event_id": c.safeNumber(c.required(wire.lastEventId, "last_event_id")),
    "dirty": c.required(wire.dirty, "dirty"),
    "pending": c.safeNumber(c.required(wire.pending, "pending")),
    "running": c.safeNumber(c.required(wire.running, "running")),
    "failed": c.safeNumber(c.required(wire.failed, "failed")),
    "last_error": wire.lastError === undefined ? null : ((value) => value)(wire.lastError),
    "phase": c.required(wire.phase, "phase"),
    "degraded": c.required(wire.degraded, "degraded"),
    "errors": wire.errors.map((value) => value),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
  })
}

export function encodeDtoQueueStats(value: unknown): d.DtoQueueStats {
  const dto = c.record(value, ["board_id","generated_at","status_counts","stale_claims","blocked_reasons","unplanned_active_tasks","active_parents_with_incomplete_required_steps"])
  return create(d.DtoQueueStatsSchema, {
    boardId: c.text(dto["board_id"]),
    generatedAt: c.int64(dto["generated_at"]),
    statusCounts: c.array(dto["status_counts"], (value) => encodeDtoStatusCount(value)),
    staleClaims: c.array(dto["stale_claims"], (value) => encodeDtoStaleClaim(value)),
    blockedReasons: c.array(dto["blocked_reasons"], (value) => encodeDtoBlockedReasonCount(value)),
    unplannedActiveTasks: c.int64(dto["unplanned_active_tasks"]),
    activeParentsWithIncompleteRequiredSteps: c.int64(dto["active_parents_with_incomplete_required_steps"]),
  })
}
export function decodeDtoQueueStats(wire: d.DtoQueueStats): Record<string, unknown> {
  return c.omitUndefined({
    "board_id": c.required(wire.boardId, "board_id"),
    "generated_at": c.safeNumber(c.required(wire.generatedAt, "generated_at")),
    "status_counts": wire.statusCounts.map((value) => decodeDtoStatusCount(value)),
    "stale_claims": wire.staleClaims.map((value) => decodeDtoStaleClaim(value)),
    "blocked_reasons": wire.blockedReasons.map((value) => decodeDtoBlockedReasonCount(value)),
    "unplanned_active_tasks": c.safeNumber(c.required(wire.unplannedActiveTasks, "unplanned_active_tasks")),
    "active_parents_with_incomplete_required_steps": c.safeNumber(c.required(wire.activeParentsWithIncompleteRequiredSteps, "active_parents_with_incomplete_required_steps")),
  })
}

const DtoReclaimTargetStatusNames = {
  "ready": d.DtoReclaimTargetStatus.READY,
  "blocked": d.DtoReclaimTargetStatus.BLOCKED,
} as const
export function encodeDtoReclaimTargetStatus(value: unknown): d.DtoReclaimTargetStatus { return c.enumValue(value, DtoReclaimTargetStatusNames) }
export function decodeDtoReclaimTargetStatus(value: d.DtoReclaimTargetStatus): string { return c.enumName(value, DtoReclaimTargetStatusNames) }

export function encodeDtoRetryPolicyPayload(value: unknown): d.DtoRetryPolicyPayload {
  const dto = c.record(value, ["max_retries"])
  return create(d.DtoRetryPolicyPayloadSchema, {
    maxRetries: c.optional(dto["max_retries"], (value) => c.int64(value)),
  })
}
export function decodeDtoRetryPolicyPayload(wire: d.DtoRetryPolicyPayload): Record<string, unknown> {
  return c.omitUndefined({
    "max_retries": wire.maxRetries === undefined ? null : ((value) => c.safeNumber(value))(wire.maxRetries),
  })
}

const DtoRunStatusNames = {
  "canceled": d.DtoRunStatus.CANCELED,
} as const
export function encodeDtoRunStatus(value: unknown): d.DtoRunStatus { return c.enumValue(value, DtoRunStatusNames) }
export function decodeDtoRunStatus(value: d.DtoRunStatus): string { return c.enumName(value, DtoRunStatusNames) }

export function encodeDtoSearchMeta(value: unknown): d.DtoSearchMeta {
  const dto = c.record(value, ["backend","stale","database_instance_id","protocol_version","generation","resolved_board_id","fallback_reason","index_version","last_event_id","index_lag_events"])
  return create(d.DtoSearchMetaSchema, {
    backend: c.text(dto["backend"]),
    stale: c.bool(dto["stale"]),
    databaseInstanceId: c.optional(dto["database_instance_id"], (value) => c.text(value)),
    protocolVersion: c.optional(dto["protocol_version"], (value) => c.int64(value)),
    generation: c.optional(dto["generation"], (value) => c.text(value)),
    resolvedBoardId: c.text(dto["resolved_board_id"]),
    fallbackReason: c.optional(dto["fallback_reason"], (value) => c.text(value)),
    indexVersion: c.optional(dto["index_version"], (value) => c.text(value)),
    lastEventId: c.optional(dto["last_event_id"], (value) => c.int64(value)),
    indexLagEvents: c.optional(dto["index_lag_events"], (value) => c.int64(value)),
  })
}
export function decodeDtoSearchMeta(wire: d.DtoSearchMeta): Record<string, unknown> {
  return c.omitUndefined({
    "backend": c.required(wire.backend, "backend"),
    "stale": c.required(wire.stale, "stale"),
    "database_instance_id": wire.databaseInstanceId === undefined ? null : ((value) => value)(wire.databaseInstanceId),
    "protocol_version": wire.protocolVersion === undefined ? null : ((value) => c.safeNumber(value))(wire.protocolVersion),
    "generation": wire.generation === undefined ? null : ((value) => value)(wire.generation),
    "resolved_board_id": c.required(wire.resolvedBoardId, "resolved_board_id"),
    "fallback_reason": wire.fallbackReason === undefined ? null : ((value) => value)(wire.fallbackReason),
    "index_version": wire.indexVersion === undefined ? null : ((value) => value)(wire.indexVersion),
    "last_event_id": wire.lastEventId === undefined ? null : ((value) => c.safeNumber(value))(wire.lastEventId),
    "index_lag_events": wire.indexLagEvents === undefined ? null : ((value) => c.safeNumber(value))(wire.indexLagEvents),
  })
}

export function encodeDtoSearchPageMeta(value: unknown): d.DtoSearchPageMeta {
  const dto = c.record(value, ["limit","offset","total"])
  return create(d.DtoSearchPageMetaSchema, {
    limit: c.int64(dto["limit"], true),
    offset: c.int64(dto["offset"], true),
    total: c.optional(dto["total"], (value) => c.int64(value, true)),
  })
}
export function decodeDtoSearchPageMeta(wire: d.DtoSearchPageMeta): Record<string, unknown> {
  return c.omitUndefined({
    "limit": c.safeNumber(c.required(wire.limit, "limit")),
    "offset": c.safeNumber(c.required(wire.offset, "offset")),
    "total": wire.total === undefined ? null : ((value) => c.safeNumber(value))(wire.total),
  })
}

export function encodeDtoSearchStatus(value: unknown): d.DtoSearchStatus {
  const dto = c.record(value, ["backend","derived_index","stale","database_instance_id","protocol_version","generation","resolved_board_id","fallback_reason","index_version","last_event_id","index_lag_events","message"])
  return create(d.DtoSearchStatusSchema, {
    backend: c.text(dto["backend"]),
    derivedIndex: c.bool(dto["derived_index"]),
    stale: c.bool(dto["stale"]),
    databaseInstanceId: c.optional(dto["database_instance_id"], (value) => c.text(value)),
    protocolVersion: c.optional(dto["protocol_version"], (value) => c.int64(value)),
    generation: c.optional(dto["generation"], (value) => c.text(value)),
    resolvedBoardId: c.text(dto["resolved_board_id"]),
    fallbackReason: c.optional(dto["fallback_reason"], (value) => c.text(value)),
    indexVersion: c.optional(dto["index_version"], (value) => c.text(value)),
    lastEventId: c.optional(dto["last_event_id"], (value) => c.int64(value)),
    indexLagEvents: c.optional(dto["index_lag_events"], (value) => c.int64(value)),
    message: c.text(dto["message"]),
  })
}
export function decodeDtoSearchStatus(wire: d.DtoSearchStatus): Record<string, unknown> {
  return c.omitUndefined({
    "backend": c.required(wire.backend, "backend"),
    "derived_index": c.required(wire.derivedIndex, "derived_index"),
    "stale": c.required(wire.stale, "stale"),
    "database_instance_id": wire.databaseInstanceId === undefined ? null : ((value) => value)(wire.databaseInstanceId),
    "protocol_version": wire.protocolVersion === undefined ? null : ((value) => c.safeNumber(value))(wire.protocolVersion),
    "generation": wire.generation === undefined ? null : ((value) => value)(wire.generation),
    "resolved_board_id": c.required(wire.resolvedBoardId, "resolved_board_id"),
    "fallback_reason": wire.fallbackReason === undefined ? null : ((value) => value)(wire.fallbackReason),
    "index_version": wire.indexVersion === undefined ? null : ((value) => value)(wire.indexVersion),
    "last_event_id": wire.lastEventId === undefined ? null : ((value) => c.safeNumber(value))(wire.lastEventId),
    "index_lag_events": wire.indexLagEvents === undefined ? null : ((value) => c.safeNumber(value))(wire.indexLagEvents),
    "message": c.required(wire.message, "message"),
  })
}

export function encodeDtoSearchTaskHit(value: unknown): d.DtoSearchTaskHit {
  const dto = c.record(value, ["task_id","seq","score","snippet","task"])
  return create(d.DtoSearchTaskHitSchema, {
    taskId: c.text(dto["task_id"]),
    seq: c.int64(dto["seq"]),
    score: c.float(dto["score"]),
    snippet: c.optional(dto["snippet"], (value) => c.text(value)),
    task: encodeDtoApiTask(dto["task"]),
  })
}
export function decodeDtoSearchTaskHit(wire: d.DtoSearchTaskHit): Record<string, unknown> {
  return c.omitUndefined({
    "task_id": c.required(wire.taskId, "task_id"),
    "seq": c.safeNumber(c.required(wire.seq, "seq")),
    "score": c.float(c.required(wire.score, "score")),
    "snippet": wire.snippet === undefined ? null : ((value) => value)(wire.snippet),
    "task": decodeDtoApiTask(c.required(wire.task, "task")),
  })
}

export function encodeDtoSearchTaskStatusWindow(value: unknown): d.DtoSearchTaskStatusWindow {
  const dto = c.record(value, ["status","tasks","search_meta","page"])
  return create(d.DtoSearchTaskStatusWindowSchema, {
    status: encodeDtoApiTaskStatus(dto["status"]),
    tasks: c.array(dto["tasks"], (value) => encodeDtoApiTask(value)),
    searchMeta: encodeDtoSearchMeta(dto["search_meta"]),
    page: encodeDtoSearchPageMeta(dto["page"]),
  })
}
export function decodeDtoSearchTaskStatusWindow(wire: d.DtoSearchTaskStatusWindow): Record<string, unknown> {
  return c.omitUndefined({
    "status": decodeDtoApiTaskStatus(c.required(wire.status, "status")),
    "tasks": wire.tasks.map((value) => decodeDtoApiTask(value)),
    "search_meta": decodeDtoSearchMeta(c.required(wire.searchMeta, "search_meta")),
    "page": decodeDtoSearchPageMeta(c.required(wire.page, "page")),
  })
}

export function encodeDtoSearchTaskStatusWindows(value: unknown): d.DtoSearchTaskStatusWindows {
  const dto = c.record(value, ["statuses"])
  return create(d.DtoSearchTaskStatusWindowsSchema, {
    statuses: c.array(dto["statuses"], (value) => encodeDtoSearchTaskStatusWindow(value)),
  })
}
export function decodeDtoSearchTaskStatusWindows(wire: d.DtoSearchTaskStatusWindows): Record<string, unknown> {
  return c.omitUndefined({
    "statuses": wire.statuses.map((value) => decodeDtoSearchTaskStatusWindow(value)),
  })
}

export function encodeDtoSearchTasksData(value: unknown): d.DtoSearchTasksData {
  const dto = c.record(value, ["hits","meta"])
  return create(d.DtoSearchTasksDataSchema, {
    hits: c.array(dto["hits"], (value) => encodeDtoSearchTaskHit(value)),
    meta: encodeDtoSearchMeta(dto["meta"]),
  })
}
export function decodeDtoSearchTasksData(wire: d.DtoSearchTasksData): Record<string, unknown> {
  return c.omitUndefined({
    "hits": wire.hits.map((value) => decodeDtoSearchTaskHit(value)),
    "meta": decodeDtoSearchMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeDtoSignalCommentRequest(value: unknown): d.DtoSignalCommentRequest {
  const dto = c.record(value, ["body"])
  return create(d.DtoSignalCommentRequestSchema, {
    body: c.optional(dto["body"], (value) => c.text(value)),
  })
}
export function decodeDtoSignalCommentRequest(wire: d.DtoSignalCommentRequest): Record<string, unknown> {
  return c.omitUndefined({
    "body": wire.body === undefined ? null : ((value) => value)(wire.body),
  })
}

export function encodeDtoSignalFilterMeta(value: unknown): d.DtoSignalFilterMeta {
  const dto = c.record(value, ["include_all","limit"])
  return create(d.DtoSignalFilterMetaSchema, {
    includeAll: c.bool(dto["include_all"]),
    limit: c.int64(dto["limit"], true),
  })
}
export function decodeDtoSignalFilterMeta(wire: d.DtoSignalFilterMeta): Record<string, unknown> {
  return c.omitUndefined({
    "include_all": c.required(wire.includeAll, "include_all"),
    "limit": c.safeNumber(c.required(wire.limit, "limit")),
  })
}

export function encodeDtoSignalObservationWire(value: unknown): d.DtoSignalObservationWire {
  const dto = c.record(value, ["id","board_id","task_id","task_ref_snapshot","run_id","comment_id","actor","agent_type","source","evidence","created_at"])
  return create(d.DtoSignalObservationWireSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    taskId: c.optional(dto["task_id"], (value) => c.text(value)),
    taskRefSnapshot: c.optional(dto["task_ref_snapshot"], (value) => c.text(value)),
    runId: c.optional(dto["run_id"], (value) => c.text(value)),
    commentId: c.optional(dto["comment_id"], (value) => c.text(value)),
    actor: c.text(dto["actor"]),
    agentType: c.optional(dto["agent_type"], (value) => c.text(value)),
    source: c.optional(dto["source"], (value) => c.text(value)),
    evidence: encodeDtoStructuredMetadataJsonObject(dto["evidence"]),
    createdAt: c.int64(dto["created_at"]),
  })
}
export function decodeDtoSignalObservationWire(wire: d.DtoSignalObservationWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),
    "task_ref_snapshot": wire.taskRefSnapshot === undefined ? null : ((value) => value)(wire.taskRefSnapshot),
    "run_id": wire.runId === undefined ? null : ((value) => value)(wire.runId),
    "comment_id": wire.commentId === undefined ? null : ((value) => value)(wire.commentId),
    "actor": c.required(wire.actor, "actor"),
    "agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),
    "source": wire.source === undefined ? null : ((value) => value)(wire.source),
    "evidence": decodeDtoStructuredMetadataJsonObject(c.required(wire.evidence, "evidence")),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
  })
}

export function encodeDtoSignalRecordResult(value: unknown): d.DtoSignalRecordResult {
  const dto = c.record(value, ["signal","backlink_comment"])
  return create(d.DtoSignalRecordResultSchema, {
    signal: encodeDtoSignalWire(dto["signal"]),
    backlinkComment: c.optional(dto["backlink_comment"], (value) => encodeDtoApiComment(value)),
  })
}
export function decodeDtoSignalRecordResult(wire: d.DtoSignalRecordResult): Record<string, unknown> {
  return c.omitUndefined({
    "signal": decodeDtoSignalWire(c.required(wire.signal, "signal")),
    "backlink_comment": wire.backlinkComment === undefined ? null : ((value) => decodeDtoApiComment(value))(wire.backlinkComment),
  })
}

export function encodeDtoSignalRecordedPayload(value: unknown): d.DtoSignalRecordedPayload {
  const dto = c.record(value, ["signal_id","observation_id","kind","status"])
  return create(d.DtoSignalRecordedPayloadSchema, {
    signalId: c.text(dto["signal_id"]),
    observationId: c.text(dto["observation_id"]),
    kind: c.text(dto["kind"]),
    status: encodeDtoSignalStatus(dto["status"]),
  })
}
export function decodeDtoSignalRecordedPayload(wire: d.DtoSignalRecordedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "signal_id": c.required(wire.signalId, "signal_id"),
    "observation_id": c.required(wire.observationId, "observation_id"),
    "kind": c.required(wire.kind, "kind"),
    "status": decodeDtoSignalStatus(c.required(wire.status, "status")),
  })
}

export function encodeDtoSignalReviewedPayload(value: unknown): d.DtoSignalReviewedPayload {
  const dto = c.record(value, ["signal_id","status","reason"])
  return create(d.DtoSignalReviewedPayloadSchema, {
    signalId: c.text(dto["signal_id"]),
    status: encodeDtoSignalStatus(dto["status"]),
    reason: c.text(dto["reason"]),
  })
}
export function decodeDtoSignalReviewedPayload(wire: d.DtoSignalReviewedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "signal_id": c.required(wire.signalId, "signal_id"),
    "status": decodeDtoSignalStatus(c.required(wire.status, "status")),
    "reason": c.required(wire.reason, "reason"),
  })
}

const DtoSignalStatusNames = {
  "open": d.DtoSignalStatus.OPEN,
  "confirmed": d.DtoSignalStatus.CONFIRMED,
  "rejected": d.DtoSignalStatus.REJECTED,
  "superseded": d.DtoSignalStatus.SUPERSEDED,
  "resolved": d.DtoSignalStatus.RESOLVED,
} as const
export function encodeDtoSignalStatus(value: unknown): d.DtoSignalStatus { return c.enumValue(value, DtoSignalStatusNames) }
export function decodeDtoSignalStatus(value: d.DtoSignalStatus): string { return c.enumName(value, DtoSignalStatusNames) }

export function encodeDtoSignalWire(value: unknown): d.DtoSignalWire {
  const dto = c.record(value, ["id","board_id","observation_id","kind","title","summary","severity","status","dedupe_key","superseded_by_signal_id","reviewed_by","reviewed_at","review_reason","created_at","updated_at","observation"])
  return create(d.DtoSignalWireSchema, {
    id: c.text(dto["id"]),
    boardId: c.text(dto["board_id"]),
    observationId: c.text(dto["observation_id"]),
    kind: c.text(dto["kind"]),
    title: c.text(dto["title"]),
    summary: c.text(dto["summary"]),
    severity: c.text(dto["severity"]),
    status: c.text(dto["status"]),
    dedupeKey: c.optional(dto["dedupe_key"], (value) => c.text(value)),
    supersededBySignalId: c.optional(dto["superseded_by_signal_id"], (value) => c.text(value)),
    reviewedBy: c.optional(dto["reviewed_by"], (value) => c.text(value)),
    reviewedAt: c.optional(dto["reviewed_at"], (value) => c.int64(value)),
    reviewReason: c.optional(dto["review_reason"], (value) => c.text(value)),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    observation: encodeDtoSignalObservationWire(dto["observation"]),
  })
}
export function decodeDtoSignalWire(wire: d.DtoSignalWire): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "observation_id": c.required(wire.observationId, "observation_id"),
    "kind": c.required(wire.kind, "kind"),
    "title": c.required(wire.title, "title"),
    "summary": c.required(wire.summary, "summary"),
    "severity": c.required(wire.severity, "severity"),
    "status": c.required(wire.status, "status"),
    "dedupe_key": wire.dedupeKey === undefined ? null : ((value) => value)(wire.dedupeKey),
    "superseded_by_signal_id": wire.supersededBySignalId === undefined ? null : ((value) => value)(wire.supersededBySignalId),
    "reviewed_by": wire.reviewedBy === undefined ? null : ((value) => value)(wire.reviewedBy),
    "reviewed_at": wire.reviewedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.reviewedAt),
    "review_reason": wire.reviewReason === undefined ? null : ((value) => value)(wire.reviewReason),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "observation": decodeDtoSignalObservationWire(c.required(wire.observation, "observation")),
  })
}

export function encodeDtoStaleClaim(value: unknown): d.DtoStaleClaim {
  const dto = c.record(value, ["task_id","seq","title","claim_owner","claim_expires_at","last_heartbeat_at","current_run_id","retry_count","max_retries"])
  return create(d.DtoStaleClaimSchema, {
    taskId: c.text(dto["task_id"]),
    seq: c.int64(dto["seq"]),
    title: c.text(dto["title"]),
    claimOwner: c.optional(dto["claim_owner"], (value) => c.text(value)),
    claimExpiresAt: c.optional(dto["claim_expires_at"], (value) => c.int64(value)),
    lastHeartbeatAt: c.optional(dto["last_heartbeat_at"], (value) => c.int64(value)),
    currentRunId: c.optional(dto["current_run_id"], (value) => c.text(value)),
    retryCount: c.int64(dto["retry_count"]),
    maxRetries: c.optional(dto["max_retries"], (value) => c.int64(value)),
  })
}
export function decodeDtoStaleClaim(wire: d.DtoStaleClaim): Record<string, unknown> {
  return c.omitUndefined({
    "task_id": c.required(wire.taskId, "task_id"),
    "seq": c.safeNumber(c.required(wire.seq, "seq")),
    "title": c.required(wire.title, "title"),
    "claim_owner": wire.claimOwner === undefined ? null : ((value) => value)(wire.claimOwner),
    "claim_expires_at": wire.claimExpiresAt === undefined ? null : ((value) => c.safeNumber(value))(wire.claimExpiresAt),
    "last_heartbeat_at": wire.lastHeartbeatAt === undefined ? null : ((value) => c.safeNumber(value))(wire.lastHeartbeatAt),
    "current_run_id": wire.currentRunId === undefined ? null : ((value) => value)(wire.currentRunId),
    "retry_count": c.safeNumber(c.required(wire.retryCount, "retry_count")),
    "max_retries": wire.maxRetries === undefined ? null : ((value) => c.safeNumber(value))(wire.maxRetries),
  })
}

export function encodeDtoStatusCount(value: unknown): d.DtoStatusCount {
  const dto = c.record(value, ["status","count"])
  return create(d.DtoStatusCountSchema, {
    status: encodeDtoApiTaskStatus(dto["status"]),
    count: c.int64(dto["count"]),
  })
}
export function decodeDtoStatusCount(wire: d.DtoStatusCount): Record<string, unknown> {
  return c.omitUndefined({
    "status": decodeDtoApiTaskStatus(c.required(wire.status, "status")),
    "count": c.safeNumber(c.required(wire.count, "count")),
  })
}

const DtoStepStatusNames = {
  "todo": d.DtoStepStatus.TODO,
  "done": d.DtoStepStatus.DONE,
  "skipped": d.DtoStepStatus.SKIPPED,
} as const
export function encodeDtoStepStatus(value: unknown): d.DtoStepStatus { return c.enumValue(value, DtoStepStatusNames) }
export function decodeDtoStepStatus(value: d.DtoStepStatus): string { return c.enumName(value, DtoStepStatusNames) }

export function encodeDtoStreamEventData(value: unknown): d.DtoStreamEventData {
  const dto = c.record(value, ["id","event_id","board_id","task_id","run_id","kind","actor","payload","created_at"])
  validateStreamEvent(dto)
  return create(d.DtoStreamEventDataSchema, {
    id: c.int64(dto["id"]),
    eventId: c.text(dto["event_id"]),
    boardId: c.text(dto["board_id"]),
    taskId: c.optional(dto["task_id"], (value) => c.text(value)),
    runId: c.optional(dto["run_id"], (value) => c.text(value)),
    kind: c.text(dto["kind"]),
    actor: c.optional(dto["actor"], (value) => c.text(value)),
    payload: encodeDtoEventPayload(dto["payload"], c.text(dto.kind)),
    createdAt: c.int64(dto["created_at"]),
  })
}
export function decodeDtoStreamEventData(wire: d.DtoStreamEventData): Record<string, unknown> {
  const value = c.omitUndefined({
    "id": c.safeNumber(c.required(wire.id, "id")),
    "event_id": c.required(wire.eventId, "event_id"),
    "board_id": c.required(wire.boardId, "board_id"),
    "task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),
    "run_id": wire.runId === undefined ? null : ((value) => value)(wire.runId),
    "kind": c.required(wire.kind, "kind"),
    "actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),
    "payload": decodeDtoEventPayload(c.required(wire.payload), c.required(wire.kind)),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
  })
  validateStreamEvent(value)
  return value
}

export function encodeDtoStructuredMetadataJsonObject(value: unknown): d.DtoStructuredMetadataJsonObject {
  return create(d.DtoStructuredMetadataJsonObjectSchema, { value: c.dictionary(value, (value) => c.encodeJson(value)) })
}
export function decodeDtoStructuredMetadataJsonObject(wire: d.DtoStructuredMetadataJsonObject): unknown {
  return Object.fromEntries(Object.entries(wire.value).map(([key, value]) => [key, c.decodeJson(value)]))
}

export function encodeDtoTaskClaimedPayload(value: unknown): d.DtoTaskClaimedPayload {
  const dto = c.record(value, ["claim_owner","metadata"])
  return create(d.DtoTaskClaimedPayloadSchema, {
    claimOwner: c.text(dto["claim_owner"]),
    metadata: c.encodeJson(dto["metadata"]),
  })
}
export function decodeDtoTaskClaimedPayload(wire: d.DtoTaskClaimedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "claim_owner": c.required(wire.claimOwner, "claim_owner"),
    "metadata": c.decodeJson(c.required(wire.metadata, "metadata")),
  })
}

export function encodeDtoTaskCommentCreatedPayload(value: unknown): d.DtoTaskCommentCreatedPayload {
  const dto = c.record(value, ["comment_id","kind","author_type","agent_type"])
  return create(d.DtoTaskCommentCreatedPayloadSchema, {
    commentId: c.text(dto["comment_id"]),
    kind: encodeDtoEventPayloadCommentKind(dto["kind"]),
    authorType: encodeDtoEventPayloadCommentAuthorType(dto["author_type"]),
    agentType: c.optional(dto["agent_type"], (value) => c.text(value)),
  })
}
export function decodeDtoTaskCommentCreatedPayload(wire: d.DtoTaskCommentCreatedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "comment_id": c.required(wire.commentId, "comment_id"),
    "kind": decodeDtoEventPayloadCommentKind(c.required(wire.kind, "kind")),
    "author_type": decodeDtoEventPayloadCommentAuthorType(c.required(wire.authorType, "author_type")),
    "agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),
  })
}

export function encodeDtoTaskDetailAggregate(value: unknown): d.DtoTaskDetailAggregate {
  const dto = c.record(value, ["task","labels","dependencies","execution_plan","steps","comments","runs","events","ontology"])
  return create(d.DtoTaskDetailAggregateSchema, {
    task: encodeDtoApiTask(dto["task"]),
    labels: c.array(dto["labels"], (value) => encodeDtoApiLabel(value)),
    dependencies: encodeDtoApiDependencies(dto["dependencies"]),
    executionPlan: encodeDtoApiExecutionPlan(dto["execution_plan"]),
    steps: c.array(dto["steps"], (value) => encodeDtoApiTaskStep(value)),
    comments: c.array(dto["comments"], (value) => encodeDtoApiComment(value)),
    runs: c.array(dto["runs"], (value) => encodeDtoApiRun(value)),
    events: c.array(dto["events"], (value) => encodeDtoStreamEventData(value)),
    ontology: encodeDtoTaskDetailOntology(dto["ontology"]),
  })
}
export function decodeDtoTaskDetailAggregate(wire: d.DtoTaskDetailAggregate): Record<string, unknown> {
  return c.omitUndefined({
    "task": decodeDtoApiTask(c.required(wire.task, "task")),
    "labels": wire.labels.map((value) => decodeDtoApiLabel(value)),
    "dependencies": decodeDtoApiDependencies(c.required(wire.dependencies, "dependencies")),
    "execution_plan": decodeDtoApiExecutionPlan(c.required(wire.executionPlan, "execution_plan")),
    "steps": wire.steps.map((value) => decodeDtoApiTaskStep(value)),
    "comments": wire.comments.map((value) => decodeDtoApiComment(value)),
    "runs": wire.runs.map((value) => decodeDtoApiRun(value)),
    "events": wire.events.map((value) => decodeDtoStreamEventData(value)),
    "ontology": decodeDtoTaskDetailOntology(c.required(wire.ontology, "ontology")),
  })
}

export function encodeDtoTaskDetailOntology(value: unknown): d.DtoTaskDetailOntology {
  const dto = c.record(value, ["summary","degraded","diagnostics"])
  return create(d.DtoTaskDetailOntologySchema, {
    summary: c.optional(dto["summary"], (value) => encodeDtoTaskOntologySummary(value)),
    degraded: c.bool(dto["degraded"]),
    diagnostics: c.array(dto["diagnostics"], (value) => c.text(value)),
  })
}
export function decodeDtoTaskDetailOntology(wire: d.DtoTaskDetailOntology): Record<string, unknown> {
  return c.omitUndefined({
    "summary": wire.summary === undefined ? null : ((value) => decodeDtoTaskOntologySummary(value))(wire.summary),
    "degraded": c.required(wire.degraded, "degraded"),
    "diagnostics": wire.diagnostics.map((value) => value),
  })
}

export function encodeDtoTaskExportSanitizedPayload(value: unknown): d.DtoTaskExportSanitizedPayload {
  const dto = c.record(value, ["from_status","to_status","run_status","original_run_id","claim_owner","claim_expires_at","reason"])
  return create(d.DtoTaskExportSanitizedPayloadSchema, {
    fromStatus: encodeDtoTaskStatus(dto["from_status"]),
    toStatus: encodeDtoTaskStatus(dto["to_status"]),
    runStatus: encodeDtoRunStatus(dto["run_status"]),
    originalRunId: c.optional(dto["original_run_id"], (value) => c.text(value)),
    claimOwner: c.optional(dto["claim_owner"], (value) => c.text(value)),
    claimExpiresAt: c.optional(dto["claim_expires_at"], (value) => c.int64(value)),
    reason: c.text(dto["reason"]),
  })
}
export function decodeDtoTaskExportSanitizedPayload(wire: d.DtoTaskExportSanitizedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "from_status": decodeDtoTaskStatus(c.required(wire.fromStatus, "from_status")),
    "to_status": decodeDtoTaskStatus(c.required(wire.toStatus, "to_status")),
    "run_status": decodeDtoRunStatus(c.required(wire.runStatus, "run_status")),
    "original_run_id": wire.originalRunId === undefined ? null : ((value) => value)(wire.originalRunId),
    "claim_owner": wire.claimOwner === undefined ? null : ((value) => value)(wire.claimOwner),
    "claim_expires_at": wire.claimExpiresAt === undefined ? null : ((value) => c.safeNumber(value))(wire.claimExpiresAt),
    "reason": c.required(wire.reason, "reason"),
  })
}

export function encodeDtoTaskGraphEdge(value: unknown): d.DtoTaskGraphEdge {
  const dto = c.record(value, ["id","source_task_id","target_task_id","kind","required","blocking"])
  return create(d.DtoTaskGraphEdgeSchema, {
    id: c.text(dto["id"]),
    sourceTaskId: c.text(dto["source_task_id"]),
    targetTaskId: c.text(dto["target_task_id"]),
    kind: encodeDtoApiTaskGraphEdgeKind(dto["kind"]),
    required: c.bool(dto["required"]),
    blocking: c.bool(dto["blocking"]),
  })
}
export function decodeDtoTaskGraphEdge(wire: d.DtoTaskGraphEdge): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "source_task_id": c.required(wire.sourceTaskId, "source_task_id"),
    "target_task_id": c.required(wire.targetTaskId, "target_task_id"),
    "kind": decodeDtoApiTaskGraphEdgeKind(c.required(wire.kind, "kind")),
    "required": c.required(wire.required, "required"),
    "blocking": c.required(wire.blocking, "blocking"),
  })
}

export function encodeDtoTaskGraphMeta(value: unknown): d.DtoTaskGraphMeta {
  const dto = c.record(value, ["depth","context_depth","generated_at","node_count","edge_count","truncated","active_statuses","active_only","include_done_context","include_archived_context","hide_isolated","limit_nodes"])
  return create(d.DtoTaskGraphMetaSchema, {
    depth: c.int64(dto["depth"], true),
    contextDepth: c.int64(dto["context_depth"], true),
    generatedAt: c.int64(dto["generated_at"]),
    nodeCount: c.int64(dto["node_count"], true),
    edgeCount: c.int64(dto["edge_count"], true),
    truncated: c.bool(dto["truncated"]),
    activeStatuses: c.array(dto["active_statuses"], (value) => encodeDtoApiTaskStatus(value)),
    activeOnly: c.bool(dto["active_only"]),
    includeDoneContext: c.bool(dto["include_done_context"]),
    includeArchivedContext: c.bool(dto["include_archived_context"]),
    hideIsolated: c.bool(dto["hide_isolated"]),
    limitNodes: c.int64(dto["limit_nodes"], true),
  })
}
export function decodeDtoTaskGraphMeta(wire: d.DtoTaskGraphMeta): Record<string, unknown> {
  return c.omitUndefined({
    "depth": c.safeNumber(c.required(wire.depth, "depth")),
    "context_depth": c.safeNumber(c.required(wire.contextDepth, "context_depth")),
    "generated_at": c.safeNumber(c.required(wire.generatedAt, "generated_at")),
    "node_count": c.safeNumber(c.required(wire.nodeCount, "node_count")),
    "edge_count": c.safeNumber(c.required(wire.edgeCount, "edge_count")),
    "truncated": c.required(wire.truncated, "truncated"),
    "active_statuses": wire.activeStatuses.map((value) => decodeDtoApiTaskStatus(value)),
    "active_only": c.required(wire.activeOnly, "active_only"),
    "include_done_context": c.required(wire.includeDoneContext, "include_done_context"),
    "include_archived_context": c.required(wire.includeArchivedContext, "include_archived_context"),
    "hide_isolated": c.required(wire.hideIsolated, "hide_isolated"),
    "limit_nodes": c.safeNumber(c.required(wire.limitNodes, "limit_nodes")),
  })
}

export function encodeDtoTaskGraphNode(value: unknown): d.DtoTaskGraphNode {
  const dto = c.record(value, ["task","role","context_only"])
  return create(d.DtoTaskGraphNodeSchema, {
    task: encodeDtoApiTask(dto["task"]),
    role: encodeDtoApiTaskGraphNodeRole(dto["role"]),
    contextOnly: c.bool(dto["context_only"]),
  })
}
export function decodeDtoTaskGraphNode(wire: d.DtoTaskGraphNode): Record<string, unknown> {
  return c.omitUndefined({
    "task": decodeDtoApiTask(c.required(wire.task, "task")),
    "role": decodeDtoApiTaskGraphNodeRole(c.required(wire.role, "role")),
    "context_only": c.required(wire.contextOnly, "context_only"),
  })
}

export function encodeDtoTaskLabelPayload(value: unknown): d.DtoTaskLabelPayload {
  const dto = c.record(value, ["label_id","label"])
  return create(d.DtoTaskLabelPayloadSchema, {
    labelId: c.text(dto["label_id"]),
    label: c.text(dto["label"]),
  })
}
export function decodeDtoTaskLabelPayload(wire: d.DtoTaskLabelPayload): Record<string, unknown> {
  return c.omitUndefined({
    "label_id": c.required(wire.labelId, "label_id"),
    "label": c.required(wire.label, "label"),
  })
}

export function encodeDtoTaskNeighborhood(value: unknown): d.DtoTaskNeighborhood {
  const dto = c.record(value, ["center_task_id","nodes","edges","meta"])
  return create(d.DtoTaskNeighborhoodSchema, {
    centerTaskId: c.text(dto["center_task_id"]),
    nodes: c.array(dto["nodes"], (value) => encodeDtoTaskGraphNode(value)),
    edges: c.array(dto["edges"], (value) => encodeDtoTaskGraphEdge(value)),
    meta: encodeDtoTaskGraphMeta(dto["meta"]),
  })
}
export function decodeDtoTaskNeighborhood(wire: d.DtoTaskNeighborhood): Record<string, unknown> {
  return c.omitUndefined({
    "center_task_id": c.required(wire.centerTaskId, "center_task_id"),
    "nodes": wire.nodes.map((value) => decodeDtoTaskGraphNode(value)),
    "edges": wire.edges.map((value) => decodeDtoTaskGraphEdge(value)),
    "meta": decodeDtoTaskGraphMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeDtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary(value: unknown): d.DtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary {
  const dto = c.record(value, ["details"])
  return create(d.DtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummarySchema, {
    details: encodeDtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary(dto["details"]),
  })
}
export function decodeDtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary(wire: d.DtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary): Record<string, unknown> {
  return c.omitUndefined({
    "details": decodeDtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary(c.required(wire.details, "details")),
  })
}

export function encodeDtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary(value: unknown): d.DtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary {
  const dto = c.record(value, ["ontology_summary"])
  return create(d.DtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummarySchema, {
    ontologySummary: c.optional(dto["ontology_summary"], (value) => encodeDtoTaskOntologySummary(value)),
  })
}
export function decodeDtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary(wire: d.DtoTaskOntologyDetailsOfOptionalDtoTaskOntologySummary): Record<string, unknown> {
  return c.omitUndefined({
    "ontology_summary": wire.ontologySummary === undefined ? null : ((value) => decodeDtoTaskOntologySummary(value))(wire.ontologySummary),
  })
}

export function encodeDtoTaskOntologySignalSummary(value: unknown): d.DtoTaskOntologySignalSummary {
  const dto = c.record(value, ["id","kind","status","proposed_action","target_label_id","target_label_name","candidate_atom_polarity","candidate_atom_kind","candidate_text","candidate_content_hash","proposed_label_name","proposed_label_name_normalized","suggest_score","suggest_rank","degraded","stale","legacy_incomparable","suggest_input_drift","created_at","updated_at","latest_action_at","action_count"])
  return create(d.DtoTaskOntologySignalSummarySchema, {
    id: c.text(dto["id"]),
    kind: c.text(dto["kind"]),
    status: c.text(dto["status"]),
    proposedAction: c.text(dto["proposed_action"]),
    targetLabelId: c.optional(dto["target_label_id"], (value) => c.text(value)),
    targetLabelName: c.optional(dto["target_label_name"], (value) => c.text(value)),
    candidateAtomPolarity: c.optional(dto["candidate_atom_polarity"], (value) => c.text(value)),
    candidateAtomKind: c.optional(dto["candidate_atom_kind"], (value) => c.text(value)),
    candidateText: c.optional(dto["candidate_text"], (value) => c.text(value)),
    candidateContentHash: c.optional(dto["candidate_content_hash"], (value) => c.text(value)),
    proposedLabelName: c.optional(dto["proposed_label_name"], (value) => c.text(value)),
    proposedLabelNameNormalized: c.optional(dto["proposed_label_name_normalized"], (value) => c.text(value)),
    suggestScore: c.optional(dto["suggest_score"], (value) => c.float(value)),
    suggestRank: c.optional(dto["suggest_rank"], (value) => c.int64(value)),
    degraded: c.bool(dto["degraded"]),
    stale: c.bool(dto["stale"]),
    legacyIncomparable: c.bool(dto["legacy_incomparable"]),
    suggestInputDrift: c.bool(dto["suggest_input_drift"]),
    createdAt: c.int64(dto["created_at"]),
    updatedAt: c.int64(dto["updated_at"]),
    latestActionAt: c.optional(dto["latest_action_at"], (value) => c.int64(value)),
    actionCount: c.int64(dto["action_count"]),
  })
}
export function decodeDtoTaskOntologySignalSummary(wire: d.DtoTaskOntologySignalSummary): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "kind": c.required(wire.kind, "kind"),
    "status": c.required(wire.status, "status"),
    "proposed_action": c.required(wire.proposedAction, "proposed_action"),
    "target_label_id": wire.targetLabelId === undefined ? null : ((value) => value)(wire.targetLabelId),
    "target_label_name": wire.targetLabelName === undefined ? null : ((value) => value)(wire.targetLabelName),
    "candidate_atom_polarity": wire.candidateAtomPolarity === undefined ? null : ((value) => value)(wire.candidateAtomPolarity),
    "candidate_atom_kind": wire.candidateAtomKind === undefined ? null : ((value) => value)(wire.candidateAtomKind),
    "candidate_text": wire.candidateText === undefined ? null : ((value) => value)(wire.candidateText),
    "candidate_content_hash": wire.candidateContentHash === undefined ? null : ((value) => value)(wire.candidateContentHash),
    "proposed_label_name": wire.proposedLabelName === undefined ? null : ((value) => value)(wire.proposedLabelName),
    "proposed_label_name_normalized": wire.proposedLabelNameNormalized === undefined ? null : ((value) => value)(wire.proposedLabelNameNormalized),
    "suggest_score": wire.suggestScore === undefined ? null : ((value) => c.float(value))(wire.suggestScore),
    "suggest_rank": wire.suggestRank === undefined ? null : ((value) => c.safeNumber(value))(wire.suggestRank),
    "degraded": c.required(wire.degraded, "degraded"),
    "stale": c.required(wire.stale, "stale"),
    "legacy_incomparable": c.required(wire.legacyIncomparable, "legacy_incomparable"),
    "suggest_input_drift": c.required(wire.suggestInputDrift, "suggest_input_drift"),
    "created_at": c.safeNumber(c.required(wire.createdAt, "created_at")),
    "updated_at": c.safeNumber(c.required(wire.updatedAt, "updated_at")),
    "latest_action_at": wire.latestActionAt === undefined ? null : ((value) => c.safeNumber(value))(wire.latestActionAt),
    "action_count": c.safeNumber(c.required(wire.actionCount, "action_count")),
  })
}

export function encodeDtoTaskOntologySummary(value: unknown): d.DtoTaskOntologySummary {
  const dto = c.record(value, ["task_id","observation_count","signal_count","open_count","confirmed_count","resolved_count","rejected_count","superseded_count","degraded_count","stale_count","suggest_input_drift_count","legacy_incomparable_count","incomparable_count","action_count","oldest_open_confirmed_signal_at","oldest_open_confirmed_signal_age_ms","latest_signal_at","latest_action_at","current_suggest_input_hash","sample_signals"])
  return create(d.DtoTaskOntologySummarySchema, {
    taskId: c.text(dto["task_id"]),
    observationCount: c.int64(dto["observation_count"]),
    signalCount: c.int64(dto["signal_count"]),
    openCount: c.int64(dto["open_count"]),
    confirmedCount: c.int64(dto["confirmed_count"]),
    resolvedCount: c.int64(dto["resolved_count"]),
    rejectedCount: c.int64(dto["rejected_count"]),
    supersededCount: c.int64(dto["superseded_count"]),
    degradedCount: c.int64(dto["degraded_count"]),
    staleCount: c.int64(dto["stale_count"]),
    suggestInputDriftCount: c.int64(dto["suggest_input_drift_count"]),
    legacyIncomparableCount: c.int64(dto["legacy_incomparable_count"]),
    incomparableCount: c.int64(dto["incomparable_count"]),
    actionCount: c.int64(dto["action_count"]),
    oldestOpenConfirmedSignalAt: c.optional(dto["oldest_open_confirmed_signal_at"], (value) => c.int64(value)),
    oldestOpenConfirmedSignalAgeMs: c.optional(dto["oldest_open_confirmed_signal_age_ms"], (value) => c.int64(value)),
    latestSignalAt: c.optional(dto["latest_signal_at"], (value) => c.int64(value)),
    latestActionAt: c.optional(dto["latest_action_at"], (value) => c.int64(value)),
    currentSuggestInputHash: c.text(dto["current_suggest_input_hash"]),
    sampleSignals: c.array(dto["sample_signals"], (value) => encodeDtoTaskOntologySignalSummary(value)),
  })
}
export function decodeDtoTaskOntologySummary(wire: d.DtoTaskOntologySummary): Record<string, unknown> {
  return c.omitUndefined({
    "task_id": c.required(wire.taskId, "task_id"),
    "observation_count": c.safeNumber(c.required(wire.observationCount, "observation_count")),
    "signal_count": c.safeNumber(c.required(wire.signalCount, "signal_count")),
    "open_count": c.safeNumber(c.required(wire.openCount, "open_count")),
    "confirmed_count": c.safeNumber(c.required(wire.confirmedCount, "confirmed_count")),
    "resolved_count": c.safeNumber(c.required(wire.resolvedCount, "resolved_count")),
    "rejected_count": c.safeNumber(c.required(wire.rejectedCount, "rejected_count")),
    "superseded_count": c.safeNumber(c.required(wire.supersededCount, "superseded_count")),
    "degraded_count": c.safeNumber(c.required(wire.degradedCount, "degraded_count")),
    "stale_count": c.safeNumber(c.required(wire.staleCount, "stale_count")),
    "suggest_input_drift_count": c.safeNumber(c.required(wire.suggestInputDriftCount, "suggest_input_drift_count")),
    "legacy_incomparable_count": c.safeNumber(c.required(wire.legacyIncomparableCount, "legacy_incomparable_count")),
    "incomparable_count": c.safeNumber(c.required(wire.incomparableCount, "incomparable_count")),
    "action_count": c.safeNumber(c.required(wire.actionCount, "action_count")),
    "oldest_open_confirmed_signal_at": wire.oldestOpenConfirmedSignalAt === undefined ? null : ((value) => c.safeNumber(value))(wire.oldestOpenConfirmedSignalAt),
    "oldest_open_confirmed_signal_age_ms": wire.oldestOpenConfirmedSignalAgeMs === undefined ? null : ((value) => c.safeNumber(value))(wire.oldestOpenConfirmedSignalAgeMs),
    "latest_signal_at": wire.latestSignalAt === undefined ? null : ((value) => c.safeNumber(value))(wire.latestSignalAt),
    "latest_action_at": wire.latestActionAt === undefined ? null : ((value) => c.safeNumber(value))(wire.latestActionAt),
    "current_suggest_input_hash": c.required(wire.currentSuggestInputHash, "current_suggest_input_hash"),
    "sample_signals": wire.sampleSignals.map((value) => decodeDtoTaskOntologySignalSummary(value)),
  })
}

export function encodeDtoTaskReadLabel(value: unknown): d.DtoTaskReadLabel {
  value = c.taskReadLabel(value)
  return create(d.DtoTaskReadLabelSchema, { value: c.text(value) })
}
export function decodeDtoTaskReadLabel(wire: d.DtoTaskReadLabel): unknown {
  return c.taskReadLabel(c.required(wire.value, "0"))
}

const DtoTaskReadPlanFilterNames = {
  "plan_needed": d.DtoTaskReadPlanFilter.PLAN_NEEDED,
  "has_steps": d.DtoTaskReadPlanFilter.HAS_STEPS,
  "incomplete_required_steps": d.DtoTaskReadPlanFilter.INCOMPLETE_REQUIRED_STEPS,
} as const
export function encodeDtoTaskReadPlanFilter(value: unknown): d.DtoTaskReadPlanFilter { return c.enumValue(value, DtoTaskReadPlanFilterNames) }
export function decodeDtoTaskReadPlanFilter(value: d.DtoTaskReadPlanFilter): string { return c.enumName(value, DtoTaskReadPlanFilterNames) }

const DtoTaskReadSortNames = {
  "seq": d.DtoTaskReadSort.SEQ,
  "-seq": d.DtoTaskReadSort.SEQ_DESC,
  "title": d.DtoTaskReadSort.TITLE,
  "-title": d.DtoTaskReadSort.TITLE_DESC,
  "status": d.DtoTaskReadSort.STATUS,
  "-status": d.DtoTaskReadSort.STATUS_DESC,
  "position": d.DtoTaskReadSort.POSITION,
  "-position": d.DtoTaskReadSort.POSITION_DESC,
  "priority": d.DtoTaskReadSort.PRIORITY,
  "-priority": d.DtoTaskReadSort.PRIORITY_DESC,
  "assignee": d.DtoTaskReadSort.ASSIGNEE,
  "-assignee": d.DtoTaskReadSort.ASSIGNEE_DESC,
  "scheduled_at": d.DtoTaskReadSort.SCHEDULED_AT,
  "-scheduled_at": d.DtoTaskReadSort.SCHEDULED_AT_DESC,
  "due_at": d.DtoTaskReadSort.DUE_AT,
  "-due_at": d.DtoTaskReadSort.DUE_AT_DESC,
  "created_at": d.DtoTaskReadSort.CREATED_AT,
  "-created_at": d.DtoTaskReadSort.CREATED_AT_DESC,
  "updated_at": d.DtoTaskReadSort.UPDATED_AT,
  "-updated_at": d.DtoTaskReadSort.UPDATED_AT_DESC,
} as const
export function encodeDtoTaskReadSort(value: unknown): d.DtoTaskReadSort { return c.enumValue(value, DtoTaskReadSortNames) }
export function decodeDtoTaskReadSort(value: d.DtoTaskReadSort): string { return c.enumName(value, DtoTaskReadSortNames) }

export function encodeDtoTaskReasonPayload(value: unknown): d.DtoTaskReasonPayload {
  const dto = c.record(value, ["reason"])
  return create(d.DtoTaskReasonPayloadSchema, {
    reason: c.text(dto["reason"]),
  })
}
export function decodeDtoTaskReasonPayload(wire: d.DtoTaskReasonPayload): Record<string, unknown> {
  return c.omitUndefined({
    "reason": c.required(wire.reason, "reason"),
  })
}

export function encodeDtoTaskReclaimedPayload(value: unknown): d.DtoTaskReclaimedPayload {
  const dto = c.record(value, ["retry_count","max_retries","to_status","reason"])
  return create(d.DtoTaskReclaimedPayloadSchema, {
    retryCount: c.int64(dto["retry_count"]),
    maxRetries: c.optional(dto["max_retries"], (value) => c.int64(value)),
    toStatus: encodeDtoTaskStatus(dto["to_status"]),
    reason: c.text(dto["reason"]),
  })
}
export function decodeDtoTaskReclaimedPayload(wire: d.DtoTaskReclaimedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "retry_count": c.safeNumber(c.required(wire.retryCount, "retry_count")),
    "max_retries": wire.maxRetries === undefined ? null : ((value) => c.safeNumber(value))(wire.maxRetries),
    "to_status": decodeDtoTaskStatus(c.required(wire.toStatus, "to_status")),
    "reason": c.required(wire.reason, "reason"),
  })
}

export function encodeDtoTaskReopenedPayload(value: unknown): d.DtoTaskReopenedPayload {
  const dto = c.record(value, ["from","to","reason","original_completed_at"])
  return create(d.DtoTaskReopenedPayloadSchema, {
    from: encodeDtoTaskStatus(dto["from"]),
    to: encodeDtoTaskStatus(dto["to"]),
    reason: c.text(dto["reason"]),
    originalCompletedAt: c.optional(dto["original_completed_at"], (value) => c.int64(value)),
  })
}
export function decodeDtoTaskReopenedPayload(wire: d.DtoTaskReopenedPayload): Record<string, unknown> {
  return c.omitUndefined({
    "from": decodeDtoTaskStatus(c.required(wire.from, "from")),
    "to": decodeDtoTaskStatus(c.required(wire.to, "to")),
    "reason": c.required(wire.reason, "reason"),
    "original_completed_at": wire.originalCompletedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.originalCompletedAt),
  })
}

export function encodeDtoTaskResultPayload(value: unknown): d.DtoTaskResultPayload {
  const dto = c.record(value, ["result"])
  return create(d.DtoTaskResultPayloadSchema, {
    result: c.optional(dto["result"], (value) => c.encodeJson(value)),
  })
}
export function decodeDtoTaskResultPayload(wire: d.DtoTaskResultPayload): Record<string, unknown> {
  return c.omitUndefined({
    "result": wire.result === undefined ? null : ((value) => c.decodeJson(value))(wire.result),
  })
}

export function encodeDtoTaskRetryPayload(value: unknown): d.DtoTaskRetryPayload {
  const dto = c.record(value, ["retry_count","max_retries"])
  return create(d.DtoTaskRetryPayloadSchema, {
    retryCount: c.int64(dto["retry_count"]),
    maxRetries: c.optional(dto["max_retries"], (value) => c.int64(value)),
  })
}
export function decodeDtoTaskRetryPayload(wire: d.DtoTaskRetryPayload): Record<string, unknown> {
  return c.omitUndefined({
    "retry_count": c.safeNumber(c.required(wire.retryCount, "retry_count")),
    "max_retries": wire.maxRetries === undefined ? null : ((value) => c.safeNumber(value))(wire.maxRetries),
  })
}

const DtoTaskStatusNames = {
  "triage": d.DtoTaskStatus.TRIAGE,
  "todo": d.DtoTaskStatus.TODO,
  "scheduled": d.DtoTaskStatus.SCHEDULED,
  "ready": d.DtoTaskStatus.READY,
  "running": d.DtoTaskStatus.RUNNING,
  "blocked": d.DtoTaskStatus.BLOCKED,
  "review": d.DtoTaskStatus.REVIEW,
  "done": d.DtoTaskStatus.DONE,
  "archived": d.DtoTaskStatus.ARCHIVED,
} as const
export function encodeDtoTaskStatus(value: unknown): d.DtoTaskStatus { return c.enumValue(value, DtoTaskStatusNames) }
export function decodeDtoTaskStatus(value: d.DtoTaskStatus): string { return c.enumName(value, DtoTaskStatusNames) }

export function encodeDtoTaskStatusPayload(value: unknown): d.DtoTaskStatusPayload {
  const dto = c.record(value, ["status"])
  return create(d.DtoTaskStatusPayloadSchema, {
    status: encodeDtoTaskStatus(dto["status"]),
  })
}
export function decodeDtoTaskStatusPayload(wire: d.DtoTaskStatusPayload): Record<string, unknown> {
  return c.omitUndefined({
    "status": decodeDtoTaskStatus(c.required(wire.status, "status")),
  })
}

export function encodeDtoTaskStepPayload(value: unknown): d.DtoTaskStepPayload {
  const dto = c.record(value, ["step_id","linked_task_id","position","required","status"])
  return create(d.DtoTaskStepPayloadSchema, {
    stepId: c.text(dto["step_id"]),
    linkedTaskId: c.optional(dto["linked_task_id"], (value) => c.text(value)),
    position: c.int64(dto["position"]),
    required: c.bool(dto["required"]),
    status: encodeDtoStepStatus(dto["status"]),
  })
}
export function decodeDtoTaskStepPayload(wire: d.DtoTaskStepPayload): Record<string, unknown> {
  return c.omitUndefined({
    "step_id": c.required(wire.stepId, "step_id"),
    "linked_task_id": wire.linkedTaskId === undefined ? null : ((value) => value)(wire.linkedTaskId),
    "position": c.safeNumber(c.required(wire.position, "position")),
    "required": c.required(wire.required, "required"),
    "status": decodeDtoStepStatus(c.required(wire.status, "status")),
  })
}

export function encodeDtoTaskToStatusPayload(value: unknown): d.DtoTaskToStatusPayload {
  const dto = c.record(value, ["to_status"])
  return create(d.DtoTaskToStatusPayloadSchema, {
    toStatus: encodeDtoTaskStatus(dto["to_status"]),
  })
}
export function decodeDtoTaskToStatusPayload(wire: d.DtoTaskToStatusPayload): Record<string, unknown> {
  return c.omitUndefined({
    "to_status": decodeDtoTaskStatus(c.required(wire.toStatus, "to_status")),
  })
}

export function encodeDtoTotalPaginationMeta(value: unknown): d.DtoTotalPaginationMeta {
  const dto = c.record(value, ["limit","offset","total"])
  return create(d.DtoTotalPaginationMetaSchema, {
    limit: c.int64(dto["limit"], true),
    offset: c.int64(dto["offset"], true),
    total: c.int64(dto["total"], true),
  })
}
export function decodeDtoTotalPaginationMeta(wire: d.DtoTotalPaginationMeta): Record<string, unknown> {
  return c.omitUndefined({
    "limit": c.safeNumber(c.required(wire.limit, "limit")),
    "offset": c.safeNumber(c.required(wire.offset, "offset")),
    "total": c.safeNumber(c.required(wire.total, "total")),
  })
}

export function encodeDtoVacuumReport(value: unknown): d.DtoVacuumReport {
  const dto = c.record(value, ["ok","before_bytes","after_bytes","source_fingerprint"])
  return create(d.DtoVacuumReportSchema, {
    ok: c.bool(dto["ok"]),
    beforeBytes: c.int64(dto["before_bytes"], true),
    afterBytes: c.int64(dto["after_bytes"], true),
    sourceFingerprint: c.text(dto["source_fingerprint"]),
  })
}
export function decodeDtoVacuumReport(wire: d.DtoVacuumReport): Record<string, unknown> {
  return c.omitUndefined({
    "ok": c.required(wire.ok, "ok"),
    "before_bytes": c.safeNumber(c.required(wire.beforeBytes, "before_bytes")),
    "after_bytes": c.safeNumber(c.required(wire.afterBytes, "after_bytes")),
    "source_fingerprint": c.required(wire.sourceFingerprint, "source_fingerprint"),
  })
}

export function encodeDtoVectorChunkResult(value: unknown): d.DtoVectorChunkResult {
  const dto = c.record(value, ["id","entity_uri","source_kind","content","content_hash","embedding_model","distance","score"])
  return create(d.DtoVectorChunkResultSchema, {
    id: c.text(dto["id"]),
    entityUri: c.optional(dto["entity_uri"], (value) => c.text(value)),
    sourceKind: c.text(dto["source_kind"]),
    content: c.text(dto["content"]),
    contentHash: c.text(dto["content_hash"]),
    embeddingModel: c.text(dto["embedding_model"]),
    distance: c.float(dto["distance"], true),
    score: c.float(dto["score"], true),
  })
}
export function decodeDtoVectorChunkResult(wire: d.DtoVectorChunkResult): Record<string, unknown> {
  return c.omitUndefined({
    "id": c.required(wire.id, "id"),
    "entity_uri": wire.entityUri === undefined ? null : ((value) => value)(wire.entityUri),
    "source_kind": c.required(wire.sourceKind, "source_kind"),
    "content": c.required(wire.content, "content"),
    "content_hash": c.required(wire.contentHash, "content_hash"),
    "embedding_model": c.required(wire.embeddingModel, "embedding_model"),
    "distance": c.float(c.required(wire.distance, "distance")),
    "score": c.float(c.required(wire.score, "score")),
  })
}

export function encodeDtoVectorConfigureRequest(value: unknown): d.DtoVectorConfigureRequest {
  const dto = c.record(value, ["provider","endpoint","model","dimensions"])
  return create(d.DtoVectorConfigureRequestSchema, {
    provider: c.text(dto["provider"]),
    endpoint: c.text(dto["endpoint"]),
    model: c.text(dto["model"]),
    dimensions: c.int64(dto["dimensions"], true),
  })
}
export function decodeDtoVectorConfigureRequest(wire: d.DtoVectorConfigureRequest): Record<string, unknown> {
  return c.omitUndefined({
    "provider": c.required(wire.provider, "provider"),
    "endpoint": c.required(wire.endpoint, "endpoint"),
    "model": c.required(wire.model, "model"),
    "dimensions": c.safeNumber(c.required(wire.dimensions, "dimensions")),
  })
}

export function encodeDtoVectorLabelAtomResult(value: unknown): d.DtoVectorLabelAtomResult {
  const dto = c.record(value, ["atom_id","label_id","label_name","board_id","polarity","kind","text","ordinal","content_hash","embedding_model","distance","vector"])
  return create(d.DtoVectorLabelAtomResultSchema, {
    atomId: c.text(dto["atom_id"]),
    labelId: c.text(dto["label_id"]),
    labelName: c.text(dto["label_name"]),
    boardId: c.text(dto["board_id"]),
    polarity: c.text(dto["polarity"]),
    kind: c.text(dto["kind"]),
    text: c.text(dto["text"]),
    ordinal: c.int64(dto["ordinal"]),
    contentHash: c.text(dto["content_hash"]),
    embeddingModel: c.text(dto["embedding_model"]),
    distance: c.float(dto["distance"], true),
    vector: c.optional(dto["vector"], (value) => encodeListOfF32(value)),
  })
}
export function decodeDtoVectorLabelAtomResult(wire: d.DtoVectorLabelAtomResult): Record<string, unknown> {
  return c.omitUndefined({
    "atom_id": c.required(wire.atomId, "atom_id"),
    "label_id": c.required(wire.labelId, "label_id"),
    "label_name": c.required(wire.labelName, "label_name"),
    "board_id": c.required(wire.boardId, "board_id"),
    "polarity": c.required(wire.polarity, "polarity"),
    "kind": c.required(wire.kind, "kind"),
    "text": c.required(wire.text, "text"),
    "ordinal": c.safeNumber(c.required(wire.ordinal, "ordinal")),
    "content_hash": c.required(wire.contentHash, "content_hash"),
    "embedding_model": c.required(wire.embeddingModel, "embedding_model"),
    "distance": c.float(c.required(wire.distance, "distance")),
    "vector": wire.vector === undefined ? undefined : ((value) => decodeListOfF32(value))(wire.vector),
  })
}

export function encodeDtoVectorStatus(value: unknown): d.DtoVectorStatus {
  const dto = c.record(value, ["backend","enabled","message","diagnostics","dirty","board_dirty","generation"])
  return create(d.DtoVectorStatusSchema, {
    backend: c.text(dto["backend"]),
    enabled: c.bool(dto["enabled"]),
    message: c.text(dto["message"]),
    diagnostics: c.array((dto["diagnostics"] ?? []), (value) => c.text(value)),
    dirty: c.optional(dto["dirty"], (value) => c.bool(value)),
    boardDirty: c.optional(dto["board_dirty"], (value) => c.bool(value)),
    generation: c.optional(dto["generation"], (value) => c.int64(value)),
  })
}
export function decodeDtoVectorStatus(wire: d.DtoVectorStatus): Record<string, unknown> {
  return c.omitUndefined({
    "backend": c.required(wire.backend, "backend"),
    "enabled": c.required(wire.enabled, "enabled"),
    "message": c.required(wire.message, "message"),
    "diagnostics": wire.diagnostics.map((value) => value),
    "dirty": wire.dirty === undefined ? null : ((value) => value)(wire.dirty),
    "board_dirty": wire.boardDirty === undefined ? null : ((value) => value)(wire.boardDirty),
    "generation": wire.generation === undefined ? undefined : ((value) => c.safeNumber(value))(wire.generation),
  })
}

export function encodeDtoVectorStoreStatusWire(value: unknown): d.DtoVectorStoreStatusWire {
  const dto = c.record(value, ["backend","enabled","message","diagnostics","dirty","board_dirty","generation"])
  return create(d.DtoVectorStoreStatusWireSchema, {
    backend: c.text(dto["backend"]),
    enabled: c.bool(dto["enabled"]),
    message: c.text(dto["message"]),
    diagnostics: c.array((dto["diagnostics"] ?? []), (value) => c.text(value)),
    dirty: c.optional(dto["dirty"], (value) => c.bool(value)),
    boardDirty: c.optional(dto["board_dirty"], (value) => c.bool(value)),
    generation: c.optional(dto["generation"], (value) => c.int64(value)),
  })
}
export function decodeDtoVectorStoreStatusWire(wire: d.DtoVectorStoreStatusWire): Record<string, unknown> {
  return c.omitUndefined({
    "backend": c.required(wire.backend, "backend"),
    "enabled": c.required(wire.enabled, "enabled"),
    "message": c.required(wire.message, "message"),
    "diagnostics": wire.diagnostics.map((value) => value),
    "dirty": wire.dirty === undefined ? null : ((value) => value)(wire.dirty),
    "board_dirty": wire.boardDirty === undefined ? null : ((value) => value)(wire.boardDirty),
    "generation": wire.generation === undefined ? null : ((value) => c.safeNumber(value))(wire.generation),
  })
}

export function encodeExplainLabelAtomResponse(value: unknown): d.ExplainLabelAtomResponse {
  const dto = c.record(value, ["data"])
  return create(d.ExplainLabelAtomResponseSchema, {
    data: encodeDtoLabelAtomExplainWire(dto["data"]),
  })
}
export function decodeExplainLabelAtomResponse(wire: d.ExplainLabelAtomResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelAtomExplainWire(c.required(wire.data, "data")),
  })
}

export function encodeGetBoardResponse(value: unknown): d.GetBoardResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetBoardResponseSchema, {
    data: encodeDtoApiBoard(dto["data"]),
  })
}
export function decodeGetBoardResponse(wire: d.GetBoardResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiBoard(c.required(wire.data, "data")),
  })
}

export function encodeGetEntityResponse(value: unknown): d.GetEntityResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetEntityResponseSchema, {
    data: encodeDtoCliEntity(dto["data"]),
  })
}
export function decodeGetEntityResponse(wire: d.GetEntityResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoCliEntity(c.required(wire.data, "data")),
  })
}

export function encodeGetHealthResponse(value: unknown): d.GetHealthResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetHealthResponseSchema, {
    data: encodeDtoHealthReport(dto["data"]),
  })
}
export function decodeGetHealthResponse(wire: d.GetHealthResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoHealthReport(c.required(wire.data, "data")),
  })
}

export function encodeGetLabelOntologyQualityResponse(value: unknown): d.GetLabelOntologyQualityResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetLabelOntologyQualityResponseSchema, {
    data: encodeDtoCliLabelOntologyQuality(dto["data"]),
  })
}
export function decodeGetLabelOntologyQualityResponse(wire: d.GetLabelOntologyQualityResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoCliLabelOntologyQuality(c.required(wire.data, "data")),
  })
}

export function encodeGetLabelOntologySignalResponse(value: unknown): d.GetLabelOntologySignalResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetLabelOntologySignalResponseSchema, {
    data: encodeDtoLabelOntologySignalDetailWire(dto["data"]),
  })
}
export function decodeGetLabelOntologySignalResponse(wire: d.GetLabelOntologySignalResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelOntologySignalDetailWire(c.required(wire.data, "data")),
  })
}

export function encodeGetLabelProposalResponse(value: unknown): d.GetLabelProposalResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetLabelProposalResponseSchema, {
    data: encodeDtoLabelSemanticProposalWire(dto["data"]),
  })
}
export function decodeGetLabelProposalResponse(wire: d.GetLabelProposalResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelSemanticProposalWire(c.required(wire.data, "data")),
  })
}

export function encodeGetLabelSemanticsResponse(value: unknown): d.GetLabelSemanticsResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetLabelSemanticsResponseSchema, {
    data: encodeDtoLabelSemanticsWire(dto["data"]),
  })
}
export function decodeGetLabelSemanticsResponse(wire: d.GetLabelSemanticsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelSemanticsWire(c.required(wire.data, "data")),
  })
}

export function encodeGetRunLogResponse(value: unknown): d.GetRunLogResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetRunLogResponseSchema, {
    data: encodeDtoApiRunLog(dto["data"]),
  })
}
export function decodeGetRunLogResponse(wire: d.GetRunLogResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiRunLog(c.required(wire.data, "data")),
  })
}

export function encodeGetRunResponse(value: unknown): d.GetRunResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetRunResponseSchema, {
    data: encodeDtoApiRun(dto["data"]),
  })
}
export function decodeGetRunResponse(wire: d.GetRunResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiRun(c.required(wire.data, "data")),
  })
}

export function encodeGetSignalResponse(value: unknown): d.GetSignalResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetSignalResponseSchema, {
    data: encodeDtoSignalWire(dto["data"]),
  })
}
export function decodeGetSignalResponse(wire: d.GetSignalResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSignalWire(c.required(wire.data, "data")),
  })
}

export function encodeGetStatsResponse(value: unknown): d.GetStatsResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetStatsResponseSchema, {
    data: encodeDtoQueueStats(dto["data"]),
  })
}
export function decodeGetStatsResponse(wire: d.GetStatsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoQueueStats(c.required(wire.data, "data")),
  })
}

export function encodeGetTaskDetailsResponse(value: unknown): d.GetTaskDetailsResponse {
  const dto = c.record(value, ["data"])
  return create(d.GetTaskDetailsResponseSchema, {
    data: encodeDtoTaskDetailAggregate(dto["data"]),
  })
}
export function decodeGetTaskDetailsResponse(wire: d.GetTaskDetailsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoTaskDetailAggregate(c.required(wire.data, "data")),
  })
}

export function encodeGetTaskResponse(value: unknown): d.GetTaskResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.GetTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
    meta: c.optional(dto["meta"], (value) => encodeDtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary(value)),
  })
}
export function decodeGetTaskResponse(wire: d.GetTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
    "meta": wire.meta === undefined ? undefined : ((value) => decodeDtoTaskOntologyDetailsMetaOfOptionalDtoTaskOntologySummary(value))(wire.meta),
  })
}

export function encodeGraphNeighborsResponse(value: unknown): d.GraphNeighborsResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.GraphNeighborsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiRelation(value)),
    meta: encodeDtoLimitMeta(dto["meta"]),
  })
}
export function decodeGraphNeighborsResponse(wire: d.GraphNeighborsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiRelation(value)),
    "meta": decodeDtoLimitMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeGraphQueryResponse(value: unknown): d.GraphQueryResponse {
  const dto = c.record(value, ["data"])
  return create(d.GraphQueryResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoCliGraphQueryRow(value)),
  })
}
export function decodeGraphQueryResponse(wire: d.GraphQueryResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoCliGraphQueryRow(value)),
  })
}

export function encodeGraphRebuildResponse(value: unknown): d.GraphRebuildResponse {
  const dto = c.record(value, ["data"])
  return create(d.GraphRebuildResponseSchema, {
    data: encodeDtoGraphMaintenance(dto["data"]),
  })
}
export function decodeGraphRebuildResponse(wire: d.GraphRebuildResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoGraphMaintenance(c.required(wire.data, "data")),
  })
}

export function encodeGraphStatusResponse(value: unknown): d.GraphStatusResponse {
  const dto = c.record(value, ["data"])
  return create(d.GraphStatusResponseSchema, {
    data: encodeDtoGraphStatus(dto["data"]),
  })
}
export function decodeGraphStatusResponse(wire: d.GraphStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoGraphStatus(c.required(wire.data, "data")),
  })
}

export function encodeGraphSyncResponse(value: unknown): d.GraphSyncResponse {
  const dto = c.record(value, ["data"])
  return create(d.GraphSyncResponseSchema, {
    data: encodeDtoGraphMaintenance(dto["data"]),
  })
}
export function decodeGraphSyncResponse(wire: d.GraphSyncResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoGraphMaintenance(c.required(wire.data, "data")),
  })
}

export function encodeHeartbeatTaskResponse(value: unknown): d.HeartbeatTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.HeartbeatTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeHeartbeatTaskResponse(wire: d.HeartbeatTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeLabelAtomIndexStatusResponse(value: unknown): d.LabelAtomIndexStatusResponse {
  const dto = c.record(value, ["data"])
  return create(d.LabelAtomIndexStatusResponseSchema, {
    data: encodeDtoVectorStoreStatusWire(dto["data"]),
  })
}
export function decodeLabelAtomIndexStatusResponse(wire: d.LabelAtomIndexStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVectorStoreStatusWire(c.required(wire.data, "data")),
  })
}

export function encodeListAttachmentsResponse(value: unknown): d.ListAttachmentsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListAttachmentsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiAttachment(value)),
  })
}
export function decodeListAttachmentsResponse(wire: d.ListAttachmentsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiAttachment(value)),
  })
}

export function encodeListBoardColumnsResponse(value: unknown): d.ListBoardColumnsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListBoardColumnsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiBoardColumn(value)),
  })
}
export function decodeListBoardColumnsResponse(wire: d.ListBoardColumnsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiBoardColumn(value)),
  })
}

export function encodeListBoardLabelProposalsResponse(value: unknown): d.ListBoardLabelProposalsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListBoardLabelProposalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelSemanticProposalWire(value)),
  })
}
export function decodeListBoardLabelProposalsResponse(wire: d.ListBoardLabelProposalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelSemanticProposalWire(value)),
  })
}

export function encodeListBoardLabelsResponse(value: unknown): d.ListBoardLabelsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListBoardLabelsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiLabel(value)),
  })
}
export function decodeListBoardLabelsResponse(wire: d.ListBoardLabelsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiLabel(value)),
  })
}

export function encodeListBoardsResponse(value: unknown): d.ListBoardsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListBoardsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiBoard(value)),
  })
}
export function decodeListBoardsResponse(wire: d.ListBoardsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiBoard(value)),
  })
}

export function encodeListCommentsResponse(value: unknown): d.ListCommentsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListCommentsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiComment(value)),
  })
}
export function decodeListCommentsResponse(wire: d.ListCommentsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiComment(value)),
  })
}

export function encodeListDependenciesResponse(value: unknown): d.ListDependenciesResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListDependenciesResponseSchema, {
    data: encodeDtoApiDependencies(dto["data"]),
  })
}
export function decodeListDependenciesResponse(wire: d.ListDependenciesResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiDependencies(c.required(wire.data, "data")),
  })
}

export function encodeListEntitiesResponse(value: unknown): d.ListEntitiesResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListEntitiesResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoCliEntity(value)),
  })
}
export function decodeListEntitiesResponse(wire: d.ListEntitiesResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoCliEntity(value)),
  })
}

export function encodeListEventsResponse(value: unknown): d.ListEventsResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ListEventsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoStreamEventData(value)),
    meta: encodeDtoNextAfterMeta(dto["meta"]),
  })
}
export function decodeListEventsResponse(wire: d.ListEventsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoStreamEventData(value)),
    "meta": decodeDtoNextAfterMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeListLabelAtomsResponse(value: unknown): d.ListLabelAtomsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListLabelAtomsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelAtomWire(value)),
  })
}
export function decodeListLabelAtomsResponse(wire: d.ListLabelAtomsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelAtomWire(value)),
  })
}

export function encodeListLabelOntologySignalsResponse(value: unknown): d.ListLabelOntologySignalsResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ListLabelOntologySignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelOntologySignalWire(value)),
    meta: encodeDtoSignalFilterMeta(dto["meta"]),
  })
}
export function decodeListLabelOntologySignalsResponse(wire: d.ListLabelOntologySignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelOntologySignalWire(value)),
    "meta": decodeDtoSignalFilterMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeListLabelSemanticsResponse(value: unknown): d.ListLabelSemanticsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListLabelSemanticsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelSemanticsWire(value)),
  })
}
export function decodeListLabelSemanticsResponse(wire: d.ListLabelSemanticsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelSemanticsWire(value)),
  })
}

export function encodeListOfF32(value: unknown): d.ListOfF32 {
  return create(d.ListOfF32Schema, { items: c.array(value, (value) => c.float(value, true)) })
}
export function decodeListOfF32(wire: d.ListOfF32): unknown {
  return wire.items.map((value) => c.float(value))
}

export function encodeListOfString(value: unknown): d.ListOfString {
  return create(d.ListOfStringSchema, { items: c.array(value, (value) => c.text(value)) })
}
export function decodeListOfString(wire: d.ListOfString): unknown {
  return wire.items.map((value) => value)
}

export function encodeListRunsResponse(value: unknown): d.ListRunsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListRunsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiRun(value)),
  })
}
export function decodeListRunsResponse(wire: d.ListRunsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiRun(value)),
  })
}

export function encodeListSignalsResponse(value: unknown): d.ListSignalsResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ListSignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoSignalWire(value)),
    meta: encodeDtoSignalFilterMeta(dto["meta"]),
  })
}
export function decodeListSignalsResponse(wire: d.ListSignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoSignalWire(value)),
    "meta": decodeDtoSignalFilterMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeListStepsResponse(value: unknown): d.ListStepsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListStepsResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeListStepsResponse(wire: d.ListStepsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeListTaskLabelProposalsResponse(value: unknown): d.ListTaskLabelProposalsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListTaskLabelProposalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelSemanticProposalWire(value)),
  })
}
export function decodeListTaskLabelProposalsResponse(wire: d.ListTaskLabelProposalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelSemanticProposalWire(value)),
  })
}

export function encodeListTaskLabelsResponse(value: unknown): d.ListTaskLabelsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ListTaskLabelsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiLabel(value)),
  })
}
export function decodeListTaskLabelsResponse(wire: d.ListTaskLabelsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiLabel(value)),
  })
}

export function encodeListTasksByStatusResponse(value: unknown): d.ListTasksByStatusResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ListTasksByStatusResponseSchema, {
    data: encodeDtoListTasksByStatusData(dto["data"]),
    meta: encodeDtoOffsetPaginationMeta(dto["meta"]),
  })
}
export function decodeListTasksByStatusResponse(wire: d.ListTasksByStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoListTasksByStatusData(c.required(wire.data, "data")),
    "meta": decodeDtoOffsetPaginationMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeListTasksResponse(value: unknown): d.ListTasksResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ListTasksResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoApiTask(value)),
    meta: encodeDtoTotalPaginationMeta(dto["meta"]),
  })
}
export function decodeListTasksResponse(wire: d.ListTasksResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoApiTask(value)),
    "meta": decodeDtoTotalPaginationMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeMaintenanceBackupResponse(value: unknown): d.MaintenanceBackupResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceBackupResponseSchema, {
    data: encodeDtoBackupReport(dto["data"]),
  })
}
export function decodeMaintenanceBackupResponse(wire: d.MaintenanceBackupResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoBackupReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceCleanupResponse(value: unknown): d.MaintenanceCleanupResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceCleanupResponseSchema, {
    data: encodeDtoMaintenanceRunReport(dto["data"]),
  })
}
export function decodeMaintenanceCleanupResponse(wire: d.MaintenanceCleanupResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoMaintenanceRunReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceExportResponse(value: unknown): d.MaintenanceExportResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceExportResponseSchema, {
    data: encodeDtoExportReport(dto["data"]),
  })
}
export function decodeMaintenanceExportResponse(wire: d.MaintenanceExportResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoExportReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceImportResponse(value: unknown): d.MaintenanceImportResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceImportResponseSchema, {
    data: encodeDtoImportReport(dto["data"]),
  })
}
export function decodeMaintenanceImportResponse(wire: d.MaintenanceImportResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoImportReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceImportV30Response(value: unknown): d.MaintenanceImportV30Response {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceImportV30ResponseSchema, {
    data: encodeDtoLegacyImportReport(dto["data"]),
  })
}
export function decodeMaintenanceImportV30Response(wire: d.MaintenanceImportV30Response): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLegacyImportReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceRebuildResponse(value: unknown): d.MaintenanceRebuildResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceRebuildResponseSchema, {
    data: encodeDtoMaintenanceRunReport(dto["data"]),
  })
}
export function decodeMaintenanceRebuildResponse(wire: d.MaintenanceRebuildResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoMaintenanceRunReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceRunResponse(value: unknown): d.MaintenanceRunResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceRunResponseSchema, {
    data: encodeDtoMaintenanceRunReport(dto["data"]),
  })
}
export function decodeMaintenanceRunResponse(wire: d.MaintenanceRunResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoMaintenanceRunReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceStatusResponse(value: unknown): d.MaintenanceStatusResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceStatusResponseSchema, {
    data: encodeDtoMaintenanceStatusReport(dto["data"]),
  })
}
export function decodeMaintenanceStatusResponse(wire: d.MaintenanceStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoMaintenanceStatusReport(c.required(wire.data, "data")),
  })
}

export function encodeMaintenanceVacuumResponse(value: unknown): d.MaintenanceVacuumResponse {
  const dto = c.record(value, ["data"])
  return create(d.MaintenanceVacuumResponseSchema, {
    data: encodeDtoVacuumReport(dto["data"]),
  })
}
export function decodeMaintenanceVacuumResponse(wire: d.MaintenanceVacuumResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVacuumReport(c.required(wire.data, "data")),
  })
}

export function encodeMapOfJsonValue(value: unknown): d.MapOfJsonValue {
  return create(d.MapOfJsonValueSchema, { entries: c.dictionary(value, (value) => c.encodeJson(value)) })
}
export function decodeMapOfJsonValue(wire: d.MapOfJsonValue): unknown {
  return Object.fromEntries(Object.entries(wire.entries).map(([key, value]) => [key, c.decodeJson(value)]))
}

export function encodeMarkExecutionPlanNotRequiredResponse(value: unknown): d.MarkExecutionPlanNotRequiredResponse {
  const dto = c.record(value, ["data"])
  return create(d.MarkExecutionPlanNotRequiredResponseSchema, {
    data: encodeDtoApiExecutionPlan(dto["data"]),
  })
}
export function decodeMarkExecutionPlanNotRequiredResponse(wire: d.MarkExecutionPlanNotRequiredResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiExecutionPlan(c.required(wire.data, "data")),
  })
}

export function encodePromoteTaskResponse(value: unknown): d.PromoteTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.PromoteTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodePromoteTaskResponse(wire: d.PromoteTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeProposeTaskLabelResponse(value: unknown): d.ProposeTaskLabelResponse {
  const dto = c.record(value, ["data"])
  return create(d.ProposeTaskLabelResponseSchema, {
    data: encodeDtoLabelProposalAttemptWire(dto["data"]),
  })
}
export function decodeProposeTaskLabelResponse(wire: d.ProposeTaskLabelResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelProposalAttemptWire(c.required(wire.data, "data")),
  })
}

export function encodeQueryLabelAtomIndexResponse(value: unknown): d.QueryLabelAtomIndexResponse {
  const dto = c.record(value, ["data"])
  return create(d.QueryLabelAtomIndexResponseSchema, {
    data: encodeDtoLabelAtomIndexQueryData(dto["data"]),
  })
}
export function decodeQueryLabelAtomIndexResponse(wire: d.QueryLabelAtomIndexResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelAtomIndexQueryData(c.required(wire.data, "data")),
  })
}

export function encodeRebuildLabelAtomIndexResponse(value: unknown): d.RebuildLabelAtomIndexResponse {
  const dto = c.record(value, ["data"])
  return create(d.RebuildLabelAtomIndexResponseSchema, {
    data: encodeDtoVectorStoreStatusWire(dto["data"]),
  })
}
export function decodeRebuildLabelAtomIndexResponse(wire: d.RebuildLabelAtomIndexResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVectorStoreStatusWire(c.required(wire.data, "data")),
  })
}

export function encodeRebuildSearchIndexResponse(value: unknown): d.RebuildSearchIndexResponse {
  const dto = c.record(value, ["data"])
  return create(d.RebuildSearchIndexResponseSchema, {
    data: encodeDtoSearchStatus(dto["data"]),
  })
}
export function decodeRebuildSearchIndexResponse(wire: d.RebuildSearchIndexResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSearchStatus(c.required(wire.data, "data")),
  })
}

export function encodeReclaimTaskResponse(value: unknown): d.ReclaimTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.ReclaimTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeReclaimTaskResponse(wire: d.ReclaimTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeRecordLabelOntologyObservationResponse(value: unknown): d.RecordLabelOntologyObservationResponse {
  const dto = c.record(value, ["data"])
  return create(d.RecordLabelOntologyObservationResponseSchema, {
    data: encodeDtoLabelOntologyObservationWire(dto["data"]),
  })
}
export function decodeRecordLabelOntologyObservationResponse(wire: d.RecordLabelOntologyObservationResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelOntologyObservationWire(c.required(wire.data, "data")),
  })
}

export function encodeRecordSignalResponse(value: unknown): d.RecordSignalResponse {
  const dto = c.record(value, ["data"])
  return create(d.RecordSignalResponseSchema, {
    data: encodeDtoSignalRecordResult(dto["data"]),
  })
}
export function decodeRecordSignalResponse(wire: d.RecordSignalResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSignalRecordResult(c.required(wire.data, "data")),
  })
}

export function encodeRejectLabelProposalResponse(value: unknown): d.RejectLabelProposalResponse {
  const dto = c.record(value, ["data"])
  return create(d.RejectLabelProposalResponseSchema, {
    data: encodeDtoLabelSemanticProposalWire(dto["data"]),
  })
}
export function decodeRejectLabelProposalResponse(wire: d.RejectLabelProposalResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelSemanticProposalWire(c.required(wire.data, "data")),
  })
}

export function encodeRejectSignalsResponse(value: unknown): d.RejectSignalsResponse {
  const dto = c.record(value, ["data"])
  return create(d.RejectSignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoSignalWire(value)),
  })
}
export function decodeRejectSignalsResponse(wire: d.RejectSignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoSignalWire(value)),
  })
}

export function encodeReleaseTaskResponse(value: unknown): d.ReleaseTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.ReleaseTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeReleaseTaskResponse(wire: d.ReleaseTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeRemoveDependencyResponse(value: unknown): d.RemoveDependencyResponse {
  const dto = c.record(value, ["data"])
  return create(d.RemoveDependencyResponseSchema, {
    data: encodeDtoApiDependencies(dto["data"]),
  })
}
export function decodeRemoveDependencyResponse(wire: d.RemoveDependencyResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiDependencies(c.required(wire.data, "data")),
  })
}

export function encodeRemoveStepResponse(value: unknown): d.RemoveStepResponse {
  const dto = c.record(value, ["data"])
  return create(d.RemoveStepResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeRemoveStepResponse(wire: d.RemoveStepResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeRemoveTaskLabelResponse(value: unknown): d.RemoveTaskLabelResponse {
  const dto = c.record(value, ["data"])
  return create(d.RemoveTaskLabelResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeRemoveTaskLabelResponse(wire: d.RemoveTaskLabelResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeReopenStepResponse(value: unknown): d.ReopenStepResponse {
  const dto = c.record(value, ["data"])
  return create(d.ReopenStepResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeReopenStepResponse(wire: d.ReopenStepResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeReopenTaskResponse(value: unknown): d.ReopenTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.ReopenTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeReopenTaskResponse(wire: d.ReopenTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeResolveSignalsResponse(value: unknown): d.ResolveSignalsResponse {
  const dto = c.record(value, ["data"])
  return create(d.ResolveSignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoSignalWire(value)),
  })
}
export function decodeResolveSignalsResponse(wire: d.ResolveSignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoSignalWire(value)),
  })
}

export function encodeRevertLabelOntologyMutationResponse(value: unknown): d.RevertLabelOntologyMutationResponse {
  const dto = c.record(value, ["data"])
  return create(d.RevertLabelOntologyMutationResponseSchema, {
    data: encodeDtoLabelOntologyActionWire(dto["data"]),
  })
}
export function decodeRevertLabelOntologyMutationResponse(wire: d.RevertLabelOntologyMutationResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelOntologyActionWire(c.required(wire.data, "data")),
  })
}

export function encodeReviewLabelOntologyResponse(value: unknown): d.ReviewLabelOntologyResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ReviewLabelOntologyResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoLabelOntologyReviewGroupWire(value)),
    meta: encodeDtoLabelOntologyReviewMeta(dto["meta"]),
  })
}
export function decodeReviewLabelOntologyResponse(wire: d.ReviewLabelOntologyResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoLabelOntologyReviewGroupWire(value)),
    "meta": decodeDtoLabelOntologyReviewMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeReviewSignalsResponse(value: unknown): d.ReviewSignalsResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.ReviewSignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoSignalWire(value)),
    meta: encodeDtoSignalFilterMeta(dto["meta"]),
  })
}
export function decodeReviewSignalsResponse(wire: d.ReviewSignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoSignalWire(value)),
    "meta": decodeDtoSignalFilterMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeSearchStatusResponse(value: unknown): d.SearchStatusResponse {
  const dto = c.record(value, ["data"])
  return create(d.SearchStatusResponseSchema, {
    data: encodeDtoSearchStatus(dto["data"]),
  })
}
export function decodeSearchStatusResponse(wire: d.SearchStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSearchStatus(c.required(wire.data, "data")),
  })
}

export function encodeSearchTasksByStatusResponse(value: unknown): d.SearchTasksByStatusResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.SearchTasksByStatusResponseSchema, {
    data: encodeDtoSearchTaskStatusWindows(dto["data"]),
    meta: encodeDtoOffsetPaginationMeta(dto["meta"]),
  })
}
export function decodeSearchTasksByStatusResponse(wire: d.SearchTasksByStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSearchTaskStatusWindows(c.required(wire.data, "data")),
    "meta": decodeDtoOffsetPaginationMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeSearchTasksResponse(value: unknown): d.SearchTasksResponse {
  const dto = c.record(value, ["data","meta"])
  return create(d.SearchTasksResponseSchema, {
    data: encodeDtoSearchTasksData(dto["data"]),
    meta: encodeDtoOffsetPaginationMeta(dto["meta"]),
  })
}
export function decodeSearchTasksResponse(wire: d.SearchTasksResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSearchTasksData(c.required(wire.data, "data")),
    "meta": decodeDtoOffsetPaginationMeta(c.required(wire.meta, "meta")),
  })
}

export function encodeSkipStepResponse(value: unknown): d.SkipStepResponse {
  const dto = c.record(value, ["data"])
  return create(d.SkipStepResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeSkipStepResponse(wire: d.SkipStepResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeSpecifyTaskResponse(value: unknown): d.SpecifyTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.SpecifyTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeSpecifyTaskResponse(wire: d.SpecifyTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeSubmitReviewTaskResponse(value: unknown): d.SubmitReviewTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.SubmitReviewTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeSubmitReviewTaskResponse(wire: d.SubmitReviewTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeSuggestTaskLabelsResponse(value: unknown): d.SuggestTaskLabelsResponse {
  const dto = c.record(value, ["data"])
  return create(d.SuggestTaskLabelsResponseSchema, {
    data: encodeDtoLabelSuggestionResultWire(dto["data"]),
  })
}
export function decodeSuggestTaskLabelsResponse(wire: d.SuggestTaskLabelsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelSuggestionResultWire(c.required(wire.data, "data")),
  })
}

export function encodeSupersedeSignalsResponse(value: unknown): d.SupersedeSignalsResponse {
  const dto = c.record(value, ["data"])
  return create(d.SupersedeSignalsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoSignalWire(value)),
  })
}
export function decodeSupersedeSignalsResponse(wire: d.SupersedeSignalsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoSignalWire(value)),
  })
}

export function encodeSyncSearchIndexResponse(value: unknown): d.SyncSearchIndexResponse {
  const dto = c.record(value, ["data"])
  return create(d.SyncSearchIndexResponseSchema, {
    data: encodeDtoSearchStatus(dto["data"]),
  })
}
export function decodeSyncSearchIndexResponse(wire: d.SyncSearchIndexResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoSearchStatus(c.required(wire.data, "data")),
  })
}

export function encodeTaskNeighborhoodResponse(value: unknown): d.TaskNeighborhoodResponse {
  const dto = c.record(value, ["data"])
  return create(d.TaskNeighborhoodResponseSchema, {
    data: encodeDtoTaskNeighborhood(dto["data"]),
  })
}
export function decodeTaskNeighborhoodResponse(wire: d.TaskNeighborhoodResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoTaskNeighborhood(c.required(wire.data, "data")),
  })
}

export function encodeUnblockTaskResponse(value: unknown): d.UnblockTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.UnblockTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeUnblockTaskResponse(wire: d.UnblockTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeUpdateStepResponse(value: unknown): d.UpdateStepResponse {
  const dto = c.record(value, ["data"])
  return create(d.UpdateStepResponseSchema, {
    data: encodeDtoApiTaskSteps(dto["data"]),
  })
}
export function decodeUpdateStepResponse(wire: d.UpdateStepResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTaskSteps(c.required(wire.data, "data")),
  })
}

export function encodeUpdateTaskResponse(value: unknown): d.UpdateTaskResponse {
  const dto = c.record(value, ["data"])
  return create(d.UpdateTaskResponseSchema, {
    data: encodeDtoApiTask(dto["data"]),
  })
}
export function decodeUpdateTaskResponse(wire: d.UpdateTaskResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoApiTask(c.required(wire.data, "data")),
  })
}

export function encodeUpsertEntityResponse(value: unknown): d.UpsertEntityResponse {
  const dto = c.record(value, ["data"])
  return create(d.UpsertEntityResponseSchema, {
    data: encodeDtoCliEntity(dto["data"]),
  })
}
export function decodeUpsertEntityResponse(wire: d.UpsertEntityResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoCliEntity(c.required(wire.data, "data")),
  })
}

export function encodeUpsertLabelSemanticsResponse(value: unknown): d.UpsertLabelSemanticsResponse {
  const dto = c.record(value, ["data"])
  return create(d.UpsertLabelSemanticsResponseSchema, {
    data: encodeDtoLabelSemanticsWire(dto["data"]),
  })
}
export function decodeUpsertLabelSemanticsResponse(wire: d.UpsertLabelSemanticsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelSemanticsWire(c.required(wire.data, "data")),
  })
}

export function encodeValidateLabelOntologyActionResponse(value: unknown): d.ValidateLabelOntologyActionResponse {
  const dto = c.record(value, ["data"])
  return create(d.ValidateLabelOntologyActionResponseSchema, {
    data: encodeDtoLabelOntologyActionWire(dto["data"]),
  })
}
export function decodeValidateLabelOntologyActionResponse(wire: d.ValidateLabelOntologyActionResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoLabelOntologyActionWire(c.required(wire.data, "data")),
  })
}

export function encodeVectorConfigureResponse(value: unknown): d.VectorConfigureResponse {
  const dto = c.record(value, ["data"])
  return create(d.VectorConfigureResponseSchema, {
    data: encodeDtoVectorConfigureRequest(dto["data"]),
  })
}
export function decodeVectorConfigureResponse(wire: d.VectorConfigureResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVectorConfigureRequest(c.required(wire.data, "data")),
  })
}

export function encodeVectorQueryChunksResponse(value: unknown): d.VectorQueryChunksResponse {
  const dto = c.record(value, ["data"])
  return create(d.VectorQueryChunksResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoVectorChunkResult(value)),
  })
}
export function decodeVectorQueryChunksResponse(wire: d.VectorQueryChunksResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoVectorChunkResult(value)),
  })
}

export function encodeVectorQueryLabelAtomsResponse(value: unknown): d.VectorQueryLabelAtomsResponse {
  const dto = c.record(value, ["data"])
  return create(d.VectorQueryLabelAtomsResponseSchema, {
    data: c.array(dto["data"], (value) => encodeDtoVectorLabelAtomResult(value)),
  })
}
export function decodeVectorQueryLabelAtomsResponse(wire: d.VectorQueryLabelAtomsResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": wire.data.map((value) => decodeDtoVectorLabelAtomResult(value)),
  })
}

export function encodeVectorRebuildResponse(value: unknown): d.VectorRebuildResponse {
  const dto = c.record(value, ["data"])
  return create(d.VectorRebuildResponseSchema, {
    data: encodeDtoVectorStatus(dto["data"]),
  })
}
export function decodeVectorRebuildResponse(wire: d.VectorRebuildResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVectorStatus(c.required(wire.data, "data")),
  })
}

export function encodeVectorStatusResponse(value: unknown): d.VectorStatusResponse {
  const dto = c.record(value, ["data"])
  return create(d.VectorStatusResponseSchema, {
    data: encodeDtoVectorStatus(dto["data"]),
  })
}
export function decodeVectorStatusResponse(wire: d.VectorStatusResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVectorStatus(c.required(wire.data, "data")),
  })
}

export function encodeVectorSyncResponse(value: unknown): d.VectorSyncResponse {
  const dto = c.record(value, ["data"])
  return create(d.VectorSyncResponseSchema, {
    data: encodeDtoVectorStatus(dto["data"]),
  })
}
export function decodeVectorSyncResponse(wire: d.VectorSyncResponse): Record<string, unknown> {
  return c.omitUndefined({
    "data": decodeDtoVectorStatus(c.required(wire.data, "data")),
  })
}

export async function invokeRpc(client: Client<typeof s.KanbanService>, call: RpcCall, options: CallOptions): Promise<unknown> {
  switch (call.method) {
    case "GetHealth": return decodeGetHealthResponse(await client.getHealth(encodeGetHealthRequest(call), options))
    case "ListBoards": return decodeListBoardsResponse(await client.listBoards(encodeListBoardsRequest(call), options))
    case "CreateBoard": return decodeCreateBoardResponse(await client.createBoard(encodeCreateBoardRequest(call), options))
    case "GetBoard": return decodeGetBoardResponse(await client.getBoard(encodeGetBoardRequest(call), options))
    case "ArchiveBoard": return decodeArchiveBoardResponse(await client.archiveBoard(encodeArchiveBoardRequest(call), options))
    case "ListBoardColumns": return decodeListBoardColumnsResponse(await client.listBoardColumns(encodeListBoardColumnsRequest(call), options))
    case "ListTasks": return decodeListTasksResponse(await client.listTasks(encodeListTasksRequest(call), options))
    case "ListTasksByStatus": return decodeListTasksByStatusResponse(await client.listTasksByStatus(encodeListTasksByStatusRequest(call), options))
    case "CreateTask": return decodeCreateTaskResponse(await client.createTask(encodeCreateTaskRequest(call), options))
    case "GetTask": return decodeGetTaskResponse(await client.getTask(encodeGetTaskRequest(call), options))
    case "UpdateTask": return decodeUpdateTaskResponse(await client.updateTask(encodeUpdateTaskRequest(call), options))
    case "SpecifyTask": return decodeSpecifyTaskResponse(await client.specifyTask(encodeSpecifyTaskRequest(call), options))
    case "PromoteTask": return decodePromoteTaskResponse(await client.promoteTask(encodePromoteTaskRequest(call), options))
    case "ClaimTask": return decodeClaimTaskResponse(await client.claimTask(encodeClaimTaskRequest(call), options))
    case "ReopenTask": return decodeReopenTaskResponse(await client.reopenTask(encodeReopenTaskRequest(call), options))
    case "ReclaimTask": return decodeReclaimTaskResponse(await client.reclaimTask(encodeReclaimTaskRequest(call), options))
    case "HeartbeatTask": return decodeHeartbeatTaskResponse(await client.heartbeatTask(encodeHeartbeatTaskRequest(call), options))
    case "ReleaseTask": return decodeReleaseTaskResponse(await client.releaseTask(encodeReleaseTaskRequest(call), options))
    case "CompleteTask": return decodeCompleteTaskResponse(await client.completeTask(encodeCompleteTaskRequest(call), options))
    case "SubmitReviewTask": return decodeSubmitReviewTaskResponse(await client.submitReviewTask(encodeSubmitReviewTaskRequest(call), options))
    case "BlockTask": return decodeBlockTaskResponse(await client.blockTask(encodeBlockTaskRequest(call), options))
    case "UnblockTask": return decodeUnblockTaskResponse(await client.unblockTask(encodeUnblockTaskRequest(call), options))
    case "ArchiveTask": return decodeArchiveTaskResponse(await client.archiveTask(encodeArchiveTaskRequest(call), options))
    case "ListSteps": return decodeListStepsResponse(await client.listSteps(encodeListStepsRequest(call), options))
    case "CreateStep": return decodeCreateStepResponse(await client.createStep(encodeCreateStepRequest(call), options))
    case "UpdateStep": return decodeUpdateStepResponse(await client.updateStep(encodeUpdateStepRequest(call), options))
    case "RemoveStep": return decodeRemoveStepResponse(await client.removeStep(encodeRemoveStepRequest(call), options))
    case "CompleteStep": return decodeCompleteStepResponse(await client.completeStep(encodeCompleteStepRequest(call), options))
    case "SkipStep": return decodeSkipStepResponse(await client.skipStep(encodeSkipStepRequest(call), options))
    case "ReopenStep": return decodeReopenStepResponse(await client.reopenStep(encodeReopenStepRequest(call), options))
    case "MarkExecutionPlanNotRequired": return decodeMarkExecutionPlanNotRequiredResponse(await client.markExecutionPlanNotRequired(encodeMarkExecutionPlanNotRequiredRequest(call), options))
    case "ListDependencies": return decodeListDependenciesResponse(await client.listDependencies(encodeListDependenciesRequest(call), options))
    case "AddDependency": return decodeAddDependencyResponse(await client.addDependency(encodeAddDependencyRequest(call), options))
    case "RemoveDependency": return decodeRemoveDependencyResponse(await client.removeDependency(encodeRemoveDependencyRequest(call), options))
    case "ListRuns": return decodeListRunsResponse(await client.listRuns(encodeListRunsRequest(call), options))
    case "GetRun": return decodeGetRunResponse(await client.getRun(encodeGetRunRequest(call), options))
    case "GetRunLog": return decodeGetRunLogResponse(await client.getRunLog(encodeGetRunLogRequest(call), options))
    case "ListComments": return decodeListCommentsResponse(await client.listComments(encodeListCommentsRequest(call), options))
    case "CreateComment": return decodeCreateCommentResponse(await client.createComment(encodeCreateCommentRequest(call), options))
    case "ListAttachments": return decodeListAttachmentsResponse(await client.listAttachments(encodeListAttachmentsRequest(call), options))
    case "CreateAttachment": return decodeCreateAttachmentResponse(await client.createAttachment(encodeCreateAttachmentRequest(call), options))
    case "DownloadAttachment": return decodeDownloadAttachmentResponse(await client.downloadAttachment(encodeDownloadAttachmentRequest(call), options))
    case "DeleteAttachment": return decodeDeleteAttachmentResponse(await client.deleteAttachment(encodeDeleteAttachmentRequest(call), options))
    case "ListEvents": return decodeListEventsResponse(await client.listEvents(encodeListEventsRequest(call), options))
    case "ListTaskLabels": return decodeListTaskLabelsResponse(await client.listTaskLabels(encodeListTaskLabelsRequest(call), options))
    case "AddTaskLabel": return decodeAddTaskLabelResponse(await client.addTaskLabel(encodeAddTaskLabelRequest(call), options))
    case "BootstrapTaskLabel": return decodeBootstrapTaskLabelResponse(await client.bootstrapTaskLabel(encodeBootstrapTaskLabelRequest(call), options))
    case "RemoveTaskLabel": return decodeRemoveTaskLabelResponse(await client.removeTaskLabel(encodeRemoveTaskLabelRequest(call), options))
    case "ListBoardLabels": return decodeListBoardLabelsResponse(await client.listBoardLabels(encodeListBoardLabelsRequest(call), options))
    case "ListBoardLabelProposals": return decodeListBoardLabelProposalsResponse(await client.listBoardLabelProposals(encodeListBoardLabelProposalsRequest(call), options))
    case "CreateBoardLabel": return decodeCreateBoardLabelResponse(await client.createBoardLabel(encodeCreateBoardLabelRequest(call), options))
    case "DeleteBoardLabel": return decodeDeleteBoardLabelResponse(await client.deleteBoardLabel(encodeDeleteBoardLabelRequest(call), options))
    case "ListLabelSemantics": return decodeListLabelSemanticsResponse(await client.listLabelSemantics(encodeListLabelSemanticsRequest(call), options))
    case "GetLabelSemantics": return decodeGetLabelSemanticsResponse(await client.getLabelSemantics(encodeGetLabelSemanticsRequest(call), options))
    case "UpsertLabelSemantics": return decodeUpsertLabelSemanticsResponse(await client.upsertLabelSemantics(encodeUpsertLabelSemanticsRequest(call), options))
    case "DeleteLabelSemantics": return decodeDeleteLabelSemanticsResponse(await client.deleteLabelSemantics(encodeDeleteLabelSemanticsRequest(call), options))
    case "ListLabelAtoms": return decodeListLabelAtomsResponse(await client.listLabelAtoms(encodeListLabelAtomsRequest(call), options))
    case "ExplainLabelAtom": return decodeExplainLabelAtomResponse(await client.explainLabelAtom(encodeExplainLabelAtomRequest(call), options))
    case "LabelAtomIndexStatus": return decodeLabelAtomIndexStatusResponse(await client.labelAtomIndexStatus(encodeLabelAtomIndexStatusRequest(call), options))
    case "RebuildLabelAtomIndex": return decodeRebuildLabelAtomIndexResponse(await client.rebuildLabelAtomIndex(encodeRebuildLabelAtomIndexRequest(call), options))
    case "QueryLabelAtomIndex": return decodeQueryLabelAtomIndexResponse(await client.queryLabelAtomIndex(encodeQueryLabelAtomIndexRequest(call), options))
    case "ListSignals": return decodeListSignalsResponse(await client.listSignals(encodeListSignalsRequest(call), options))
    case "ReviewSignals": return decodeReviewSignalsResponse(await client.reviewSignals(encodeReviewSignalsRequest(call), options))
    case "GetSignal": return decodeGetSignalResponse(await client.getSignal(encodeGetSignalRequest(call), options))
    case "RecordSignal": return decodeRecordSignalResponse(await client.recordSignal(encodeRecordSignalRequest(call), options))
    case "ConfirmSignals": return decodeConfirmSignalsResponse(await client.confirmSignals(encodeConfirmSignalsRequest(call), options))
    case "RejectSignals": return decodeRejectSignalsResponse(await client.rejectSignals(encodeRejectSignalsRequest(call), options))
    case "ResolveSignals": return decodeResolveSignalsResponse(await client.resolveSignals(encodeResolveSignalsRequest(call), options))
    case "SupersedeSignals": return decodeSupersedeSignalsResponse(await client.supersedeSignals(encodeSupersedeSignalsRequest(call), options))
    case "SuggestTaskLabels": return decodeSuggestTaskLabelsResponse(await client.suggestTaskLabels(encodeSuggestTaskLabelsRequest(call), options))
    case "ListTaskLabelProposals": return decodeListTaskLabelProposalsResponse(await client.listTaskLabelProposals(encodeListTaskLabelProposalsRequest(call), options))
    case "ProposeTaskLabel": return decodeProposeTaskLabelResponse(await client.proposeTaskLabel(encodeProposeTaskLabelRequest(call), options))
    case "RecordLabelOntologyObservation": return decodeRecordLabelOntologyObservationResponse(await client.recordLabelOntologyObservation(encodeRecordLabelOntologyObservationRequest(call), options))
    case "ListLabelOntologySignals": return decodeListLabelOntologySignalsResponse(await client.listLabelOntologySignals(encodeListLabelOntologySignalsRequest(call), options))
    case "ReviewLabelOntology": return decodeReviewLabelOntologyResponse(await client.reviewLabelOntology(encodeReviewLabelOntologyRequest(call), options))
    case "CreateLabelOntologyAction": return decodeCreateLabelOntologyActionResponse(await client.createLabelOntologyAction(encodeCreateLabelOntologyActionRequest(call), options))
    case "ApplyLabelOntologyAtom": return decodeApplyLabelOntologyAtomResponse(await client.applyLabelOntologyAtom(encodeApplyLabelOntologyAtomRequest(call), options))
    case "RevertLabelOntologyMutation": return decodeRevertLabelOntologyMutationResponse(await client.revertLabelOntologyMutation(encodeRevertLabelOntologyMutationRequest(call), options))
    case "ValidateLabelOntologyAction": return decodeValidateLabelOntologyActionResponse(await client.validateLabelOntologyAction(encodeValidateLabelOntologyActionRequest(call), options))
    case "GetLabelOntologySignal": return decodeGetLabelOntologySignalResponse(await client.getLabelOntologySignal(encodeGetLabelOntologySignalRequest(call), options))
    case "GetLabelProposal": return decodeGetLabelProposalResponse(await client.getLabelProposal(encodeGetLabelProposalRequest(call), options))
    case "AcceptLabelProposal": return decodeAcceptLabelProposalResponse(await client.acceptLabelProposal(encodeAcceptLabelProposalRequest(call), options))
    case "RejectLabelProposal": return decodeRejectLabelProposalResponse(await client.rejectLabelProposal(encodeRejectLabelProposalRequest(call), options))
    case "BoardTaskMap": return decodeBoardTaskMapResponse(await client.boardTaskMap(encodeBoardTaskMapRequest(call), options))
    case "TaskNeighborhood": return decodeTaskNeighborhoodResponse(await client.taskNeighborhood(encodeTaskNeighborhoodRequest(call), options))
    case "SearchTasks": return decodeSearchTasksResponse(await client.searchTasks(encodeSearchTasksRequest(call), options))
    case "SearchTasksByStatus": return decodeSearchTasksByStatusResponse(await client.searchTasksByStatus(encodeSearchTasksByStatusRequest(call), options))
    case "SearchStatus": return decodeSearchStatusResponse(await client.searchStatus(encodeSearchStatusRequest(call), options))
    case "RebuildSearchIndex": return decodeRebuildSearchIndexResponse(await client.rebuildSearchIndex(encodeRebuildSearchIndexRequest(call), options))
    case "SyncSearchIndex": return decodeSyncSearchIndexResponse(await client.syncSearchIndex(encodeSyncSearchIndexRequest(call), options))
    case "BuildContext": return decodeBuildContextResponse(await client.buildContext(encodeBuildContextRequest(call), options))
    case "GraphStatus": return decodeGraphStatusResponse(await client.graphStatus(encodeGraphStatusRequest(call), options))
    case "GraphNeighbors": return decodeGraphNeighborsResponse(await client.graphNeighbors(encodeGraphNeighborsRequest(call), options))
    case "GraphQuery": return decodeGraphQueryResponse(await client.graphQuery(encodeGraphQueryRequest(call), options))
    case "GraphRebuild": return decodeGraphRebuildResponse(await client.graphRebuild(encodeGraphRebuildRequest(call), options))
    case "GraphSync": return decodeGraphSyncResponse(await client.graphSync(encodeGraphSyncRequest(call), options))
    case "ListEntities": return decodeListEntitiesResponse(await client.listEntities(encodeListEntitiesRequest(call), options))
    case "UpsertEntity": return decodeUpsertEntityResponse(await client.upsertEntity(encodeUpsertEntityRequest(call), options))
    case "GetEntity": return decodeGetEntityResponse(await client.getEntity(encodeGetEntityRequest(call), options))
    case "VectorStatus": return decodeVectorStatusResponse(await client.vectorStatus(encodeVectorStatusRequest(call), options))
    case "VectorConfigure": return decodeVectorConfigureResponse(await client.vectorConfigure(encodeVectorConfigureRequest(call), options))
    case "VectorRebuild": return decodeVectorRebuildResponse(await client.vectorRebuild(encodeVectorRebuildRequest(call), options))
    case "VectorSync": return decodeVectorSyncResponse(await client.vectorSync(encodeVectorSyncRequest(call), options))
    case "VectorQueryChunks": return decodeVectorQueryChunksResponse(await client.vectorQueryChunks(encodeVectorQueryChunksRequest(call), options))
    case "VectorQueryLabelAtoms": return decodeVectorQueryLabelAtomsResponse(await client.vectorQueryLabelAtoms(encodeVectorQueryLabelAtomsRequest(call), options))
    case "GetStats": return decodeGetStatsResponse(await client.getStats(encodeGetStatsRequest(call), options))
    case "Doctor": return decodeDoctorResponse(await client.doctor(encodeDoctorRequest(call), options))
    case "Checkpoint": return decodeCheckpointResponse(await client.checkpoint(encodeCheckpointRequest(call), options))
    case "MaintenanceBackup": return decodeMaintenanceBackupResponse(await client.maintenanceBackup(encodeMaintenanceBackupRequest(call), options))
    case "MaintenanceExport": return decodeMaintenanceExportResponse(await client.maintenanceExport(encodeMaintenanceExportRequest(call), options))
    case "MaintenanceImport": return decodeMaintenanceImportResponse(await client.maintenanceImport(encodeMaintenanceImportRequest(call), options))
    case "MaintenanceVacuum": return decodeMaintenanceVacuumResponse(await client.maintenanceVacuum(encodeMaintenanceVacuumRequest(call), options))
    case "MaintenanceStatus": return decodeMaintenanceStatusResponse(await client.maintenanceStatus(encodeMaintenanceStatusRequest(call), options))
    case "MaintenanceRun": return decodeMaintenanceRunResponse(await client.maintenanceRun(encodeMaintenanceRunRequest(call), options))
    case "MaintenanceRebuild": return decodeMaintenanceRebuildResponse(await client.maintenanceRebuild(encodeMaintenanceRebuildRequest(call), options))
    case "MaintenanceCleanup": return decodeMaintenanceCleanupResponse(await client.maintenanceCleanup(encodeMaintenanceCleanupRequest(call), options))
    case "MaintenanceImportV30": return decodeMaintenanceImportV30Response(await client.maintenanceImportV30(encodeMaintenanceImportV30Request(call), options))
    case "GetTaskDetails": return decodeGetTaskDetailsResponse(await client.getTaskDetails(encodeGetTaskDetailsRequest(call), options))
    case "GetLabelOntologyQuality": return decodeGetLabelOntologyQualityResponse(await client.getLabelOntologyQuality(encodeGetLabelOntologyQualityRequest(call), options))
  }
}

/** 用同一份字段映射生成真实 binary RPC 测试响应。 */
export function encodeRpcResponse(method: RpcMethod, payload: unknown): Uint8Array {
  switch (method) {
    case "GetHealth": return toBinary(d.GetHealthResponseSchema, encodeGetHealthResponse(payload))
    case "ListBoards": return toBinary(d.ListBoardsResponseSchema, encodeListBoardsResponse(payload))
    case "CreateBoard": return toBinary(d.CreateBoardResponseSchema, encodeCreateBoardResponse(payload))
    case "GetBoard": return toBinary(d.GetBoardResponseSchema, encodeGetBoardResponse(payload))
    case "ArchiveBoard": return toBinary(d.ArchiveBoardResponseSchema, encodeArchiveBoardResponse(payload))
    case "ListBoardColumns": return toBinary(d.ListBoardColumnsResponseSchema, encodeListBoardColumnsResponse(payload))
    case "ListTasks": return toBinary(d.ListTasksResponseSchema, encodeListTasksResponse(payload))
    case "ListTasksByStatus": return toBinary(d.ListTasksByStatusResponseSchema, encodeListTasksByStatusResponse(payload))
    case "CreateTask": return toBinary(d.CreateTaskResponseSchema, encodeCreateTaskResponse(payload))
    case "GetTask": return toBinary(d.GetTaskResponseSchema, encodeGetTaskResponse(payload))
    case "UpdateTask": return toBinary(d.UpdateTaskResponseSchema, encodeUpdateTaskResponse(payload))
    case "SpecifyTask": return toBinary(d.SpecifyTaskResponseSchema, encodeSpecifyTaskResponse(payload))
    case "PromoteTask": return toBinary(d.PromoteTaskResponseSchema, encodePromoteTaskResponse(payload))
    case "ClaimTask": return toBinary(d.ClaimTaskResponseSchema, encodeClaimTaskResponse(payload))
    case "ReopenTask": return toBinary(d.ReopenTaskResponseSchema, encodeReopenTaskResponse(payload))
    case "ReclaimTask": return toBinary(d.ReclaimTaskResponseSchema, encodeReclaimTaskResponse(payload))
    case "HeartbeatTask": return toBinary(d.HeartbeatTaskResponseSchema, encodeHeartbeatTaskResponse(payload))
    case "ReleaseTask": return toBinary(d.ReleaseTaskResponseSchema, encodeReleaseTaskResponse(payload))
    case "CompleteTask": return toBinary(d.CompleteTaskResponseSchema, encodeCompleteTaskResponse(payload))
    case "SubmitReviewTask": return toBinary(d.SubmitReviewTaskResponseSchema, encodeSubmitReviewTaskResponse(payload))
    case "BlockTask": return toBinary(d.BlockTaskResponseSchema, encodeBlockTaskResponse(payload))
    case "UnblockTask": return toBinary(d.UnblockTaskResponseSchema, encodeUnblockTaskResponse(payload))
    case "ArchiveTask": return toBinary(d.ArchiveTaskResponseSchema, encodeArchiveTaskResponse(payload))
    case "ListSteps": return toBinary(d.ListStepsResponseSchema, encodeListStepsResponse(payload))
    case "CreateStep": return toBinary(d.CreateStepResponseSchema, encodeCreateStepResponse(payload))
    case "UpdateStep": return toBinary(d.UpdateStepResponseSchema, encodeUpdateStepResponse(payload))
    case "RemoveStep": return toBinary(d.RemoveStepResponseSchema, encodeRemoveStepResponse(payload))
    case "CompleteStep": return toBinary(d.CompleteStepResponseSchema, encodeCompleteStepResponse(payload))
    case "SkipStep": return toBinary(d.SkipStepResponseSchema, encodeSkipStepResponse(payload))
    case "ReopenStep": return toBinary(d.ReopenStepResponseSchema, encodeReopenStepResponse(payload))
    case "MarkExecutionPlanNotRequired": return toBinary(d.MarkExecutionPlanNotRequiredResponseSchema, encodeMarkExecutionPlanNotRequiredResponse(payload))
    case "ListDependencies": return toBinary(d.ListDependenciesResponseSchema, encodeListDependenciesResponse(payload))
    case "AddDependency": return toBinary(d.AddDependencyResponseSchema, encodeAddDependencyResponse(payload))
    case "RemoveDependency": return toBinary(d.RemoveDependencyResponseSchema, encodeRemoveDependencyResponse(payload))
    case "ListRuns": return toBinary(d.ListRunsResponseSchema, encodeListRunsResponse(payload))
    case "GetRun": return toBinary(d.GetRunResponseSchema, encodeGetRunResponse(payload))
    case "GetRunLog": return toBinary(d.GetRunLogResponseSchema, encodeGetRunLogResponse(payload))
    case "ListComments": return toBinary(d.ListCommentsResponseSchema, encodeListCommentsResponse(payload))
    case "CreateComment": return toBinary(d.CreateCommentResponseSchema, encodeCreateCommentResponse(payload))
    case "ListAttachments": return toBinary(d.ListAttachmentsResponseSchema, encodeListAttachmentsResponse(payload))
    case "CreateAttachment": return toBinary(d.CreateAttachmentResponseSchema, encodeCreateAttachmentResponse(payload))
    case "DownloadAttachment": return toBinary(d.DownloadAttachmentResponseSchema, encodeDownloadAttachmentResponse(payload))
    case "DeleteAttachment": return toBinary(d.DeleteAttachmentResponseSchema, encodeDeleteAttachmentResponse(payload))
    case "ListEvents": return toBinary(d.ListEventsResponseSchema, encodeListEventsResponse(payload))
    case "ListTaskLabels": return toBinary(d.ListTaskLabelsResponseSchema, encodeListTaskLabelsResponse(payload))
    case "AddTaskLabel": return toBinary(d.AddTaskLabelResponseSchema, encodeAddTaskLabelResponse(payload))
    case "BootstrapTaskLabel": return toBinary(d.BootstrapTaskLabelResponseSchema, encodeBootstrapTaskLabelResponse(payload))
    case "RemoveTaskLabel": return toBinary(d.RemoveTaskLabelResponseSchema, encodeRemoveTaskLabelResponse(payload))
    case "ListBoardLabels": return toBinary(d.ListBoardLabelsResponseSchema, encodeListBoardLabelsResponse(payload))
    case "ListBoardLabelProposals": return toBinary(d.ListBoardLabelProposalsResponseSchema, encodeListBoardLabelProposalsResponse(payload))
    case "CreateBoardLabel": return toBinary(d.CreateBoardLabelResponseSchema, encodeCreateBoardLabelResponse(payload))
    case "DeleteBoardLabel": return toBinary(d.DeleteBoardLabelResponseSchema, encodeDeleteBoardLabelResponse(payload))
    case "ListLabelSemantics": return toBinary(d.ListLabelSemanticsResponseSchema, encodeListLabelSemanticsResponse(payload))
    case "GetLabelSemantics": return toBinary(d.GetLabelSemanticsResponseSchema, encodeGetLabelSemanticsResponse(payload))
    case "UpsertLabelSemantics": return toBinary(d.UpsertLabelSemanticsResponseSchema, encodeUpsertLabelSemanticsResponse(payload))
    case "DeleteLabelSemantics": return toBinary(d.DeleteLabelSemanticsResponseSchema, encodeDeleteLabelSemanticsResponse(payload))
    case "ListLabelAtoms": return toBinary(d.ListLabelAtomsResponseSchema, encodeListLabelAtomsResponse(payload))
    case "ExplainLabelAtom": return toBinary(d.ExplainLabelAtomResponseSchema, encodeExplainLabelAtomResponse(payload))
    case "LabelAtomIndexStatus": return toBinary(d.LabelAtomIndexStatusResponseSchema, encodeLabelAtomIndexStatusResponse(payload))
    case "RebuildLabelAtomIndex": return toBinary(d.RebuildLabelAtomIndexResponseSchema, encodeRebuildLabelAtomIndexResponse(payload))
    case "QueryLabelAtomIndex": return toBinary(d.QueryLabelAtomIndexResponseSchema, encodeQueryLabelAtomIndexResponse(payload))
    case "ListSignals": return toBinary(d.ListSignalsResponseSchema, encodeListSignalsResponse(payload))
    case "ReviewSignals": return toBinary(d.ReviewSignalsResponseSchema, encodeReviewSignalsResponse(payload))
    case "GetSignal": return toBinary(d.GetSignalResponseSchema, encodeGetSignalResponse(payload))
    case "RecordSignal": return toBinary(d.RecordSignalResponseSchema, encodeRecordSignalResponse(payload))
    case "ConfirmSignals": return toBinary(d.ConfirmSignalsResponseSchema, encodeConfirmSignalsResponse(payload))
    case "RejectSignals": return toBinary(d.RejectSignalsResponseSchema, encodeRejectSignalsResponse(payload))
    case "ResolveSignals": return toBinary(d.ResolveSignalsResponseSchema, encodeResolveSignalsResponse(payload))
    case "SupersedeSignals": return toBinary(d.SupersedeSignalsResponseSchema, encodeSupersedeSignalsResponse(payload))
    case "SuggestTaskLabels": return toBinary(d.SuggestTaskLabelsResponseSchema, encodeSuggestTaskLabelsResponse(payload))
    case "ListTaskLabelProposals": return toBinary(d.ListTaskLabelProposalsResponseSchema, encodeListTaskLabelProposalsResponse(payload))
    case "ProposeTaskLabel": return toBinary(d.ProposeTaskLabelResponseSchema, encodeProposeTaskLabelResponse(payload))
    case "RecordLabelOntologyObservation": return toBinary(d.RecordLabelOntologyObservationResponseSchema, encodeRecordLabelOntologyObservationResponse(payload))
    case "ListLabelOntologySignals": return toBinary(d.ListLabelOntologySignalsResponseSchema, encodeListLabelOntologySignalsResponse(payload))
    case "ReviewLabelOntology": return toBinary(d.ReviewLabelOntologyResponseSchema, encodeReviewLabelOntologyResponse(payload))
    case "CreateLabelOntologyAction": return toBinary(d.CreateLabelOntologyActionResponseSchema, encodeCreateLabelOntologyActionResponse(payload))
    case "ApplyLabelOntologyAtom": return toBinary(d.ApplyLabelOntologyAtomResponseSchema, encodeApplyLabelOntologyAtomResponse(payload))
    case "RevertLabelOntologyMutation": return toBinary(d.RevertLabelOntologyMutationResponseSchema, encodeRevertLabelOntologyMutationResponse(payload))
    case "ValidateLabelOntologyAction": return toBinary(d.ValidateLabelOntologyActionResponseSchema, encodeValidateLabelOntologyActionResponse(payload))
    case "GetLabelOntologySignal": return toBinary(d.GetLabelOntologySignalResponseSchema, encodeGetLabelOntologySignalResponse(payload))
    case "GetLabelProposal": return toBinary(d.GetLabelProposalResponseSchema, encodeGetLabelProposalResponse(payload))
    case "AcceptLabelProposal": return toBinary(d.AcceptLabelProposalResponseSchema, encodeAcceptLabelProposalResponse(payload))
    case "RejectLabelProposal": return toBinary(d.RejectLabelProposalResponseSchema, encodeRejectLabelProposalResponse(payload))
    case "BoardTaskMap": return toBinary(d.BoardTaskMapResponseSchema, encodeBoardTaskMapResponse(payload))
    case "TaskNeighborhood": return toBinary(d.TaskNeighborhoodResponseSchema, encodeTaskNeighborhoodResponse(payload))
    case "SearchTasks": return toBinary(d.SearchTasksResponseSchema, encodeSearchTasksResponse(payload))
    case "SearchTasksByStatus": return toBinary(d.SearchTasksByStatusResponseSchema, encodeSearchTasksByStatusResponse(payload))
    case "SearchStatus": return toBinary(d.SearchStatusResponseSchema, encodeSearchStatusResponse(payload))
    case "RebuildSearchIndex": return toBinary(d.RebuildSearchIndexResponseSchema, encodeRebuildSearchIndexResponse(payload))
    case "SyncSearchIndex": return toBinary(d.SyncSearchIndexResponseSchema, encodeSyncSearchIndexResponse(payload))
    case "BuildContext": return toBinary(d.BuildContextResponseSchema, encodeBuildContextResponse(payload))
    case "GraphStatus": return toBinary(d.GraphStatusResponseSchema, encodeGraphStatusResponse(payload))
    case "GraphNeighbors": return toBinary(d.GraphNeighborsResponseSchema, encodeGraphNeighborsResponse(payload))
    case "GraphQuery": return toBinary(d.GraphQueryResponseSchema, encodeGraphQueryResponse(payload))
    case "GraphRebuild": return toBinary(d.GraphRebuildResponseSchema, encodeGraphRebuildResponse(payload))
    case "GraphSync": return toBinary(d.GraphSyncResponseSchema, encodeGraphSyncResponse(payload))
    case "ListEntities": return toBinary(d.ListEntitiesResponseSchema, encodeListEntitiesResponse(payload))
    case "UpsertEntity": return toBinary(d.UpsertEntityResponseSchema, encodeUpsertEntityResponse(payload))
    case "GetEntity": return toBinary(d.GetEntityResponseSchema, encodeGetEntityResponse(payload))
    case "VectorStatus": return toBinary(d.VectorStatusResponseSchema, encodeVectorStatusResponse(payload))
    case "VectorConfigure": return toBinary(d.VectorConfigureResponseSchema, encodeVectorConfigureResponse(payload))
    case "VectorRebuild": return toBinary(d.VectorRebuildResponseSchema, encodeVectorRebuildResponse(payload))
    case "VectorSync": return toBinary(d.VectorSyncResponseSchema, encodeVectorSyncResponse(payload))
    case "VectorQueryChunks": return toBinary(d.VectorQueryChunksResponseSchema, encodeVectorQueryChunksResponse(payload))
    case "VectorQueryLabelAtoms": return toBinary(d.VectorQueryLabelAtomsResponseSchema, encodeVectorQueryLabelAtomsResponse(payload))
    case "GetStats": return toBinary(d.GetStatsResponseSchema, encodeGetStatsResponse(payload))
    case "Doctor": return toBinary(d.DoctorResponseSchema, encodeDoctorResponse(payload))
    case "Checkpoint": return toBinary(d.CheckpointResponseSchema, encodeCheckpointResponse(payload))
    case "MaintenanceBackup": return toBinary(d.MaintenanceBackupResponseSchema, encodeMaintenanceBackupResponse(payload))
    case "MaintenanceExport": return toBinary(d.MaintenanceExportResponseSchema, encodeMaintenanceExportResponse(payload))
    case "MaintenanceImport": return toBinary(d.MaintenanceImportResponseSchema, encodeMaintenanceImportResponse(payload))
    case "MaintenanceVacuum": return toBinary(d.MaintenanceVacuumResponseSchema, encodeMaintenanceVacuumResponse(payload))
    case "MaintenanceStatus": return toBinary(d.MaintenanceStatusResponseSchema, encodeMaintenanceStatusResponse(payload))
    case "MaintenanceRun": return toBinary(d.MaintenanceRunResponseSchema, encodeMaintenanceRunResponse(payload))
    case "MaintenanceRebuild": return toBinary(d.MaintenanceRebuildResponseSchema, encodeMaintenanceRebuildResponse(payload))
    case "MaintenanceCleanup": return toBinary(d.MaintenanceCleanupResponseSchema, encodeMaintenanceCleanupResponse(payload))
    case "MaintenanceImportV30": return toBinary(d.MaintenanceImportV30ResponseSchema, encodeMaintenanceImportV30Response(payload))
    case "GetTaskDetails": return toBinary(d.GetTaskDetailsResponseSchema, encodeGetTaskDetailsResponse(payload))
    case "GetLabelOntologyQuality": return toBinary(d.GetLabelOntologyQualityResponseSchema, encodeGetLabelOntologyQualityResponse(payload))
  }
}

/** 解码请求以供 transport fixture 核对业务 parts，不解析 HTTP URL。 */
export function decodeRpcRequest(method: RpcMethod, bytes: Uint8Array): Pick<RpcCall, "path" | "query" | "input"> {
  switch (method) {
    case "GetHealth": { const wire = fromBinary(s.GetHealthRequestSchema, bytes); void wire; return {} }
    case "ListBoards": { const wire = fromBinary(s.ListBoardsRequestSchema, bytes); return {query: c.omitUndefined({"include_archived": wire.includeArchived === undefined ? undefined : c.required(wire.includeArchived, "include_archived"),}),} }
    case "CreateBoard": { const wire = fromBinary(s.CreateBoardRequestSchema, bytes); return {input: c.omitUndefined({"slug": c.required(wire.slug, "slug"),"name": c.required(wire.name, "name"),"description": wire.description === undefined ? null : ((value) => value)(wire.description),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "GetBoard": { const wire = fromBinary(s.GetBoardRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "ArchiveBoard": { const wire = fromBinary(s.ArchiveBoardRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "ListBoardColumns": { const wire = fromBinary(s.ListBoardColumnsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "ListTasks": { const wire = fromBinary(s.ListTasksRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"status": wire.status.map((value) => decodeDtoApiTaskStatus(value)),"priority": wire.priority.map((value) => decodeDtoApiTaskPriority(value)),"label": wire.label.map((value) => decodeDtoTaskReadLabel(value)),"plan_filter": wire.planFilter.map((value) => decodeDtoTaskReadPlanFilter(value)),"assignee": wire.assignee === undefined ? null : ((value) => value)(wire.assignee),"q": wire.q === undefined ? null : ((value) => value)(wire.q),"include_archived": wire.includeArchived === undefined ? undefined : c.required(wire.includeArchived, "include_archived"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"offset": wire.offset === undefined ? undefined : c.safeNumber(c.required(wire.offset, "offset")),"sort": wire.sort === undefined ? undefined : decodeDtoTaskReadSort(c.required(wire.sort, "sort")),}),} }
    case "ListTasksByStatus": { const wire = fromBinary(s.ListTasksByStatusRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"status": wire.status.map((value) => decodeDtoApiTaskStatus(value)),"priority": wire.priority.map((value) => decodeDtoApiTaskPriority(value)),"label": wire.label.map((value) => decodeDtoTaskReadLabel(value)),"plan_filter": wire.planFilter.map((value) => decodeDtoTaskReadPlanFilter(value)),"assignee": wire.assignee === undefined ? null : ((value) => value)(wire.assignee),"q": wire.q === undefined ? null : ((value) => value)(wire.q),"include_archived": wire.includeArchived === undefined ? undefined : c.required(wire.includeArchived, "include_archived"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"offset": wire.offset === undefined ? undefined : c.safeNumber(c.required(wire.offset, "offset")),"sort": wire.sort === undefined ? undefined : decodeDtoTaskReadSort(c.required(wire.sort, "sort")),}),} }
    case "CreateTask": { const wire = fromBinary(s.CreateTaskRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),"idempotency_key": wire.idempotencyKey === undefined ? null : ((value) => value)(wire.idempotencyKey),"title": c.required(wire.title, "title"),"description": wire.description === undefined ? null : ((value) => value)(wire.description),"status": wire.status === undefined ? null : ((value) => decodeDtoApiCreateTaskStatus(value))(wire.status),"assignee": wire.assignee === undefined ? null : ((value) => value)(wire.assignee),"priority": wire.priority === undefined ? undefined : c.safeNumber(c.required(wire.priority, "priority")),"scheduled_at": wire.scheduledAt === undefined ? null : ((value) => c.safeNumber(value))(wire.scheduledAt),"due_at": wire.dueAt === undefined ? null : ((value) => c.safeNumber(value))(wire.dueAt),"max_retries": wire.maxRetries === undefined ? null : ((value) => c.safeNumber(value))(wire.maxRetries),"metadata": wire.metadata === undefined ? null : ((value) => decodeMapOfJsonValue(value))(wire.metadata),"labels": wire.labels.map((value) => value),"depends_on": wire.dependsOn.map((value) => value),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "GetTask": { const wire = fromBinary(s.GetTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"include": wire.include === undefined ? null : ((value) => value)(wire.include),}),} }
    case "UpdateTask": { const wire = fromBinary(s.UpdateTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"title": wire.title === undefined ? undefined : ((value) => value)(wire.title),"description": c.decodePatch(wire.description?.change, (change) => change.value),"assignee": c.decodePatch(wire.assignee?.change, (change) => change.value),"priority": wire.priority === undefined ? undefined : ((value) => c.safeNumber(value))(wire.priority),"scheduled_at": c.decodePatch(wire.scheduledAt?.change, (change) => c.safeNumber(change.value)),"due_at": c.decodePatch(wire.dueAt?.change, (change) => c.safeNumber(change.value)),"max_retries": c.decodePatch(wire.maxRetries?.change, (change) => c.safeNumber(change.value)),"metadata": c.decodePatch(wire.metadata?.change, (change) => c.decodeJson(change.value)),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),"expected_lock_version": wire.expectedLockVersion === undefined ? undefined : ((value) => c.safeNumber(value))(wire.expectedLockVersion),}),} }
    case "SpecifyTask": { const wire = fromBinary(s.SpecifyTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"description": wire.description === undefined ? null : ((value) => value)(wire.description),"scheduled_at": wire.scheduledAt === undefined ? null : ((value) => c.safeNumber(value))(wire.scheduledAt),}),} }
    case "PromoteTask": { const wire = fromBinary(s.PromoteTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "ClaimTask": { const wire = fromBinary(s.ClaimTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"ttl_ms": wire.ttlMs === undefined ? undefined : c.safeNumber(c.required(wire.ttlMs, "ttl_ms")),"worker_profile": wire.workerProfile === undefined ? null : ((value) => value)(wire.workerProfile),"metadata": wire.metadata === undefined ? null : ((value) => c.decodeJson(value))(wire.metadata),}),} }
    case "ReopenTask": { const wire = fromBinary(s.ReopenTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"reason": c.required(wire.reason, "reason"),}),} }
    case "ReclaimTask": { const wire = fromBinary(s.ReclaimTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"force": wire.force === undefined ? undefined : c.required(wire.force, "force"),"to_status": wire.toStatus === undefined ? null : ((value) => decodeDtoReclaimTargetStatus(value))(wire.toStatus),"reason": wire.reason === undefined ? null : ((value) => value)(wire.reason),}),} }
    case "HeartbeatTask": { const wire = fromBinary(s.HeartbeatTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"claim_token": c.required(wire.claimToken, "claim_token"),"ttl_ms": wire.ttlMs === undefined ? undefined : c.safeNumber(c.required(wire.ttlMs, "ttl_ms")),"note": wire.note === undefined ? null : ((value) => value)(wire.note),}),} }
    case "ReleaseTask": { const wire = fromBinary(s.ReleaseTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"claim_token": c.required(wire.claimToken, "claim_token"),}),} }
    case "CompleteTask": { const wire = fromBinary(s.CompleteTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"claim_token": wire.claimToken === undefined ? null : ((value) => value)(wire.claimToken),"force": wire.force === undefined ? undefined : c.required(wire.force, "force"),"summary": wire.summary === undefined ? null : ((value) => value)(wire.summary),"result": wire.result === undefined ? null : ((value) => c.decodeJson(value))(wire.result),}),} }
    case "SubmitReviewTask": { const wire = fromBinary(s.SubmitReviewTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"claim_token": wire.claimToken === undefined ? null : ((value) => value)(wire.claimToken),"force": wire.force === undefined ? undefined : c.required(wire.force, "force"),"summary": wire.summary === undefined ? null : ((value) => value)(wire.summary),}),} }
    case "BlockTask": { const wire = fromBinary(s.BlockTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"reason": c.required(wire.reason, "reason"),"claim_token": wire.claimToken === undefined ? null : ((value) => value)(wire.claimToken),"force": wire.force === undefined ? undefined : c.required(wire.force, "force"),}),} }
    case "UnblockTask": { const wire = fromBinary(s.UnblockTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "ArchiveTask": { const wire = fromBinary(s.ArchiveTaskRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"force": wire.force === undefined ? undefined : c.required(wire.force, "force"),}),} }
    case "ListSteps": { const wire = fromBinary(s.ListStepsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "CreateStep": { const wire = fromBinary(s.CreateStepRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"idempotency_key": wire.idempotencyKey === undefined ? null : ((value) => value)(wire.idempotencyKey),"title": c.required(wire.title, "title"),"body": wire.body === undefined ? null : ((value) => value)(wire.body),"linked_task_ref": wire.linkedTaskRef === undefined ? null : ((value) => value)(wire.linkedTaskRef),"position": wire.position === undefined ? null : ((value) => c.safeNumber(value))(wire.position),"required": wire.required === undefined ? undefined : c.required(wire.required, "required"),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "UpdateStep": { const wire = fromBinary(s.UpdateStepRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"step_id": c.required(wire.stepId, "step_id"),}),input: c.omitUndefined({"title": wire.title === undefined ? null : ((value) => value)(wire.title),"body": wire.body === undefined ? null : ((value) => value)(wire.body),"linked_task_ref": wire.linkedTaskRef === undefined ? null : ((value) => value)(wire.linkedTaskRef),"unlink_task": wire.unlinkTask === undefined ? undefined : c.required(wire.unlinkTask, "unlink_task"),"position": wire.position === undefined ? null : ((value) => c.safeNumber(value))(wire.position),"required": wire.required === undefined ? null : ((value) => value)(wire.required),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "RemoveStep": { const wire = fromBinary(s.RemoveStepRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"step_id": c.required(wire.stepId, "step_id"),}),} }
    case "CompleteStep": { const wire = fromBinary(s.CompleteStepRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"step_id": c.required(wire.stepId, "step_id"),}),input: c.omitUndefined({"note": c.required(wire.note, "note"),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "SkipStep": { const wire = fromBinary(s.SkipStepRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"step_id": c.required(wire.stepId, "step_id"),}),input: c.omitUndefined({"reason": c.required(wire.reason, "reason"),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "ReopenStep": { const wire = fromBinary(s.ReopenStepRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"step_id": c.required(wire.stepId, "step_id"),}),input: c.omitUndefined({"reason": c.required(wire.reason, "reason"),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "MarkExecutionPlanNotRequired": { const wire = fromBinary(s.MarkExecutionPlanNotRequiredRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"reason": c.required(wire.reason, "reason"),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "ListDependencies": { const wire = fromBinary(s.ListDependenciesRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "AddDependency": { const wire = fromBinary(s.AddDependencyRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"parent_task_id": c.required(wire.parentTaskId, "parent_task_id"),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),}),} }
    case "RemoveDependency": { const wire = fromBinary(s.RemoveDependencyRequestSchema, bytes); return {path: c.omitUndefined({"child_task_id": c.required(wire.childTaskId, "child_task_id"),"parent_task_id": c.required(wire.parentTaskId, "parent_task_id"),}),} }
    case "ListRuns": { const wire = fromBinary(s.ListRunsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "GetRun": { const wire = fromBinary(s.GetRunRequestSchema, bytes); return {path: c.omitUndefined({"run_id": c.required(wire.runId, "run_id"),}),} }
    case "GetRunLog": { const wire = fromBinary(s.GetRunLogRequestSchema, bytes); return {path: c.omitUndefined({"run_id": c.required(wire.runId, "run_id"),}),} }
    case "ListComments": { const wire = fromBinary(s.ListCommentsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "CreateComment": { const wire = fromBinary(s.CreateCommentRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"idempotency_key": wire.idempotencyKey === undefined ? null : ((value) => value)(wire.idempotencyKey),"author": wire.author === undefined ? null : ((value) => value)(wire.author),"body": c.required(wire.body, "body"),"kind": wire.kind === undefined ? null : ((value) => decodeDtoCommentsCommentKind(value))(wire.kind),"author_type": wire.authorType === undefined ? null : ((value) => decodeDtoCommentsCommentAuthorType(value))(wire.authorType),"agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),"metadata": wire.metadata === undefined ? null : ((value) => c.decodeJson(value))(wire.metadata),}),} }
    case "ListAttachments": { const wire = fromBinary(s.ListAttachmentsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "CreateAttachment": { const wire = fromBinary(s.CreateAttachmentRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"id": wire.id === undefined ? undefined : ((value) => value)(wire.id),"filename": c.required(wire.filename, "filename"),"content": wire.content,"content_type": wire.contentType === undefined ? undefined : ((value) => value)(wire.contentType),"rel_path": wire.relPath === undefined ? undefined : ((value) => value)(wire.relPath),"sha256": wire.sha256 === undefined ? undefined : ((value) => value)(wire.sha256),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),}),} }
    case "DownloadAttachment": { const wire = fromBinary(s.DownloadAttachmentRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"attachment_id": c.required(wire.attachmentId, "attachment_id"),}),} }
    case "DeleteAttachment": { const wire = fromBinary(s.DeleteAttachmentRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"attachment_id": c.required(wire.attachmentId, "attachment_id"),}),} }
    case "ListEvents": { const wire = fromBinary(s.ListEventsRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),"after": wire.after === undefined ? undefined : c.safeNumber(c.required(wire.after, "after")),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "ListTaskLabels": { const wire = fromBinary(s.ListTaskLabelsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "AddTaskLabel": { const wire = fromBinary(s.AddTaskLabelRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"name": wire.name === undefined ? undefined : ((value) => value)(wire.name),"names": wire.names === undefined ? undefined : ((value) => decodeListOfString(value))(wire.names),"create_missing": wire.createMissing === undefined ? undefined : c.required(wire.createMissing, "create_missing"),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),}),} }
    case "BootstrapTaskLabel": { const wire = fromBinary(s.BootstrapTaskLabelRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),input: c.omitUndefined({"name": c.required(wire.name, "name"),"description": wire.description === undefined ? undefined : ((value) => value)(wire.description),"applies_when": wire.appliesWhen.map((value) => value),"excludes_when": wire.excludesWhen.map((value) => value),"positive_examples": wire.positiveExamples.map((value) => value),"negative_examples": wire.negativeExamples.map((value) => value),"verify": wire.verify === undefined ? undefined : c.required(wire.verify, "verify"),"min_verify_score": wire.minVerifyScore === undefined ? undefined : c.float(c.required(wire.minVerifyScore, "min_verify_score")),"vector_config": wire.vectorConfig === undefined ? undefined : ((value) => decodeDtoVectorConfigureRequest(value))(wire.vectorConfig),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),}),} }
    case "RemoveTaskLabel": { const wire = fromBinary(s.RemoveTaskLabelRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),"label_id": c.required(wire.labelId, "label_id"),}),} }
    case "ListBoardLabels": { const wire = fromBinary(s.ListBoardLabelsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "ListBoardLabelProposals": { const wire = fromBinary(s.ListBoardLabelProposalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"status": wire.status === undefined ? undefined : ((value) => decodeDtoLabelProposalStatusWire(value))(wire.status),}),} }
    case "CreateBoardLabel": { const wire = fromBinary(s.CreateBoardLabelRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"name": c.required(wire.name, "name"),"color": wire.color === undefined ? undefined : ((value) => value)(wire.color),}),} }
    case "DeleteBoardLabel": { const wire = fromBinary(s.DeleteBoardLabelRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),"label_id": c.required(wire.labelId, "label_id"),}),query: c.omitUndefined({"force": wire.force === undefined ? undefined : c.required(wire.force, "force"),}),} }
    case "ListLabelSemantics": { const wire = fromBinary(s.ListLabelSemanticsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "GetLabelSemantics": { const wire = fromBinary(s.GetLabelSemanticsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),"label_id": c.required(wire.labelId, "label_id"),}),} }
    case "UpsertLabelSemantics": { const wire = fromBinary(s.UpsertLabelSemanticsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),"label_id": c.required(wire.labelId, "label_id"),}),input: c.omitUndefined({"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),"expected_semantics_hash": wire.expectedSemanticsHash === undefined ? undefined : ((value) => value)(wire.expectedSemanticsHash),"replace": wire.replace === undefined ? undefined : c.required(wire.replace, "replace"),"reason": wire.reason === undefined ? undefined : ((value) => value)(wire.reason),"source_signal_ids": wire.sourceSignalIds.map((value) => value),"description": wire.description === undefined ? undefined : ((value) => value)(wire.description),"applies_when": wire.appliesWhen === undefined ? undefined : ((value) => decodeListOfString(value))(wire.appliesWhen),"excludes_when": wire.excludesWhen === undefined ? undefined : ((value) => decodeListOfString(value))(wire.excludesWhen),"positive_examples": wire.positiveExamples === undefined ? undefined : ((value) => decodeListOfString(value))(wire.positiveExamples),"negative_examples": wire.negativeExamples === undefined ? undefined : ((value) => decodeListOfString(value))(wire.negativeExamples),"remove_applies_when": wire.removeAppliesWhen.map((value) => value),"remove_excludes_when": wire.removeExcludesWhen.map((value) => value),"remove_positive_examples": wire.removePositiveExamples.map((value) => value),"remove_negative_examples": wire.removeNegativeExamples.map((value) => value),}),} }
    case "DeleteLabelSemantics": { const wire = fromBinary(s.DeleteLabelSemanticsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),"label_id": c.required(wire.labelId, "label_id"),}),query: c.omitUndefined({"expected_semantics_hash": c.required(wire.expectedSemanticsHash, "expected_semantics_hash"),"reason": c.required(wire.reason, "reason"),}),} }
    case "ListLabelAtoms": { const wire = fromBinary(s.ListLabelAtomsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "ExplainLabelAtom": { const wire = fromBinary(s.ExplainLabelAtomRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),"atom_ref": c.required(wire.atomRef, "atom_ref"),}),} }
    case "LabelAtomIndexStatus": { const wire = fromBinary(s.LabelAtomIndexStatusRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "RebuildLabelAtomIndex": { const wire = fromBinary(s.RebuildLabelAtomIndexRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),} }
    case "QueryLabelAtomIndex": { const wire = fromBinary(s.QueryLabelAtomIndexRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"q": wire.q === undefined ? undefined : ((value) => value)(wire.q),"vector_json": wire.vectorJson === undefined ? undefined : ((value) => value)(wire.vectorJson),"embedding_model": wire.embeddingModel === undefined ? undefined : ((value) => value)(wire.embeddingModel),"include_vector": wire.includeVector === undefined ? undefined : c.required(wire.includeVector, "include_vector"),"polarity": wire.polarity === undefined ? undefined : ((value) => value)(wire.polarity),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "ListSignals": { const wire = fromBinary(s.ListSignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"status": wire.status.map((value) => value),"kind": wire.kind.map((value) => value),"task_ref": wire.taskRef === undefined ? undefined : ((value) => value)(wire.taskRef),"include_all": wire.includeAll === undefined ? undefined : c.required(wire.includeAll, "include_all"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "ReviewSignals": { const wire = fromBinary(s.ReviewSignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"status": wire.status.map((value) => value),"kind": wire.kind.map((value) => value),"task_ref": wire.taskRef === undefined ? undefined : ((value) => value)(wire.taskRef),"include_all": wire.includeAll === undefined ? undefined : c.required(wire.includeAll, "include_all"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "GetSignal": { const wire = fromBinary(s.GetSignalRequestSchema, bytes); return {path: c.omitUndefined({"signal_id": c.required(wire.signalId, "signal_id"),}),} }
    case "RecordSignal": { const wire = fromBinary(s.RecordSignalRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"kind": c.required(wire.kind, "kind"),"title": c.required(wire.title, "title"),"summary": c.required(wire.summary, "summary"),"severity": wire.severity === undefined ? null : ((value) => value)(wire.severity),"task_ref": wire.taskRef === undefined ? null : ((value) => value)(wire.taskRef),"task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),"run_id": wire.runId === undefined ? null : ((value) => value)(wire.runId),"comment_id": wire.commentId === undefined ? null : ((value) => value)(wire.commentId),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"agent_type": wire.agentType === undefined ? null : ((value) => value)(wire.agentType),"dedupe_key": wire.dedupeKey === undefined ? null : ((value) => value)(wire.dedupeKey),"source": wire.source === undefined ? null : ((value) => value)(wire.source),"evidence": wire.evidence === undefined ? null : ((value) => decodeDtoStructuredMetadataJsonObject(value))(wire.evidence),"comment": wire.comment === undefined ? null : ((value) => decodeDtoSignalCommentRequest(value))(wire.comment),}),} }
    case "ConfirmSignals": { const wire = fromBinary(s.ConfirmSignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"signal_ids": wire.signalIds.map((value) => value),"reason": c.required(wire.reason, "reason"),"replacement_signal_id": wire.replacementSignalId === undefined ? null : ((value) => value)(wire.replacementSignalId),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"expected_updated_at": wire.expectedUpdatedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.expectedUpdatedAt),}),} }
    case "RejectSignals": { const wire = fromBinary(s.RejectSignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"signal_ids": wire.signalIds.map((value) => value),"reason": c.required(wire.reason, "reason"),"replacement_signal_id": wire.replacementSignalId === undefined ? null : ((value) => value)(wire.replacementSignalId),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"expected_updated_at": wire.expectedUpdatedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.expectedUpdatedAt),}),} }
    case "ResolveSignals": { const wire = fromBinary(s.ResolveSignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"signal_ids": wire.signalIds.map((value) => value),"reason": c.required(wire.reason, "reason"),"replacement_signal_id": wire.replacementSignalId === undefined ? null : ((value) => value)(wire.replacementSignalId),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"expected_updated_at": wire.expectedUpdatedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.expectedUpdatedAt),}),} }
    case "SupersedeSignals": { const wire = fromBinary(s.SupersedeSignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"signal_ids": wire.signalIds.map((value) => value),"reason": c.required(wire.reason, "reason"),"replacement_signal_id": wire.replacementSignalId === undefined ? null : ((value) => value)(wire.replacementSignalId),"actor": wire.actor === undefined ? null : ((value) => value)(wire.actor),"expected_updated_at": wire.expectedUpdatedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.expectedUpdatedAt),}),} }
    case "SuggestTaskLabels": { const wire = fromBinary(s.SuggestTaskLabelsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"board": wire.board === undefined ? null : ((value) => value)(wire.board),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"candidate_limit": wire.candidateLimit === undefined ? undefined : c.safeNumber(c.required(wire.candidateLimit, "candidate_limit")),"atom_limit": wire.atomLimit === undefined ? undefined : c.safeNumber(c.required(wire.atomLimit, "atom_limit")),"max_selected_labels": wire.maxSelectedLabels === undefined ? undefined : c.safeNumber(c.required(wire.maxSelectedLabels, "max_selected_labels")),"min_score": wire.minScore === undefined ? undefined : c.float(c.required(wire.minScore, "min_score")),}),} }
    case "ListTaskLabelProposals": { const wire = fromBinary(s.ListTaskLabelProposalsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"board": wire.board === undefined ? null : ((value) => value)(wire.board),"status": wire.status === undefined ? null : ((value) => value)(wire.status),}),} }
    case "ProposeTaskLabel": { const wire = fromBinary(s.ProposeTaskLabelRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"board": wire.board === undefined ? null : ((value) => value)(wire.board),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"candidate_limit": wire.candidateLimit === undefined ? undefined : c.safeNumber(c.required(wire.candidateLimit, "candidate_limit")),"atom_limit": wire.atomLimit === undefined ? undefined : c.safeNumber(c.required(wire.atomLimit, "atom_limit")),"max_selected_labels": wire.maxSelectedLabels === undefined ? undefined : c.safeNumber(c.required(wire.maxSelectedLabels, "max_selected_labels")),"min_score": wire.minScore === undefined ? undefined : c.float(c.required(wire.minScore, "min_score")),}),input: c.omitUndefined({"proposal": wire.proposal === undefined ? undefined : ((value) => decodeDtoLabelProposalCandidateWire(value))(wire.proposal),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),"source_signal_ids": wire.sourceSignalIds.map((value) => value),"ontology_actor": wire.ontologyActor === undefined ? undefined : ((value) => decodeDtoLabelOntologyActorWire(value))(wire.ontologyActor),"allow_retarget": wire.allowRetarget === undefined ? undefined : c.required(wire.allowRetarget, "allow_retarget"),"retarget_reason": wire.retargetReason === undefined ? undefined : ((value) => value)(wire.retargetReason),}),} }
    case "RecordLabelOntologyObservation": { const wire = fromBinary(s.RecordLabelOntologyObservationRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"board": wire.board === undefined ? null : ((value) => value)(wire.board),}),input: c.omitUndefined({"actor": decodeDtoLabelOntologyActorWire(c.required(wire.actor, "actor")),"agent_candidates": wire.agentCandidates === undefined ? undefined : c.decodeJson(wire.agentCandidates),"suggestion_snapshot": wire.suggestionSnapshot === undefined ? undefined : c.decodeJson(wire.suggestionSnapshot),"final_decision": wire.finalDecision === undefined ? undefined : c.decodeJson(wire.finalDecision),"suggest_coverage": wire.suggestCoverage === undefined ? undefined : ((value) => c.float(value))(wire.suggestCoverage),"suggest_coverage_cosine": wire.suggestCoverageCosine === undefined ? undefined : ((value) => c.float(value))(wire.suggestCoverageCosine),"suggest_residual_norm": wire.suggestResidualNorm === undefined ? undefined : ((value) => c.float(value))(wire.suggestResidualNorm),"suggest_needs_new_label": wire.suggestNeedsNewLabel === undefined ? undefined : ((value) => value)(wire.suggestNeedsNewLabel),"suggest_degraded": wire.suggestDegraded === undefined ? undefined : ((value) => value)(wire.suggestDegraded),"diagnostics": wire.diagnostics === undefined ? undefined : c.decodeJson(wire.diagnostics),"capture_fingerprint": wire.captureFingerprint === undefined ? undefined : ((value) => value)(wire.captureFingerprint),"signals": wire.signals.map((value) => decodeDtoLabelOntologySignalRequest(value)),}),} }
    case "ListLabelOntologySignals": { const wire = fromBinary(s.ListLabelOntologySignalsRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"status": wire.status.map((value) => value),"kind": wire.kind.map((value) => value),"task_ref": wire.taskRef === undefined ? undefined : ((value) => value)(wire.taskRef),"target_label_ref": wire.targetLabelRef === undefined ? undefined : ((value) => value)(wire.targetLabelRef),"proposed_label_name": wire.proposedLabelName === undefined ? undefined : ((value) => value)(wire.proposedLabelName),"include_all": wire.includeAll === undefined ? undefined : c.required(wire.includeAll, "include_all"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "ReviewLabelOntology": { const wire = fromBinary(s.ReviewLabelOntologyRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"group_by": wire.groupBy === undefined ? undefined : decodeDtoLabelOntologyReviewGroupByWire(c.required(wire.groupBy, "group_by")),"include_all": wire.includeAll === undefined ? undefined : c.required(wire.includeAll, "include_all"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "CreateLabelOntologyAction": { const wire = fromBinary(s.CreateLabelOntologyActionRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"actor": decodeDtoLabelOntologyActorWire(c.required(wire.actor, "actor")),"idempotency_key": wire.idempotencyKey === undefined ? undefined : ((value) => value)(wire.idempotencyKey),"action_type": decodeDtoLabelOntologyActionTypeWire(c.required(wire.actionType, "action_type")),"signal_ids": wire.signalIds.map((value) => value),"reason": c.required(wire.reason, "reason"),"superseded_by_signal_id": wire.supersededBySignalId === undefined ? undefined : ((value) => value)(wire.supersededBySignalId),"parent_action_id": wire.parentActionId === undefined ? undefined : ((value) => value)(wire.parentActionId),"target_label_ref": wire.targetLabelRef === undefined ? undefined : ((value) => value)(wire.targetLabelRef),"result_label_ref": wire.resultLabelRef === undefined ? undefined : ((value) => value)(wire.resultLabelRef),"result_atom_id": wire.resultAtomId === undefined ? undefined : ((value) => value)(wire.resultAtomId),"result_atom_content_hash": wire.resultAtomContentHash === undefined ? undefined : ((value) => value)(wire.resultAtomContentHash),"result_proposal_id": wire.resultProposalId === undefined ? undefined : ((value) => value)(wire.resultProposalId),"canonical_before_hash": wire.canonicalBeforeHash === undefined ? undefined : ((value) => value)(wire.canonicalBeforeHash),"canonical_after_hash": wire.canonicalAfterHash === undefined ? undefined : ((value) => value)(wire.canonicalAfterHash),"change": wire.change === undefined ? undefined : c.decodeJson(wire.change),"validation_status": wire.validationStatus === undefined ? undefined : ((value) => decodeDtoLabelOntologyValidationStatusWire(value))(wire.validationStatus),"validation": wire.validation === undefined ? undefined : c.decodeJson(wire.validation),}),} }
    case "ApplyLabelOntologyAtom": { const wire = fromBinary(s.ApplyLabelOntologyAtomRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"actor": decodeDtoLabelOntologyActorWire(c.required(wire.actor, "actor")),"signal_ids": wire.signalIds.map((value) => value),"label_ref": c.required(wire.labelRef, "label_ref"),"kind": c.required(wire.kind, "kind"),"text": c.required(wire.text, "text"),"reason": c.required(wire.reason, "reason"),"allow_retarget": wire.allowRetarget === undefined ? undefined : c.required(wire.allowRetarget, "allow_retarget"),"retarget_reason": wire.retargetReason === undefined ? undefined : ((value) => value)(wire.retargetReason),}),} }
    case "RevertLabelOntologyMutation": { const wire = fromBinary(s.RevertLabelOntologyMutationRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"actor": decodeDtoLabelOntologyActorWire(c.required(wire.actor, "actor")),"target_action_id": c.required(wire.targetActionId, "target_action_id"),"expected_current_hash": wire.expectedCurrentHash === undefined ? undefined : ((value) => value)(wire.expectedCurrentHash),"reason": c.required(wire.reason, "reason"),}),} }
    case "ValidateLabelOntologyAction": { const wire = fromBinary(s.ValidateLabelOntologyActionRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),input: c.omitUndefined({"actor": decodeDtoLabelOntologyActorWire(c.required(wire.actor, "actor")),"parent_action_id": c.required(wire.parentActionId, "parent_action_id"),"signal_ids": wire.signalIds.map((value) => value),"reason": c.required(wire.reason, "reason"),"validation_status": decodeDtoLabelOntologyValidationStatusWire(c.required(wire.validationStatus, "validation_status")),"validation": wire.validation === undefined ? undefined : c.decodeJson(wire.validation),}),} }
    case "GetLabelOntologySignal": { const wire = fromBinary(s.GetLabelOntologySignalRequestSchema, bytes); return {path: c.omitUndefined({"signal_id": c.required(wire.signalId, "signal_id"),}),} }
    case "GetLabelProposal": { const wire = fromBinary(s.GetLabelProposalRequestSchema, bytes); return {path: c.omitUndefined({"proposal_id": c.required(wire.proposalId, "proposal_id"),}),} }
    case "AcceptLabelProposal": { const wire = fromBinary(s.AcceptLabelProposalRequestSchema, bytes); return {path: c.omitUndefined({"proposal_id": c.required(wire.proposalId, "proposal_id"),}),input: c.omitUndefined({"reason": wire.reason === undefined ? undefined : ((value) => value)(wire.reason),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),"source_signal_ids": wire.sourceSignalIds.map((value) => value),"ontology_actor": wire.ontologyActor === undefined ? undefined : ((value) => decodeDtoLabelOntologyActorWire(value))(wire.ontologyActor),"allow_retarget": wire.allowRetarget === undefined ? undefined : c.required(wire.allowRetarget, "allow_retarget"),"retarget_reason": wire.retargetReason === undefined ? undefined : ((value) => value)(wire.retargetReason),}),} }
    case "RejectLabelProposal": { const wire = fromBinary(s.RejectLabelProposalRequestSchema, bytes); return {path: c.omitUndefined({"proposal_id": c.required(wire.proposalId, "proposal_id"),}),input: c.omitUndefined({"reason": wire.reason === undefined ? undefined : ((value) => value)(wire.reason),"actor": wire.actor === undefined ? undefined : ((value) => value)(wire.actor),"source_signal_ids": wire.sourceSignalIds.map((value) => value),"ontology_actor": wire.ontologyActor === undefined ? undefined : ((value) => decodeDtoLabelOntologyActorWire(value))(wire.ontologyActor),"allow_retarget": wire.allowRetarget === undefined ? undefined : c.required(wire.allowRetarget, "allow_retarget"),"retarget_reason": wire.retargetReason === undefined ? undefined : ((value) => value)(wire.retargetReason),}),} }
    case "BoardTaskMap": { const wire = fromBinary(s.BoardTaskMapRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"active_only": wire.activeOnly === undefined ? undefined : c.required(wire.activeOnly, "active_only"),"context_depth": wire.contextDepth === undefined ? undefined : c.safeNumber(c.required(wire.contextDepth, "context_depth")),"limit_nodes": wire.limitNodes === undefined ? undefined : c.safeNumber(c.required(wire.limitNodes, "limit_nodes")),"include_done_context": wire.includeDoneContext === undefined ? undefined : c.required(wire.includeDoneContext, "include_done_context"),"include_archived_context": wire.includeArchivedContext === undefined ? undefined : c.required(wire.includeArchivedContext, "include_archived_context"),"hide_isolated": wire.hideIsolated === undefined ? undefined : c.required(wire.hideIsolated, "hide_isolated"),}),} }
    case "TaskNeighborhood": { const wire = fromBinary(s.TaskNeighborhoodRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"depth": wire.depth === undefined ? undefined : c.safeNumber(c.required(wire.depth, "depth")),"limit_nodes": wire.limitNodes === undefined ? undefined : c.safeNumber(c.required(wire.limitNodes, "limit_nodes")),"include_archived_context": wire.includeArchivedContext === undefined ? undefined : c.required(wire.includeArchivedContext, "include_archived_context"),}),} }
    case "SearchTasks": { const wire = fromBinary(s.SearchTasksRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"q": wire.q === undefined ? null : ((value) => value)(wire.q),"status": wire.status.map((value) => decodeDtoApiTaskStatus(value)),"label": wire.label.map((value) => value),"include_archived": wire.includeArchived === undefined ? undefined : c.required(wire.includeArchived, "include_archived"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"offset": wire.offset === undefined ? undefined : c.safeNumber(c.required(wire.offset, "offset")),"assignee": wire.assignee === undefined ? null : ((value) => value)(wire.assignee),}),} }
    case "SearchTasksByStatus": { const wire = fromBinary(s.SearchTasksByStatusRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"q": wire.q === undefined ? null : ((value) => value)(wire.q),"status": wire.status.map((value) => decodeDtoApiTaskStatus(value)),"label": wire.label.map((value) => value),"include_archived": wire.includeArchived === undefined ? undefined : c.required(wire.includeArchived, "include_archived"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"offset": wire.offset === undefined ? undefined : c.safeNumber(c.required(wire.offset, "offset")),"assignee": wire.assignee === undefined ? null : ((value) => value)(wire.assignee),}),} }
    case "SearchStatus": { const wire = fromBinary(s.SearchStatusRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "RebuildSearchIndex": { const wire = fromBinary(s.RebuildSearchIndexRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "SyncSearchIndex": { const wire = fromBinary(s.SyncSearchIndexRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "BuildContext": { const wire = fromBinary(s.BuildContextRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"lexical_limit": wire.lexicalLimit === undefined ? undefined : c.safeNumber(c.required(wire.lexicalLimit, "lexical_limit")),"graph_limit": wire.graphLimit === undefined ? undefined : c.safeNumber(c.required(wire.graphLimit, "graph_limit")),"vector_limit": wire.vectorLimit === undefined ? undefined : c.safeNumber(c.required(wire.vectorLimit, "vector_limit")),"max_items": wire.maxItems === undefined ? undefined : c.safeNumber(c.required(wire.maxItems, "max_items")),"task": wire.task === undefined ? undefined : ((value) => value)(wire.task),"reference": wire.reference === undefined ? undefined : ((value) => value)(wire.reference),"query": wire.query === undefined ? undefined : ((value) => value)(wire.query),"depth": wire.depth === undefined ? undefined : c.safeNumber(c.required(wire.depth, "depth")),"budget": wire.budget === undefined ? undefined : ((value) => c.safeNumber(value))(wire.budget),}),} }
    case "GraphStatus": { const wire = fromBinary(s.GraphStatusRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "GraphNeighbors": { const wire = fromBinary(s.GraphNeighborsRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"entity_uri": c.required(wire.entityUri, "entity_uri"),"predicate": wire.predicate === undefined ? null : ((value) => value)(wire.predicate),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "GraphQuery": { const wire = fromBinary(s.GraphQueryRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"query": wire.query === undefined ? undefined : c.required(wire.query, "query"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "GraphRebuild": { const wire = fromBinary(s.GraphRebuildRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "GraphSync": { const wire = fromBinary(s.GraphSyncRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "ListEntities": { const wire = fromBinary(s.ListEntitiesRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? null : ((value) => value)(wire.board),"kind": wire.kind === undefined ? null : ((value) => value)(wire.kind),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),}),} }
    case "UpsertEntity": { const wire = fromBinary(s.UpsertEntityRequestSchema, bytes); return {input: c.omitUndefined({"uri": c.required(wire.uri, "uri"),"kind": c.required(wire.kind, "kind"),"source_table": c.required(wire.sourceTable, "source_table"),"source_id": c.required(wire.sourceId, "source_id"),"board": wire.board === undefined ? null : ((value) => value)(wire.board),"task_id": wire.taskId === undefined ? null : ((value) => value)(wire.taskId),"title": wire.title === undefined ? null : ((value) => value)(wire.title),"summary": wire.summary === undefined ? null : ((value) => value)(wire.summary),"content_hash": wire.contentHash === undefined ? null : ((value) => value)(wire.contentHash),"archived_at": wire.archivedAt === undefined ? null : ((value) => c.safeNumber(value))(wire.archivedAt),}),} }
    case "GetEntity": { const wire = fromBinary(s.GetEntityRequestSchema, bytes); return {path: c.omitUndefined({"uri": c.required(wire.uri, "uri"),}),} }
    case "VectorStatus": { const wire = fromBinary(s.VectorStatusRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "VectorConfigure": { const wire = fromBinary(s.VectorConfigureRequestSchema, bytes); return {input: c.omitUndefined({"provider": c.required(wire.provider, "provider"),"endpoint": c.required(wire.endpoint, "endpoint"),"model": c.required(wire.model, "model"),"dimensions": c.safeNumber(c.required(wire.dimensions, "dimensions")),}),} }
    case "VectorRebuild": { const wire = fromBinary(s.VectorRebuildRequestSchema, bytes); return {input: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "VectorSync": { const wire = fromBinary(s.VectorSyncRequestSchema, bytes); return {input: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "VectorQueryChunks": { const wire = fromBinary(s.VectorQueryChunksRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"q": c.required(wire.q, "q"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"embedding_model": wire.embeddingModel === undefined ? undefined : ((value) => value)(wire.embeddingModel),"polarity": wire.polarity === undefined ? undefined : ((value) => value)(wire.polarity),"include_vector": wire.includeVector === undefined ? undefined : c.required(wire.includeVector, "include_vector"),}),} }
    case "VectorQueryLabelAtoms": { const wire = fromBinary(s.VectorQueryLabelAtomsRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),"q": c.required(wire.q, "q"),"limit": wire.limit === undefined ? undefined : c.safeNumber(c.required(wire.limit, "limit")),"embedding_model": wire.embeddingModel === undefined ? undefined : ((value) => value)(wire.embeddingModel),"polarity": wire.polarity === undefined ? undefined : ((value) => value)(wire.polarity),"include_vector": wire.includeVector === undefined ? undefined : c.required(wire.includeVector, "include_vector"),}),} }
    case "GetStats": { const wire = fromBinary(s.GetStatsRequestSchema, bytes); return {query: c.omitUndefined({"board": wire.board === undefined ? undefined : c.required(wire.board, "board"),}),} }
    case "Doctor": { const wire = fromBinary(s.DoctorRequestSchema, bytes); void wire; return {} }
    case "Checkpoint": { const wire = fromBinary(s.CheckpointRequestSchema, bytes); void wire; return {} }
    case "MaintenanceBackup": { const wire = fromBinary(s.MaintenanceBackupRequestSchema, bytes); return {input: c.omitUndefined({"path": c.required(wire.path, "path"),}),} }
    case "MaintenanceExport": { const wire = fromBinary(s.MaintenanceExportRequestSchema, bytes); return {input: c.omitUndefined({"path": c.required(wire.path, "path"),}),} }
    case "MaintenanceImport": { const wire = fromBinary(s.MaintenanceImportRequestSchema, bytes); return {input: c.omitUndefined({"path": c.required(wire.path, "path"),"replace": wire.replace === undefined ? undefined : c.required(wire.replace, "replace"),}),} }
    case "MaintenanceVacuum": { const wire = fromBinary(s.MaintenanceVacuumRequestSchema, bytes); void wire; return {} }
    case "MaintenanceStatus": { const wire = fromBinary(s.MaintenanceStatusRequestSchema, bytes); void wire; return {} }
    case "MaintenanceRun": { const wire = fromBinary(s.MaintenanceRunRequestSchema, bytes); return {input: c.omitUndefined({"owner": wire.owner === undefined ? null : ((value) => value)(wire.owner),"action": wire.action === undefined ? null : ((value) => value)(wire.action),}),} }
    case "MaintenanceRebuild": { const wire = fromBinary(s.MaintenanceRebuildRequestSchema, bytes); return {input: c.omitUndefined({"owner": wire.owner === undefined ? null : ((value) => value)(wire.owner),"action": wire.action === undefined ? null : ((value) => value)(wire.action),}),} }
    case "MaintenanceCleanup": { const wire = fromBinary(s.MaintenanceCleanupRequestSchema, bytes); return {input: c.omitUndefined({"owner": wire.owner === undefined ? null : ((value) => value)(wire.owner),"action": wire.action === undefined ? null : ((value) => value)(wire.action),}),} }
    case "MaintenanceImportV30": { const wire = fromBinary(s.MaintenanceImportV30RequestSchema, bytes); return {input: c.omitUndefined({"path": c.required(wire.path, "path"),"canonical_attachment_root": wire.canonicalAttachmentRoot === undefined ? null : ((value) => value)(wire.canonicalAttachmentRoot),}),} }
    case "GetTaskDetails": { const wire = fromBinary(s.GetTaskDetailsRequestSchema, bytes); return {path: c.omitUndefined({"task_id": c.required(wire.taskId, "task_id"),}),} }
    case "GetLabelOntologyQuality": { const wire = fromBinary(s.GetLabelOntologyQualityRequestSchema, bytes); return {path: c.omitUndefined({"board": c.required(wire.board, "board"),}),query: c.omitUndefined({"sample_limit": wire.sampleLimit === undefined ? undefined : c.safeNumber(c.required(wire.sampleLimit, "sample_limit")),}),} }
  }
}
