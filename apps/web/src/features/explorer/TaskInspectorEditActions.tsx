import { useEffect, useRef, type FormEvent, type ReactNode } from "react"

import type { Locale } from "../../lib/preferences"
import styles from "./TaskInspector.module.css"
import {
  actionLabel,
  inspectorActionViews,
  type InspectorActionId,
  type InspectorActionView,
  type InspectorEditDraft,
} from "./TaskInspector.edit-actions"
import type { InspectorCopy, TaskInspectorViewModel } from "./TaskInspector"
import type { TaskInspectorMutationError } from "./task-inspector-mutation-state"

function PanelSection({ id, title, children }: { readonly id: string; readonly title: string; readonly children: ReactNode }) {
  return (
    <section className={styles.section} id={id} data-testid={id}>
      <h2>{title}</h2>
      {children}
    </section>
  )
}

export function TaskInspectorEditForm({
  draft,
  dirty,
  pending,
  error,
  copy,
  onChange,
  onSave,
  onCancel,
}: {
  readonly draft: InspectorEditDraft
  readonly dirty: boolean
  readonly pending: boolean
  readonly error: string | null
  readonly copy: InspectorCopy
  readonly onChange: (draft: InspectorEditDraft) => void
  readonly onSave: (event: FormEvent<HTMLFormElement>) => void
  readonly onCancel: () => void
}) {
  return (
    <PanelSection id="inspector-edit" title={copy.edit}>
      <form className={styles.editor} data-testid="inspector-edit-form" onSubmit={onSave}>
        {dirty ? <p className={styles.unsaved} role="status">{copy.unsavedChanges}</p> : null}
        {error ? <p className={styles.error} role="alert">{error}</p> : null}
        <label>
          <span>{copy.editTitle}</span>
          <input name="task-title" autoComplete="off" value={draft.title} onChange={(event) => onChange({ ...draft, title: event.target.value })} required />
        </label>
        <label>
          <span>{copy.editDescription}</span>
          <textarea name="task-description" autoComplete="off" value={draft.description} onChange={(event) => onChange({ ...draft, description: event.target.value })} rows={4} />
        </label>
        <label>
          <span>{copy.editAssignee}</span>
          <input name="task-assignee" autoComplete="off" value={draft.assignee} onChange={(event) => onChange({ ...draft, assignee: event.target.value })} />
        </label>
        <label>
          <span>{copy.editPriority}</span>
          <select name="task-priority" value={String(draft.priority)} onChange={(event) => onChange({ ...draft, priority: Number(event.target.value) })}>
            {[0, 1, 2, 3].map((priority) => <option key={priority} value={priority}>P{priority}</option>)}
          </select>
        </label>
        <label>
          <span>{copy.editScheduledAt}</span>
          <input type="datetime-local" name="task-scheduled-at" autoComplete="off" value={draft.scheduledAt} onChange={(event) => onChange({ ...draft, scheduledAt: event.target.value })} />
        </label>
        <label>
          <span>{copy.editDueAt}</span>
          <input type="datetime-local" name="task-due-at" autoComplete="off" value={draft.dueAt} onChange={(event) => onChange({ ...draft, dueAt: event.target.value })} />
        </label>
        <div className={styles.editorActions}>
          <button type="submit" disabled={pending || draft.title.trim().length === 0} aria-busy={pending || undefined}>
            {pending ? copy.saving : copy.save}
          </button>
          <button type="button" className={styles.secondaryButton} disabled={pending} onClick={onCancel}>{copy.cancel}</button>
        </div>
      </form>
    </PanelSection>
  )
}

export function TaskInspectorActionPanel({
  task,
  claimToken,
  locale,
  copy,
  pending,
  error,
  onAction,
  onRetry,
}: {
  readonly task: TaskInspectorViewModel["task"]
  readonly claimToken: string | null
  readonly locale: Locale
  readonly copy: InspectorCopy
  readonly pending: boolean
  readonly error: TaskInspectorMutationError | null
  readonly onAction: (view: InspectorActionView, trigger: HTMLButtonElement) => void
  readonly onRetry: (() => void) | null
}) {
  const views = inspectorActionViews(task, claimToken, copy)
  return (
    <PanelSection id="inspector-actions" title={copy.actions}>
      <div className={styles.actionGrid} role="group" aria-label={copy.actions}>
        {views.map((view) => {
          const label = actionLabel(view.action, locale)
          const disabled = pending || !view.enabled
          return (
            <button
              key={view.action}
              type="button"
              className={view.action === "archive" || view.action === "block" ? styles.dangerButton : styles.actionButton}
              data-action={view.action}
              disabled={disabled}
              title={view.disabledReason ?? undefined}
              aria-disabled={disabled || undefined}
              aria-busy={pending || undefined}
              onClick={(event) => onAction(view, event.currentTarget)}
            >
              <span>{label}</span>
              {pending ? <span className={styles.pendingLabel}> · {copy.actionPending}</span> : null}
            </button>
          )
        })}
      </div>
      {error ? (
        <div className={styles.mutationError} role="alert" aria-live="polite">
          <span>{error.message || copy.mutationError}</span>
          {onRetry ? <button type="button" onClick={onRetry}>{copy.retryAction}</button> : null}
        </div>
      ) : null}
    </PanelSection>
  )
}

export type InspectorActionDialogState =
  | { readonly kind: "description"; readonly action: "specify"; readonly description: string; readonly trigger: HTMLButtonElement | null }
  | { readonly kind: "reason"; readonly action: "block"; readonly reason: string; readonly trigger: HTMLButtonElement | null }
  | { readonly kind: "confirm"; readonly action: Exclude<InspectorActionId, "specify" | "block">; readonly trigger: HTMLButtonElement | null }

export function TaskInspectorActionDialog({
  dialog,
  locale,
  copy,
  onDescriptionChange,
  onReasonChange,
  onCancel,
  onSubmit,
}: {
  readonly dialog: InspectorActionDialogState
  readonly locale: Locale
  readonly copy: InspectorCopy
  readonly onDescriptionChange: (value: string) => void
  readonly onReasonChange: (value: string) => void
  readonly onCancel: () => void
  readonly onSubmit: (event: FormEvent<HTMLFormElement>) => void
}) {
  const dialogInputRef = useRef<HTMLTextAreaElement | null>(null)
  const dialogConfirmRef = useRef<HTMLButtonElement | null>(null)
  useEffect(() => {
    const target = dialogInputRef.current ?? dialogConfirmRef.current
    target?.focus()
  }, [])
  const title = dialog.kind === "description" ? copy.actionDescriptionTitle : dialog.kind === "reason" ? copy.actionReasonTitle : copy.actionConfirmTitle
  const submitLabel = actionLabel(dialog.action, locale)
  const invalid = dialog.kind === "description" ? dialog.description.trim().length === 0 : dialog.kind === "reason" ? dialog.reason.trim().length === 0 : false
  return (
    <div className={styles.dialogBackdrop} role="presentation">
      <div className={styles.dialog} role="dialog" aria-modal="true" aria-labelledby="inspector-action-dialog-title" aria-describedby="inspector-action-dialog-description">
        <form onSubmit={onSubmit}>
          <h2 id="inspector-action-dialog-title">{title}</h2>
          <p id="inspector-action-dialog-description">{dialog.kind === "description" ? copy.actionDescriptionHint : dialog.kind === "reason" ? copy.actionReasonHint : copy.actionConfirmDescription}</p>
          {dialog.kind === "description" ? <label><span>{copy.editDescription}</span><textarea ref={dialogInputRef} name="action-description" value={dialog.description} onChange={(event) => onDescriptionChange(event.target.value)} rows={5} required /></label> : null}
          {dialog.kind === "reason" ? <label><span>{copy.actionReasonTitle}</span><textarea ref={dialogInputRef} name="action-reason" value={dialog.reason} onChange={(event) => onReasonChange(event.target.value)} rows={4} required /></label> : null}
          {invalid ? <p className={styles.error} role="status">{dialog.kind === "description" ? copy.descriptionRequired : copy.reasonRequired}</p> : null}
          <div className={styles.dialogActions}>
            <button type="button" className={styles.secondaryButton} onClick={onCancel}>{copy.cancel}</button>
            <button ref={dialogConfirmRef} type="submit" disabled={invalid}>{submitLabel}</button>
          </div>
        </form>
      </div>
    </div>
  )
}
