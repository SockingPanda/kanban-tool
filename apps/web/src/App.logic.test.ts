import { describe, expect, test } from "vitest"

import { asCanonicalBoardId } from "./lib/sync/contracts"
import {
  appendExplorerEventBatch,
  coalesceExplorerBoundary,
  EVENT_APPLIED_DEBOUNCE_MS,
  EXPLORER_EVENT_BATCH_OVERFLOW_BOUNDARY,
  explorerEventInvalidation,
  explorerEventInvalidationPlan,
  MAX_PENDING_EXPLORER_EVENTS,
  shouldRecoverExplorerEventBatch,
} from "./App.logic"
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

  test("bounds the 200ms event window before the 150-event projection window", () => {
    expect(EVENT_APPLIED_DEBOUNCE_MS).toBe(200)
    expect(MAX_PENDING_EXPLORER_EVENTS).toBe(150)
    expect(shouldRecoverExplorerEventBatch(MAX_PENDING_EXPLORER_EVENTS - 1)).toBe(false)
    expect(shouldRecoverExplorerEventBatch(MAX_PENDING_EXPLORER_EVENTS)).toBe(true)
    expect(coalesceExplorerBoundary([EXPLORER_EVENT_BATCH_OVERFLOW_BOUNDARY])).toEqual({
      invalidationDelta: 1,
      eventsRefreshDelta: 1,
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
    const comment = explorerEventInvalidation({
      ...event(3),
      task_id: "t_3",
      kind: "task.comment.created",
      payload: { comment_id: "comment-3", kind: "note", author_type: "user", agent_type: null },
    }, asCanonicalBoardId("b_default"))
    expect(comment.board).toBe(false)
    expect(comment.projects).toBe(false)
    expect(comment.inspector).toBe(true)
    expect(comment.runs).toBe(false)
  })

  test("projects board.archived onto the canonical Projects list", () => {
    const archived = explorerEventInvalidation({
      ...event(7),
      kind: "board.archived",
      payload: {},
    }, asCanonicalBoardId("b_default"))

    expect(archived.projects).toBe(true)
    expect(archived.board).toBe(true)
    expect(archived.fullRefetch).toBe(false)
  })

  test("derives dependency parent scope through the generated adapter", () => {
    const plan = explorerEventInvalidationPlan({
      ...event(4),
      task_id: "child-task",
      kind: "dependency.added",
      payload: { parent_task_id: "parent-task" },
    }, asCanonicalBoardId("b_default"))
    expect(plan.fullRefetch).toBe(false)
    expect(plan.targets).toEqual(expect.arrayContaining([
      expect.objectContaining({ root: "task-detail", taskId: "parent-task" }),
      expect.objectContaining({ root: "task-dependencies", taskId: "parent-task" }),
      expect.objectContaining({ root: "task-neighborhood", taskId: "parent-task" }),
    ]))
  })

  test("derives linked step scope without falling back to a global refetch", () => {
    const plan = explorerEventInvalidationPlan({
      ...event(5),
      task_id: "parent-task",
      kind: "task.step.updated",
      payload: { step_id: "step-5", linked_task_id: "linked-task", position: 1, required: true, status: "todo" },
    }, asCanonicalBoardId("b_default"))
    expect(plan.fullRefetch).toBe(false)
    expect(plan.targets).toEqual(expect.arrayContaining([
      expect.objectContaining({ root: "task-neighborhood", taskId: "linked-task" }),
    ]))
  })

  test("derives signal scope while keeping board projections untouched", () => {
    const plan = explorerEventInvalidationPlan({
      ...event(6),
      kind: "signal.recorded",
      payload: { signal_id: "signal-6", observation_id: "observation-6", kind: "quality", status: "open" },
    }, asCanonicalBoardId("b_default"))
    expect(plan.fullRefetch).toBe(false)
    expect(plan.targets).toEqual(expect.arrayContaining([
      expect.objectContaining({ root: "signal", signalId: "signal-6" }),
    ]))
    expect(plan.targets.some((target) => ["columns", "tasks", "stats", "board-task-map"].includes(target.root))).toBe(false)
  })
})
