import { GitBranch, X } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import type { Task, TaskStatus } from "@/lib/api"

import { Section } from "./task-detail-shared"

export function TaskDependencyPanel({
  parents,
  children,
  dependencyInput,
  pending,
  setDependencyInput,
  onAddDependency,
  onRemoveDependency,
  onSelectTask,
}: {
  parents: Task[]
  children: Task[]
  dependencyInput: string
  pending: boolean
  setDependencyInput: (value: string) => void
  onAddDependency: () => void
  onRemoveDependency: (parentTaskId: string) => void
  onSelectTask: (taskId: string) => void
}) {
  return (
    <Section title="Dependency controls">
      <div className="space-y-3">
        <DependencyGroup title="Parents" tasks={parents} pending={pending} onSelect={onSelectTask} onRemove={onRemoveDependency} />
        <DependencyGroup title="Children" tasks={children} onSelect={onSelectTask} />
        <Field>
          <FieldLabel>Parent task id</FieldLabel>
          <InputGroup>
            <InputGroupInput
              aria-label="Parent task id"
              name="parent-task-id"
              autoComplete="off"
              value={dependencyInput}
              onChange={(event) => setDependencyInput(event.target.value)}
              placeholder="Parent task id"
            />
            <InputGroupButton
              variant="outline"
              aria-label="Add parent dependency"
              disabled={!dependencyInput.trim() || pending}
              onClick={onAddDependency}
            >
              <GitBranch className="h-4 w-4" />
            </InputGroupButton>
          </InputGroup>
        </Field>
      </div>
    </Section>
  )
}

export function DependencyGroup({
  title,
  tasks,
  pending = false,
  onSelect,
  onRemove,
}: {
  title: string
  tasks: Task[]
  pending?: boolean
  onSelect?: (taskId: string) => void
  onRemove?: (taskId: string) => void
}) {
  const dependencyKind = title === "Parents" ? "parent" : "child"

  return (
    <div>
      <div className="mb-1 text-xs text-muted-foreground">{title}</div>
      <div className="flex flex-wrap gap-1">
        {tasks.length ? (
          tasks.map((task) => (
            <span key={task.id} className="inline-flex items-center overflow-hidden rounded-md border border-border bg-muted">
              <Button
                type="button"
                variant="ghost"
                className="h-auto border-0 bg-transparent p-0 text-left"
                aria-label={`Open ${dependencyKind} dependency #${task.seq} ${task.title}`}
                title={`Open ${task.title}`}
                onClick={() => onSelect?.(task.id)}
              >
                <Badge variant={dependencyBadgeVariant(task.status)}>
                  #{task.seq} {task.status}
                </Badge>
              </Button>
              {onRemove ? (
                <Button
                  type="button"
                  variant="ghost"
                  className="h-auto rounded-none px-1.5 text-muted-foreground hover:text-destructive"
                  disabled={pending}
                  aria-label={`Remove parent dependency #${task.seq} ${task.title}`}
                  title="Remove parent dependency"
                  onClick={() => onRemove(task.id)}
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              ) : null}
            </span>
          ))
        ) : (
          <span className="text-sm text-muted-foreground">none</span>
        )}
      </div>
    </div>
  )
}

function dependencyBadgeVariant(status: TaskStatus): "ready" | "blocked" | "secondary" {
  if (status === "done") return "ready"
  if (status === "blocked") return "blocked"
  return "secondary"
}
