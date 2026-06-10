import type { KanbanApi, Task, TaskStatus } from "@/lib/api"
import { archiveTaskBody, canSpecifyTask, specifyTaskBody } from "@/lib/action-policy"

export type DragTransitionPlan =
  | {
      ok: true
      action: "specify" | "promote" | "claim" | "complete" | "block" | "unblock" | "archive"
      body: Record<string, unknown>
      confirm?: string
      promptReason?: boolean
      message: string
    }
  | { ok: false; reason: string }

export function planDragTransition(
  task: Task,
  targetStatus: TaskStatus,
  claimToken: string | null,
  blockReason: string,
): DragTransitionPlan {
  if (task.status === targetStatus) return { ok: false, reason: "Already in that column." }

  if (targetStatus === "archived") {
    if (task.status === "archived") return { ok: false, reason: "Already archived." }
    if (task.status === "running") {
      return {
        ok: true,
        action: "archive",
        body: archiveTaskBody(task.status),
        confirm: `Force archive running task #${task.seq}?`,
        message: "Archive requested.",
      }
    }
    return { ok: true, action: "archive", body: archiveTaskBody(task.status), message: "Archive requested." }
  }

  if (task.status === "triage" && targetStatus === "todo") {
    if (!canSpecifyTask(task.status, task.description)) return { ok: false, reason: "Triage tasks need a description before specify." }
    return {
      ok: true,
      action: "specify",
      body: specifyTaskBody(task.description),
      message: "Specify requested.",
    }
  }

  if ((task.status === "todo" || task.status === "scheduled") && targetStatus === "ready") {
    return { ok: true, action: "promote", body: {}, message: "Promote requested." }
  }

  if (task.status === "ready" && targetStatus === "running") {
    return {
      ok: true,
      action: "claim",
      body: { ttl_ms: 300_000, worker_profile: "manual" },
      message: "Claim requested.",
    }
  }

  if (task.status === "running" && targetStatus === "done") {
    return claimToken
      ? {
          ok: true,
          action: "complete",
          body: { claim_token: claimToken },
          message: "Complete requested.",
        }
      : {
          ok: true,
          action: "complete",
          body: { force: true },
          confirm: `Force complete running task #${task.seq} without a claim token?`,
          message: "Force complete requested.",
        }
  }

  if (task.status === "running" && targetStatus === "blocked") {
    const reason = blockReason.trim()
    if (claimToken) {
      return reason
        ? {
            ok: true,
            action: "block",
            body: { claim_token: claimToken, reason },
            message: "Block requested.",
          }
        : { ok: false, reason: "A block reason is required." }
    }
    return {
      ok: true,
      action: "block",
      body: reason ? { force: true, reason } : { force: true },
      confirm: `Force block running task #${task.seq} without a claim token?`,
      promptReason: !reason,
      message: "Force block requested.",
    }
  }

  if (task.status === "blocked" && isExecutableTarget(targetStatus)) {
    return {
      ok: true,
      action: "unblock",
      body: {},
      message: "Unblock requested; the service will recompute the target state.",
    }
  }

  return { ok: false, reason: `${task.status} cannot be dropped on ${targetStatus}.` }
}

export async function executeDragTransition(
  api: KanbanApi,
  task: Task,
  plan: Extract<DragTransitionPlan, { ok: true }>,
) {
  return api.transition(task, plan.action, plan.body)
}

function isExecutableTarget(status: TaskStatus) {
  return status === "todo" || status === "scheduled" || status === "ready" || status === "running"
}
