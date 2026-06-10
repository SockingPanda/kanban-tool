import { describe, expect, it } from "vitest"

import {
  blockTaskBody,
  canArchiveTask,
  canBlockTask,
  canCompleteTask,
  completeTaskBody,
  requiresForceConfirmation,
} from "./action-policy"

describe("task action policy", () => {
  it("allows explicit force bodies for running tasks without a claim token", () => {
    expect(canCompleteTask("running")).toBe(true)
    expect(canBlockTask("running", null, "waiting")).toBe(true)
    expect(canArchiveTask("running")).toBe(false)
    expect(completeTaskBody("running", null)).toEqual({ force: true })
    expect(blockTaskBody(null, "waiting")).toEqual({ force: true, reason: "waiting" })
    expect(requiresForceConfirmation("running", "complete", null)).toBe(true)
    expect(requiresForceConfirmation("running", "block", null)).toBe(true)
  })

  it("allows token-bound running transitions", () => {
    expect(canCompleteTask("running")).toBe(true)
    expect(canBlockTask("running", "claim_123", "waiting")).toBe(true)
    expect(completeTaskBody("running", "claim_123")).toEqual({ claim_token: "claim_123" })
    expect(blockTaskBody("claim_123", " waiting ")).toEqual({
      claim_token: "claim_123",
      reason: "waiting",
    })
    expect(requiresForceConfirmation("running", "complete", "claim_123")).toBe(false)
  })

  it("allows review completion without a claim token", () => {
    expect(canCompleteTask("review")).toBe(true)
    expect(completeTaskBody("review", null)).toEqual({})
  })
})
