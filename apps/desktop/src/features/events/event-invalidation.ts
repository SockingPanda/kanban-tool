import type { EventMeta, EventRecord } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type AffectedQueries = {
  taskIds: Set<string>
  invalidateBoards?: boolean
  invalidateBoardTasks: boolean
  invalidateEvents: boolean
}

const BOARD_AFFECTING_TASK_KINDS = new Set([
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
  "task.comment.created",
  "task.retry_policy.updated",
  "dependency.added",
  "dependency.removed",
])

const BOARD_LIFECYCLE_KINDS = new Set(["board.created", "board.updated", "board.archived"])

export function nextEventCursor(current: number, events: EventRecord[], meta: EventMeta) {
  if (typeof meta.next_after === "number" && Number.isFinite(meta.next_after)) return meta.next_after
  return Math.max(current, ...events.map((event) => event.id))
}

export function affectedQueriesForEvents(events: EventRecord[]): AffectedQueries {
  let invalidateBoards = false
  let invalidateBoardTasks = false
  const taskIds = new Set<string>()

  for (const event of events) {
    if (event.task_id) taskIds.add(event.task_id)
    if (BOARD_LIFECYCLE_KINDS.has(event.kind)) invalidateBoards = true
    if (!event.task_id || BOARD_AFFECTING_TASK_KINDS.has(event.kind)) invalidateBoardTasks = true
  }

  return {
    taskIds,
    ...(invalidateBoards ? { invalidateBoards } : {}),
    invalidateBoardTasks,
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
  if (affected.invalidateBoardTasks) {
    keys.push(queryKeys.boardTasksRoot(board))
    keys.push(queryKeys.stats(board))
    keys.push(queryKeys.searchStatus(board))
  }
  for (const taskId of affected.taskIds) keys.push(queryKeys.taskDetail(taskId))

  return keys
}
