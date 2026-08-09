import type {
  AddTaskLabelIntent,
  CreateAttachmentIntent,
  CreateCommentIntent,
  CreateStepIntent,
  MarkExecutionPlanNotRequiredIntent,
  MutationRequestOptions,
  TaskMutationClient,
  TaskTransitionIntent,
  UpdateTaskIntent,
} from "../../lib/api/task-mutations"
import type { AttachmentDownloadClient, DownloadedAttachment } from "../../lib/api/attachment-download"
import type { ApiLabelSuggestionQueryContract } from "../../lib/api/generated/contracts/api-label-suggestion-query"
import type { ApiSuggestTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-suggest-task-labels-response"

/** Committed mutation kinds consumed by Explorer invalidation. */
export const INSPECTOR_MUTATION_KINDS = [
  "edit",
  "transition",
  "dependency",
  "step",
  "comment",
  "label",
  "attachment",
] as const

export type InspectorMutationKind = (typeof INSPECTOR_MUTATION_KINDS)[number]

/** Logical operation names used for pending/error/retry scopes. */
export type InspectorMutationOperation =
  | "saveTask"
  | "transition"
  | "addDependency"
  | "removeDependency"
  | "createStep"
  | "linkStep"
  | "markPlanNotRequired"
  | "addLabel"
  | "removeLabel"
  | "applySuggestedLabel"
  | "addComment"
  | "uploadAttachment"
  | "downloadAttachment"
  | "deleteAttachment"
  | "suggestLabels"

export interface TaskInspectorMutationScope {
  /** Runtime/session + canonical board/task identity. */
  readonly identity: string
  readonly boardId: string
  readonly taskId: string
}

export interface TaskClaimTokenStore {
  get(taskId: string): string | null | undefined
  set(taskId: string, token: string): void
  delete(taskId: string): void
}

/** Small Map-backed store that can be injected into both Board and Inspector controllers. */
export function createTaskClaimTokenStore(initial?: Readonly<Record<string, string>>): TaskClaimTokenStore {
  const tokens = new Map<string, string>(Object.entries(initial ?? {}))
  return {
    get: (taskId) => tokens.get(taskId) ?? null,
    set: (taskId, token) => {
      tokens.set(taskId, token)
    },
    delete: (taskId) => {
      tokens.delete(taskId)
    },
  }
}

/** The generated TaskMutationClient subset needed by the rendered Inspector writes. */
export type InspectorTaskMutationClient = Pick<
  TaskMutationClient,
  | "updateTask"
  | "transitionTask"
  | "addDependency"
  | "removeDependency"
  | "createStep"
  | "markExecutionPlanNotRequired"
  | "addTaskLabel"
  | "removeTaskLabel"
  | "createComment"
  | "createAttachment"
  | "deleteAttachment"
>

/** Optional read seam reserved for the manual label-suggestion action. */
export interface InspectorTaskLabelSuggestionClient {
  suggestTaskLabels(
    taskId: string,
    query?: InspectorSuggestTaskLabelsQuery,
    options?: MutationRequestOptions,
  ): Promise<ApiSuggestTaskLabelsResponseContract>
}

/** Alias the generated query contract; transport remains the validator owner. */
export type InspectorSuggestTaskLabelsQuery = Partial<ApiLabelSuggestionQueryContract>

export interface TaskInspectorMutationCommitted {
  readonly kind: InspectorMutationKind
  readonly taskId: string
}

export function createCommittedMutationEvent(kind: InspectorMutationKind, taskId: string): TaskInspectorMutationCommitted {
  return { kind, taskId }
}

export interface TaskInspectorMutationSurface {
  readonly scope: TaskInspectorMutationScope
  readonly client: InspectorTaskMutationClient
  /** Shared with the Board task mutation controller; this is not a second mutation path. */
  readonly claimTokens?: TaskClaimTokenStore
  /** Called after a server commit/conflict settle, and awaited before reconciliation is reported. */
  readonly onCanonicalReload: (event: TaskInspectorMutationCommitted, scope: TaskInspectorMutationScope) => Promise<void> | void
  /** Observer only; throwing here must never turn a committed write into a retry. */
  readonly onMutationCommitted?: (event: TaskInspectorMutationCommitted) => void
  /** Optional read-only suggestion endpoint; no generated transport is duplicated here. */
  readonly suggestTaskLabels?: InspectorTaskLabelSuggestionClient["suggestTaskLabels"]
  /** Optional bytes client composed beside TaskMutationClient for downloads. */
  readonly attachmentDownload?: AttachmentDownloadClient
}

export type InspectorSaveTaskInput = UpdateTaskIntent
export type InspectorTransitionCommand = TaskTransitionIntent
export type InspectorCreateStepInput = CreateStepIntent
export type InspectorLinkStepInput = CreateStepIntent
export type InspectorPlanNotRequiredInput = MarkExecutionPlanNotRequiredIntent
export type InspectorAddLabelInput = AddTaskLabelIntent
export type InspectorApplySuggestedLabelInput = AddTaskLabelIntent
export type InspectorCommentInput = CreateCommentIntent
export type InspectorUploadAttachmentInput = CreateAttachmentIntent

export interface InspectorRemoveLabelInput {
  readonly labelId: string
}

export interface InspectorDownloadAttachmentInput {
  readonly attachmentId: string
}

export interface InspectorDeleteAttachmentInput {
  readonly attachmentId: string
}

/** Result of a server write and its scoped canonical reconciliation. */
export interface InspectorMutationOutcome {
  /** The canonical mutation endpoint accepted and committed the write. */
  readonly committed: boolean
  /** Canonical state was refreshed after settle while this identity remained current. */
  readonly reconciled: boolean
}

export interface TaskInspectorMutationHandlers {
  saveTask(input: InspectorSaveTaskInput): Promise<InspectorMutationOutcome>
  transition(command: InspectorTransitionCommand): Promise<InspectorMutationOutcome>
  addDependency(parentTaskId: string): Promise<InspectorMutationOutcome>
  removeDependency(parentTaskId: string): Promise<InspectorMutationOutcome>
  createStep(input: InspectorCreateStepInput): Promise<InspectorMutationOutcome>
  linkStep(input: InspectorLinkStepInput): Promise<InspectorMutationOutcome>
  markPlanNotRequired(input: InspectorPlanNotRequiredInput): Promise<InspectorMutationOutcome>
  addLabel(input: InspectorAddLabelInput): Promise<InspectorMutationOutcome>
  removeLabel(input: InspectorRemoveLabelInput | string): Promise<InspectorMutationOutcome>
  applySuggestedLabel(input: InspectorApplySuggestedLabelInput): Promise<InspectorMutationOutcome>
  addComment(input: InspectorCommentInput): Promise<InspectorMutationOutcome>
  uploadAttachment(input: InspectorUploadAttachmentInput): Promise<InspectorMutationOutcome>
  downloadAttachment(input: InspectorDownloadAttachmentInput | string): Promise<DownloadedAttachment | null>
  deleteAttachment(input: InspectorDeleteAttachmentInput | string): Promise<InspectorMutationOutcome>
  suggestLabels(query?: InspectorSuggestTaskLabelsQuery): Promise<ApiSuggestTaskLabelsResponseContract | null>
  retry(key?: string): Promise<InspectorMutationOutcome>
}

export interface TaskInspectorMutationError {
  readonly operation: InspectorMutationOperation | "reload"
  readonly taskId: string
  readonly kind: "error" | "conflict" | "stale"
  readonly message: string
  readonly status: number | null
  readonly code: string | null
  readonly recoverable: true
}

export type TaskInspectorMutationRetryIntent =
  | { readonly operation: "reload"; readonly taskId: string; readonly event?: TaskInspectorMutationCommitted }
  | { readonly operation: "saveTask"; readonly taskId: string; readonly input: InspectorSaveTaskInput }
  | { readonly operation: "transition"; readonly taskId: string; readonly command: InspectorTransitionCommand }
  | { readonly operation: "addDependency"; readonly taskId: string; readonly parentTaskId: string }
  | { readonly operation: "removeDependency"; readonly taskId: string; readonly parentTaskId: string }
  | { readonly operation: "createStep"; readonly taskId: string; readonly input: InspectorCreateStepInput }
  | { readonly operation: "linkStep"; readonly taskId: string; readonly input: InspectorLinkStepInput }
  | { readonly operation: "markPlanNotRequired"; readonly taskId: string; readonly input: InspectorPlanNotRequiredInput }
  | { readonly operation: "addLabel"; readonly taskId: string; readonly input: InspectorAddLabelInput }
  | { readonly operation: "removeLabel"; readonly taskId: string; readonly labelId: string }
  | { readonly operation: "applySuggestedLabel"; readonly taskId: string; readonly input: InspectorApplySuggestedLabelInput }
  | { readonly operation: "addComment"; readonly taskId: string; readonly input: InspectorCommentInput }
  | { readonly operation: "uploadAttachment"; readonly taskId: string; readonly input: InspectorUploadAttachmentInput }
  | { readonly operation: "downloadAttachment"; readonly taskId: string; readonly attachmentId: string }
  | { readonly operation: "deleteAttachment"; readonly taskId: string; readonly attachmentId: string }
  | { readonly operation: "suggestLabels"; readonly taskId: string; readonly query?: InspectorSuggestTaskLabelsQuery }

export interface TaskInspectorMutationSnapshot {
  readonly scope: TaskInspectorMutationScope
  readonly generation: number
  readonly pending: ReadonlySet<string>
  readonly errors: ReadonlyMap<string, TaskInspectorMutationError>
  readonly retries: ReadonlyMap<string, TaskInspectorMutationRetryIntent>
}

export function inspectorMutationScopeKey(scope: TaskInspectorMutationScope): string {
  return scope.identity
}

export function inspectorMutationKey(
  operation: InspectorMutationOperation | InspectorMutationKind | "reload",
  taskId: string,
): string {
  return `${operation}:${taskId}`
}
