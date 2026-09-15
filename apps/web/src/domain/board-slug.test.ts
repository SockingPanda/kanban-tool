import { describe, expect, test } from "vitest"

import {
  assertCanonicalBoardSlug,
  parseCanonicalBoardSlug,
  validateCanonicalBoardSlug,
} from "./board-slug"

describe("canonical board slug", () => {
  test("accepts service-compatible identities", () => {
    const slug = parseCanonicalBoardSlug("alpha-1.team")
    expect(slug).toBe("alpha-1.team")
    expect(validateCanonicalBoardSlug(slug)).toBeNull()
  })

  test("rejects invalid, overlong, and reserved identities", () => {
    for (const value of ["", "Alpha", "_alpha", "alpha/one", "alpha+one", "b_board", "col_tasks", "e_event", "a".repeat(65)]) {
      expect(parseCanonicalBoardSlug(value)).toBeNull()
      expect(validateCanonicalBoardSlug(value)).not.toBeNull()
    }
  })

  test("exposes a typed assertion error instead of coercing a selector", () => {
    try {
      assertCanonicalBoardSlug("selector:active")
      throw new Error("expected canonical slug assertion to fail")
    } catch (error) {
      expect(error).toMatchObject({
        kind: "invalid-canonical-board-slug",
        reason: "invalid-character",
      })
    }
  })
})
