import type { Task } from "@/lib/api"

export type ClaimTokenMap = Record<string, string>

export function reconcileClaimTokenForTask(tokens: ClaimTokenMap, task: Task, actor: string | null): ClaimTokenMap {
  if (isClaimTokenValidForTask(task, actor)) return tokens
  if (!(task.id in tokens)) return tokens
  const next = { ...tokens }
  delete next[task.id]
  return next
}

export function reconcileClaimTokensForTasks(tokens: ClaimTokenMap, tasks: Task[], actor: string | null): ClaimTokenMap {
  let next = tokens
  for (const task of tasks) {
    next = reconcileClaimTokenForTask(next, task, actor)
  }
  return next
}

function isClaimTokenValidForTask(task: Task, actor: string | null) {
  return Boolean(actor && task.status === "running" && task.claim_owner === actor)
}
