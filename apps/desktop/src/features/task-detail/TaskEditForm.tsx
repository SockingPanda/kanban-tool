import { Save, X } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Field } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { MenuSelect } from "@/components/ui/menu-select"
import { useI18n } from "@/i18n"
import type { KanbanApi } from "@/lib/api"

import { AutosizeDescriptionTextarea, priorityOptions, Section } from "./task-detail-shared"
import type { TaskEditDraft } from "./task-draft"

export function TaskEditForm({
  api,
  editDraft,
  draftDirty,
  pendingAction,
  setEditDraft,
  onSave,
  onCancel,
}: {
  api: KanbanApi | null
  editDraft: TaskEditDraft
  draftDirty: boolean
  pendingAction: string | null
  setEditDraft: (value: TaskEditDraft) => void
  onSave: () => void
  onCancel: () => void
}) {
  const { t } = useI18n()
  return (
    <Section title={t("Task detail")}>
      <div className="max-w-3xl space-y-2">
        {draftDirty ? <div className="text-xs font-medium text-amber-700">{t("Unsaved changes")}</div> : null}
        <Input
          aria-label={t("Task title")}
          name="task-title"
          autoComplete="off"
          value={editDraft.title}
          onChange={(event) => setEditDraft({ ...editDraft, title: event.target.value })}
        />
        <AutosizeDescriptionTextarea
          value={editDraft.description}
          onChange={(value) => setEditDraft({ ...editDraft, description: value })}
          placeholder={t("Description")}
        />
        <div className="grid grid-cols-2 gap-2 max-sm:grid-cols-1">
          <Input
            aria-label={t("Task assignee")}
            name="task-assignee"
            autoComplete="off"
            value={editDraft.assignee}
            onChange={(event) => setEditDraft({ ...editDraft, assignee: event.target.value })}
            placeholder={t("Assignee")}
          />
          <MenuSelect
            ariaLabel={t("Task priority")}
            options={priorityOptions}
            value={editDraft.priority}
            onValueChange={(priority) => setEditDraft({ ...editDraft, priority })}
            triggerClassName="h-10 w-full"
          />
          <Input
            type="datetime-local"
            aria-label={t("Scheduled at")}
            name="task-scheduled-at"
            autoComplete="off"
            value={editDraft.scheduledAt}
            onChange={(event) => setEditDraft({ ...editDraft, scheduledAt: event.target.value })}
          />
          <Input
            type="datetime-local"
            aria-label={t("Due at")}
            name="task-due-at"
            autoComplete="off"
            value={editDraft.dueAt}
            onChange={(event) => setEditDraft({ ...editDraft, dueAt: event.target.value })}
          />
        </div>
        <Field className="flex flex-wrap gap-2">
          <Button disabled={!api || pendingAction === "save" || !editDraft.title.trim()} onClick={onSave}>
            <Save className="h-4 w-4" />
            {pendingAction === "save" ? t("Saving…") : t("Save")}
          </Button>
          <Button variant="outline" disabled={pendingAction === "save"} onClick={onCancel}>
            <X className="h-4 w-4" />
            {t("Cancel")}
          </Button>
        </Field>
      </div>
    </Section>
  )
}
