import { GitBranch, X } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { useI18n } from "@/i18n"
import type { Task, TaskStatus } from "@/lib/api"

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
  const { t } = useI18n()
  return (
    <div className="space-y-3">
      <DependencyGroup
        title={t("Parents")}
        kind="parent"
        t={t}
        tasks={parents}
        pending={pending}
        onSelect={onSelectTask}
        onRemove={onRemoveDependency}
        noneLabel={t("none")}
        removeTitle={t("Remove parent dependency")}
        openLabel={(task) => t("Open {kind} dependency #{seq} {title}", { kind: t("parent"), seq: task.seq, title: task.title })}
        removeLabel={(task) => t("Remove parent dependency #{seq} {title}", { seq: task.seq, title: task.title })}
      />
      <DependencyGroup
        title={t("Children")}
        kind="child"
        t={t}
        tasks={children}
        onSelect={onSelectTask}
        noneLabel={t("none")}
        openLabel={(task) => t("Open {kind} dependency #{seq} {title}", { kind: t("child"), seq: task.seq, title: task.title })}
      />
      <Field>
        <FieldLabel>{t("Parent task id")}</FieldLabel>
        <InputGroup>
          <InputGroupInput
            aria-label={t("Parent task id")}
            name="parent-task-id"
            autoComplete="off"
            value={dependencyInput}
            onChange={(event) => setDependencyInput(event.target.value)}
            placeholder={t("Parent task id")}
          />
          <InputGroupButton
            variant="outline"
            aria-label={t("Add parent dependency")}
            disabled={!dependencyInput.trim() || pending}
            onClick={onAddDependency}
          >
            <GitBranch className="h-4 w-4" />
          </InputGroupButton>
        </InputGroup>
      </Field>
    </div>
  )
}

export function DependencyGroup({
  title,
  tasks,
  pending = false,
  t = englishTranslate,
  onSelect,
  onRemove,
  kind,
  noneLabel = "none",
  openLabel,
  removeLabel,
  removeTitle = "Remove parent dependency",
}: {
  title: string
  tasks: Task[]
  pending?: boolean
  t?: (key: string, values?: Record<string, string | number>) => string
  onSelect?: (taskId: string) => void
  onRemove?: (taskId: string) => void
  kind?: "parent" | "child"
  noneLabel?: string
  openLabel?: (task: Task) => string
  removeLabel?: (task: Task) => string
  removeTitle?: string
}) {
  const dependencyKind = kind ?? (title === "Parents" ? "parent" : "child")

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
                aria-label={openLabel?.(task) ?? t("Open {kind} dependency #{seq} {title}", { kind: t(dependencyKind), seq: task.seq, title: task.title })}
                title={t("Open {title}", { title: task.title })}
                onClick={() => onSelect?.(task.id)}
              >
                <Badge variant={dependencyBadgeVariant(task.status)}>
                  #{task.seq} {t(task.status)}
                </Badge>
              </Button>
              {onRemove ? (
                <Button
                  type="button"
                  variant="ghost"
                  className="h-auto rounded-none px-1.5 text-muted-foreground hover:text-destructive"
                  disabled={pending}
                  aria-label={removeLabel?.(task) ?? t("Remove parent dependency #{seq} {title}", { seq: task.seq, title: task.title })}
                  title={removeTitle === "Remove parent dependency" ? t(removeTitle) : removeTitle}
                  onClick={() => onRemove(task.id)}
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              ) : null}
            </span>
          ))
        ) : (
          <span className="text-sm text-muted-foreground">{noneLabel}</span>
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

function englishTranslate(key: string, values: Record<string, string | number> = {}) {
  return key.replace(/\{([a-zA-Z0-9_]+)\}/g, (match, name) => {
    const value = values[name]
    return value === undefined ? match : String(value)
  })
}
