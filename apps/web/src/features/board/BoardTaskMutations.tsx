import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import {
  CheckboxInput,
  Dialog,
  SafeHStack,
  SafeVStack,
  TextArea,
  TextInput,
} from "@/ui/astryx"

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
    <Dialog
      isOpen
      onOpenChange={(open) => {
        if (!open) controller.closeDialog()
      }}
      size="lg"
      role={dialog.kind === "transition" && dialog.option.requiresConfirmation ? "alertdialog" : "dialog"}
      aria-labelledby="task-mutation-dialog-title"
      data-testid="task-mutation-dialog"
      className="max-h-screen"
    >
      <SafeVStack as="section" gap={4} padding={5} className="min-w-0 overflow-y-auto" aria-label={title}>
        <SafeHStack as="header" justify="between" align="center" gap={3} className="min-w-0">
          <Heading level={2} id="task-mutation-dialog-title" className="min-w-0 break-words">{title}</Heading>
          <Button
            label={copy.close}
            variant="secondary"
            size="sm"
            isDisabled={pending}
            onClick={controller.closeDialog}
          />
        </SafeHStack>
        {controller.notice !== null ? <MutationNotice controller={controller} copy={copy} /> : null}
        <form
          className="grid min-w-0 gap-4"
          onSubmit={(event) => {
            event.preventDefault()
            controller.submitDialog()
          }}
        >
          {dialog.kind === "transition" ? (
            <>
              {dialog.option.requiresDescription ? (
                <TextArea
                  label={copy.taskDescriptionLabel}
                  value={dialog.description}
                  placeholder={copy.taskDescriptionPlaceholder}
                  onChange={(value) => controller.setDialogDescription(value)}
                  isDisabled={pending}
                  isRequired
                  hasAutoFocus
                  data-autofocus="true"
                  data-testid="task-description-input"
                  htmlName="task-description"
                  autoComplete="off"
                />
              ) : null}
              {dialog.option.requiresReason ? (
                <TextInput
                  label={copy.blockReasonLabel}
                  value={dialog.reason}
                  placeholder={copy.blockReasonPlaceholder}
                  onChange={(value) => controller.setDialogReason(value)}
                  isDisabled={pending}
                  isRequired
                  hasAutoFocus={!dialog.option.requiresDescription}
                  data-autofocus={!dialog.option.requiresDescription ? "true" : undefined}
                  data-testid="task-block-reason"
                  htmlName="block-reason"
                  autoComplete="off"
                />
              ) : null}
              {dialog.option.requiresConfirmation ? (
                <CheckboxInput
                  label={copy.forceConfirmationLabel}
                  value={dialog.confirmed}
                  onChange={(checked) => controller.setDialogConfirmed(checked)}
                  isDisabled={pending}
                  isRequired
                  data-testid="task-force-confirmation"
                  htmlName="force-confirmation"
                />
              ) : null}
            </>
          ) : dialog.kind === "create" ? (
            <>
              <TextInput
                label={copy.taskTitleLabel}
                value={dialog.title}
                placeholder={copy.taskTitleLabel}
                onChange={(value) => controller.setDialogTitle(value)}
                isDisabled={pending}
                isRequired
                hasAutoFocus
                data-autofocus="true"
                data-testid="task-title-input"
                htmlName="task-title"
                autoComplete="off"
              />
              <TextArea
                label={copy.taskDescriptionLabel}
                value={dialog.description}
                placeholder={copy.taskDescriptionPlaceholder}
                onChange={(value) => controller.setDialogDescription(value)}
                isDisabled={pending}
                data-testid="task-description-input"
                htmlName="task-description"
                autoComplete="off"
              />
              <TextInput
                label={copy.firstRequiredStepLabel}
                value={dialog.firstStepTitle}
                placeholder={copy.firstRequiredStepPlaceholder}
                onChange={(value) => controller.setDialogFirstStepTitle(value)}
                isDisabled={pending}
                data-testid="first-required-step-input"
                htmlName="first-required-step"
                autoComplete="off"
              />
            </>
          ) : (
            <TextInput
              label={copy.taskTitleLabel}
              value={dialog.title}
              onChange={(value) => controller.setDialogTitle(value)}
              isDisabled={pending}
              isRequired
              hasAutoFocus
              data-autofocus="true"
              data-testid="task-title-input"
              htmlName="task-title"
              autoComplete="off"
            />
          )}
          <SafeHStack as="footer" justify="end" gap={2} wrap="wrap" className="min-w-0">
            <Button label={copy.cancel} variant="secondary" type="button" isDisabled={pending} onClick={controller.closeDialog} />
            <Button
              label={pending ? copy.mutationPending : dialog.kind === "create" ? copy.create : copy.save}
              variant="primary"
              type="submit"
              isDisabled={pending}
              isLoading={pending}
            />
          </SafeHStack>
        </form>
      </SafeVStack>
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
  const title = controller.notice.kind === "conflict"
    ? copy.conflictDescription
    : controller.notice.kind === "stale"
      ? copy.reconcileStale
      : copy.mutationError
  return (
    <Banner
      status={controller.notice.kind === "conflict" ? "warning" : "error"}
      title={title}
      description={controller.notice.kind === "conflict" ? undefined : controller.notice.message}
      endContent={controller.retryIntent !== null ? <Button label={retryLabel} variant="secondary" size="sm" onClick={controller.retryMutation} data-testid="mutation-retry" /> : undefined}
      aria-live="assertive"
      data-testid="mutation-notice"
      data-notice-kind={controller.notice.kind}
    />
  )
}
