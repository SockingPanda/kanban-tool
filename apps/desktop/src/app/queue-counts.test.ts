import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"

import { queueCountsFromStats, queueCountsFromTasks } from "./queue-counts"

describe("queue counts", () => {
  it("uses canonical stats counts instead of the current board task window", () => {
    const fallback = queueCountsFromTasks([{ status: "running" }] as Task[])

    expect(
      queueCountsFromStats(
        [
          { status: "ready", count: 4 },
          { status: "running", count: 1 },
          { status: "blocked", count: 6 },
          { status: "done", count: 92 },
        ],
        fallback,
      ),
    ).toEqual({ ready: 4, running: 1, blocked: 6 })
  })

  it("falls back to loaded tasks while stats are not available", () => {
    const fallback = queueCountsFromTasks([{ status: "ready" }, { status: "blocked" }] as Task[])

    expect(queueCountsFromStats(undefined, fallback)).toEqual({ ready: 1, running: 0, blocked: 1 })
  })
})
