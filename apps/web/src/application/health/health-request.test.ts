import { describe, expect, test } from "vitest"

import { isCurrentHealthRequest } from "./health-request"

describe("health request identity fence", () => {
  test("rejects stale success after replacement or abort", () => {
    const first = new AbortController()
    const second = new AbortController()

    expect(isCurrentHealthRequest(first, first)).toBe(true)
    expect(isCurrentHealthRequest(first, second)).toBe(false)

    first.abort()
    expect(isCurrentHealthRequest(first, first)).toBe(false)
    expect(isCurrentHealthRequest(second, second)).toBe(true)
  })
})
