import { describe, expect, test } from "vitest"

import { asCanonicalBoardId } from "./lib/sync/contracts"
import { appendExplorerEventBatch, coalesceExplorerBoundary, explorerEventInvalidation } from "./App.logic"
import type { ExplorerEvent } from "./lib/api/explorer-read-model"

const event = (id: number, eventId = `event-${id}`): ExplorerEvent => ({
  id,
  event_id: eventId,
  board_id: "b_default",
  task_id: null,
  run_id: null,
  kind: "task.updated",
  actor: "tester",
  payload: {},
  created_at: id,
})

describe("Explorer invalidation boundary", () => {
  test("coalesces one poll boundary and completion into one refresh", () => {
    expect(coalesceExplorerBoundary(["poll-boundary-complete", "poll-complete"])).toEqual({
      invalidationDelta: 1,
      eventsRefreshDelta: 1,
    })
  })

  test("coalesces transport/recovery failures into one degraded refresh", () => {
    expect(coalesceExplorerBoundary(["transport-failure", "recovery-failure", "poll-failure"])).toEqual({
      invalidationDelta: 1,
      eventsRefreshDelta: 1,
    })
  })

  test("ignores normal event telemetry because events append through batch", () => {
    expect(coalesceExplorerBoundary(["event-applied"])).toEqual({
      invalidationDelta: 0,
      eventsRefreshDelta: 0,
    })
  })

  test("deduplicates a burst by id and event_id while returning ASC order", () => {
    const merged = appendExplorerEventBatch([event(2)], [event(4), event(1), event(2), event(3)], asCanonicalBoardId("b_default"))
    expect(merged.map((value) => value.id)).toEqual([1, 2, 3, 4])
  })

  test("rejects duplicate id/event_id with a changed full fingerprint", () => {
    expect(() => appendExplorerEventBatch([event(2)], [{ ...event(2), payload: { changed: true } }], asCanonicalBoardId("b_default"))).toThrow(/fingerprint/)
  })

  test("projects event kinds to mounted Explorer targets", () => {
    const comment = explorerEventInvalidation({ ...event(3), task_id: "t_3", kind: "task.comment.created" }, asCanonicalBoardId("b_default"))
    expect(comment.board).toBe(false)
    expect(comment.inspector).toBe(true)
    expect(comment.runs).toBe(false)
  })
})
