import { describe, expect, it } from "vitest"

import type { EventRecord } from "@/lib/api"

import { affectedQueriesForEvents, nextEventCursor } from "./event-invalidation"

describe("event invalidation helpers", () => {
  it("uses event envelope next_after before falling back to row ids", () => {
    expect(nextEventCursor(10, [eventRecord({ id: 12 })], { next_after: 30 })).toBe(30)
    expect(nextEventCursor(10, [eventRecord({ id: 12 }), eventRecord({ id: 14 })], {})).toBe(14)
    expect(nextEventCursor(10, [], {})).toBe(10)
  })

  it("invalidates affected task detail and board task queries without a blind refresh for every event", () => {
    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_1", kind: "task.comment.created" })]),
    ).toEqual({
      taskIds: new Set(["t_1"]),
      invalidateBoardTasks: false,
      invalidateEvents: true,
    })

    expect(
      affectedQueriesForEvents([eventRecord({ task_id: "t_2", kind: "task.completed" })]),
    ).toEqual({
      taskIds: new Set(["t_2"]),
      invalidateBoardTasks: true,
      invalidateEvents: true,
    })

    expect(
      affectedQueriesForEvents([eventRecord({ task_id: null, kind: "board.archived" })]),
    ).toEqual({
      taskIds: new Set<string>(),
      invalidateBoardTasks: true,
      invalidateEvents: true,
    })
  })
})

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
