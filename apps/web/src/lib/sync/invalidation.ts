import type {
  InvalidationPlan,
  QueryRoot,
  QueryTarget,
  ValidatedBusinessEvent,
} from "./contracts"

const BOARD_PROJECTION_ROOTS: readonly QueryRoot[] = ["tasks", "stats", "search-status", "board-task-map"]
const FULL_BOARD_ROOTS: readonly QueryRoot[] = [
  "columns",
  "tasks",
  "stats",
  "search-status",
  "board-task-map",
  "events",
  "signals",
  "label-ontology",
  "maintenance-status",
]

function boardTarget(root: QueryRoot, event: ValidatedBusinessEvent, observedOnly = false): QueryTarget {
  return { root, boardId: event.boardId, ...(observedOnly ? { observedOnly: true } : {}) }
}

function taskTarget(root: QueryRoot, taskId: string, observedOnly = false): QueryTarget {
  return { root, taskId, ...(observedOnly ? { observedOnly: true } : {}) }
}

function runTarget(root: "task-runs" | "task-run-log", event: ValidatedBusinessEvent): QueryTarget | null {
  if (event.runId === null) return null
  if (root === "task-runs") return event.taskId === null ? null : taskTarget(root, event.taskId, true)
  return { root, runId: event.runId, observedOnly: true }
}

function addTargets(targets: QueryTarget[], event: ValidatedBusinessEvent, roots: readonly QueryRoot[]): void {
  for (const root of roots) targets.push(boardTarget(root, event))
}

function addBoardProjection(targets: QueryTarget[], event: ValidatedBusinessEvent): void {
  addTargets(targets, event, BOARD_PROJECTION_ROOTS)
  targets.push(boardTarget("task-neighborhood", event, true))
}

function addTaskDetail(targets: QueryTarget[], event: ValidatedBusinessEvent): void {
  if (event.taskId !== null) targets.push(taskTarget("task-detail", event.taskId))
}

function addRunProjection(targets: QueryTarget[], event: ValidatedBusinessEvent): void {
  if (event.runId === null) return
  const taskRuns = runTarget("task-runs", event)
  const runLog = runTarget("task-run-log", event)
  if (taskRuns !== null) targets.push(taskRuns)
  if (runLog !== null) targets.push(runLog)
}

function addTimeline(targets: QueryTarget[], event: ValidatedBusinessEvent): void {
  targets.push(boardTarget("events", event))
  if (event.taskId !== null) targets.push(taskTarget("task-events", event.taskId))
}

function baseTargets(event: ValidatedBusinessEvent): QueryTarget[] {
  const targets: QueryTarget[] = []
  addTimeline(targets, event)
  targets.push(boardTarget("search-status", event))
  if (event.taskId !== null) targets.push({ root: "maintenance-status", observedOnly: true })
  return targets
}

function finish(event: ValidatedBusinessEvent, targets: QueryTarget[], fullRefetch = false, reason?: string): InvalidationPlan {
  const unique = new Map<string, QueryTarget>()
  for (const target of targets) unique.set(targetKey(target), target)
  return {
    kind: event.known ? "known" : "unknown",
    eventKind: event.kind,
    timeline: true,
    fullRefetch,
    targets: [...unique.values()],
    ...(reason ? { reason } : {}),
  }
}

export function targetKey(target: QueryTarget): string {
  if (target.taskId !== undefined) return `${target.root}(${target.taskId})`
  if (target.runId !== undefined) return `${target.root}(${target.runId})`
  if (target.signalId !== undefined) return `${target.root}(${target.signalId})`
  if (target.atomRef !== undefined) return `${target.root}(${target.atomRef})`
  if (target.boardId !== undefined) return `${target.root}(${target.boardId})`
  return target.root
}

export function fullRefetchPlan(eventKind: string, kind: "known" | "unknown", boardId: string): InvalidationPlan {
  const targets: QueryTarget[] = FULL_BOARD_ROOTS.map((root) => ({ root, ...(root === "maintenance-status" ? {} : { boardId }) }))
  targets.push(
    { root: "task-detail", boardId, observedOnly: true },
    { root: "task-attachments", boardId, observedOnly: true },
    { root: "task-label-suggestions", boardId, observedOnly: true },
    { root: "task-dependencies", boardId, observedOnly: true },
    { root: "task-neighborhood", boardId, observedOnly: true },
    { root: "task-steps", boardId, observedOnly: true },
    { root: "task-comments", boardId, observedOnly: true },
    { root: "task-runs", boardId, observedOnly: true },
    { root: "task-run-log", boardId, observedOnly: true },
    { root: "task-events", boardId, observedOnly: true },
    { root: "signals", boardId, observedOnly: true },
    { root: "signal", boardId, observedOnly: true },
    { root: "label-ontology-signal", boardId, observedOnly: true },
    { root: "label-ontology-atom", boardId, observedOnly: true },
  )
  return {
    kind,
    eventKind,
    timeline: true,
    fullRefetch: true,
    targets,
    reason: "unknown-or-protocol-anomaly",
  }
}

export function classifyEvent(event: ValidatedBusinessEvent): InvalidationPlan {
  if (!event.known) return fullRefetchPlan(event.kind, "unknown", event.boardId)

  const targets = baseTargets(event)
  switch (event.kind) {
    case "board.created":
      targets.push(boardTarget("columns", event))
      break
    case "board.archived":
      targets.push(boardTarget("boards", event), boardTarget("columns", event))
      addBoardProjection(targets, event)
      break
    case "dependency.added":
    case "dependency.removed": {
      if (event.taskId !== null) {
        targets.push(taskTarget("task-detail", event.taskId), taskTarget("task-dependencies", event.taskId), taskTarget("task-neighborhood", event.taskId))
      }
      const parent = event.scope.parentTaskId
      if (parent !== null && parent !== undefined && parent !== event.taskId) {
        targets.push(taskTarget("task-detail", parent), taskTarget("task-dependencies", parent), taskTarget("task-neighborhood", parent))
      }
      targets.push(boardTarget("tasks", event), boardTarget("board-task-map", event), boardTarget("task-neighborhood", event, true))
      break
    }
    case "label.created":
      targets.push(boardTarget("label-ontology", event))
      addTaskDetail(targets, event)
      break
    case "label.deleted":
      targets.push(
        boardTarget("task-detail", event, true),
        boardTarget("task-label-suggestions", event, true),
        boardTarget("label-ontology", event),
        boardTarget("label-ontology-atom", event, true),
        boardTarget("tasks", event),
      )
      break
    case "signal.recorded":
    case "signal.reviewed":
      targets.push(boardTarget("signals", event))
      if (event.scope.signalId !== null && event.scope.signalId !== undefined) {
        targets.push({ root: "signal", signalId: event.scope.signalId })
      }
      if (event.taskId !== null) {
        targets.push(taskTarget("task-detail", event.taskId), taskTarget("task-comments", event.taskId))
      }
      break
    case "task.comment.created":
      if (event.taskId !== null) targets.push(taskTarget("task-comments", event.taskId))
      break
    case "task.created":
    case "task.updated":
      addBoardProjection(targets, event)
      addTaskDetail(targets, event)
      if (event.taskId !== null) targets.push(taskTarget("task-label-suggestions", event.taskId, true))
      addRunProjection(targets, event)
      break
    case "task.archived":
    case "task.completed":
    case "task.reopened":
      addBoardProjection(targets, event)
      addTaskDetail(targets, event)
      for (const root of ["task-detail", "task-dependencies", "task-neighborhood"] as const) {
        targets.push(boardTarget(root, event, true))
      }
      addRunProjection(targets, event)
      break
    case "task.specified":
    case "task.promoted":
    case "task.claimed":
    case "task.blocked":
    case "task.unblocked":
    case "task.recomputed":
    case "task.released":
    case "task.submitted_for_review":
    case "task.reclaimed":
      addBoardProjection(targets, event)
      addTaskDetail(targets, event)
      addRunProjection(targets, event)
      break
    case "task.execution_plan.not_required":
    case "task.execution_plan.planned":
    case "task.execution_plan.unplanned":
      addBoardProjection(targets, event)
      addTaskDetail(targets, event)
      if (event.taskId !== null) targets.push(taskTarget("task-steps", event.taskId), taskTarget("task-neighborhood", event.taskId))
      break
    case "task.heartbeat":
      addTaskDetail(targets, event)
      targets.push(boardTarget("tasks", event), boardTarget("stats", event))
      addRunProjection(targets, event)
      break
    case "task.label.added":
    case "task.label.removed":
      addTaskDetail(targets, event)
      if (event.taskId !== null) targets.push(taskTarget("task-label-suggestions", event.taskId, true))
      targets.push(boardTarget("tasks", event))
      break
    case "task.label_proposal.proposed":
    case "task.label_proposal.accepted":
    case "task.label_proposal.rejected":
      targets.push(boardTarget("label-ontology", event))
      addTaskDetail(targets, event)
      break
    case "task.retry_policy.updated":
      addTaskDetail(targets, event)
      targets.push(boardTarget("stats", event))
      addRunProjection(targets, event)
      break
    case "task.step.created":
    case "task.step.done":
    case "task.step.removed":
    case "task.step.reopened":
    case "task.step.skipped":
    case "task.step.updated":
      addBoardProjection(targets, event)
      addTaskDetail(targets, event)
      if (event.taskId !== null) {
        targets.push(taskTarget("task-steps", event.taskId), taskTarget("task-neighborhood", event.taskId))
      }
      if (event.scope.linkedTaskId !== null && event.scope.linkedTaskId !== undefined && event.scope.linkedTaskId !== event.taskId) {
        targets.push(taskTarget("task-neighborhood", event.scope.linkedTaskId))
      }
      break
    case "task.export_sanitized":
      addBoardProjection(targets, event)
      addTaskDetail(targets, event)
      addRunProjection(targets, event)
      break
    default:
      return fullRefetchPlan(event.kind, "known", event.boardId)
  }
  return finish(event, targets)
}
