import type { HttpTransportError } from "../../lib/api/http-transport"
import type {
  ArchiveTaskIntent,
  BlockTaskIntent,
  ClaimTaskIntent,
  HeartbeatTaskIntent,
  CompleteTaskIntent,
  PromoteTaskIntent,
  SpecifyTaskIntent,
  SubmitReviewTaskIntent,
  TaskMutationClient,
  TaskTransitionAction,
  UnblockTaskIntent,
} from "../../lib/api/task-mutations"
import type { BoardTaskStatus, BoardTaskViewModel, BoardViewModel } from "./types"

export interface BoardTaskTransitionOption {
  readonly action: TaskTransitionAction
  readonly targetStatus: BoardTaskStatus
  readonly requiresReason: boolean
  readonly requiresDescription: boolean
  readonly requiresConfirmation: boolean
}

export type BoardTaskMutationClient = Pick<TaskMutationClient, "createTask" | "createStep" | "updateTask" | "transitionTask">

export interface BoardTaskMutationCommitted {
  readonly kind: "create" | "edit" | "transition"
  readonly taskId: string
  readonly boardSlug?: string
}

export interface BoardTaskCanonicalReloadOptions {
  /** Distinguish an initial conflict reconcile from a stale retry. */
  readonly reason?: "initial" | "retry"
  /** Preserve transition-specific invalidation when retrying a reload. */
  readonly mutationKind?: BoardTaskMutationCommitted["kind"]
}

export interface BoardTaskMutationSurface {
  readonly client: BoardTaskMutationClient
  readonly onCanonicalReload?: (options?: BoardTaskCanonicalReloadOptions) => Promise<BoardViewModel | null> | void
  /** Called once after the server mutation writes commit, before reconcile. */
  readonly onMutationCommitted?: (event: BoardTaskMutationCommitted) => void
}

const TRANSITIONS: Readonly<Record<BoardTaskStatus, readonly BoardTaskTransitionOption[]>> = {
  triage: [
    { action: "specify", targetStatus: "todo", requiresReason: false, requiresDescription: true, requiresConfirmation: false },
    { action: "block", targetStatus: "blocked", requiresReason: true, requiresDescription: false, requiresConfirmation: false },
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  todo: [
    { action: "promote", targetStatus: "ready", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "block", targetStatus: "blocked", requiresReason: true, requiresDescription: false, requiresConfirmation: false },
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  scheduled: [
    { action: "promote", targetStatus: "ready", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "block", targetStatus: "blocked", requiresReason: true, requiresDescription: false, requiresConfirmation: false },
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  ready: [
    { action: "claim", targetStatus: "running", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "block", targetStatus: "blocked", requiresReason: true, requiresDescription: false, requiresConfirmation: false },
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  running: [
    { action: "heartbeat", targetStatus: "running", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "submit-review", targetStatus: "review", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "complete", targetStatus: "done", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "block", targetStatus: "blocked", requiresReason: true, requiresDescription: false, requiresConfirmation: false },
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  blocked: [
    { action: "unblock", targetStatus: "todo", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
  ],
  review: [
    { action: "complete", targetStatus: "done", requiresReason: false, requiresDescription: false, requiresConfirmation: false },
    { action: "block", targetStatus: "blocked", requiresReason: true, requiresDescription: false, requiresConfirmation: false },
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  done: [
    { action: "archive", targetStatus: "archived", requiresReason: false, requiresDescription: false, requiresConfirmation: true },
  ],
  archived: [],
}

export function transitionOptionsForStatus(status: BoardTaskStatus): readonly BoardTaskTransitionOption[] {
  return TRANSITIONS[status]
}

/** Promote is accepted only when the read projection proves server readiness. */
export function canPromoteTask(task: BoardTaskViewModel, now = Date.now()): boolean {
  if (task.status !== "todo" && task.status !== "scheduled") return false
  if (task.title.trim().length === 0 || (task.description ?? "").trim().length === 0) return false
  if (task.readiness.dependencyBlocked) return false
  if (task.readiness.executionPlanState === "unplanned") return false
  if (task.status === "scheduled" && (task.scheduledAt === undefined || task.scheduledAt === null || task.scheduledAt > now)) return false
  return true
}

/** Complete/review cannot bypass incomplete required steps from the board summary. */
export function canCompleteTask(task: BoardTaskViewModel): boolean {
  if (task.status !== "running" && task.status !== "review") return true
  return task.readiness.completedRequiredStepCount >= task.readiness.requiredStepCount
}

export function transitionForTarget(
  status: BoardTaskStatus,
  targetStatus: BoardTaskStatus,
): BoardTaskTransitionOption | null {
  return transitionOptionsForStatus(status).find((option) => option.targetStatus === targetStatus) ?? null
}

function withContext(task: BoardTaskViewModel, option: BoardTaskTransitionOption, claimToken: string | null): BoardTaskTransitionOption {
  return {
    ...option,
    requiresConfirmation:
      option.requiresConfirmation
      || (task.status === "running" && (option.action === "complete" || option.action === "block") && claimToken === null)
      || (task.status === "running" && option.action === "archive"),
  }
}

/** Return actions legal for this task, including claim-token/force requirements. */
export function transitionOptionsForTask(task: BoardTaskViewModel, claimToken: string | null = null): readonly BoardTaskTransitionOption[] {
  return transitionOptionsForStatus(task.status)
    .filter((option) => option.action !== "submit-review" || claimToken !== null)
    .filter((option) => option.action !== "heartbeat" || claimToken !== null)
    .filter((option) => option.action !== "promote" || canPromoteTask(task))
    .filter((option) => option.action !== "complete" || canCompleteTask(task))
    .map((option) => withContext(task, option, claimToken))
}

/** Resolve a target column using the task's current claim context. */
export function transitionForTaskTarget(
  task: BoardTaskViewModel,
  targetStatus: BoardTaskStatus,
  claimToken: string | null = null,
): BoardTaskTransitionOption | null {
  if (task.status === "blocked" && targetStatus === "todo") {
    return { action: "unblock", targetStatus, requiresReason: false, requiresDescription: false, requiresConfirmation: false }
  }
  const option = transitionOptionsForTask(task, claimToken).find((candidate) => candidate.targetStatus === targetStatus)
  return option ?? null
}

export interface BoardTaskTransitionContext {
  readonly description?: string
  readonly reason?: string
  readonly claimToken?: string | null
  readonly confirmed?: boolean
}

export type BoardTaskTransitionCommand =
  | { readonly action: "specify"; readonly input: SpecifyTaskIntent }
  | { readonly action: "promote"; readonly input: PromoteTaskIntent }
  | { readonly action: "claim"; readonly input: ClaimTaskIntent }
  | { readonly action: "heartbeat"; readonly input: HeartbeatTaskIntent }
  | { readonly action: "complete"; readonly input: CompleteTaskIntent }
  | { readonly action: "submit-review"; readonly input: SubmitReviewTaskIntent }
  | { readonly action: "block"; readonly input: BlockTaskIntent }
  | { readonly action: "unblock"; readonly input: UnblockTaskIntent }
  | { readonly action: "archive"; readonly input: ArchiveTaskIntent }

/** Build the exact typed API intent for a visible transition. */
export function transitionCommandForTask(
  task: BoardTaskViewModel,
  option: BoardTaskTransitionOption,
  context: BoardTaskTransitionContext = {},
): BoardTaskTransitionCommand | null {
  const claimToken = context.claimToken ?? null
  if (option.requiresReason && (context.reason ?? "").trim().length === 0) return null
  if (option.requiresDescription && (context.description ?? "").trim().length === 0) return null
  if (option.requiresConfirmation && context.confirmed !== true) return null
  switch (option.action) {
    case "specify":
      return { action: "specify", input: { description: (context.description ?? "").trim() } }
    case "promote":
      return { action: "promote", input: {} }
    case "claim":
      return { action: "claim", input: { ttl_ms: 300_000, worker_profile: "manual" } }
    case "heartbeat":
      return claimToken === null ? null : { action: "heartbeat", input: { claim_token: claimToken, ttl_ms: 300_000 } }
    case "submit-review":
      return claimToken === null ? null : { action: "submit-review", input: { claim_token: claimToken } }
    case "complete":
      if (task.status !== "running") return { action: "complete", input: {} }
      return claimToken !== null
        ? { action: "complete", input: { claim_token: claimToken } }
        : context.confirmed === true ? { action: "complete", input: { force: true } } : null
    case "block":
      return claimToken !== null
        ? { action: "block", input: { claim_token: claimToken, reason: (context.reason ?? "").trim() } }
        : context.confirmed === true
          ? { action: "block", input: { force: true, reason: (context.reason ?? "").trim() } }
          : { action: "block", input: { reason: (context.reason ?? "").trim() } }
    case "unblock":
      return { action: "unblock", input: {} }
    case "archive":
      return { action: "archive", input: { force: true } }
    default:
      return null
  }
}

/** Execute a discriminated command without erasing the overloaded client type. */
export function executeBoardTaskTransition(
  client: BoardTaskMutationClient,
  taskId: string,
  command: BoardTaskTransitionCommand,
) {
  switch (command.action) {
    case "specify": return client.transitionTask(taskId, "specify", command.input)
    case "promote": return client.transitionTask(taskId, "promote", command.input)
    case "claim": return client.transitionTask(taskId, "claim", command.input)
    case "heartbeat": return client.transitionTask(taskId, "heartbeat", command.input)
    case "complete": return client.transitionTask(taskId, "complete", command.input)
    case "submit-review": return client.transitionTask(taskId, "submit-review", command.input)
    case "block": return client.transitionTask(taskId, "block", command.input)
    case "unblock": return client.transitionTask(taskId, "unblock", command.input)
    case "archive": return client.transitionTask(taskId, "archive", command.input)
  }
}

export function moveTaskOptimistically(
  model: BoardViewModel,
  taskId: string,
  targetStatus: BoardTaskStatus,
): BoardViewModel {
  let moved: BoardTaskViewModel | null = null
  let sourceStatus: BoardTaskStatus | null = null
  const groups: Record<string, readonly BoardTaskViewModel[]> = {}

  for (const [status, tasks] of Object.entries(model.tasksByStatus)) {
    const retained = tasks.filter((task) => {
      if (task.id !== taskId) return true
      moved = task
      sourceStatus = status as BoardTaskStatus
      return false
    })
    groups[status] = retained
  }
  if (moved === null || sourceStatus === targetStatus) return model
  const sourceTask = moved as BoardTaskViewModel
  const transition = transitionForTaskTarget(sourceTask, targetStatus)
  // `unblock` has a server-computed canonical destination; keep the blocked
  // projection until the reload rather than inventing a target locally.
  if (transition === null || transition.action === "unblock") return model

  const targetTasks = groups[targetStatus] ?? []
  const nextPosition = targetTasks.reduce((maximum, task) => Math.max(maximum, task.position), -1) + 1
  const nextTask = { ...sourceTask, status: targetStatus, position: nextPosition }
  groups[targetStatus] = [...targetTasks, nextTask]
  return { ...model, tasksByStatus: groups }
}

export function updateTaskOptimistically(model: BoardViewModel, taskId: string, title: string): BoardViewModel {
  const groups: Record<string, readonly BoardTaskViewModel[]> = {}
  let changed = false
  for (const [status, tasks] of Object.entries(model.tasksByStatus)) {
    groups[status] = tasks.map((task) => {
      if (task.id !== taskId) return task
      changed = true
      return { ...task, title }
    })
  }
  return changed ? { ...model, tasksByStatus: groups } : model
}

export function updateTaskDescriptionOptimistically(model: BoardViewModel, taskId: string, description: string): BoardViewModel {
  const groups: Record<string, readonly BoardTaskViewModel[]> = {}
  let changed = false
  for (const [status, tasks] of Object.entries(model.tasksByStatus)) {
    groups[status] = tasks.map((task) => {
      if (task.id !== taskId) return task
      changed = true
      return { ...task, description }
    })
  }
  return changed ? { ...model, tasksByStatus: groups } : model
}

/** Restore one task's prior optimistic patch while preserving other task patches. */
export function rollbackTaskOptimistically(
  current: BoardViewModel,
  snapshot: BoardViewModel,
  taskId: string,
): BoardViewModel {
  let previous: BoardTaskViewModel | null = null
  let previousStatus: BoardTaskStatus | null = null
  let previousIndex = 0
  for (const [status, tasks] of Object.entries(snapshot.tasksByStatus)) {
    const index = tasks.findIndex((task) => task.id === taskId)
    if (index >= 0) {
      previous = tasks[index] ?? null
      previousStatus = status as BoardTaskStatus
      previousIndex = index
      break
    }
  }
  if (previous === null || previousStatus === null) return current
  const groups: Record<string, readonly BoardTaskViewModel[]> = {}
  let found = false
  for (const [status, tasks] of Object.entries(current.tasksByStatus)) {
    const retained = tasks.filter((task) => {
      if (task.id !== taskId) return true
      found = true
      return false
    })
    groups[status] = retained
  }
  if (!found) groups[previousStatus] = groups[previousStatus] ?? []
  const target = [...(groups[previousStatus] ?? [])]
  target.splice(Math.min(previousIndex, target.length), 0, previous)
  groups[previousStatus] = target
  return { ...current, tasksByStatus: groups }
}

/** 409 and typed conflict codes require canonical reload before an explicit retry. */
export function isMutationConflict(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false
  const candidate = error as Partial<Pick<HttpTransportError, "status" | "apiError">>
  const code = candidate.apiError?.code
  return candidate.status === 409
    || code === "conflict"
    || code === "claim_conflict"
    || code === "claim_token_mismatch"
    || code === "idempotency_conflict"
}

export function isClaimTokenConflict(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false
  const candidate = error as Partial<Pick<HttpTransportError, "status" | "apiError">>
  return candidate.apiError?.code === "claim_token_mismatch"
    || (candidate.status === 403 && candidate.apiError?.code === "claim_conflict")
}
