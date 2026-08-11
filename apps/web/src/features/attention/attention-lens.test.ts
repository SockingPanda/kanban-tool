import { describe, expect, test } from "vitest"

import { activeAttentionLens, attentionCounts, attentionStatusSelection, attentionTasks } from "./attention-lens"

describe("attention lens", () => {
  const rows = [
    { status: "ready" },
    { status: "running" },
    { status: "ready" },
    { status: "done" },
  ] as const

  test("derives counts from canonical status values", () => {
    expect(attentionCounts(rows)).toEqual({ ready: 2, running: 1, blocked: 0, review: 0 })
    expect(attentionTasks(rows, "ready")).toHaveLength(2)
    expect(attentionTasks(rows, null)).toHaveLength(4)
  })

  test("only treats one attention status as an active lens", () => {
    expect(activeAttentionLens(["ready"])).toBe("ready")
    expect(activeAttentionLens(["ready", "running"])).toBeNull()
    expect(activeAttentionLens([])).toBeNull()
    expect(activeAttentionLens(["done"])).toBeNull()
  })

  test("toggles the canonical status selection without retaining a second lens", () => {
    expect(attentionStatusSelection(["ready"], "ready")).toEqual([])
    expect(attentionStatusSelection(["running"], "ready")).toEqual(["ready"])
    expect(attentionStatusSelection(["ready", "running"], "ready")).toEqual(["ready"])
  })
})
