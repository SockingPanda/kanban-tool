import { describe, expect, it } from "vitest"

import type { EventRecord } from "@/lib/api"

import { affectedQueriesForEvents, nextEventCursor, queryKeysForAffectedEvents, type AffectedQueries } from "./event-invalidation"

describe("event invalidation helpers", () => {
  it("uses event envelope next_after before falling back to row ids", () => {
    expect(nextEventCursor(10, [eventRecord({ id: 12 })], { next_after: 30 })).toBe(30)
    expect(nextEventCursor(10, [eventRecord({ id: 12 }), eventRecord({ id: 14 })], {})).toBe(14)
    expect(nextEventCursor(10, [], {})).toBe(10)
  })

  it("invalidates comments without refreshing board rows or status counters", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_1", kind: "task.comment.created" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_1"]),
      taskCommentIds: new Set(["t_1"]),
      taskEventIds: new Set(["t_1"]),
      invalidateBoardTasks: false,
      invalidateStats: false,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: false,
      invalidateEvents: true,
    }))
  })

  it("invalidates affected task detail and board task queries without a blind refresh for every event", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind: "task.submitted_for_review" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_2"]),
      taskDetailIds: new Set(["t_2"]),
      taskEventIds: new Set(["t_2"]),
      invalidateBoardTasks: true,
      invalidateStats: true,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateEvents: true,
    }))

    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_3", kind: "task.recomputed" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_3"]),
      taskDetailIds: new Set(["t_3"]),
      taskEventIds: new Set(["t_3"]),
      invalidateBoardTasks: true,
      invalidateStats: true,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateEvents: true,
    }))
  })

  it("invalidates dependency row and graph data without refreshing status counters", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_4", kind: "dependency.added" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_4"]),
      taskDependencyIds: new Set(["t_4"]),
      taskNeighborhoodIds: new Set(["t_4"]),
      taskEventIds: new Set(["t_4"]),
      invalidateBoardTasks: true,
      invalidateStats: false,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateEvents: true,
    }))

    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_4", kind: "dependency.removed" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_4"]),
      taskDependencyIds: new Set(["t_4"]),
      taskNeighborhoodIds: new Set(["t_4"]),
      taskEventIds: new Set(["t_4"]),
      invalidateBoardTasks: true,
      invalidateStats: false,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateEvents: true,
    }))
  })

  it("keeps heartbeat on board rows because the board card renders last heartbeat", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_5", kind: "task.heartbeat" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_5"]),
      taskDetailIds: new Set(["t_5"]),
      taskEventIds: new Set(["t_5"]),
      invalidateBoardTasks: true,
      invalidateStats: false,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: false,
      invalidateEvents: true,
    }))
  })

  it("invalidates status counters for status-changing task events", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind: "task.reopened" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_2"]),
      taskDetailIds: new Set(["t_2"]),
      taskEventIds: new Set(["t_2"]),
      invalidateBoardTasks: true,
      invalidateStats: true,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateEvents: true,
    }))

    expect(
      affectedQueriesForEvents([eventRecord({ task_id: null, kind: "board.archived" })]),
    ).toEqual(affected({
      taskIds: new Set<string>(),
      invalidateBoardTasks: true,
      invalidateStats: true,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateBoards: true,
      invalidateEvents: true,
    }))
  })

  it.each(["task.updated", "task.specified", "task.promoted", "task.claimed"])(
    "invalidates board rows and task detail for %s",
    (kind) => {
      expect(
        affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind })]),
      ).toEqual(affected({
        taskIds: new Set(["t_2"]),
        taskDetailIds: new Set(["t_2"]),
        taskEventIds: new Set(["t_2"]),
        invalidateBoardTasks: true,
        invalidateStats: kind === "task.updated" ? false : true,
        invalidateSearchStatus: true,
        invalidateBoardTaskMap: true,
        invalidateEvents: true,
      }))
    },
  )

  it.each(["task.completed", "task.blocked", "task.unblocked", "task.archived", "task.restored"])(
    "invalidates status counters for lifecycle event %s",
    (kind) => {
      expect(
        affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind })]),
      ).toEqual(affected({
        taskIds: new Set(["t_2"]),
        taskDetailIds: new Set(["t_2"]),
        taskEventIds: new Set(["t_2"]),
        invalidateBoardTasks: true,
        invalidateStats: true,
        invalidateSearchStatus: true,
        invalidateBoardTaskMap: true,
        invalidateEvents: true,
      }))
    },
  )

  it("invalidates board queries for board update lifecycle events", () => {
    const affected = affectedQueriesForEvents([eventRecord({ task_id: null, kind: "board.updated" })])

    expect(queryKeysForAffectedEvents({ affected, board: "default" })).toEqual([
      ["events", "default"],
      ["boards"],
      ["tasks", "default"],
      ["stats", "default"],
      ["search-status", "default"],
      ["board-task-map", "default"],
    ])
  })

  it("invalidates run metadata and run log queries for task events with run ids", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_2", run_id: "r_1", kind: "task.claimed" })]),
    ).toEqual(affected({
      taskIds: new Set(["t_2"]),
      taskDetailIds: new Set(["t_2"]),
      taskRunIds: new Set(["t_2"]),
      taskRunLogIds: new Set(["r_1"]),
      taskEventIds: new Set(["t_2"]),
      invalidateBoardTasks: true,
      invalidateStats: true,
      invalidateSearchStatus: true,
      invalidateBoardTaskMap: true,
      invalidateEvents: true,
    }))

    const completed = affectedQueriesForEvents([eventRecord({ task_id: "t_2", run_id: "r_2", kind: "task.completed" })])
    expect(queryKeysForAffectedEvents({ affected: completed, board: "default" })).toEqual([
      ["events", "default"],
      ["tasks", "default"],
      ["stats", "default"],
      ["search-status", "default"],
      ["board-task-map", "default"],
      ["task-detail", "t_2"],
      ["task-runs", "t_2"],
      ["task-run-log", "r_2"],
      ["task-events", "t_2"],
    ])
  })

  it("invalidates the board switcher list for board lifecycle events", () => {
    const affected = affectedQueriesForEvents([eventRecord({ task_id: null, kind: "board.created" })])

    expect(queryKeysForAffectedEvents({ affected, board: "default" })).toEqual([
      ["events", "default"],
      ["boards"],
      ["tasks", "default"],
      ["stats", "default"],
      ["search-status", "default"],
      ["board-task-map", "default"],
    ])
  })

  it("maps event changes to canonical cache keys", () => {
    const affected = affectedQueriesForEvents([
      eventRecord({ task_id: "t_2", kind: "task.completed" }),
      eventRecord({ task_id: "t_3", kind: "task.comment.created" }),
    ])

    expect(queryKeysForAffectedEvents({ affected, board: "default" })).toEqual([
      ["events", "default"],
      ["tasks", "default"],
      ["stats", "default"],
      ["search-status", "default"],
      ["board-task-map", "default"],
      ["task-detail", "t_2"],
      ["task-comments", "t_3"],
      ["task-events", "t_2"],
      ["task-events", "t_3"],
    ])
  })

  it("invalidates only event, search status, and task comment/event keys for task-scoped comment events", () => {
    const affected = affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind: "task.comment.created" })])

    expect(queryKeysForAffectedEvents({ affected, board: "default" })).toEqual([
      ["events", "default"],
      ["search-status", "default"],
      ["task-comments", "t_2"],
      ["task-events", "t_2"],
    ])
  })

  it("keeps dependency changes out of status counter keys while refreshing rows and graph data", () => {
    const affected = affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind: "dependency.added" })])

    expect(queryKeysForAffectedEvents({ affected, board: "default" })).toEqual([
      ["events", "default"],
      ["tasks", "default"],
      ["search-status", "default"],
      ["board-task-map", "default"],
      ["task-dependencies", "t_2"],
      ["task-neighborhood", "t_2"],
      ["task-events", "t_2"],
    ])
  })

  it("keeps heartbeat out of status counter and graph keys", () => {
    const affected = affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind: "task.heartbeat" })])

    expect(queryKeysForAffectedEvents({ affected, board: "default" })).toEqual([
      ["events", "default"],
      ["tasks", "default"],
      ["search-status", "default"],
      ["task-detail", "t_2"],
      ["task-events", "t_2"],
    ])
  })
})

function affected(overrides: Partial<AffectedQueries>): AffectedQueries {
  return {
    taskIds: new Set(),
    taskDetailIds: new Set(),
    taskDependencyIds: new Set(),
    taskNeighborhoodIds: new Set(),
    taskStepIds: new Set(),
    taskRunIds: new Set(),
    taskRunLogIds: new Set(),
    taskCommentIds: new Set(),
    taskEventIds: new Set(),
    invalidateBoardTasks: false,
    invalidateStats: false,
    invalidateSearchStatus: false,
    invalidateBoardTaskMap: false,
    invalidateEvents: false,
    ...overrides,
  }
}

function eventRecord(overrides: Partial<EventRecord> = {}): EventRecord {
  return {
    id: 1,
    event_id: "e_1",
    board_id: "b_1",
    task_id: "t_1",
    run_id: null,
    kind: "task.updated",
    actor: "seed",
    payload: {},
    created_at: 1,
    ...overrides,
  }
}
