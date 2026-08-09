import { afterEach, describe, expect, test, vi } from "vitest"

import { HEALTH_REFRESH_EVENT, requestHealthRefresh } from "./health-refresh"

describe("health refresh seam", () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  test("dispatches a browser event while remaining SSR-safe", () => {
    const dispatchEvent = vi.fn()
    vi.stubGlobal("window", { dispatchEvent })

    requestHealthRefresh()

    expect(dispatchEvent).toHaveBeenCalledTimes(1)
    expect(dispatchEvent.mock.calls[0]?.[0]).toMatchObject({ type: HEALTH_REFRESH_EVENT })
  })
})
