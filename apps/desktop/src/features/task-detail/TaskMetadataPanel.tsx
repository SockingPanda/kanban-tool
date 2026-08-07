import { useI18n } from "@/i18n"
import type { Task } from "@/lib/api"
import { formatRelativeTime } from "@/lib/utils"

import { InfoRow, Section } from "./task-detail-shared"

export function TaskMetadataPanel({ task }: { task: Task }) {
  const { t } = useI18n()
  return (
    <Section title={t("Metadata")}>
      <div className="space-y-2 text-sm">
        <InfoRow label={t("Ref")} value={task.ref} />
        <InfoRow label={t("status")} value={t(task.status)} />
        <InfoRow label={t("assignee")} value={task.assignee ?? "-"} />
        <InfoRow label={t("plan")} value={t(task.execution_plan_state)} />
        <InfoRow label={t("created")} value={formatRelativeTime(task.created_at)} />
        <InfoRow label={t("updated")} value={formatRelativeTime(task.updated_at)} />
      </div>
    </Section>
  )
}
