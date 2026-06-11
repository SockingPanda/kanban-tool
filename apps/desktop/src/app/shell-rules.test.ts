import { describe, expect, it } from "vitest"

import { apiEndpointLabel, shouldShowTaskDetail, shouldShowTaskExplorerToolbar } from "./shell-rules"

const views = ["board", "list", "events", "runs", "maintenance", "health", "settings"] as const

describe("desktop shell display rules", () => {
  it("shows task creation and pagination only on task explorer list surfaces", () => {
    expect(views.filter(shouldShowTaskExplorerToolbar)).toEqual(["board", "list"])
  })

  it("keeps the detail panel on board, list, and runs only", () => {
    expect(views.filter(shouldShowTaskDetail)).toEqual(["board", "list", "runs"])
  })

  it("labels relative API base paths without requiring an absolute URL", () => {
    expect(apiEndpointLabel("")).toBe("same-origin")
    expect(apiEndpointLabel("/api")).toBe("same-origin")
    expect(apiEndpointLabel("http://127.0.0.1:8721")).toBe("8721")
  })
})
