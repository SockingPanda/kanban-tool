import type { EventMeta, EventRecord } from "@/lib/api"

export type AffectedQueries = {
  taskIds: Set<string>
  invalidateBoardTasks: boolean
  invalidateEvents: boolean
}

const BOARD_AFFECTING_TASK_KINDS = new Set([
  "task.created",
  "task.updated",
  "task.specified",
  "task.promoted",
  "task.claimed",
  "task.completed",
  "task.review.submitted",
  "task.blocked",
  "task.unblocked",
  "task.reclaimed",
  "task.archived",
  "task.dependency.added",
  "task.dependency.removed",
])

export function nextEventCursor(current: number, events: EventRecord[], meta: EventMeta) {
  if (typeof meta.next_after === "number" && Number.isFinite(meta.next_after)) return meta.next_after
  return Math.max(current, ...events.map((event) => event.id))
}

export function affectedQueriesForEvents(events: EventRecord[]): AffectedQueries {
  let invalidateBoardTasks = false
  const taskIds = new Set<string>()

  for (const event of events) {
    if (event.task_id) taskIds.add(event.task_id)
    if (!event.task_id || BOARD_AFFECTING_TASK_KINDS.has(event.kind)) invalidateBoardTasks = true
  }

  return {
    taskIds,
    invalidateBoardTasks,
    invalidateEvents: events.length > 0,
  }
}
