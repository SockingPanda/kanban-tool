import { describe, expect, it } from "vitest"

import {
  blockTaskBody,
  canArchiveTask,
  canBlockTask,
  canCompleteTask,
  completeTaskBody,
} from "./action-policy"

describe("task action policy", () => {
  it("does not force-mutate running tasks without a claim token", () => {
    expect(canCompleteTask("running", null)).toBe(false)
    expect(canBlockTask("running", null, "waiting")).toBe(false)
    expect(canArchiveTask("running")).toBe(false)
    expect(completeTaskBody("running", null)).not.toHaveProperty("force")
    expect(blockTaskBody(null, "waiting")).not.toHaveProperty("force")
  })

  it("allows token-bound running transitions", () => {
    expect(canCompleteTask("running", "claim_123")).toBe(true)
    expect(canBlockTask("running", "claim_123", "waiting")).toBe(true)
    expect(completeTaskBody("running", "claim_123")).toEqual({ claim_token: "claim_123" })
    expect(blockTaskBody("claim_123", " waiting ")).toEqual({
      claim_token: "claim_123",
      reason: "waiting",
    })
  })

  it("allows review completion without a claim token", () => {
    expect(canCompleteTask("review", null)).toBe(true)
    expect(completeTaskBody("review", null)).toEqual({})
  })
})
