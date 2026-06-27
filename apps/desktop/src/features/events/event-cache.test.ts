import { describe, expect, it } from "vitest"

import type { EventRecord } from "@/lib/api"

import { mergeBoardEventPage } from "./event-cache"

describe("event cache helpers", () => {
  it("merges incoming poller events into the board event page without duplicates", () => {
    const merged = mergeBoardEventPage(
      {
        events: [eventRecord({ id: 3 }), eventRecord({ id: 2 }), eventRecord({ id: 1 })],
        meta: { next_after: 3 },
      },
      [eventRecord({ id: 4 }), eventRecord({ id: 2, kind: "task.comment.created" })],
      3,
    )

    expect(merged.events.map((event) => event.id)).toEqual([4, 3, 2])
    expect(merged.events.find((event) => event.id === 2)?.kind).toBe("task.comment.created")
    expect(merged.meta).toEqual({ next_after: 3 })
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
