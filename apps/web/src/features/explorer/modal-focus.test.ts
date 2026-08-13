import { describe, expect, test } from "vitest"

import { modalFocusableElements } from "./modal-focus"

type FakeElement = {
  readonly tagName: string
  readonly closest: (selector: string) => object | null
}

function fakeElement(tagName: string, ancestor: "closed-details" | "aria-hidden" | null): FakeElement {
  return {
    tagName,
    closest: (selector) => {
      if (ancestor === "closed-details" && selector === "details:not([open])") return {}
      if (ancestor === "aria-hidden" && selector === "[hidden], [aria-hidden='true'], [inert]") return {}
      return null
    },
  }
}

describe("Inspector modal focus candidates", () => {
  test("ignores hidden details descendants while retaining their summary", () => {
    const summary = fakeElement("SUMMARY", "closed-details")
    const hiddenButton = fakeElement("BUTTON", "closed-details")
    const visibleButton = fakeElement("BUTTON", null)
    const root = { querySelectorAll: () => [summary, hiddenButton, visibleButton] } as unknown as ParentNode

    expect(modalFocusableElements(root)).toEqual([summary, visibleButton])
  })

  test("ignores explicitly hidden focus candidates", () => {
    const hiddenButton = fakeElement("BUTTON", "aria-hidden")
    const visibleButton = fakeElement("BUTTON", null)
    const root = { querySelectorAll: () => [hiddenButton, visibleButton] } as unknown as ParentNode

    expect(modalFocusableElements(root)).toEqual([visibleButton])
  })
})
