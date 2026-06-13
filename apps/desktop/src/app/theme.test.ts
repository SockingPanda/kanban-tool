import { describe, expect, it, vi } from "vitest"

import { applyRootTheme, effectiveTheme, nextThemeMode, parseThemeMode } from "./theme"

describe("theme helpers", () => {
  it("parses persisted modes and falls back to system", () => {
    expect(parseThemeMode("light")).toBe("light")
    expect(parseThemeMode("dark")).toBe("dark")
    expect(parseThemeMode("system")).toBe("system")
    expect(parseThemeMode("sepia")).toBe("system")
    expect(parseThemeMode(null)).toBe("system")
  })

  it("resolves system mode from the current system preference", () => {
    expect(effectiveTheme("system", true)).toBe("dark")
    expect(effectiveTheme("system", false)).toBe("light")
    expect(effectiveTheme("dark", false)).toBe("dark")
    expect(effectiveTheme("light", true)).toBe("light")
  })

  it("toggles the dark class on the document root", () => {
    const root = { add: vi.fn(), remove: vi.fn() }

    applyRootTheme(root, "dark")
    applyRootTheme(root, "light")

    expect(root.add).toHaveBeenCalledWith("dark")
    expect(root.remove).toHaveBeenCalledWith("dark")
  })

  it("cycles system, light, dark", () => {
    expect(nextThemeMode("system")).toBe("light")
    expect(nextThemeMode("light")).toBe("dark")
    expect(nextThemeMode("dark")).toBe("system")
  })
})
