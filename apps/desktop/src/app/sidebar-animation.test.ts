import { describe, expect, it } from "vitest"

import { nextSidebarContentOpen } from "./sidebar-animation"

describe("sidebar animation state", () => {
  it("keeps internal content visible while the sidebar collapses", () => {
    expect(nextSidebarContentOpen(true, { type: "width-transition-start", sidebarOpen: false })).toBe(true)
  })

  it("opens internal content as soon as the sidebar starts expanding", () => {
    expect(nextSidebarContentOpen(false, { type: "width-transition-start", sidebarOpen: true })).toBe(true)
  })

  it("matches internal content to the sidebar target after the width transition finishes", () => {
    expect(nextSidebarContentOpen(true, { type: "width-transition-finish", sidebarOpen: false })).toBe(false)
    expect(nextSidebarContentOpen(false, { type: "width-transition-finish", sidebarOpen: true })).toBe(true)
  })
})
