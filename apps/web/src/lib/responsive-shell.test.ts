import { describe, expect, test } from "vitest"

import { shellViewportModeForWidth } from "./responsive-shell"

describe("responsive shell viewport ownership", () => {
  test.each([
    [320, "mobile"],
    [767, "mobile"],
    [768, "tablet"],
    [1023, "tablet"],
    [1024, "desktop"],
    [1440, "desktop"],
  ] as const)("maps %dpx to %s", (width, expected) => {
    expect(shellViewportModeForWidth(width)).toBe(expected)
  })

  test("keeps invalid and below-supported widths in the mobile mode", () => {
    expect(shellViewportModeForWidth(319)).toBe("mobile")
    expect(shellViewportModeForWidth(0)).toBe("mobile")
    expect(shellViewportModeForWidth(Number.NaN)).toBe("mobile")
  })
})
