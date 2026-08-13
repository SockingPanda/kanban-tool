import { useEffect, useLayoutEffect, useRef, useSyncExternalStore } from "react"

import type { DownloadedAttachment } from "../../lib/api/attachment-download"
import type { ApiSuggestTaskLabelsResponseContract } from "../../lib/api/generated/contracts/api-suggest-task-labels-response"
import type {
  BlockTaskIntent,
  CompleteTaskIntent,
  HeartbeatTaskIntent,
  ReleaseTaskIntent,
  SubmitReviewTaskIntent,
  TaskTransitionResponse,
} from "../../lib/api/task-mutations"
import {
  createCommittedMutationEvent,
  createTaskClaimTokenStore,
  inspectorMutationKey,
  type InspectorAddLabelInput,
  type InspectorApplySuggestedLabelInput,
  type InspectorCommentInput,
  type InspectorCreateStepInput,
  type InspectorDeleteAttachmentInput,
  type InspectorDownloadAttachmentInput,
  type InspectorLinkStepInput,
  type InspectorMutationKind,
  type InspectorMutationOperation,
  type InspectorMutationOutcome,
  type InspectorPlanNotRequiredInput,
  type InspectorRemoveLabelInput,
  type InspectorSaveTaskInput,
  type InspectorSuggestTaskLabelsQuery,
  type InspectorTaskMutationClient,
  type InspectorTransitionCommand,
  type InspectorUploadAttachmentInput,
  type TaskClaimTokenStore,
  type TaskInspectorMutationError,
  type TaskInspectorMutationHandlers,
  type TaskInspectorMutationRetryIntent,
  type TaskInspectorMutationScope,
  type TaskInspectorMutationSnapshot,
  type TaskInspectorMutationSurface,
} from "./task-inspector-mutation-state"

type Listener = () => void

interface ActiveOperation {
  readonly generation: number
  readonly scope: TaskInspectorMutationScope
  readonly operation: InspectorMutationOperation | "reload"
  readonly abortController: AbortController
}

interface OperationResult<T> {
  readonly ok: boolean
  readonly value: T | null
}

interface WriteOperationResult<T> {
  readonly outcome: InspectorMutationOutcome
  readonly value: T | null
}

interface ErrorDescriptor {
  readonly kind: TaskInspectorMutationError["kind"]
  readonly status: number | null
  readonly code: string | null
  readonly message: string
}

const emptyScope: TaskInspectorMutationScope = Object.freeze({ identity: "", boardId: "", taskId: "" })
const notCommittedOutcome: InspectorMutationOutcome = Object.freeze({ committed: false, reconciled: false })
const notCommittedReconciledOutcome: InspectorMutationOutcome = Object.freeze({ committed: false, reconciled: true })
const committedNotReconciledOutcome: InspectorMutationOutcome = Object.freeze({ committed: true, reconciled: false })
const committedReconciledOutcome: InspectorMutationOutcome = Object.freeze({ committed: true, reconciled: true })

const transitionActions = new Set<InspectorTransitionCommand["action"]>([
  "specify",
  "promote",
  "claim",
  "release",
  "heartbeat",
  "complete",
  "submit-review",
  "block",
  "unblock",
  "archive",
])

const tokenizedActions = new Set<InspectorTransitionCommand["action"]>([
  "heartbeat",
  "release",
  "submit-review",
  "complete",
  "block",
])

const claimRequiredActions = new Set<InspectorTransitionCommand["action"]>([
  "heartbeat",
  "release",
  "submit-review",
])

const terminalActions = new Set<InspectorTransitionCommand["action"]>([
  "release",
  "submit-review",
  "complete",
  "block",
  "unblock",
  "archive",
])

function sameScope(left: TaskInspectorMutationScope | null, right: TaskInspectorMutationScope | null): boolean {
  if (left === null || right === null) return left === right
  return left.identity === right.identity && left.boardId === right.boardId && left.taskId === right.taskId
}

function recordValue(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null ? value as Record<string, unknown> : null
}

function errorDescriptor(error: unknown): ErrorDescriptor {
  const candidate = recordValue(error)
  const apiError = recordValue(candidate?.apiError)
  const status = typeof candidate?.status === "number" ? candidate.status : null
  const code = typeof apiError?.code === "string" ? apiError.code : null
  const conflict = status === 409 || code === "conflict" || code === "claim_conflict" || code === "claim_token_mismatch" || code === "idempotency_conflict"
  const message = conflict
    ? "conflict"
    : status === 401 || status === 403
      ? "unauthorized"
      : status === 404 || code === "not_found"
        ? "not_found"
        : typeof status === "number" && status >= 500
          ? "unavailable"
          : "mutation_failed"
  return {
    kind: conflict ? "conflict" : "error",
    status,
    code,
    message,
  }
}

function isMutationConflict(error: unknown): boolean {
  return errorDescriptor(error).kind === "conflict"
}

function isClaimTokenConflict(error: unknown): boolean {
  const candidate = recordValue(error)
  const apiError = recordValue(candidate?.apiError)
  return apiError?.code === "claim_token_mismatch" || (candidate?.status === 403 && apiError?.code === "claim_conflict")
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function tokenFromTransitionResponse(value: unknown): string | null {
  const data = recordValue(recordValue(value)?.data)
  return typeof data?.claim_token === "string" && data.claim_token.length > 0 ? data.claim_token : null
}

function normalizedText(value: string): string {
  return value.trim()
}

function clientUuid(prefix: string): string {
  const uuid = typeof globalThis.crypto?.randomUUID === "function"
    ? globalThis.crypto.randomUUID()
    : `${Date.now()}-${Math.random()}`
  return `${prefix}${uuid}`
}

function omitUndefined<T extends object>(input: T): T {
  const output: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(input)) {
    if (value !== undefined) output[key] = value
  }
  return output as T
}

function normalizedSaveInput(input: InspectorSaveTaskInput): InspectorSaveTaskInput {
  const title = typeof input.title === "string" ? normalizedText(input.title) : input.title
  const description = input.description === undefined
    ? undefined
    : input.description === null
      ? null
      : normalizedText(input.description) || null
  return omitUndefined({
    ...omitUndefined(input),
    ...(title === undefined ? {} : { title }),
    ...(description === undefined ? {} : { description }),
  })
}

function normalizedStepInput(input: InspectorCreateStepInput): InspectorCreateStepInput {
  return omitUndefined({
    ...omitUndefined(input),
    title: normalizedText(input.title),
    ...(input.body === undefined || input.body === null ? {} : { body: normalizedText(input.body) || null }),
    ...(input.linked_task_ref === undefined || input.linked_task_ref === null ? {} : { linked_task_ref: normalizedText(input.linked_task_ref) || null }),
  })
}

function normalizedCommentInput(input: InspectorCommentInput): InspectorCommentInput {
  return omitUndefined({ ...omitUndefined(input), body: normalizedText(input.body) })
}

function normalizedAttachmentInput(input: InspectorUploadAttachmentInput): InspectorUploadAttachmentInput {
  return omitUndefined({ ...omitUndefined(input), filename: normalizedText(input.filename) })
}

function normalizedLabelInput(input: InspectorAddLabelInput): InspectorAddLabelInput {
  return omitUndefined({
    ...omitUndefined(input),
    ...(input.name === undefined || input.name === null ? {} : { name: normalizedText(input.name) || null }),
    ...(input.names === undefined || input.names === null ? {} : { names: input.names.map(normalizedText).filter((name) => name.length > 0) }),
  })
}

function stableRequestKey(value: string | null | undefined, prefix: string): string {
  const candidate = typeof value === "string" ? value.trim() : ""
  return candidate || clientUuid(prefix)
}

function transitionCommandWithToken(
  command: InspectorTransitionCommand,
  token: string | null,
): InspectorTransitionCommand {
  if (token === null || !tokenizedActions.has(command.action)) return command
  switch (command.action) {
    case "heartbeat": {
      const input: HeartbeatTaskIntent = { ...command.input, claim_token: command.input.claim_token || token }
      return { action: command.action, input }
    }
    case "submit-review": {
      const input: SubmitReviewTaskIntent = { ...command.input, claim_token: command.input.claim_token || token }
      return { action: command.action, input }
    }
    case "release": {
      const input: ReleaseTaskIntent = { ...command.input, claim_token: token ?? "" }
      return { action: command.action, input }
    }
    case "complete": {
      const input: CompleteTaskIntent = { ...command.input, claim_token: command.input.claim_token || token }
      return { action: command.action, input }
    }
    case "block": {
      const input: BlockTaskIntent = { ...command.input, claim_token: command.input.claim_token || token }
      return { action: command.action, input }
    }
    default:
      return command
  }
}

function transitionInputHasToken(command: InspectorTransitionCommand): boolean {
  if (!tokenizedActions.has(command.action)) return true
  const input = recordValue(command.input)
  return typeof input?.claim_token === "string" && input.claim_token.length > 0
}

async function executeTransition(
  client: InspectorTaskMutationClient,
  taskId: string,
  command: InspectorTransitionCommand,
  signal: AbortSignal,
): Promise<TaskTransitionResponse> {
  switch (command.action) {
    case "specify": return client.transitionTask(taskId, "specify", command.input, { signal })
    case "promote": return client.transitionTask(taskId, "promote", command.input, { signal })
    case "claim": return client.transitionTask(taskId, "claim", command.input, { signal })
    case "release": return client.transitionTask(taskId, "release", command.input, { signal })
    case "heartbeat": return client.transitionTask(taskId, "heartbeat", command.input, { signal })
    case "complete": return client.transitionTask(taskId, "complete", command.input, { signal })
    case "submit-review": return client.transitionTask(taskId, "submit-review", command.input, { signal })
    case "block": return client.transitionTask(taskId, "block", command.input, { signal })
    case "unblock": return client.transitionTask(taskId, "unblock", command.input, { signal })
    case "archive": return client.transitionTask(taskId, "archive", command.input, { signal })
  }
}

function operationKind(operation: InspectorMutationOperation): InspectorMutationKind {
  switch (operation) {
    case "saveTask": return "edit"
    case "transition": return "transition"
    case "addDependency":
    case "removeDependency": return "dependency"
    case "createStep":
    case "linkStep":
    case "markPlanNotRequired": return "step"
    case "addLabel":
    case "removeLabel":
    case "applySuggestedLabel": return "label"
    case "addComment": return "comment"
    case "uploadAttachment":
    case "downloadAttachment":
    case "deleteAttachment": return "attachment"
    case "suggestLabels": throw new Error("suggestLabels is a read operation")
  }
}

function isWriteOperation(operation: InspectorMutationOperation | "reload"): boolean {
  return operation !== "downloadAttachment" && operation !== "suggestLabels" && operation !== "reload"
}

/** UI-agnostic mutation state machine used by Task Inspector and Board adapters. */
export class TaskInspectorMutationController implements TaskInspectorMutationHandlers {
  private surface: TaskInspectorMutationSurface | null
  private readonly fallbackClaimTokens = createTaskClaimTokenStore()
  private readonly listeners = new Set<Listener>()
  private readonly active = new Map<string, ActiveOperation>()
  private readonly writeTasks = new Set<string>()
  private pending = new Set<string>()
  private errors = new Map<string, TaskInspectorMutationError>()
  private retries = new Map<string, TaskInspectorMutationRetryIntent>()
  private generation = 0
  private disposed = false
  private currentSnapshot: TaskInspectorMutationSnapshot

  constructor(surface: TaskInspectorMutationSurface | null = null) {
    this.surface = surface
    this.currentSnapshot = this.makeSnapshot()
  }

  get snapshot(): TaskInspectorMutationSnapshot {
    return this.currentSnapshot
  }

  getSnapshot = (): TaskInspectorMutationSnapshot => this.currentSnapshot

  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  setSurface(surface: TaskInspectorMutationSurface | null): void {
    if (this.disposed) return
    const previous = this.surface
    const changed = !sameScope(previous?.scope ?? null, surface?.scope ?? null)
    this.surface = surface
    if (!changed) return
    const rebindsSameTask = previous !== null
      && previous !== undefined
      && surface !== null
      && previous.scope.taskId === surface.scope.taskId
      && previous.scope.identity !== surface.scope.identity
    if (rebindsSameTask) {
      previous.claimTokens?.delete(previous.scope.taskId)
      this.fallbackClaimTokens.delete(previous.scope.taskId)
    }
    this.generation += 1
    for (const operation of this.active.values()) operation.abortController.abort()
    this.active.clear()
    this.writeTasks.clear()
    this.pending = new Set()
    this.errors = new Map()
    this.retries = new Map()
    this.emit()
  }

  dispose(): void {
    if (this.disposed) return
    this.disposed = true
    this.generation += 1
    for (const operation of this.active.values()) operation.abortController.abort()
    this.active.clear()
    this.writeTasks.clear()
    this.surface = null
    this.pending = new Set()
    this.errors = new Map()
    this.retries = new Map()
    this.emit()
    this.listeners.clear()
  }

  isPending(operation: InspectorMutationOperation | "reload", taskId = this.surface?.scope.taskId ?? ""): boolean {
    return this.pending.has(inspectorMutationKey(operation, taskId))
  }

  errorFor(operation: InspectorMutationOperation | "reload", taskId = this.surface?.scope.taskId ?? ""): TaskInspectorMutationError | null {
    return this.errors.get(inspectorMutationKey(operation, taskId)) ?? null
  }

  retryIntentFor(operation: InspectorMutationOperation | "reload", taskId = this.surface?.scope.taskId ?? ""): TaskInspectorMutationRetryIntent | null {
    return this.retries.get(inspectorMutationKey(operation, taskId)) ?? null
  }

  private makeSnapshot(): TaskInspectorMutationSnapshot {
    return Object.freeze({
      scope: this.surface?.scope ?? emptyScope,
      generation: this.generation,
      pending: new Set(this.pending),
      errors: new Map(this.errors),
      retries: new Map(this.retries),
    })
  }

  private emit(): void {
    this.currentSnapshot = this.makeSnapshot()
    for (const listener of this.listeners) {
      try {
        listener()
      } catch {
        // A UI subscriber cannot prevent pending/error state from settling.
      }
    }
  }

  private currentFor(generation: number, scope: TaskInspectorMutationScope): boolean {
    return !this.disposed && generation === this.generation && sameScope(this.surface?.scope ?? null, scope)
  }

  private claimTokens(surface: TaskInspectorMutationSurface): TaskClaimTokenStore {
    return surface.claimTokens ?? this.fallbackClaimTokens
  }

  private begin(
    operation: InspectorMutationOperation | "reload",
    taskId: string,
    intent?: TaskInspectorMutationRetryIntent,
  ): ActiveOperation | null {
    const surface = this.surface
    if (surface === null || this.disposed || taskId !== surface.scope.taskId) return null
    const key = inspectorMutationKey(operation, taskId)
    const reloadKey = inspectorMutationKey("reload", taskId)
    if (
      this.pending.has(key)
      || (operation === "reload" && this.writeTasks.has(taskId))
      || (isWriteOperation(operation) && this.pending.has(reloadKey))
      || (isWriteOperation(operation) && this.writeTasks.has(taskId))
    ) return null
    const active: ActiveOperation = {
      generation: this.generation,
      scope: surface.scope,
      operation,
      abortController: new AbortController(),
    }
    this.active.set(key, active)
    if (isWriteOperation(operation)) this.writeTasks.add(taskId)
    this.pending = new Set(this.pending).add(key)
    this.errors = new Map(this.errors)
    this.errors.delete(key)
    this.retries = new Map(this.retries)
    this.retries.delete(key)
    this.emit()
    // Keep intent in the closure but do not expose it while the write is in flight.
    void intent
    return active
  }

  private finish(key: string, active: ActiveOperation): void {
    if (this.active.get(key) !== active) return
    this.active.delete(key)
    if (isWriteOperation(active.operation)) this.writeTasks.delete(active.scope.taskId)
    this.pending = new Set(this.pending)
    this.pending.delete(key)
    this.emit()
  }

  private setFailure(
    key: string,
    active: ActiveOperation,
    operation: InspectorMutationOperation | "reload",
    intent: TaskInspectorMutationRetryIntent | null,
    error: unknown,
    kindOverride?: TaskInspectorMutationError["kind"],
  ): void {
    if (!this.currentFor(active.generation, active.scope)) return
    const descriptor = errorDescriptor(error)
    this.errors = new Map(this.errors)
    this.errors.set(key, {
      operation,
      taskId: active.scope.taskId,
      kind: kindOverride ?? descriptor.kind,
      message: kindOverride === "stale" ? "stale" : descriptor.message,
      status: descriptor.status,
      code: descriptor.code,
      recoverable: true,
    })
    this.retries = new Map(this.retries)
    if (intent === null) this.retries.delete(key)
    else this.retries.set(key, intent)
    this.finish(key, active)
  }

  /** Reconcile a rejected write without retaining an obsolete write intent. */
  private async reconcileConflict(
    operation: InspectorMutationOperation,
    taskId: string,
  ): Promise<boolean> {
    const surface = this.surface
    if (surface === null || surface.scope.taskId !== taskId) return false
    const event = createCommittedMutationEvent(operationKind(operation), taskId)
    const reloadIntent: TaskInspectorMutationRetryIntent = { operation: "reload", taskId, event }
    const active = this.begin("reload", taskId, reloadIntent)
    if (active === null || !this.currentFor(active.generation, active.scope)) return false
    const currentSurface = this.surface
    if (currentSurface === null) return false
    const key = inspectorMutationKey("reload", taskId)
    try {
      await currentSurface.onCanonicalReload(event, active.scope)
      if (!this.currentFor(active.generation, active.scope)) return false
      this.errors = new Map(this.errors)
      this.errors.delete(key)
      this.retries = new Map(this.retries)
      this.retries.delete(key)
      this.finish(key, active)
      return true
    } catch (error) {
      if (this.currentFor(active.generation, active.scope)) {
        this.setFailure(key, active, "reload", reloadIntent, error, "stale")
      }
      return false
    }
  }

  private async runWrite<T>(
    operation: InspectorMutationOperation,
    intent: TaskInspectorMutationRetryIntent,
    action: (surface: TaskInspectorMutationSurface, taskId: string, signal: AbortSignal) => Promise<T>,
  ): Promise<WriteOperationResult<T>> {
    const taskId = intent.taskId
    const active = this.begin(operation, taskId, intent)
    const surface = this.surface
    if (active === null || surface === null || !this.currentFor(active.generation, active.scope)) return { outcome: notCommittedOutcome, value: null }
    const key = inspectorMutationKey(operation, taskId)
    try {
      const value = await action(surface, taskId, active.abortController.signal)
      // The server response is the commit boundary. A scope replacement after
      // this point fences observer/reload work but must still report the write
      // as committed so callers do not replay the request.
      if (!this.currentFor(active.generation, active.scope)) return { outcome: committedNotReconciledOutcome, value }
      // A render may replace the surface object without changing identity. Use
      // that latest surface for all post-commit callbacks and claim storage.
      let currentSurface = this.surface
      if (currentSurface === null) return { outcome: committedNotReconciledOutcome, value }
      if (operation === "transition") {
        const transition = intent.operation === "transition" ? intent.command : null
        const tokens = this.claimTokens(currentSurface)
        if (transition?.action === "claim") {
          const token = tokenFromTransitionResponse(value)
          if (token !== null) tokens.set(taskId, token)
        } else if (transition !== null && terminalActions.has(transition.action)) {
          tokens.delete(taskId)
        }
      }
      const event = createCommittedMutationEvent(operationKind(operation), taskId)
      try {
        currentSurface.onMutationCommitted?.(event)
      } catch {
        // An observer cannot turn a committed server write into a local retry.
      }
      if (!this.currentFor(active.generation, active.scope)) return { outcome: committedNotReconciledOutcome, value }
      currentSurface = this.surface
      if (currentSurface === null) return { outcome: committedNotReconciledOutcome, value }
      try {
        await currentSurface.onCanonicalReload(event, active.scope)
      } catch (error) {
        if (this.currentFor(active.generation, active.scope)) {
          const reloadKey = inspectorMutationKey("reload", taskId)
          const reloadIntent: TaskInspectorMutationRetryIntent = { operation: "reload", taskId, event }
          this.errors = new Map(this.errors)
          const descriptor = errorDescriptor(error)
          this.errors.set(reloadKey, {
            operation: "reload",
            taskId,
            kind: "stale",
            message: "stale",
            status: descriptor.status,
            code: descriptor.code,
            recoverable: true,
          })
          this.retries = new Map(this.retries)
          this.retries.set(reloadKey, reloadIntent)
          this.finish(key, active)
        }
        return { outcome: committedNotReconciledOutcome, value }
      }
      if (this.currentFor(active.generation, active.scope)) {
        const reloadKey = inspectorMutationKey("reload", taskId)
        this.errors = new Map(this.errors)
        this.errors.delete(key)
        this.errors.delete(reloadKey)
        this.retries = new Map(this.retries)
        this.retries.delete(key)
        this.retries.delete(reloadKey)
        this.finish(key, active)
        return { outcome: committedReconciledOutcome, value }
      }
      return { outcome: committedNotReconciledOutcome, value }
    } catch (error) {
      if (!this.currentFor(active.generation, active.scope)) return { outcome: notCommittedOutcome, value: null }
      if (isAbortError(error)) {
        this.finish(key, active)
        return { outcome: notCommittedOutcome, value: null }
      }
      if (isMutationConflict(error)) {
        const currentSurface = this.surface
        if (operation === "transition" && isClaimTokenConflict(error) && currentSurface !== null) {
          currentSurface.claimTokens?.delete(taskId)
          if (currentSurface.claimTokens === undefined) this.fallbackClaimTokens.delete(taskId)
        }
        const descriptor = errorDescriptor(error)
        this.errors = new Map(this.errors)
        this.errors.set(key, {
          operation,
          taskId,
          kind: "conflict",
          message: descriptor.message,
          status: descriptor.status,
          code: descriptor.code,
          recoverable: true,
        })
        this.retries = new Map(this.retries)
        this.retries.delete(key)
        this.finish(key, active)
        const reconciled = await this.reconcileConflict(operation, taskId)
        return { outcome: reconciled ? notCommittedReconciledOutcome : notCommittedOutcome, value: null }
      }
      this.setFailure(key, active, operation, intent, error)
      return { outcome: notCommittedOutcome, value: null }
    }
  }

  private async runRead<T>(
    operation: InspectorMutationOperation,
    taskId: string,
    action: (surface: TaskInspectorMutationSurface, taskId: string, signal: AbortSignal) => Promise<T>,
  ): Promise<OperationResult<T>> {
    const active = this.begin(operation, taskId)
    const surface = this.surface
    if (active === null || surface === null || !this.currentFor(active.generation, active.scope)) return { ok: false, value: null }
    const key = inspectorMutationKey(operation, taskId)
    try {
      const value = await action(surface, taskId, active.abortController.signal)
      if (!this.currentFor(active.generation, active.scope)) return { ok: false, value: null }
      this.errors = new Map(this.errors)
      this.errors.delete(key)
      this.retries = new Map(this.retries)
      this.retries.delete(key)
      this.finish(key, active)
      return { ok: true, value }
    } catch (error) {
      if (!this.currentFor(active.generation, active.scope)) return { ok: false, value: null }
      if (isAbortError(error)) {
        this.finish(key, active)
        return { ok: false, value: null }
      }
      this.setFailure(key, active, operation, null, error)
      return { ok: false, value: null }
    }
  }

  async saveTask(input: InspectorSaveTaskInput): Promise<InspectorMutationOutcome> {
    const normalized = normalizedSaveInput(input)
    if (typeof normalized.title === "string" && normalized.title.trim().length === 0) return notCommittedOutcome
    return (await this.runWrite("saveTask", { operation: "saveTask", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.updateTask(taskId, normalized, { signal }))).outcome
  }

  async transition(command: InspectorTransitionCommand): Promise<InspectorMutationOutcome> {
    if (!transitionActions.has(command.action)) return notCommittedOutcome
    const surface = this.surface
    const taskId = surface?.scope.taskId ?? ""
    const token = surface === null ? null : (this.claimTokens(surface).get(taskId) ?? null)
    const decorated = transitionCommandWithToken(command, token)
    if (decorated.action === "release" && (surface?.claimTokens === undefined || token === null)) return notCommittedOutcome
    if (claimRequiredActions.has(decorated.action) && !transitionInputHasToken(decorated)) return notCommittedOutcome
    return (await this.runWrite("transition", { operation: "transition", taskId, command: decorated }, (current, id, signal) => executeTransition(current.client, id, decorated, signal))).outcome
  }

  async addDependency(parentTaskId: string): Promise<InspectorMutationOutcome> {
    const parent = normalizedText(parentTaskId)
    if (!parent) return notCommittedOutcome
    return (await this.runWrite("addDependency", { operation: "addDependency", taskId: this.surface?.scope.taskId ?? "", parentTaskId: parent }, (surface, taskId, signal) => surface.client.addDependency(taskId, parent, { signal }))).outcome
  }

  async removeDependency(parentTaskId: string): Promise<InspectorMutationOutcome> {
    const parent = normalizedText(parentTaskId)
    if (!parent) return notCommittedOutcome
    return (await this.runWrite("removeDependency", { operation: "removeDependency", taskId: this.surface?.scope.taskId ?? "", parentTaskId: parent }, (surface, taskId, signal) => surface.client.removeDependency(taskId, parent, { signal }))).outcome
  }

  async createStep(input: InspectorCreateStepInput): Promise<InspectorMutationOutcome> {
    const normalized = {
      ...normalizedStepInput(input),
      idempotency_key: stableRequestKey(input.idempotency_key, "step:"),
    }
    if (!normalized.title.trim()) return notCommittedOutcome
    return (await this.runWrite("createStep", { operation: "createStep", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.createStep(taskId, normalized, { signal }))).outcome
  }

  async linkStep(input: InspectorLinkStepInput): Promise<InspectorMutationOutcome> {
    const normalized = {
      ...normalizedStepInput(input),
      idempotency_key: stableRequestKey(input.idempotency_key, "step:"),
    }
    if (!normalized.title.trim() || !normalized.linked_task_ref?.trim()) return notCommittedOutcome
    return (await this.runWrite("linkStep", { operation: "linkStep", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.createStep(taskId, normalized, { signal }))).outcome
  }

  async markPlanNotRequired(input: InspectorPlanNotRequiredInput): Promise<InspectorMutationOutcome> {
    const reason = normalizedText(input.reason)
    if (!reason) return notCommittedOutcome
    const normalized = { ...input, reason }
    return (await this.runWrite("markPlanNotRequired", { operation: "markPlanNotRequired", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.markExecutionPlanNotRequired(taskId, normalized, { signal }))).outcome
  }

  async addLabel(input: InspectorAddLabelInput): Promise<InspectorMutationOutcome> {
    const normalized = normalizedLabelInput(input)
    if (!normalized.name && (!normalized.names || normalized.names.length === 0)) return notCommittedOutcome
    return (await this.runWrite("addLabel", { operation: "addLabel", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.addTaskLabel(taskId, normalized, { signal }))).outcome
  }

  async removeLabel(input: InspectorRemoveLabelInput | string): Promise<InspectorMutationOutcome> {
    const labelId = typeof input === "string" ? normalizedText(input) : normalizedText(input.labelId)
    if (!labelId) return notCommittedOutcome
    return (await this.runWrite("removeLabel", { operation: "removeLabel", taskId: this.surface?.scope.taskId ?? "", labelId }, (surface, taskId, signal) => surface.client.removeTaskLabel(taskId, labelId, { signal }))).outcome
  }

  async applySuggestedLabel(input: InspectorApplySuggestedLabelInput): Promise<InspectorMutationOutcome> {
    const normalized = normalizedLabelInput(input)
    if (!normalized.name && (!normalized.names || normalized.names.length === 0)) return notCommittedOutcome
    return (await this.runWrite("applySuggestedLabel", { operation: "applySuggestedLabel", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.addTaskLabel(taskId, normalized, { signal }))).outcome
  }

  async addComment(input: InspectorCommentInput): Promise<InspectorMutationOutcome> {
    const normalized = {
      ...normalizedCommentInput(input),
      idempotency_key: stableRequestKey(input.idempotency_key, "comment:"),
    }
    if (!normalized.body) return notCommittedOutcome
    return (await this.runWrite("addComment", { operation: "addComment", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.createComment(taskId, normalized, { signal }))).outcome
  }

  async uploadAttachment(input: InspectorUploadAttachmentInput): Promise<InspectorMutationOutcome> {
    const normalized = {
      ...normalizedAttachmentInput(input),
      id: stableRequestKey(input.id, "a_"),
    }
    if (!normalized.filename) return notCommittedOutcome
    return (await this.runWrite("uploadAttachment", { operation: "uploadAttachment", taskId: this.surface?.scope.taskId ?? "", input: normalized }, (surface, taskId, signal) => surface.client.createAttachment(taskId, normalized, { signal }))).outcome
  }

  async downloadAttachment(input: InspectorDownloadAttachmentInput | string): Promise<DownloadedAttachment | null> {
    const attachmentId = typeof input === "string" ? normalizedText(input) : normalizedText(input.attachmentId)
    const surface = this.surface
    if (!attachmentId || surface?.attachmentDownload === undefined) return null
    const result = await this.runRead("downloadAttachment", surface.scope.taskId, (current, taskId, signal) => current.attachmentDownload!.downloadAttachment(taskId, attachmentId, { signal }))
    return result.ok ? result.value : null
  }

  async deleteAttachment(input: InspectorDeleteAttachmentInput | string): Promise<InspectorMutationOutcome> {
    const attachmentId = typeof input === "string" ? normalizedText(input) : normalizedText(input.attachmentId)
    if (!attachmentId) return notCommittedOutcome
    return (await this.runWrite("deleteAttachment", { operation: "deleteAttachment", taskId: this.surface?.scope.taskId ?? "", attachmentId }, (surface, taskId, signal) => surface.client.deleteAttachment(taskId, attachmentId, { signal }))).outcome
  }

  async suggestLabels(query: InspectorSuggestTaskLabelsQuery = {}): Promise<ApiSuggestTaskLabelsResponseContract | null> {
    const surface = this.surface
    if (surface?.suggestTaskLabels === undefined) return null
    const result = await this.runRead(
      "suggestLabels",
      surface.scope.taskId,
      (current, taskId, signal) => current.suggestTaskLabels!(taskId, query, { signal }),
    )
    return result.ok ? result.value : null
  }

  async retry(key?: string): Promise<InspectorMutationOutcome> {
    const candidate = key === undefined
      ? this.retries.values().next().value as TaskInspectorMutationRetryIntent | undefined
      : this.retries.get(key)
    if (!candidate) return notCommittedOutcome
    if (candidate.operation === "reload") {
      const surface = this.surface
      if (surface === null || surface.scope.taskId !== candidate.taskId) return notCommittedOutcome
      const active = this.begin("reload", candidate.taskId, candidate)
      if (active === null) return notCommittedOutcome
      if (!this.currentFor(active.generation, active.scope)) return notCommittedOutcome
      const currentSurface = this.surface
      if (currentSurface === null) return notCommittedOutcome
      const pendingKey = inspectorMutationKey("reload", candidate.taskId)
      try {
        await currentSurface.onCanonicalReload(candidate.event ?? createCommittedMutationEvent("edit", candidate.taskId), active.scope)
        if (!this.currentFor(active.generation, active.scope)) return notCommittedOutcome
        this.errors = new Map(this.errors)
        this.errors.delete(key ?? inspectorMutationKey("reload", candidate.taskId))
        this.retries = new Map(this.retries)
        this.retries.delete(key ?? inspectorMutationKey("reload", candidate.taskId))
        this.finish(pendingKey, active)
        return notCommittedReconciledOutcome
      } catch (error) {
        if (this.currentFor(active.generation, active.scope)) {
          this.setFailure(key ?? inspectorMutationKey("reload", candidate.taskId), active, "reload", candidate, error, "stale")
        }
        return notCommittedOutcome
      }
    }
    switch (candidate.operation) {
      case "saveTask": return this.saveTask(candidate.input)
      case "transition": return this.transition(candidate.command)
      case "addDependency": return this.addDependency(candidate.parentTaskId)
      case "removeDependency": return this.removeDependency(candidate.parentTaskId)
      case "createStep": return this.createStep(candidate.input)
      case "linkStep": return this.linkStep(candidate.input)
      case "markPlanNotRequired": return this.markPlanNotRequired(candidate.input)
      case "addLabel": return this.addLabel(candidate.input)
      case "removeLabel": return this.removeLabel(candidate.labelId)
      case "applySuggestedLabel": return this.applySuggestedLabel(candidate.input)
      case "addComment": return this.addComment(candidate.input)
      case "uploadAttachment": return this.uploadAttachment(candidate.input)
      case "deleteAttachment": return this.deleteAttachment(candidate.attachmentId)
    }
  }
}

/** React adapter; all mutation semantics remain in the UI-agnostic controller above. */
export function useTaskInspectorMutationController(
  surface: TaskInspectorMutationSurface | undefined,
): TaskInspectorMutationController | null {
  const controllerRef = useRef<TaskInspectorMutationController | null>(null)
  if (controllerRef.current === null) controllerRef.current = new TaskInspectorMutationController(surface ?? null)
  const controller = controllerRef.current
  useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot)
  useLayoutEffect(() => {
    controller.setSurface(surface ?? null)
  }, [controller, surface])
  useEffect(() => () => controller.setSurface(null), [controller])
  return surface === undefined ? null : controller
}
