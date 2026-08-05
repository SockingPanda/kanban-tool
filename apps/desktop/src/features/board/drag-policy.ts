import type { KanbanApi, Task, TaskStatus } from "@/lib/api"
import { isBlockableStatus } from "@/lib/action-policy"
import type { I18nMessage } from "@/i18n"

export type DragTransitionPlan =
  | {
      ok: true
      action: "promote" | "claim" | "complete" | "submit-review" | "block"
      body: Record<string, unknown>
      confirm?: I18nMessage
      promptReason?: boolean
      message: I18nMessage
    }
  | { ok: false; reason: I18nMessage }

export function planDragTransition(
  task: Task,
  targetStatus: TaskStatus,
  claimToken: string | null,
): DragTransitionPlan {
  if (task.status === targetStatus) return { ok: false, reason: message("Already in that column.") }

  if ((task.status === "todo" || task.status === "scheduled") && targetStatus === "ready") {
    return { ok: true, action: "promote", body: {}, message: message("Promote requested.") }
  }

  if (task.status === "ready" && targetStatus === "running") {
    return {
      ok: true,
      action: "claim",
      body: { ttl_ms: 300_000, worker_profile: "manual" },
      message: message("Claim requested."),
    }
  }

  if (task.status === "running" && targetStatus === "done") {
    return claimToken
      ? {
          ok: true,
          action: "complete",
          body: { claim_token: claimToken },
          message: message("Complete requested."),
        }
      : {
          ok: true,
          action: "complete",
          body: { force: true },
          confirm: message("Force complete running task #{seq} without a claim token?", { seq: task.seq }),
          message: message("Force complete requested."),
        }
  }

  if (task.status === "running" && targetStatus === "review") {
    if (!claimToken) return { ok: false, reason: message("Submit for review requires a claim token.") }
    return {
      ok: true,
      action: "submit-review",
      body: { claim_token: claimToken },
      message: message("Submit for review requested."),
    }
  }

  if (task.status === "review" && targetStatus === "done") {
    return {
      ok: true,
      action: "complete",
      body: {},
      message: message("Complete requested."),
    }
  }

  if (task.status === "running" && targetStatus === "blocked") {
    if (claimToken) {
      return {
        ok: true,
        action: "block",
        body: { claim_token: claimToken },
        promptReason: true,
        message: message("Block requested."),
      }
    }
    return {
      ok: true,
      action: "block",
      body: { force: true },
      confirm: message("Force block running task #{seq} without a claim token?", { seq: task.seq }),
      promptReason: true,
      message: message("Force block requested."),
    }
  }

  if (targetStatus === "blocked" && isBlockableStatus(task.status)) {
    return {
      ok: true,
      action: "block",
      body: {},
      promptReason: true,
      message: message("Block requested."),
    }
  }

  return { ok: false, reason: message("{status} cannot be dropped on {targetStatus}.", { status: task.status, targetStatus }) }
}

export async function executeDragTransition(
  api: KanbanApi,
  task: Task,
  plan: Extract<DragTransitionPlan, { ok: true }>,
) {
  return api.transition(task, plan.action, plan.body)
}

function message(key: string, values?: Record<string, string | number>): I18nMessage {
  return values ? { key, values } : { key }
}
