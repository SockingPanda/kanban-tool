import type { HttpTransportError } from "../../lib/api/http-transport"
import type { TaskMutationClient, TaskTransitionAction } from "../../lib/api/task-mutations"
import type { BoardTaskStatus, BoardTaskViewModel, BoardViewModel } from "./types"

export interface BoardTaskTransitionOption {
  readonly action: TaskTransitionAction
  readonly targetStatus: BoardTaskStatus
  readonly requiresReason: boolean
}

export type BoardTaskMutationClient = Pick<TaskMutationClient, "createTask" | "updateTask" | "transitionTask">

export interface BoardTaskMutationSurface {
  readonly client: BoardTaskMutationClient
  readonly onCanonicalReload?: () => Promise<void> | void
}

const TRANSITIONS: Readonly<Record<BoardTaskStatus, readonly BoardTaskTransitionOption[]>> = {
  triage: [
    { action: "specify", targetStatus: "todo", requiresReason: false },
    { action: "block", targetStatus: "blocked", requiresReason: true },
  ],
  todo: [
    { action: "promote", targetStatus: "ready", requiresReason: false },
    { action: "block", targetStatus: "blocked", requiresReason: true },
  ],
  scheduled: [
    { action: "promote", targetStatus: "ready", requiresReason: false },
    { action: "block", targetStatus: "blocked", requiresReason: true },
  ],
  ready: [
    { action: "claim", targetStatus: "running", requiresReason: false },
    { action: "block", targetStatus: "blocked", requiresReason: true },
  ],
  running: [
    { action: "submit-review", targetStatus: "review", requiresReason: false },
    { action: "complete", targetStatus: "done", requiresReason: false },
    { action: "block", targetStatus: "blocked", requiresReason: true },
  ],
  blocked: [
    { action: "unblock", targetStatus: "todo", requiresReason: false },
  ],
  review: [
    { action: "complete", targetStatus: "done", requiresReason: false },
    { action: "block", targetStatus: "blocked", requiresReason: true },
  ],
  done: [
    { action: "archive", targetStatus: "archived", requiresReason: false },
  ],
  archived: [],
}

export function transitionOptionsForStatus(status: BoardTaskStatus): readonly BoardTaskTransitionOption[] {
  return TRANSITIONS[status]
}

export function transitionForTarget(
  status: BoardTaskStatus,
  targetStatus: BoardTaskStatus,
): BoardTaskTransitionOption | null {
  return transitionOptionsForStatus(status).find((option) => option.targetStatus === targetStatus) ?? null
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
  if (transitionForTarget(sourceTask.status, targetStatus) === null) return model

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

/** 409 and typed conflict codes require canonical reload before an explicit retry. */
export function isMutationConflict(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false
  const candidate = error as Partial<Pick<HttpTransportError, "status" | "apiError">>
  const code = candidate.apiError?.code
  return candidate.status === 409 || code === "conflict" || code === "claim_conflict" || code === "idempotency_conflict"
}
