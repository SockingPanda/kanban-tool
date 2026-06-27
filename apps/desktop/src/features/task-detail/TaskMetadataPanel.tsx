import type { Task } from "@/lib/api"
import { formatRelativeTime } from "@/lib/utils"

import { InfoRow, Section } from "./task-detail-shared"

export function TaskMetadataPanel({ task }: { task: Task }) {
  return (
    <Section title="Metadata">
      <div className="space-y-2 text-sm">
        <InfoRow label="ref" value={task.ref} />
        <InfoRow label="status" value={task.status} />
        <InfoRow label="assignee" value={task.assignee ?? "-"} />
        <InfoRow label="plan" value={task.execution_plan_state} />
        <InfoRow label="created" value={formatRelativeTime(task.created_at)} />
        <InfoRow label="updated" value={formatRelativeTime(task.updated_at)} />
      </div>
    </Section>
  )
}
