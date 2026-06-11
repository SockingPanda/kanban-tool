import { describe, expect, it } from "vitest"

import { DESCRIPTION_COLLAPSE_LIMIT, isLongDescription, visibleDescription } from "./description-state"

describe("task description collapse helpers", () => {
  it("uses a default description when the task has no description", () => {
    expect(visibleDescription("", false)).toBe("No description yet.")
  })

  it("leaves short descriptions unchanged", () => {
    expect(isLongDescription("short spec")).toBe(false)
    expect(visibleDescription("short spec", false)).toBe("short spec")
  })

  it("collapses long descriptions until expanded", () => {
    const description = "a".repeat(DESCRIPTION_COLLAPSE_LIMIT + 20)

    expect(isLongDescription(description)).toBe(true)
    expect(visibleDescription(description, false)).toHaveLength(DESCRIPTION_COLLAPSE_LIMIT + 3)
    expect(visibleDescription(description, true)).toBe(description)
  })
})
