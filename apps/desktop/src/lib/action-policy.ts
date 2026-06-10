import type { TaskStatus } from "./api"

export function canCompleteTask(status: TaskStatus) {
  return status === "review" || status === "running"
}

export function completeTaskBody(status: TaskStatus, claimToken: string | null) {
  if (status !== "running") return {}
  return claimToken ? { claim_token: claimToken } : { force: true }
}

export function canBlockTask(status: TaskStatus, claimToken: string | null, blockReason: string) {
  void claimToken
  return status === "running" && blockReason.trim().length > 0
}

export function blockTaskBody(claimToken: string | null, blockReason: string) {
  return claimToken
    ? { claim_token: claimToken, reason: blockReason.trim() }
    : { force: true, reason: blockReason.trim() }
}

export function canArchiveTask(status: TaskStatus) {
  return status !== "archived" && status !== "running"
}

export function requiresForceConfirmation(status: TaskStatus, action: "complete" | "block" | "archive", claimToken: string | null) {
  if (status !== "running") return false
  if (action === "archive") return true
  return !claimToken
}
