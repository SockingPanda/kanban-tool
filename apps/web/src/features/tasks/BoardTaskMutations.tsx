import { TaskForm } from "./task-form";
import { Dialog } from "../../components/ui/dialog";
import { Button } from "../../components/ui/button"


import type { BoardMessages } from "../../domain/tasks/board"
import type { BoardTaskMutationController } from "../../application/tasks/task-mutation-controller"

export function MutationDialog({ controller, copy }: { readonly controller: BoardTaskMutationController; readonly copy: BoardMessages }) {
  const dialog = controller.dialog
  if (dialog === null) return null
  if (dialog.kind === "create") return <TaskForm controller={controller} copy={copy} dialog={dialog} />
  const title = dialog.kind === "edit"
      ? copy.editTaskTitle
      : copy.transitionTaskTitle
  const pendingKey = `${dialog.kind === "edit" ? "edit" : "transition"}:${dialog.taskId}`
  const pending = controller.isPending(pendingKey)

  return (
      <Dialog open title={title} onClose={controller.closeDialog} dismissDisabled={pending} role={dialog.kind === "transition" && dialog.option.requiresConfirmation ? "alertdialog" : "dialog"} testId="task-mutation-dialog">
        {controller.notice !== null ? <MutationNotice controller={controller} copy={copy} /> : null}
        <form
          className="form-stack"
          onSubmit={(event) => {
            event.preventDefault()
            controller.submitDialog()
          }}
        >
          {dialog.kind === "transition" ? (
            <>
              {dialog.option.requiresDescription ? (
                <label className="ui-field">
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
                <label className="ui-field">
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
                <label className="checkbox-row">
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
          ) : (
            <label className="ui-field">
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
          <div className="dialog-footer">
            <Button label={copy.cancel} variant="secondary" type="button" isDisabled={pending} onClick={controller.closeDialog} />
            <Button label={pending ? copy.mutationPending : copy.save} variant="primary" type="submit" isDisabled={pending} isLoading={pending} />
          </div>
        </form>
      </Dialog>
  )
}

export function MutationNotice({ controller, copy }: { readonly controller: BoardTaskMutationController; readonly copy: BoardMessages }) {
  if (controller.notice === null) return null
  const retryLabel = controller.retryIntent?.kind === "create" && controller.retryIntent.taskCreated
    ? copy.retryCreateStep
    : controller.notice.kind === "stale"
      ? copy.retryReload
      : copy.retryMutation
  return (
    <div className="paper-banner banner-error" role="alert" aria-live="assertive" data-testid="mutation-notice" data-notice-kind={controller.notice.kind}>
      <strong>{controller.notice.kind === "conflict" ? copy.conflictDescription : controller.notice.kind === "stale" ? copy.reconcileStale : copy.mutationError}</strong>
      {controller.notice.kind !== "conflict" ? <span>{controller.notice.message}</span> : null}
      {controller.retryIntent !== null ? <Button label={retryLabel} variant="secondary" size="sm" onClick={controller.retryMutation} data-testid="mutation-retry" /> : null}
    </div>
  )
}
