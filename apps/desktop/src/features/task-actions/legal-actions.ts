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
  blockTaskBody,
  canArchiveTask,
  canBlockTask,
  canCompleteTask,
  completeTaskBody,
} from "@/lib/action-policy"

export type LegalTaskAction = {
  label: string
  icon: LucideIcon
  enabled: boolean
  danger?: boolean
  run: (api: KanbanApi, task: Task) => Promise<unknown>
}

export function legalActions(task: Task, claimToken: string | null, blockReason: string): LegalTaskAction[] {
  return [
    {
      label: "Specify",
      icon: ListChecks,
      enabled: task.status === "triage",
      run: (api, item) => api.transition(item, "specify", { description: item.description ?? "ready spec" }),
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
      enabled: canCompleteTask(task.status, claimToken),
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
      danger: true,
      run: (api, item) => api.transition(item, "block", blockTaskBody(claimToken, blockReason)),
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
      danger: true,
      run: (api, item) => api.transition(item, "archive"),
    },
  ]
}
