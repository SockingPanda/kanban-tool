import { useEffect, useRef, type FormEvent, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react"

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
  onRetry,
  retryBlocksSubmit,
  copy,
  onChange,
  onSave,
  onCancel,
}: {
  readonly draft: InspectorEditDraft
  readonly dirty: boolean
  readonly pending: boolean
  readonly error: string | null
  readonly onRetry: (() => void) | null
  readonly retryBlocksSubmit: boolean
  readonly copy: InspectorCopy
  readonly onChange: (draft: InspectorEditDraft) => void
  readonly onSave: (event: FormEvent<HTMLFormElement>) => void
  readonly onCancel: () => void
}) {
  return (
    <PanelSection id="inspector-edit" title={copy.edit}>
      <form className={styles.editor} data-testid="inspector-edit-form" onSubmit={onSave}>
        {dirty ? <p className={styles.unsaved} role="status">{copy.unsavedChanges}</p> : null}
        {error ? <div className={styles.mutationError} role="alert" aria-live="polite"><span>{error}</span>{onRetry ? <button type="button" onClick={onRetry}>{copy.retryAction}</button> : null}</div> : null}
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
          <button type="submit" disabled={pending || retryBlocksSubmit || draft.title.trim().length === 0} aria-busy={pending || undefined}>
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
  retryAction,
}: {
  readonly task: TaskInspectorViewModel["task"]
  readonly claimToken: string | null
  readonly locale: Locale
  readonly copy: InspectorCopy
  readonly pending: boolean
  readonly error: TaskInspectorMutationError | null
  readonly onAction: (view: InspectorActionView, trigger: HTMLButtonElement) => void
  readonly onRetry: (() => void) | null
  readonly retryAction: InspectorActionId | null
}) {
  const views = inspectorActionViews(task, claimToken, copy)
  return (
    <PanelSection id="inspector-actions" title={copy.actions}>
      <div className={styles.actionGrid} role="group" aria-label={copy.actions}>
        {views.map((view) => {
          const label = actionLabel(view.action, locale)
          const disabled = pending || !view.enabled || view.action === retryAction
          const reasonId = `inspector-action-reason-${view.action}`
          return (
            <div key={view.action} className={styles.actionItem}>
              <button
                type="button"
                className={view.action === "archive" || view.action === "block" ? styles.dangerButton : styles.actionButton}
                data-action={view.action}
                disabled={disabled}
                title={view.disabledReason ?? undefined}
                aria-describedby={disabled && view.disabledReason ? reasonId : undefined}
                aria-disabled={disabled || undefined}
                aria-busy={pending || undefined}
                onClick={(event) => onAction(view, event.currentTarget)}
              >
                <span>{label}</span>
                {pending ? <span className={styles.pendingLabel}> · {copy.actionPending}</span> : null}
              </button>
              {disabled && view.disabledReason ? <span id={reasonId} className={styles.disabledReason}>{view.disabledReason}</span> : null}
            </div>
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
  | { readonly kind: "reason"; readonly action: "block"; readonly reason: string; readonly confirmed: boolean; readonly requiresConfirmation: boolean; readonly trigger: HTMLButtonElement | null }
  | { readonly kind: "confirm"; readonly action: Exclude<InspectorActionId, "specify" | "block">; readonly trigger: HTMLButtonElement | null }

export function TaskInspectorActionDialog({
  dialog,
  locale,
  copy,
  pending,
  error,
  onRetry,
  retryBlocksSubmit,
  onDescriptionChange,
  onReasonChange,
  onConfirmationChange,
  onCancel,
  onSubmit,
}: {
  readonly dialog: InspectorActionDialogState
  readonly locale: Locale
  readonly copy: InspectorCopy
  readonly pending: boolean
  readonly error: string | null
  readonly onRetry: (() => void) | null
  readonly retryBlocksSubmit: boolean
  readonly onDescriptionChange: (value: string) => void
  readonly onReasonChange: (value: string) => void
  readonly onConfirmationChange: (value: boolean) => void
  readonly onCancel: () => void
  readonly onSubmit: (event: FormEvent<HTMLFormElement>) => void
}) {
  const dialogRef = useRef<HTMLDivElement | null>(null)
  const dialogInputRef = useRef<HTMLTextAreaElement | null>(null)
  const dialogConfirmRef = useRef<HTMLButtonElement | null>(null)
  useEffect(() => {
    const target = dialogInputRef.current ?? dialogConfirmRef.current
    target?.focus()
  }, [])
  const onDialogKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      // The action dialog is nested inside the mobile inspector sheet. Keep
      // Escape scoped to this inner surface and let the owning sheet remain
      // open; closeActionDialog restores the original action trigger.
      event.preventDefault()
      event.stopPropagation()
      onCancel()
      return
    }
    if (event.key !== "Tab") return

    // Stop the outer sheet's document-level trap before it can recalculate
    // focusables that include this nested dialog.
    event.stopPropagation()
    const dialog = dialogRef.current
    if (dialog === null) return
    const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(
      "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
    ))
    if (focusable.length === 0) {
      event.preventDefault()
      dialog.focus()
      return
    }
    const active = document.activeElement
    const activeInside = active instanceof HTMLElement && dialog.contains(active)
    if (!activeInside) {
      event.preventDefault()
      ;(event.shiftKey ? focusable[focusable.length - 1] : focusable[0])?.focus()
      return
    }
    if (event.shiftKey && active === focusable[0]) {
      event.preventDefault()
      focusable[focusable.length - 1]?.focus()
    } else if (!event.shiftKey && active === focusable[focusable.length - 1]) {
      event.preventDefault()
      focusable[0]?.focus()
    }
  }
  const title = dialog.kind === "description" ? copy.actionDescriptionTitle : dialog.kind === "reason" ? copy.actionReasonTitle : copy.actionConfirmTitle
  const submitLabel = actionLabel(dialog.action, locale)
  const invalid = dialog.kind === "description"
    ? dialog.description.trim().length === 0
    : dialog.kind === "reason"
      ? dialog.reason.trim().length === 0 || (dialog.requiresConfirmation && !dialog.confirmed)
      : false
  return (
    <div className={styles.dialogBackdrop} role="presentation">
      <div ref={dialogRef} className={styles.dialog} role="dialog" aria-modal="true" aria-labelledby="inspector-action-dialog-title" aria-describedby="inspector-action-dialog-description" tabIndex={-1} onKeyDown={onDialogKeyDown}>
        <form onSubmit={onSubmit}>
          <h2 id="inspector-action-dialog-title">{title}</h2>
          <p id="inspector-action-dialog-description">{dialog.kind === "description" ? copy.actionDescriptionHint : dialog.kind === "reason" ? copy.actionReasonHint : copy.actionConfirmDescription}</p>
          {error ? <div className={styles.mutationError} role="alert" aria-live="polite"><span>{error}</span>{onRetry ? <button type="button" onClick={onRetry}>{copy.retryAction}</button> : null}</div> : null}
          {dialog.kind === "description" ? <label><span>{copy.editDescription}</span><textarea ref={dialogInputRef} name="action-description" value={dialog.description} onChange={(event) => onDescriptionChange(event.target.value)} rows={5} required /></label> : null}
          {dialog.kind === "reason" ? <label><span>{copy.actionReasonTitle}</span><textarea ref={dialogInputRef} name="action-reason" value={dialog.reason} onChange={(event) => onReasonChange(event.target.value)} rows={4} required /></label> : null}
          {dialog.kind === "reason" && dialog.requiresConfirmation ? <label className={styles.confirmation}><input type="checkbox" name="action-force-confirmation" checked={dialog.confirmed} onChange={(event) => onConfirmationChange(event.target.checked)} required /><span>{copy.actionForceConfirmation}</span></label> : null}
          {invalid ? <p className={styles.error} role="status">{dialog.kind === "description" ? copy.descriptionRequired : dialog.kind === "reason" && dialog.requiresConfirmation && dialog.reason.trim().length > 0 && !dialog.confirmed ? copy.confirmationRequired : copy.reasonRequired}</p> : null}
          <div className={styles.dialogActions}>
            <button type="button" className={styles.secondaryButton} onClick={onCancel}>{copy.cancel}</button>
            <button ref={dialogConfirmRef} type="submit" disabled={invalid || retryBlocksSubmit || pending} aria-busy={pending || undefined}>{submitLabel}</button>
          </div>
        </form>
      </div>
    </div>
  )
}
