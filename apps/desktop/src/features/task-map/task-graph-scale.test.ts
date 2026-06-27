import { describe, expect, it } from "vitest"

import { clampTaskGraphScale } from "./task-graph-scale"

describe("clampTaskGraphScale", () => {
  it("normalizes non-finite values before clamping", () => {
    expect(clampTaskGraphScale(Number.NaN)).toBe(1)
    expect(clampTaskGraphScale(Number.POSITIVE_INFINITY)).toBe(1)
    expect(clampTaskGraphScale(Number.NEGATIVE_INFINITY)).toBe(1)
  })

  it("bounds finite scale values", () => {
    expect(clampTaskGraphScale(0)).toBe(0.65)
    expect(clampTaskGraphScale(1)).toBe(1)
    expect(clampTaskGraphScale(2)).toBe(1.5)
  })
})
