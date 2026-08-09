import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"

import styles from "./BoardView.module.css"
import type { BoardMessages } from "./types"
import type { BoardTaskMutationController } from "./task-mutation-controller"

export function MutationDialog({ controller, copy }: { readonly controller: BoardTaskMutationController; readonly copy: BoardMessages }) {
  const dialog = controller.dialog
  if (dialog === null) return null
  const title = dialog.kind === "create"
    ? copy.createTaskTitle
    : dialog.kind === "edit"
      ? copy.editTaskTitle
      : copy.transitionTaskTitle
  const pendingKey = dialog.kind === "create" ? "create" : `${dialog.kind === "edit" ? "edit" : "transition"}:${dialog.taskId}`
  const pending = controller.isPending(pendingKey)

  return (
    <div className={styles.dialogBackdrop} role="presentation">
      <section className={styles.dialog} role="dialog" aria-modal="true" aria-labelledby="task-mutation-dialog-title" data-testid="task-mutation-dialog">
        <div className={styles.dialogHeader}>
          <Heading level={2} id="task-mutation-dialog-title">{title}</Heading>
          <Button label={copy.close} variant="secondary" size="sm" isDisabled={pending} onClick={controller.closeDialog} />
        </div>
        <form
          className={styles.dialogForm}
          onSubmit={(event) => {
            event.preventDefault()
            controller.submitDialog()
          }}
        >
          {dialog.kind === "transition" ? (
            <label className={styles.dialogField}>
              <span>{copy.blockReasonLabel}</span>
              <input
                value={dialog.reason}
                placeholder={copy.blockReasonPlaceholder}
                onChange={(event) => controller.setDialogReason(event.currentTarget.value)}
                disabled={pending}
                required
                autoFocus
                data-testid="task-block-reason"
              />
            </label>
          ) : (
            <label className={styles.dialogField}>
              <span>{copy.taskTitleLabel}</span>
              <input
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
      </section>
    </div>
  )
}

export function MutationNotice({ controller, copy }: { readonly controller: BoardTaskMutationController; readonly copy: BoardMessages }) {
  if (controller.notice === null) return null
  return (
    <div className={styles.mutationNotice} role="alert" data-testid="mutation-notice" data-notice-kind={controller.notice.kind}>
      <strong>{controller.notice.kind === "conflict" ? copy.conflictDescription : copy.mutationError}</strong>
      {controller.notice.kind === "error" ? <span>{controller.notice.message}</span> : null}
      {controller.retryIntent !== null ? <Button label={copy.retryMutation} variant="secondary" size="sm" onClick={controller.retryMutation} data-testid="mutation-retry" /> : null}
    </div>
  )
}
