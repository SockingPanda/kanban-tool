import { describe, expect, it } from "vitest"

import { nextSidebarContentOpen } from "./sidebar-animation"

describe("sidebar animation state", () => {
  it("keeps internal content in its previous state while width is animating", () => {
    expect(nextSidebarContentOpen(true, { type: "width-transition-start", sidebarOpen: false })).toBe(true)
    expect(nextSidebarContentOpen(false, { type: "width-transition-start", sidebarOpen: true })).toBe(false)
  })

  it("matches internal content to the sidebar target after the width transition finishes", () => {
    expect(nextSidebarContentOpen(true, { type: "width-transition-finish", sidebarOpen: false })).toBe(false)
    expect(nextSidebarContentOpen(false, { type: "width-transition-finish", sidebarOpen: true })).toBe(true)
  })
})
