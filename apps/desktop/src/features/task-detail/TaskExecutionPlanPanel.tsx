import { ListChecks, Network, Plus } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { TaskStatusBadge } from "@/components/ui/composites"
import type { Task, TaskStep, TaskSteps } from "@/lib/api"
import { cn } from "@/lib/utils"

export function TaskExecutionPlanPanel({
  task,
  steps,
  pending,
  stepTitle,
  attachStepId,
  notRequiredReason,
  setStepTitle,
  setAttachStepId,
  setNotRequiredReason,
  onCreateStep,
  onAttachStep,
  onMarkNotRequired,
  onSelectTask,
}: {
  task: Task
  steps: TaskSteps | null
  pending: boolean
  stepTitle: string
  attachStepId: string
  notRequiredReason: string
  setStepTitle: (value: string) => void
  setAttachStepId: (value: string) => void
  setNotRequiredReason: (value: string) => void
  onCreateStep: () => void
  onAttachStep: () => void
  onMarkNotRequired: () => void
  onSelectTask: (taskId: string) => void
}) {
  const items = steps?.steps ?? []
  const required = items.filter((item) => item.required)
  const doneRequired = required.filter((item) => item.status === "done" || item.status === "skipped").length
  const running = items.filter((item) => item.linked_task?.status === "running").length
  const blocked = items.filter((item) => item.linked_task?.status === "blocked" || item.linked_task?.dependency_blocked).length

  return (
    <div className="space-y-3">
      <div className="flex min-w-0 flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <Badge variant={task.execution_plan_state === "unplanned" ? "secondary" : "ready"}>
          {task.execution_plan_state}
        </Badge>
        <span>
          {doneRequired}/{required.length} steps
        </span>
        <span>{running} linked running</span>
        <span>{blocked} linked blocked</span>
      </div>
      {items.length ? <TaskStepRows items={items} onSelectTask={onSelectTask} /> : <EmptyExecutionPlan />}
      <div className="grid gap-2 md:grid-cols-2">
        <Field>
          <FieldLabel>New step title</FieldLabel>
          <InputGroup>
            <InputGroupInput
              aria-label="New step title"
              name="new-step-title"
              autoComplete="off"
              value={stepTitle}
              onChange={(event) => setStepTitle(event.target.value)}
              placeholder="Add text step"
            />
            <InputGroupButton variant="outline" aria-label="Add step" disabled={pending || !stepTitle.trim()} onClick={onCreateStep}>
              <Plus className="h-4 w-4" />
            </InputGroupButton>
          </InputGroup>
        </Field>
        <Field>
          <FieldLabel>Linked task ref</FieldLabel>
          <InputGroup>
            <InputGroupInput
              aria-label="Linked task ref"
              name="linked-task-ref"
              autoComplete="off"
              value={attachStepId}
              onChange={(event) => setAttachStepId(event.target.value)}
              placeholder="Task ref or id"
            />
            <InputGroupButton
              variant="outline"
              aria-label="Add linked step"
              disabled={pending || !attachStepId.trim()}
              onClick={onAttachStep}
            >
              <Network className="h-4 w-4" />
            </InputGroupButton>
          </InputGroup>
        </Field>
      </div>
      <Field>
        <FieldLabel>Not required reason</FieldLabel>
        <InputGroup>
          <InputGroupInput
            aria-label="Not required reason"
            name="not-required-reason"
            autoComplete="off"
            value={notRequiredReason}
            onChange={(event) => setNotRequiredReason(event.target.value)}
            placeholder="Reason this task does not need steps"
          />
          <InputGroupButton
            variant="outline"
            aria-label="Mark execution plan not required"
            disabled={pending || !notRequiredReason.trim()}
            onClick={onMarkNotRequired}
          >
            <ListChecks className="h-4 w-4" />
          </InputGroupButton>
        </InputGroup>
      </Field>
    </div>
  )
}

function TaskStepRows({ items, onSelectTask }: { items: TaskStep[]; onSelectTask: (taskId: string) => void }) {
  return (
    <div className="space-y-1.5">
      {items.map((item, index) => {
        const linkedTask = item.linked_task
        return (
          <div key={item.id} className={cn("flex min-w-0 items-start gap-2 rounded-md border px-2 py-2 text-sm", stepRowClass(item.status))}>
            <Badge variant="secondary" className="mt-0.5 shrink-0">
              S{index + 1}
            </Badge>
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{item.title}</div>
              {item.body ? <div className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">{item.body}</div> : null}
              {linkedTask ? (
                <Button type="button" variant="ghost" size="sm" className="mt-1 h-7 px-1.5 text-xs" onClick={() => onSelectTask(linkedTask.id)}>
                  <Network className="h-3.5 w-3.5" />#{linkedTask.seq} {linkedTask.title}
                </Button>
              ) : (
                <div className="mt-1 text-xs text-muted-foreground">Text step</div>
              )}
            </div>
            {linkedTask ? (
              <div className="flex shrink-0 flex-col items-end gap-1">
                <TaskStatusBadge status={linkedTask.status} />
              </div>
            ) : null}
          </div>
        )
      })}
    </div>
  )
}

function EmptyExecutionPlan() {
  return (
    <Empty className="items-start rounded-md border border-border bg-muted/20 p-3 text-left">
      <EmptyDescription>Execution plan is not planned. Add steps before starting, or record why this task does not need them.</EmptyDescription>
    </Empty>
  )
}

function stepRowClass(status: TaskStep["status"]) {
  if (status === "done") return "border-lime-300 bg-lime-50 text-lime-950 dark:border-lime-900 dark:bg-lime-950/30 dark:text-lime-100"
  if (status === "skipped") return "border-border bg-muted/30 text-muted-foreground"
  return "border-border bg-card"
}
