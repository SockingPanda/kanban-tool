import type { TaskStatus } from "./api"

export function canCompleteTask(status: TaskStatus, claimToken: string | null) {
  return status === "review" || (status === "running" && Boolean(claimToken))
}

export function completeTaskBody(status: TaskStatus, claimToken: string | null) {
  if (status !== "running") return {}
  return claimToken ? { claim_token: claimToken } : {}
}

export function canBlockTask(status: TaskStatus, claimToken: string | null, blockReason: string) {
  return status === "running" && Boolean(claimToken) && blockReason.trim().length > 0
}

export function blockTaskBody(claimToken: string | null, blockReason: string) {
  return claimToken ? { claim_token: claimToken, reason: blockReason.trim() } : { reason: blockReason.trim() }
}

export function canArchiveTask(status: TaskStatus) {
  return status !== "archived" && status !== "running"
}
