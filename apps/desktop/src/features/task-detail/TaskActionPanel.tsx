import { CheckCircle2, ListChecks, MoreHorizontal } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Field, FieldLabel } from "@/components/ui/field"
import { Textarea } from "@/components/ui/textarea"
import { isBlockableStatus } from "@/lib/action-policy"
import type { KanbanApi, Task } from "@/lib/api"
import type { LegalTaskAction } from "@/features/task-actions/legal-actions"

export type ActionViewItem = { action: LegalTaskAction; enabled: boolean; disabledReason: string | null }
export type ActionView = { primary: ActionViewItem | null; items: ActionViewItem[]; planBlocked: boolean; incompleteRequiredSteps: number }

export function taskActionView(task: Task, actions: LegalTaskAction[]): ActionView {
  const planBlocked = executionPlanBlocksStart(task)
  const incompleteRequiredSteps = incompleteRequiredStepsFor(task)
  const items = actions.map((action) => {
    const disabledReason = actionDisabledReason(action.label, planBlocked, incompleteRequiredSteps)
    return { action, enabled: action.enabled && !disabledReason, disabledReason }
  })
  const preferred = ["Claim", "Promote", "Specify", "Heartbeat", "Complete", "Review", "Unblock", "Block", "Archive"]
  const primary = preferred.map((label) => items.find((item) => item.action.label === label && item.enabled)).find(Boolean) ?? null
  return { primary, items, planBlocked, incompleteRequiredSteps }
}

export function TaskActionPanel({
  api,
  task,
  pendingAction,
  blockReason,
  setBlockReason,
  actionView,
  onRun,
  onConfirm,
}: {
  api: KanbanApi | null
  task: Task
  pendingAction: string | null
  blockReason: string
  setBlockReason: (value: string) => void
  actionView: ActionView
  onRun: (action: LegalTaskAction) => void
  onConfirm: (action: LegalTaskAction) => void
}) {
  const primary = actionView.primary
  const busy = Boolean(pendingAction)
  return (
    <div className="space-y-3">
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        {actionView.planBlocked ? (
          <Button disabled>
            <ListChecks className="h-4 w-4" />
            Plan steps first
          </Button>
        ) : primary ? (
          <ActionButton item={primary} api={api} busy={busy} onRun={onRun} onConfirm={onConfirm} />
        ) : (
          <Button disabled>
            <CheckCircle2 className="h-4 w-4" />
            No primary action
          </Button>
        )}
        <MoreActionsMenu items={actionView.items} api={api} busy={busy} onRun={onRun} onConfirm={onConfirm} />
      </div>
      {actionView.incompleteRequiredSteps > 0 ? (
        <div className="text-xs text-muted-foreground">
          {actionView.incompleteRequiredSteps} required step{actionView.incompleteRequiredSteps === 1 ? "" : "s"} must finish before Complete.
        </div>
      ) : null}
      {task.status === "blocked" ? <div className="text-xs text-muted-foreground">Unblock asks the service to recompute schedule and dependency state.</div> : null}
      {isBlockableStatus(task.status) ? (
        <Field>
          <FieldLabel>Block reason</FieldLabel>
          <Textarea aria-label="Block reason" name="block-reason" autoComplete="off" placeholder="Block reason" value={blockReason} onChange={(event) => setBlockReason(event.target.value)} />
        </Field>
      ) : null}
    </div>
  )
}

function executionPlanBlocksStart(task: Task) {
  return task.execution_plan_state === "unplanned" && (task.status === "todo" || task.status === "scheduled" || task.status === "ready")
}

function incompleteRequiredStepsFor(task: Task) {
  return Math.max(0, task.required_step_count - task.completed_required_step_count)
}

function actionDisabledReason(label: string, planBlocked: boolean, incompleteRequiredSteps: number) {
  if (planBlocked && (label === "Promote" || label === "Claim")) return "Add steps before starting"
  if (label === "Complete" && incompleteRequiredSteps > 0) return "Complete required steps first"
  return null
}

function ActionButton({
  item,
  api,
  busy,
  onRun,
  onConfirm,
}: {
  item: ActionViewItem
  api: KanbanApi | null
  busy: boolean
  onRun: (action: LegalTaskAction) => void
  onConfirm: (action: LegalTaskAction) => void
}) {
  const Icon = item.action.icon
  return (
    <Button
      variant={item.action.danger ? "destructive" : "default"}
      disabled={!api || busy || !item.enabled}
      title={item.disabledReason ?? undefined}
      onClick={() => {
        if (item.action.confirmation) onConfirm(item.action)
        else onRun(item.action)
      }}
    >
      <Icon className="h-4 w-4" />
      {item.action.label}
    </Button>
  )
}

function MoreActionsMenu({
  items,
  api,
  busy,
  onRun,
  onConfirm,
}: {
  items: ActionViewItem[]
  api: KanbanApi | null
  busy: boolean
  onRun: (action: LegalTaskAction) => void
  onConfirm: (action: LegalTaskAction) => void
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" disabled={!api || busy}>
          <MoreHorizontal className="h-4 w-4" />
          More actions
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-56">
        {items.map((item, index) => {
          const Icon = item.action.icon
          return (
            <div key={item.action.label}>
              {index === 6 ? <DropdownMenuSeparator /> : null}
              <DropdownMenuItem
                disabled={!item.enabled}
                title={item.disabledReason ?? undefined}
                onSelect={(event) => {
                  event.preventDefault()
                  if (!item.enabled) return
                  if (item.action.confirmation) onConfirm(item.action)
                  else onRun(item.action)
                }}
              >
                <Icon className="h-4 w-4" />
                <span>{item.action.label}</span>
                {item.disabledReason ? <span className="ml-auto text-xs text-muted-foreground">blocked</span> : null}
              </DropdownMenuItem>
            </div>
          )
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
