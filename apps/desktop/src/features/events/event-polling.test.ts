import { describe, expect, it } from "vitest"

import type { EventRecord } from "@/lib/api"

import { planEventPollResult } from "./event-polling"

describe("event polling helpers", () => {
  it("seeds the cursor on the first poll without invalidating historical events", () => {
    const plan = planEventPollResult({
      board: "default",
      currentCursor: 0,
      events: [eventRecord({ id: 40, kind: "task.completed" })],
      meta: { next_after: 41 },
      seeded: false,
    })

    expect(plan).toEqual({
      nextCursor: 41,
      seedOnly: true,
      eventsForCache: [],
      queryKeysToInvalidate: [],
    })
  })

  it("updates the event cache and invalidates affected non-event keys after the cursor is seeded", () => {
    const plan = planEventPollResult({
      board: "default",
      currentCursor: 41,
      events: [eventRecord({ id: 42, kind: "task.comment.created" })],
      meta: { next_after: 43 },
      seeded: true,
    })

    expect(plan.nextCursor).toBe(43)
    expect(plan.seedOnly).toBe(false)
    expect(plan.eventsForCache.map((event) => event.id)).toEqual([42])
    expect(plan.queryKeysToInvalidate).toEqual([
      ["search-status", "default"],
      ["task-comments", "t_1"],
      ["task-events", "t_1"],
    ])
  })

  it("allows a full event refetch when a poll page reaches the limit", () => {
    const plan = planEventPollResult({
      board: "default",
      currentCursor: 1,
      events: [eventRecord({ id: 2 }), eventRecord({ id: 3 })],
      meta: { next_after: 4 },
      seeded: true,
      pollLimit: 2,
    })

    expect(plan.queryKeysToInvalidate.at(-1)).toEqual(["events", "default"])
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
