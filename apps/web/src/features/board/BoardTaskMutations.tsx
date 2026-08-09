import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { useEffect, useRef } from "react"

import styles from "./BoardView.module.css"
import type { BoardMessages } from "./types"
import type { BoardTaskMutationController } from "./task-mutation-controller"

export function MutationDialog({ controller, copy }: { readonly controller: BoardTaskMutationController; readonly copy: BoardMessages }) {
  const dialog = controller.dialog
  const dialogRef = useRef<HTMLDialogElement | null>(null)
  const controllerRef = useRef(controller)
  controllerRef.current = controller
  const isOpen = dialog !== null
  useEffect(() => {
    const node = dialogRef.current
    if (node === null) return
    if (!node.open) node.showModal()
    const initialFocus = node.querySelector<HTMLElement>("[data-testid='task-title-input'], [data-testid='task-description-input'], [data-testid='task-block-reason']")
    initialFocus?.focus()
    const focusTimer = window.setTimeout(() => initialFocus?.focus(), 0)
    const onCancel = (event: Event) => {
      event.preventDefault()
      controllerRef.current.closeDialog()
    }
    const onClose = () => {
      if (controllerRef.current.dialog !== null) controllerRef.current.closeDialog()
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return
      const focusable = Array.from(node.querySelectorAll<HTMLElement>(
        "button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [href], [tabindex]:not([tabindex='-1'])",
      ))
      if (focusable.length === 0) return
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    node.addEventListener("cancel", onCancel)
    node.addEventListener("close", onClose)
    node.addEventListener("keydown", onKeyDown, true)
    return () => {
      node.removeEventListener("cancel", onCancel)
      node.removeEventListener("close", onClose)
      node.removeEventListener("keydown", onKeyDown, true)
      window.clearTimeout(focusTimer)
      if (node.open) node.close()
    }
  }, [isOpen])
  if (dialog === null) return null
  const title = dialog.kind === "create"
    ? copy.createTaskTitle
    : dialog.kind === "edit"
      ? copy.editTaskTitle
      : copy.transitionTaskTitle
  const pendingKey = dialog.kind === "create" ? "create" : `${dialog.kind === "edit" ? "edit" : "transition"}:${dialog.taskId}`
  const pending = controller.isPending(pendingKey)

  return (
      <dialog ref={dialogRef} className={styles.dialog} role={dialog.kind === "transition" && dialog.option.requiresConfirmation ? "alertdialog" : "dialog"} aria-modal="true" aria-labelledby="task-mutation-dialog-title" data-testid="task-mutation-dialog">
        <div className={styles.dialogHeader}>
          <Heading level={2} id="task-mutation-dialog-title">{title}</Heading>
          <Button label={copy.close} variant="secondary" size="sm" isDisabled={pending} onClick={controller.closeDialog} />
        </div>
        {controller.notice !== null ? <MutationNotice controller={controller} copy={copy} /> : null}
        <form
          className={styles.dialogForm}
          onSubmit={(event) => {
            event.preventDefault()
            controller.submitDialog()
          }}
        >
          {dialog.kind === "transition" ? (
            <>
              {dialog.option.requiresDescription ? (
                <label className={styles.dialogField}>
                  <span>{copy.taskDescriptionLabel}</span>
                  <textarea
                    name="task-description"
                    autoComplete="off"
                    value={dialog.description}
                    placeholder={copy.taskDescriptionPlaceholder}
                    onChange={(event) => controller.setDialogDescription(event.currentTarget.value)}
                    disabled={pending}
                    required
                    autoFocus
                    data-testid="task-description-input"
                  />
                </label>
              ) : null}
              {dialog.option.requiresReason ? (
                <label className={styles.dialogField}>
                  <span>{copy.blockReasonLabel}</span>
                  <input
                    name="block-reason"
                    autoComplete="off"
                    value={dialog.reason}
                    placeholder={copy.blockReasonPlaceholder}
                    onChange={(event) => controller.setDialogReason(event.currentTarget.value)}
                    disabled={pending}
                    required
                    autoFocus={!dialog.option.requiresDescription}
                    data-testid="task-block-reason"
                  />
                </label>
              ) : null}
              {dialog.option.requiresConfirmation ? (
                <label className={styles.dialogCheckField}>
                  <input
                    name="force-confirmation"
                    autoComplete="off"
                    type="checkbox"
                    checked={dialog.confirmed}
                    onChange={(event) => controller.setDialogConfirmed(event.currentTarget.checked)}
                    disabled={pending}
                    required
                    data-testid="task-force-confirmation"
                  />
                  <span>{copy.forceConfirmationLabel}</span>
                </label>
              ) : null}
            </>
          ) : dialog.kind === "create" ? (
            <>
              <label className={styles.dialogField}>
                <span>{copy.taskTitleLabel}</span>
                <input
                  name="task-title"
                  autoComplete="off"
                  value={dialog.title}
                  onChange={(event) => controller.setDialogTitle(event.currentTarget.value)}
                  placeholder={copy.taskTitleLabel}
                  disabled={pending}
                  required
                  autoFocus
                  data-testid="task-title-input"
                />
              </label>
              <label className={styles.dialogField}>
                <span>{copy.taskDescriptionLabel}</span>
                <textarea
                  name="task-description"
                  autoComplete="off"
                  value={dialog.description}
                  onChange={(event) => controller.setDialogDescription(event.currentTarget.value)}
                  placeholder={copy.taskDescriptionPlaceholder}
                  disabled={pending}
                  data-testid="task-description-input"
                />
              </label>
              <label className={styles.dialogField}>
                <span>{copy.firstRequiredStepLabel}</span>
                <input
                  name="first-required-step"
                  autoComplete="off"
                  value={dialog.firstStepTitle}
                  onChange={(event) => controller.setDialogFirstStepTitle(event.currentTarget.value)}
                  placeholder={copy.firstRequiredStepPlaceholder}
                  disabled={pending}
                  data-testid="first-required-step-input"
                />
              </label>
            </>
          ) : (
            <label className={styles.dialogField}>
              <span>{copy.taskTitleLabel}</span>
              <input
                name="task-title"
                autoComplete="off"
                value={dialog.title}
                onChange={(event) => controller.setDialogTitle(event.currentTarget.value)}
                disabled={pending}
                required
                autoFocus
                data-testid="task-title-input"
              />
            </label>
          )}
          <div className={styles.dialogActions}>
            <Button label={copy.cancel} variant="secondary" type="button" isDisabled={pending} onClick={controller.closeDialog} />
            <Button label={pending ? copy.mutationPending : dialog.kind === "create" ? copy.create : copy.save} variant="primary" type="submit" isDisabled={pending} isLoading={pending} />
          </div>
        </form>
      </dialog>
  )
}

export function MutationNotice({ controller, copy }: { readonly controller: BoardTaskMutationController; readonly copy: BoardMessages }) {
  if (controller.notice === null) return null
  return (
    <div className={styles.mutationNotice} role="alert" aria-live="assertive" data-testid="mutation-notice" data-notice-kind={controller.notice.kind}>
      <strong>{controller.notice.kind === "conflict" ? copy.conflictDescription : controller.notice.kind === "stale" ? copy.reconcileStale : copy.mutationError}</strong>
      {controller.notice.kind !== "conflict" ? <span>{controller.notice.message}</span> : null}
      {controller.retryIntent !== null ? <Button label={controller.notice.kind === "stale" ? copy.retryReload : copy.retryMutation} variant="secondary" size="sm" onClick={controller.retryMutation} data-testid="mutation-retry" /> : null}
    </div>
  )
}
