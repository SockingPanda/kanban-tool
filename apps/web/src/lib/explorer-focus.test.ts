import { describe, expect, test, vi } from "vitest"

import { restoreExplorerFocus, taskOpenerSelector, type ExplorerFocusElement } from "./explorer-focus"

function element(overrides: Partial<ExplorerFocusElement> = {}): ExplorerFocusElement & { readonly focused: ReturnType<typeof vi.fn> } {
  const focused = vi.fn()
  return { isConnected: true, disabled: false, focus: focused, ...overrides, focused }
}

describe("Explorer Inspector focus return", () => {
  test("restores a connected task opener after closing the inspector", () => {
    const opener = element()
    const documentLike = { querySelector: vi.fn() }

    expect(restoreExplorerFocus({ taskId: "t_1", element: opener }, documentLike)).toBe("opener")
    expect(opener.focused).toHaveBeenCalledTimes(1)
    expect(documentLike.querySelector).not.toHaveBeenCalled()
  })

  test("falls back to the Explorer heading when the opener was unmounted", () => {
    const fallback = element()
    const documentLike = { querySelector: vi.fn((selector: string) => selector === "[data-explorer-focus-fallback]" ? fallback : null) }

    expect(restoreExplorerFocus({ taskId: "t_1", element: element({ isConnected: false }) }, documentLike)).toBe("fallback")
    expect(fallback.focused).toHaveBeenCalledTimes(1)
    expect(taskOpenerSelector("t_1")).toBe('[data-task-opener="t_1"]')
  })

  test("re-finds an opener when its view remounts during the transition", () => {
    const remounted = element()
    const documentLike = { querySelector: vi.fn((selector: string) => selector === taskOpenerSelector("t_1") ? remounted : null) }

    expect(restoreExplorerFocus({ taskId: "t_1", element: element({ isConnected: false }) }, documentLike)).toBe("opener")
    expect(remounted.focused).toHaveBeenCalledTimes(1)
    expect(documentLike.querySelector).toHaveBeenCalledWith(taskOpenerSelector("t_1"))
  })
})
