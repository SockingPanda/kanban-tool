import { describe, expect, it } from "vitest"

import { hasNextPage, hasPreviousPage, lastPageOffset, pageRangeLabel } from "./pagination"

describe("pagination helpers", () => {
  it("enables next page for unknown totals only when the current page is full", () => {
    expect(hasNextPage({ limit: 100, offset: 0, total: null }, 100)).toBe(true)
    expect(hasNextPage({ limit: 100, offset: 0, total: null }, 12)).toBe(false)
  })

  it("uses exact totals when the backend provides them", () => {
    expect(hasNextPage({ limit: 100, offset: 100, total: 250 }, 100)).toBe(true)
    expect(hasNextPage({ limit: 100, offset: 200, total: 250 }, 50)).toBe(false)
  })

  it("tracks previous page availability from the offset", () => {
    expect(hasPreviousPage({ limit: 50, offset: 0, total: 120 })).toBe(false)
    expect(hasPreviousPage({ limit: 50, offset: 50, total: 120 })).toBe(true)
  })

  it("computes the last page offset only when totals are exact", () => {
    expect(lastPageOffset({ limit: 50, offset: 0, total: 120 })).toBe(100)
    expect(lastPageOffset({ limit: 50, offset: 0, total: 50 })).toBe(0)
    expect(lastPageOffset({ limit: 50, offset: 0, total: 0 })).toBe(null)
    expect(lastPageOffset({ limit: 50, offset: 0, total: null })).toBe(null)
  })

  it("labels unknown totals without inventing an exact count", () => {
    expect(pageRangeLabel({ limit: 100, offset: 100, total: null }, 100)).toBe("showing 101-200 of at least 200")
    expect(pageRangeLabel({ limit: 100, offset: 0, total: 12 }, 12)).toBe("showing 1-12 of 12")
  })
})
