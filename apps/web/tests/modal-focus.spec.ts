import { expect, test } from "@playwright/test"

import { modalFocusableSelector } from "../src/features/explorer/modal-focus"

test("keeps a closed details summary and excludes hidden descendants from modal focus candidates", async ({ page }) => {
  await page.setContent(`
    <main id="dialog">
      <details>
        <summary id="closed-summary">Open details</summary>
        <button id="closed-button">Hidden while closed</button>
      </details>
      <div hidden>
        <button id="hidden-button">Hidden ancestor</button>
      </div>
      <button id="visible-button">Visible action</button>
    </main>
  `)

  const candidates = await page.locator("#dialog").evaluate((root, selector) => {
    const selected = Array.from(root.querySelectorAll<HTMLElement>(selector)).map((element) => element.id)
    const focusable = Array.from(root.querySelectorAll<HTMLElement>(selector))
      .filter((element) => {
        const closedDetails = element.closest("details:not([open])")
        if (closedDetails !== null && element.tagName !== "SUMMARY") return false
        return element.closest("[hidden], [aria-hidden='true'], [inert]") === null
      })
      .map((element) => element.id)
    return { selected, focusable }
  }, modalFocusableSelector)

  expect(candidates.selected).toEqual(["closed-summary", "closed-button", "hidden-button", "visible-button"])
  expect(candidates.focusable).toEqual(["closed-summary", "visible-button"])
})
