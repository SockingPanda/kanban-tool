import type { TaskStatus } from "./api"

export function canSpecifyTask(status: TaskStatus, description: string | null) {
  return status === "triage" && Boolean(description?.trim())
}

export function specifyTaskBody(description: string | null) {
  return { description: description?.trim() ?? "" }
}

export function canCompleteTask(status: TaskStatus) {
  return status === "review" || status === "running"
}

export function completeTaskBody(status: TaskStatus, claimToken: string | null) {
  if (status !== "running") return {}
  return claimToken ? { claim_token: claimToken } : { force: true }
}

export function canBlockTask(status: TaskStatus, claimToken: string | null, blockReason: string) {
  void claimToken
  return isBlockableStatus(status) && blockReason.trim().length > 0
}

export function blockTaskBody(status: TaskStatus, claimToken: string | null, blockReason: string) {
  const reason = blockReason.trim()
  if (status !== "running") return { reason }
  return claimToken ? { claim_token: claimToken, reason } : { force: true, reason }
}

export function canArchiveTask(status: TaskStatus) {
  return status !== "archived"
}

export function archiveTaskBody(status: TaskStatus) {
  return status === "running" ? { force: true } : {}
}

export function requiresForceConfirmation(status: TaskStatus, action: "complete" | "block" | "archive", claimToken: string | null) {
  if (status !== "running") return false
  if (action === "archive") return true
  return !claimToken
}

export function isBlockableStatus(status: TaskStatus) {
  return (
    status === "triage" ||
    status === "todo" ||
    status === "scheduled" ||
    status === "ready" ||
    status === "running" ||
    status === "review"
  )
}
