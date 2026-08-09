import type { Locale } from "../../lib/preferences"
import type { BoardTaskViewModel } from "../board/types"
import {
  canCompleteTask,
  canPromoteTask,
  transitionCommandForTask,
  transitionOptionsForStatus,
  type BoardTaskTransitionOption as PolicyTransitionOption,
} from "../board/task-mutation-state"
import type {
  InspectorMutationOutcome,
  InspectorSaveTaskInput,
  InspectorTransitionCommand,
  TaskInspectorMutationRetryIntent,
} from "./task-inspector-mutation-state"
import type { TaskInspectorViewModel } from "./TaskInspector"

export const inspectorActionIds = ["specify", "promote", "claim", "heartbeat", "complete", "submit-review", "block", "unblock", "archive"] as const
export type InspectorActionId = (typeof inspectorActionIds)[number]

export const inspectorActionLabels: Readonly<Record<Locale, readonly string[]>> = {
  zh: ["指定", "晋级", "认领", "发送心跳", "完成", "提交审核", "阻塞", "解除阻塞", "归档"],
  en: ["Specify", "Promote", "Claim", "Heartbeat", "Complete", "Submit Review", "Block", "Unblock", "Archive"],
}

export function inspectorMutationCommitted(outcome: InspectorMutationOutcome | null): boolean {
  return outcome?.committed === true
}

function stableIntentValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableIntentValue)
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, stableIntentValue(entry)]),
    )
  }
  return value
}

function sameIntentValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(stableIntentValue(left)) === JSON.stringify(stableIntentValue(right))
}

function userIntentValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(userIntentValue)
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .filter(([key]) => key !== "expected_lock_version" && key !== "claim_token" && key !== "lock_version" && key !== "revision")
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, userIntentValue(entry)]),
    )
  }
  return value
}

function sameUserIntentValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(userIntentValue(left)) === JSON.stringify(userIntentValue(right))
}

/** Compare a current editor/action value with the exact intent retained by the controller. */
export function inspectorRetryIntentMatches(
  intent: TaskInspectorMutationRetryIntent | undefined,
  operation: "saveTask" | "transition",
  current: InspectorSaveTaskInput | InspectorTransitionCommand | null,
): boolean {
  if (current === null || intent?.operation !== operation) return false
  if (operation === "saveTask" && intent.operation === "saveTask") return sameIntentValue(intent.input, current)
  if (operation === "transition" && intent.operation === "transition") return sameIntentValue(intent.command, current)
  return false
}

/** Compare only user-editable fields when deciding whether a committed result may close the surface. */
export function inspectorRetryUserIntentMatches(
  intent: TaskInspectorMutationRetryIntent | undefined,
  operation: "saveTask" | "transition",
  current: InspectorSaveTaskInput | InspectorTransitionCommand | null,
): boolean {
  if (current === null || intent?.operation !== operation) return false
  if (operation === "saveTask" && intent.operation === "saveTask") return sameUserIntentValue(intent.input, current)
  if (operation === "transition" && intent.operation === "transition") return sameUserIntentValue(intent.command, current)
  return false
}

export interface InspectorEditDraft {
  readonly title: string
  readonly description: string
  readonly assignee: string
  readonly priority: number
  readonly scheduledAt: string
  readonly dueAt: string
}

export interface InspectorActionCopy {
  readonly actionReasons: {
    readonly description: string
    readonly dependencies: string
    readonly plan: string
    readonly promote: string
    readonly claim: string
    readonly requiredSteps: string
    readonly status: string
  }
}

function parseDateTimeInput(value: string): number | null {
  if (value.trim().length === 0) return null
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : null
}

function dateTimeInputValue(value: number | null | undefined): string {
  if (value === null || value === undefined) return ""
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ""
  const offset = date.getTimezoneOffset() * 60_000
  return new Date(date.getTime() - offset).toISOString().slice(0, 16)
}

export function inspectorEditDraft(task: TaskInspectorViewModel["task"]): InspectorEditDraft {
  return {
    title: task.title,
    description: task.description ?? "",
    assignee: task.assignee ?? "",
    priority: task.priority,
    scheduledAt: dateTimeInputValue(task.scheduledAt),
    dueAt: dateTimeInputValue(task.dueAt),
  }
}

export function buildInspectorSaveTaskInput(task: TaskInspectorViewModel["task"], draft: InspectorEditDraft): InspectorSaveTaskInput {
  return {
    title: draft.title.trim(),
    description: draft.description.trim() || null,
    assignee: draft.assignee.trim() || null,
    priority: draft.priority,
    scheduled_at: parseDateTimeInput(draft.scheduledAt),
    due_at: parseDateTimeInput(draft.dueAt),
    expected_lock_version: task.lockVersion ?? 0,
  }
}

export function boardTaskView(task: TaskInspectorViewModel["task"]): BoardTaskViewModel {
  return {
    id: task.id,
    seq: 0,
    ref: task.ref,
    title: task.title,
    description: task.description,
    status: task.status,
    position: 0,
    scheduledAt: task.scheduledAt,
    lockVersion: task.lockVersion ?? 0,
    priority: task.priority as 0 | 1 | 2 | 3,
    assignee: task.assignee,
    readiness: {
      dependencyBlocked: task.dependencyBlocked,
      unfinishedParentCount: task.unfinishedParentCount,
      executionPlanState: task.executionPlanState,
      requiredStepCount: task.requiredStepCount,
      completedRequiredStepCount: task.completedRequiredStepCount,
      optionalStepCount: task.optionalStepCount,
    },
  }
}

export function actionForStatus(task: BoardTaskViewModel, action: InspectorActionId): PolicyTransitionOption | null {
  return transitionOptionsForStatus(task.status).find((option) => option.action === action) ?? null
}

function actionRequiresConfirmation(task: BoardTaskViewModel, option: PolicyTransitionOption, claimToken: string | null): boolean {
  return option.requiresConfirmation
    || (task.status === "running" && (option.action === "complete" || option.action === "block") && claimToken === null)
    || (task.status === "running" && option.action === "archive")
}

export interface InspectorTransitionContext {
  readonly description?: string
  readonly reason?: string
  readonly confirmed?: boolean
}

/** Build one exact typed transition command using the shared board policy. */
export function buildInspectorTransitionCommand(
  task: TaskInspectorViewModel["task"],
  action: InspectorActionId,
  context: InspectorTransitionContext = {},
  claimToken: string | null = null,
): InspectorTransitionCommand | null {
  const boardTask = boardTaskView(task)
  const option = actionForStatus(boardTask, action)
  if (option === null) return null
  if (!actionEnabled(boardTask, action, claimToken)) return null
  return transitionCommandForTask(
    boardTask,
    { ...option, requiresConfirmation: actionRequiresConfirmation(boardTask, option, claimToken) },
    { ...context, claimToken },
  ) as InspectorTransitionCommand | null
}

function actionEnabled(task: BoardTaskViewModel, action: InspectorActionId, claimToken: string | null): boolean {
  const option = actionForStatus(task, action)
  if (option === null) return false
  switch (action) {
    // The description is collected in the explicit Specify dialog; the button
    // remains available so an underspecified task has a clear recovery path.
    case "specify": return true
    case "promote": return canPromoteTask(task)
    case "claim": return true
    case "heartbeat":
    case "submit-review": return claimToken !== null
    case "complete": return canCompleteTask(task)
    case "block":
    case "unblock":
    case "archive": return true
  }
}

function actionDisabledReason(task: BoardTaskViewModel, action: InspectorActionId, claimToken: string | null, copy: InspectorActionCopy): string | null {
  if (actionEnabled(task, action, claimToken)) return null
  switch (action) {
    case "specify": return copy.actionReasons.description
    case "promote": return task.readiness.dependencyBlocked ? copy.actionReasons.dependencies : task.readiness.executionPlanState === "unplanned" ? copy.actionReasons.plan : copy.actionReasons.promote
    case "heartbeat":
    case "submit-review": return copy.actionReasons.claim
    case "complete": return copy.actionReasons.requiredSteps
    default: return copy.actionReasons.status
  }
}

export interface InspectorActionView {
  readonly action: InspectorActionId
  readonly option: PolicyTransitionOption
  readonly enabled: boolean
  readonly requiresConfirmation: boolean
  readonly disabledReason: string | null
}

export function inspectorActionViews(task: TaskInspectorViewModel["task"], claimToken: string | null, copy: InspectorActionCopy): readonly InspectorActionView[] {
  const boardTask = boardTaskView(task)
  return inspectorActionIds.flatMap((action) => {
    const option = actionForStatus(boardTask, action)
    if (option === null) return []
    return [{
      action,
      option,
      enabled: actionEnabled(boardTask, action, claimToken),
      requiresConfirmation: actionRequiresConfirmation(boardTask, option, claimToken),
      disabledReason: actionDisabledReason(boardTask, action, claimToken, copy),
    }]
  })
}

export function actionLabel(action: InspectorActionId, locale: Locale): string {
  return inspectorActionLabels[locale][inspectorActionIds.indexOf(action)] ?? action
}
