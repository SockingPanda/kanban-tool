import type { EventMeta, EventRecord } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type AffectedQueries = {
  taskIds: Set<string>
  taskDetailIds: Set<string>
  taskDependencyIds: Set<string>
  taskNeighborhoodIds: Set<string>
  taskStepIds: Set<string>
  taskRunIds: Set<string>
  taskRunLogIds: Set<string>
  taskCommentIds: Set<string>
  taskEventIds: Set<string>
  invalidateBoards?: boolean
  invalidateBoardTasks: boolean
  invalidateStats: boolean
  invalidateSearchStatus: boolean
  invalidateBoardTaskMap: boolean
  invalidateEvents: boolean
}

const BOARD_ROW_TASK_KINDS = new Set([
  "task.created",
  "task.updated",
  "task.specified",
  "task.promoted",
  "task.claimed",
  "task.heartbeat",
  "task.completed",
  "task.submitted_for_review",
  "task.blocked",
  "task.unblocked",
  "task.recomputed",
  "task.reclaimed",
  "task.archived",
  "task.restored",
  "task.deleted",
  "task.export_sanitized",
  "task.retry_policy.updated",
  "dependency.added",
  "dependency.removed",
])

const STATUS_COUNTER_TASK_KINDS = new Set([
  "task.created",
  "task.specified",
  "task.promoted",
  "task.claimed",
  "task.completed",
  "task.submitted_for_review",
  "task.blocked",
  "task.unblocked",
  "task.recomputed",
  "task.reclaimed",
  "task.archived",
  "task.restored",
  "task.deleted",
])

const BOARD_TASK_MAP_KINDS = new Set([
  "task.created",
  "task.updated",
  "task.specified",
  "task.promoted",
  "task.claimed",
  "task.completed",
  "task.submitted_for_review",
  "task.blocked",
  "task.unblocked",
  "task.recomputed",
  "task.reclaimed",
  "task.archived",
  "task.restored",
  "task.deleted",
  "dependency.added",
  "dependency.removed",
])

const BOARD_LIFECYCLE_KINDS = new Set(["board.created", "board.updated", "board.archived"])
const COMMENT_KINDS = new Set(["task.comment.created"])
const DEPENDENCY_KINDS = new Set(["dependency.added", "dependency.removed"])

export function nextEventCursor(current: number, events: EventRecord[], meta: EventMeta) {
  if (typeof meta.next_after === "number" && Number.isFinite(meta.next_after)) return meta.next_after
  return Math.max(current, ...events.map((event) => event.id))
}

export function affectedQueriesForEvents(events: EventRecord[]): AffectedQueries {
  let invalidateBoards = false
  let invalidateBoardTasks = false
  let invalidateStats = false
  let invalidateBoardTaskMap = false
  const taskIds = new Set<string>()
  const taskDetailIds = new Set<string>()
  const taskDependencyIds = new Set<string>()
  const taskNeighborhoodIds = new Set<string>()
  const taskStepIds = new Set<string>()
  const taskRunIds = new Set<string>()
  const taskRunLogIds = new Set<string>()
  const taskCommentIds = new Set<string>()
  const taskEventIds = new Set<string>()

  for (const event of events) {
    if (event.task_id) {
      taskIds.add(event.task_id)
      taskEventIds.add(event.task_id)
      if (event.run_id) {
        taskRunIds.add(event.task_id)
        taskRunLogIds.add(event.run_id)
      }
      if (COMMENT_KINDS.has(event.kind)) {
        taskCommentIds.add(event.task_id)
      } else if (DEPENDENCY_KINDS.has(event.kind)) {
        taskDependencyIds.add(event.task_id)
        taskNeighborhoodIds.add(event.task_id)
      } else if (event.kind.startsWith("step.") || event.kind.includes(".step.")) {
        taskStepIds.add(event.task_id)
        taskNeighborhoodIds.add(event.task_id)
      } else if (event.kind.startsWith("run.") || event.kind.includes(".run.")) {
        taskRunIds.add(event.task_id)
      } else {
        taskDetailIds.add(event.task_id)
      }
    }
    if (BOARD_LIFECYCLE_KINDS.has(event.kind)) invalidateBoards = true
    if (!event.task_id || BOARD_ROW_TASK_KINDS.has(event.kind)) invalidateBoardTasks = true
    if (!event.task_id || STATUS_COUNTER_TASK_KINDS.has(event.kind)) invalidateStats = true
    if (!event.task_id || BOARD_TASK_MAP_KINDS.has(event.kind)) invalidateBoardTaskMap = true
  }

  return {
    taskIds,
    taskDetailIds,
    taskDependencyIds,
    taskNeighborhoodIds,
    taskStepIds,
    taskRunIds,
    taskRunLogIds,
    taskCommentIds,
    taskEventIds,
    ...(invalidateBoards ? { invalidateBoards } : {}),
    invalidateBoardTasks,
    invalidateStats,
    invalidateSearchStatus: events.length > 0,
    invalidateBoardTaskMap,
    invalidateEvents: events.length > 0,
  }
}

export function queryKeysForAffectedEvents({
  affected,
  board,
}: {
  affected: AffectedQueries
  board: string
}) {
  const keys = []

  if (affected.invalidateEvents) keys.push(queryKeys.events(board))
  if (affected.invalidateBoards) keys.push(queryKeys.boards())
  if (affected.invalidateBoardTasks) keys.push(queryKeys.boardTasksRoot(board))
  if (affected.invalidateStats) keys.push(queryKeys.stats(board))
  if (affected.invalidateSearchStatus) keys.push(queryKeys.searchStatus(board))
  if (affected.invalidateBoardTaskMap) keys.push(queryKeys.boardTaskMapRoot(board))
  for (const taskId of affected.taskDetailIds) keys.push(queryKeys.taskDetail(taskId))
  for (const taskId of affected.taskDependencyIds) keys.push(queryKeys.taskDependencies(taskId))
  for (const taskId of affected.taskNeighborhoodIds) keys.push(queryKeys.taskNeighborhood(taskId))
  for (const taskId of affected.taskStepIds) keys.push(queryKeys.taskSteps(taskId))
  for (const taskId of affected.taskRunIds) keys.push(queryKeys.taskRuns(taskId))
  for (const runId of affected.taskRunLogIds) keys.push(queryKeys.taskRunLog(runId))
  for (const taskId of affected.taskCommentIds) keys.push(queryKeys.taskComments(taskId))
  for (const taskId of affected.taskEventIds) keys.push(queryKeys.taskEvents(taskId))

  return keys
}
