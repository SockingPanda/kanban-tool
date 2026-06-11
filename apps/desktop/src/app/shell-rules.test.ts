import { describe, expect, it } from "vitest"

import { apiEndpointLabel, shouldShowTaskExplorerToolbar } from "./shell-rules"

const views = ["board", "list", "events", "runs", "maintenance", "health", "settings"] as const

describe("desktop shell display rules", () => {
  it("shows task creation and pagination only on task explorer list surfaces", () => {
    expect(views.filter(shouldShowTaskExplorerToolbar)).toEqual(["board", "list"])
  })

  it("labels relative API base paths without requiring an absolute URL", () => {
    expect(apiEndpointLabel("")).toBe("same-origin")
    expect(apiEndpointLabel("/api")).toBe("same-origin")
    expect(apiEndpointLabel("http://127.0.0.1:8721")).toBe("8721")
  })
})
