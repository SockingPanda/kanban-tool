import {
  Archive,
  CheckCircle2,
  HeartPulse,
  ListChecks,
  PauseCircle,
  Play,
  RefreshCcw,
  XCircle,
  type LucideIcon,
} from "lucide-react"

import type { KanbanApi, Task } from "@/lib/api"
import {
  archiveTaskBody,
  blockTaskBody,
  canArchiveTask,
  canBlockTask,
  canCompleteTask,
  canSpecifyTask,
  completeTaskBody,
  requiresForceConfirmation,
  specifyTaskBody,
} from "@/lib/action-policy"
import type { I18nMessage } from "@/i18n"

export type LegalTaskAction = {
  label: string
  icon: LucideIcon
  enabled: boolean
  danger?: boolean
  confirmation?: I18nMessage
  run: (api: KanbanApi, task: Task) => Promise<unknown>
}

export function legalActions(task: Task, claimToken: string | null, blockReason: string): LegalTaskAction[] {
  return [
    {
      label: "Specify",
      icon: ListChecks,
      enabled: canSpecifyTask(task.status, task.description),
      run: (api, item) => api.transition(item, "specify", specifyTaskBody(item.description)),
    },
    {
      label: "Promote",
      icon: Play,
      enabled: task.status === "todo" || task.status === "scheduled",
      run: (api, item) => api.transition(item, "promote"),
    },
    {
      label: "Claim",
      icon: Play,
      enabled: task.status === "ready",
      run: (api, item) => api.transition(item, "claim", { ttl_ms: 300_000, worker_profile: "manual" }),
    },
    {
      label: "Heartbeat",
      icon: HeartPulse,
      enabled: task.status === "running" && Boolean(claimToken),
      run: (api, item) => api.transition(item, "heartbeat", { claim_token: claimToken, ttl_ms: 300_000 }),
    },
    {
      label: "Complete",
      icon: CheckCircle2,
      enabled: canCompleteTask(task.status),
      confirmation: requiresForceConfirmation(task.status, "complete", claimToken)
        ? message("Force complete running task #{seq} without a claim token?", { seq: task.seq })
        : undefined,
      run: (api, item) => api.transition(item, "complete", completeTaskBody(item.status, claimToken)),
    },
    {
      label: "Review",
      icon: PauseCircle,
      enabled: task.status === "running" && Boolean(claimToken),
      run: (api, item) => api.transition(item, "submit-review", { claim_token: claimToken }),
    },
    {
      label: "Block",
      icon: XCircle,
      enabled: canBlockTask(task.status, claimToken, blockReason),
      confirmation: requiresForceConfirmation(task.status, "block", claimToken)
        ? message("Force block running task #{seq} without a claim token?", { seq: task.seq })
        : undefined,
      danger: true,
      run: (api, item) => api.transition(item, "block", blockTaskBody(item.status, claimToken, blockReason)),
    },
    {
      label: "Unblock",
      icon: RefreshCcw,
      enabled: task.status === "blocked",
      run: (api, item) => api.transition(item, "unblock"),
    },
    {
      label: "Archive",
      icon: Archive,
      enabled: canArchiveTask(task.status),
      confirmation: requiresForceConfirmation(task.status, "archive", claimToken)
        ? message("Force archive running task #{seq}?", { seq: task.seq })
        : undefined,
      danger: true,
      run: (api, item) => api.transition(item, "archive", archiveTaskBody(item.status)),
    },
  ]
}

function message(key: string, values?: Record<string, string | number>): I18nMessage {
  return values ? { key, values } : { key }
}
