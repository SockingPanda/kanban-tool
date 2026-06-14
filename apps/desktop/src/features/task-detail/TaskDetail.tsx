import { CircleDot, FileText, GitBranch, MessageSquare, Pencil, Save, X } from "lucide-react"
import { useEffect, useLayoutEffect, useRef, useState, type ChangeEvent, type ReactNode } from "react"
import ReactMarkdown, { defaultUrlTransform } from "react-markdown"
import remarkGfm from "remark-gfm"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupButton, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Textarea } from "@/components/ui/textarea"
import { legalActions } from "@/features/task-actions/legal-actions"
import { isBlockableStatus } from "@/lib/action-policy"
import type { KanbanApi, Run, Task, TaskStatus } from "@/lib/api"
import { priorityBadgeClass, priorityLabel, priorityLevels } from "@/lib/priority"
import { cn, formatRelativeTime, shortId } from "@/lib/utils"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"

import { isLongDescription, visibleDescription } from "./description-state"
import type { DetailState } from "./detail-state"
import type { TaskEditDraft } from "./task-draft"

const priorityOptions: MenuSelectOption<string>[] = priorityLevels.map((priority) => ({
  value: String(priority),
  label: priorityLabel(priority),
}))

export function TaskDetail({
  api,
  task,
  detail,
  activeRun,
  blockReason,
  setBlockReason,
  dependencyInput,
  setDependencyInput,
  claimToken,
  commentBody,
  setCommentBody,
  editDraft,
  draftDirty,
  setEditDraft,
  detailLoading,
  pendingAction,
  onAction,
  onAddDependency,
  onRemoveDependency,
  onSelectTask,
  onSaveTask,
  onCancelEdit,
  onAddComment,
}: {
  api: KanbanApi | null
  task: Task | null
  detail: DetailState
  activeRun?: Run
  blockReason: string
  setBlockReason: (value: string) => void
  dependencyInput: string
  setDependencyInput: (value: string) => void
  claimToken: string | null
  commentBody: string
  setCommentBody: (value: string) => void
  editDraft: TaskEditDraft | null
  draftDirty: boolean
  setEditDraft: (value: TaskEditDraft) => void
  detailLoading: boolean
  pendingAction: string | null
  onAction: (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null }) => Promise<unknown>
  onAddDependency: () => Promise<void>
  onRemoveDependency: (parentTaskId: string) => Promise<void>
  onSelectTask: (taskId: string) => void
  onSaveTask: () => Promise<boolean>
  onCancelEdit: () => void
  onAddComment: () => Promise<void>
}) {
  const [descriptionExpanded, setDescriptionExpanded] = useState(false)
  const [editing, setEditing] = useState(false)

  useEffect(() => {
    setDescriptionExpanded(false)
    setEditing(false)
  }, [task?.id])

  if (!task) return null

  const actions = legalActions(task, claimToken, blockReason)
  const longDescription = isLongDescription(task.description)
  const renderedDescription = visibleDescription(task.description, descriptionExpanded)

  async function saveAndClose() {
    const saved = await onSaveTask()
    if (saved) setEditing(false)
  }

  function cancelEdit() {
    onCancelEdit()
    setEditing(false)
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-border p-4 pr-12">
        <div className="flex items-start justify-between gap-3">
          <SheetHeader className="min-w-0">
            <div className="text-xs text-muted-foreground">#{task.seq} {shortId(task.id)}</div>
            <SheetTitle className="mt-1">{editing ? "Edit task" : task.title}</SheetTitle>
            <SheetDescription className="sr-only">
              Task details, legal transitions, dependencies, comments, runs, and event timeline.
            </SheetDescription>
          </SheetHeader>
          <div className="flex shrink-0 flex-col items-end gap-1">
            <Badge variant={badgeVariant(task.status)}>{task.status}</Badge>
            <Badge variant="secondary" className={priorityBadgeClass(task.priority)}>
              {priorityLabel(task.priority)}
            </Badge>
            {detailLoading ? <span className="text-xs text-muted-foreground">refreshing</span> : null}
          </div>
        </div>
        {!editing ? (
          <Button className="mt-3" variant="outline" size="sm" disabled={!editDraft} onClick={() => setEditing(true)}>
            <Pencil className="h-3.5 w-3.5" />
            Edit
          </Button>
        ) : null}
      </div>

      <ScrollArea className="flex-1 p-4">
        {!editing ? (
          <>
            <Section title="Description">
              <MarkdownDescription>{renderedDescription}</MarkdownDescription>
              {longDescription ? (
                <Button
                  className="mt-2 px-0"
                  variant="ghost"
                  size="sm"
                  onClick={() => setDescriptionExpanded((current) => !current)}
                >
                  {descriptionExpanded ? "Show less" : "Show more"}
                </Button>
              ) : null}
            </Section>

            <Separator className="my-4" />
          </>
        ) : null}

        {editing && editDraft ? (
          <>
            <Section title="Task detail">
              <div className="space-y-2">
                {draftDirty ? <div className="text-xs font-medium text-amber-700">Unsaved changes</div> : null}
                <Input
                  aria-label="Task title"
                  name="task-title"
                  autoComplete="off"
                  value={editDraft.title}
                  onChange={(event) => setEditDraft({ ...editDraft, title: event.target.value })}
                />
                <AutosizeDescriptionTextarea
                  value={editDraft.description}
                  onChange={(value) => setEditDraft({ ...editDraft, description: value })}
                  placeholder="Description"
                />
                <div className="grid grid-cols-2 gap-2">
                  <Input
                    aria-label="Task assignee"
                    name="task-assignee"
                    autoComplete="off"
                    value={editDraft.assignee}
                    onChange={(event) => setEditDraft({ ...editDraft, assignee: event.target.value })}
                    placeholder="Assignee"
                  />
                  <MenuSelect
                    ariaLabel="Task priority"
                    options={priorityOptions}
                    value={editDraft.priority}
                    onValueChange={(priority) => setEditDraft({ ...editDraft, priority })}
                    triggerClassName="h-10 w-full"
                  />
                  <Input
                    type="datetime-local"
                    aria-label="Scheduled at"
                    name="task-scheduled-at"
                    autoComplete="off"
                    value={editDraft.scheduledAt}
                    onChange={(event) => setEditDraft({ ...editDraft, scheduledAt: event.target.value })}
                  />
                  <Input
                    type="datetime-local"
                    aria-label="Due at"
                    name="task-due-at"
                    autoComplete="off"
                    value={editDraft.dueAt}
                    onChange={(event) => setEditDraft({ ...editDraft, dueAt: event.target.value })}
                  />
                </div>
                <div className="flex gap-2">
                  <Button disabled={!api || pendingAction === "save" || !editDraft.title.trim()} onClick={() => void saveAndClose()}>
                    <Save className="h-4 w-4" />
                    {pendingAction === "save" ? "Saving…" : "Save"}
                  </Button>
                  <Button variant="outline" disabled={pendingAction === "save"} onClick={cancelEdit}>
                    <X className="h-4 w-4" />
                    Cancel
                  </Button>
                </div>
              </div>
            </Section>

            <Separator className="my-4" />
          </>
        ) : null}

        <Section title="Legal transitions">
          <div className="grid grid-cols-2 gap-2">
            {actions.map((action) => {
              const actionButton = (
                <Button
                  key={action.label}
                  variant={action.danger ? "destructive" : "secondary"}
                  disabled={!api || Boolean(pendingAction) || !action.enabled}
                  onClick={
                    action.confirmation
                      ? undefined
                      : () => {
                          if (!api) return
                          void onAction(() => action.run(api, task), { fallbackTaskId: task.id, label: action.label.toLowerCase() })
                        }
                  }
                >
                  <action.icon className="h-4 w-4" />
                  {action.label}
                </Button>
              )
              if (!action.confirmation) return actionButton
              return (
                <AlertDialog key={action.label}>
                  <AlertDialogTrigger asChild>{actionButton}</AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Confirm action</AlertDialogTitle>
                      <AlertDialogDescription>{action.confirmation}</AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Cancel</AlertDialogCancel>
                      <AlertDialogAction
                        variant={action.danger ? "destructive" : "default"}
                        onClick={() => {
                          if (!api) return
                          void onAction(() => action.run(api, task), { fallbackTaskId: task.id, label: action.label.toLowerCase() })
                        }}
                      >
                        Continue
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              )
            })}
          </div>
          {task.status === "blocked" ? (
            <div className="mt-2 text-xs text-muted-foreground">Unblock asks the service to recompute schedule and dependency state.</div>
          ) : null}
          {isBlockableStatus(task.status) ? (
            <Field className="mt-3">
              <FieldLabel>Block reason</FieldLabel>
              <Textarea
                aria-label="Block reason"
                name="block-reason"
                autoComplete="off"
                placeholder="Block reason"
                value={blockReason}
                onChange={(event) => setBlockReason(event.target.value)}
              />
            </Field>
          ) : null}
        </Section>

        <Separator className="my-4" />

        <Section title="Dependencies">
          <div className="space-y-3">
            <DependencyGroup
              title="Parents"
              tasks={detail.dependencies.parents}
              pending={pendingAction === "dependency"}
              onSelect={onSelectTask}
              onRemove={(parentTaskId) => void onRemoveDependency(parentTaskId)}
            />
            <DependencyGroup title="Children" tasks={detail.dependencies.children} onSelect={onSelectTask} />
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
                  disabled={!dependencyInput.trim() || pendingAction === "dependency"}
                  onClick={() => void onAddDependency()}
                >
                  <GitBranch className="h-4 w-4" />
                </InputGroupButton>
              </InputGroup>
            </Field>
          </div>
        </Section>

        <Separator className="my-4" />

        <Section title="Comments">
          <div className="space-y-3">
            <div className="space-y-2">
              {detail.comments.length ? (
                detail.comments.slice(-4).map((comment) => (
                  <Card key={comment.id} className="p-2 text-sm">
                    <div className="mb-1 flex items-center justify-between text-xs text-muted-foreground">
                      <span>{comment.author}</span>
                      <span>{formatRelativeTime(comment.created_at)}</span>
                    </div>
                    <MarkdownDescription className="mt-1 text-card-foreground">{comment.body}</MarkdownDescription>
                  </Card>
                ))
              ) : (
                <Empty className="items-start p-0 text-left">
                  <EmptyDescription>No comments yet.</EmptyDescription>
                </Empty>
              )}
            </div>
            <Field>
              <FieldLabel>Comment body</FieldLabel>
              <InputGroup>
                <InputGroupTextarea
                  className="min-h-20 resize-y py-2"
                  aria-label="Comment body"
                  name="comment-body"
                  autoComplete="off"
                  value={commentBody}
                  onChange={(event) => setCommentBody(event.target.value)}
                  placeholder="Add handoff note"
                />
                <InputGroupButton
                  className="h-auto self-stretch"
                  variant="outline"
                  aria-label="Add comment"
                  disabled={!commentBody.trim() || pendingAction === "comment"}
                  onClick={() => void onAddComment()}
                >
                  <MessageSquare className="h-4 w-4" />
                </InputGroupButton>
              </InputGroup>
            </Field>
          </div>
        </Section>

        <Separator className="my-4" />

        <Section title="Run summary">
          {activeRun ? (
            <div className="space-y-2 text-sm">
              <InfoRow label="run" value={shortId(activeRun.id)} />
              <InfoRow label="status" value={activeRun.status} />
              <InfoRow label="worker" value={activeRun.worker_profile ?? "manual"} />
              <InfoRow label="owner" value={activeRun.claim_owner} />
              <InfoRow label="started" value={formatRelativeTime(activeRun.started_at)} />
              <InfoRow label="log" value={activeRun.log_path ?? "-"} />
              {detail.runLog ? (
                <div className="mt-3 rounded-md border border-border bg-terminal-bg p-2 text-xs text-terminal-fg">
                  <div className="mb-2 flex items-center justify-between text-terminal-muted-foreground">
                    <span className="flex items-center gap-1">
                      <FileText className="h-3.5 w-3.5" />
                      log
                    </span>
                    {detail.runLog.truncated ? <span>truncated</span> : null}
                  </div>
                  <pre className="whitespace-pre-wrap font-mono leading-relaxed">
                    {detail.runLog.content || "(empty)"}
                  </pre>
                </div>
              ) : null}
            </div>
          ) : (
            <Empty className="items-start p-0 text-left">
              <EmptyDescription>No runs yet.</EmptyDescription>
            </Empty>
          )}
        </Section>

        <Separator className="my-4" />

        <Section title="Event timeline">
          <div className="space-y-2">
            {detail.events.slice().reverse().map((event) => (
              <div key={event.id} className="grid grid-cols-[auto_1fr] gap-2 text-sm">
                <CircleDot className="mt-0.5 h-4 w-4 text-muted-foreground" />
                <div>
                  <div className="font-medium">{event.kind}</div>
                  <div className="text-xs text-muted-foreground">
                    {formatRelativeTime(event.created_at)} by {event.actor ?? "system"}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Section>
      </ScrollArea>
    </div>
  )
}

export function MarkdownDescription({ children, className }: { children: string; className?: string }) {
  return (
    <div className={cn("task-markdown mt-2 text-sm text-foreground", className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        urlTransform={safeMarkdownUrl}
        components={{
          a: ({ href, children, ...props }) => (
            <a href={href} target="_blank" rel="noreferrer noopener" {...props}>
              {children}
            </a>
          ),
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  )
}

function safeMarkdownUrl(value: string) {
  return defaultUrlTransform(value)
}

function AutosizeDescriptionTextarea({
  value,
  onChange,
  placeholder,
}: {
  value: string
  onChange: (value: string) => void
  placeholder: string
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)

  useLayoutEffect(() => {
    autosizeTextarea(textareaRef.current)
  }, [value])

  function handleChange(event: ChangeEvent<HTMLTextAreaElement>) {
    onChange(event.target.value)
    autosizeTextarea(event.currentTarget)
  }

  return (
    <Textarea
      ref={textareaRef}
      className="min-h-28 overflow-y-hidden"
      aria-label={placeholder}
      name="task-description"
      autoComplete="off"
      value={value}
      onChange={handleChange}
      placeholder={placeholder}
    />
  )
}

function autosizeTextarea(textarea: HTMLTextAreaElement | null) {
  if (!textarea) return
  textarea.style.height = "auto"
  textarea.style.height = `${textarea.scrollHeight}px`
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-normal text-muted-foreground">{title}</h3>
      {children}
    </section>
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

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}

function badgeVariant(status: TaskStatus) {
  if (status === "ready" || status === "done") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  return "secondary"
}

function dependencyBadgeVariant(status: TaskStatus): "ready" | "blocked" | "secondary" {
  if (status === "done") return "ready"
  if (status === "blocked") return "blocked"
  return "secondary"
}
