import { describe, expect, test } from "vitest"

import { retryIntentWithCurrentDialog, type MutationDialog, type RetryIntent } from "./task-mutation-controller"

const transitionOption = {
  action: "block" as const,
  targetStatus: "blocked" as const,
  requiresReason: true,
  requiresDescription: false,
  requiresConfirmation: false,
}

describe("retry intent rebasing", () => {
  test("uses current edit title", () => {
    const retry: RetryIntent = { kind: "edit", taskId: "t_1", title: "stale title" }
    const dialog: MutationDialog = { kind: "edit", taskId: "t_1", title: "current title" }

    expect(retryIntentWithCurrentDialog(retry, dialog)).toEqual({ ...retry, title: "current title" })
  })

  test("uses current transition reason, description, and confirmation", () => {
    const retry: RetryIntent = {
      kind: "transition",
      taskId: "t_1",
      option: transitionOption,
      reason: "stale reason",
      description: "stale description",
      confirmed: false,
    }
    const dialog: MutationDialog = {
      kind: "transition",
      taskId: "t_1",
      option: transitionOption,
      reason: "current reason",
      description: "current description",
      confirmed: true,
    }

    expect(retryIntentWithCurrentDialog(retry, dialog)).toEqual({
      ...retry,
      reason: "current reason",
      description: "current description",
      confirmed: true,
    })
  })
})
