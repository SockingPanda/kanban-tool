import { ChevronDown, CircleDot, FileText, GitBranch, Loader2, MessageSquare, Pencil, Plus, Save, Sparkles, X } from "lucide-react"
import { useEffect, useLayoutEffect, useRef, useState, type ChangeEvent, type ReactNode } from "react"
import ReactMarkdown, { defaultUrlTransform } from "react-markdown"
import remarkGfm from "remark-gfm"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupButton, InputGroupInput, InputGroupTextarea } from "@/components/ui/input-group"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { Textarea } from "@/components/ui/textarea"
import { legalActions } from "@/features/task-actions/legal-actions"
import { isBlockableStatus } from "@/lib/action-policy"
import type { CommentRecord, KanbanApi, LabelSuggestionResult, Run, Task, TaskStatus } from "@/lib/api"
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
import { commentPageState, type CommentSortOrder } from "./comment-list-state"
import type { DetailState } from "./detail-state"
import type { TaskEditDraft } from "./task-draft"

const priorityOptions: MenuSelectOption<string>[] = priorityLevels.map((priority) => ({
  value: String(priority),
  label: priorityLabel(priority),
}))

const commentSortOptions: MenuSelectOption<CommentSortOrder>[] = [
  { value: "newest", label: "Newest first" },
  { value: "oldest", label: "Oldest first" },
]

export function TaskDetail({
  api,
  task,
  detail,
  labelSuggestions = null,
  labelSuggestionsRequested = false,
  labelSuggestionsLoading = false,
  labelSuggestionsError = null,
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
  onRequestLabelSuggestions,
  onSelectTask,
  onSaveTask,
  onCancelEdit,
  onAddComment,
}: {
  api: KanbanApi | null
  task: Task | null
  detail: DetailState
  labelSuggestions?: LabelSuggestionResult | null
  labelSuggestionsRequested?: boolean
  labelSuggestionsLoading?: boolean
  labelSuggestionsError?: string | null
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
  onRequestLabelSuggestions?: () => void
  onSelectTask: (taskId: string) => void
  onSaveTask: () => Promise<boolean>
  onCancelEdit: () => void
  onAddComment: () => Promise<void>
}) {
  const [descriptionExpanded, setDescriptionExpanded] = useState(false)
  const [editing, setEditing] = useState(false)
  const [labelInput, setLabelInput] = useState("")
  const [commentSortOrder, setCommentSortOrder] = useState<CommentSortOrder>("newest")
  const [commentPage, setCommentPage] = useState(0)

  useEffect(() => {
    setDescriptionExpanded(false)
    setEditing(false)
    setLabelInput("")
    setCommentSortOrder("newest")
    setCommentPage(0)
  }, [task?.id])

  if (!task) return null
  const currentTask = task

  const actions = legalActions(task, claimToken, blockReason)
  const longDescription = isLongDescription(task.description)
  const renderedDescription = visibleDescription(task.description, descriptionExpanded)
  const commentsPage = commentPageState({
    comments: detail.comments,
    page: commentPage,
    sortOrder: commentSortOrder,
  })

  async function saveAndClose() {
    const saved = await onSaveTask()
    if (saved) setEditing(false)
  }

  function cancelEdit() {
    onCancelEdit()
    setEditing(false)
  }

  async function addLabel() {
    if (!api || !labelInput.trim()) return
    const name = labelInput.trim()
    await onAction(async () => {
      const updated = await api.addTaskLabel(currentTask.id, name)
      setLabelInput("")
      return updated
    }, { fallbackTaskId: currentTask.id, label: "label" })
  }

  async function removeLabel(labelId: string) {
    if (!api) return
    await onAction(() => api.removeTaskLabel(currentTask.id, labelId), { fallbackTaskId: currentTask.id, label: "label" })
  }

  async function applySuggestedLabel(labelName: string) {
    await applySuggestedTaskLabel(api, currentTask.id, labelName, onAction)
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

        <Section title="Labels">
          <div className="space-y-3">
            <div className="flex flex-wrap gap-1.5">
              {task.labels.length ? (
                task.labels.map((label) => (
                  <span key={label.id} className="inline-flex max-w-full items-center overflow-hidden rounded-md border border-border bg-muted">
                    <Badge variant="secondary" className="max-w-48 truncate rounded-r-none px-2">
                      {label.name}
                    </Badge>
                    <Button
                      type="button"
                      variant="ghost"
                      className="h-6 rounded-none px-1.5 text-muted-foreground hover:text-destructive"
                      disabled={!api || pendingAction === "label"}
                      aria-label={`Remove label ${label.name}`}
                      onClick={() => void removeLabel(label.id)}
                    >
                      <X className="h-3.5 w-3.5" />
                    </Button>
                  </span>
                ))
              ) : (
                <span className="text-sm text-muted-foreground">none</span>
              )}
            </div>
            <LabelSuggestionsPanel
              suggestions={labelSuggestions}
              requested={labelSuggestionsRequested}
              loading={labelSuggestionsLoading}
              error={labelSuggestionsError}
              pending={pendingAction === "label"}
              disabled={!api}
              onRequest={onRequestLabelSuggestions}
              onApply={(labelName) => void applySuggestedLabel(labelName)}
            />
            <Field>
              <FieldLabel>Label name</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  aria-label="Label name"
                  name="label-name"
                  autoComplete="off"
                  value={labelInput}
                  onChange={(event) => setLabelInput(event.target.value)}
                  placeholder="Label name"
                />
                <InputGroupButton
                  variant="outline"
                  aria-label="Add label"
                  disabled={!api || !labelInput.trim() || pendingAction === "label"}
                  onClick={() => void addLabel()}
                >
                  <Plus className="h-4 w-4" />
                </InputGroupButton>
              </InputGroup>
            </Field>
          </div>
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
            {commentsPage.total ? (
              <div className="flex items-center justify-between gap-2">
                <div className="text-xs text-muted-foreground">
                  {commentsPage.total} comment{commentsPage.total === 1 ? "" : "s"}
                </div>
                <MenuSelect
                  ariaLabel="Comment sort order"
                  options={commentSortOptions}
                  value={commentSortOrder}
                  onValueChange={(value) => {
                    setCommentSortOrder(value)
                    setCommentPage(0)
                  }}
                  triggerClassName="h-8 w-36"
                />
              </div>
            ) : null}
            <div className="space-y-2">
              {commentsPage.total ? (
                commentsPage.comments.map((comment) => (
                  <Card key={comment.id} className="p-2 text-sm">
                    <div className="mb-1 flex items-center justify-between text-xs text-muted-foreground">
                      <span className="flex items-center gap-1.5">
                        {comment.author}
                        {comment.kind === "decision" ? <Badge variant="secondary">decision</Badge> : null}
                      </span>
                      <span>{formatRelativeTime(comment.created_at)}</span>
                    </div>
                    <MarkdownDescription className="mt-1 text-card-foreground">{comment.body}</MarkdownDescription>
                    {comment.kind === "decision" ? <DecisionComment comment={comment} /> : null}
                  </Card>
                ))
              ) : (
                <Empty className="items-start p-0 text-left">
                  <EmptyDescription>No comments yet.</EmptyDescription>
                </Empty>
              )}
            </div>
            {commentsPage.pageCount > 1 ? (
              <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
                <Button
                  variant="outline"
                  size="sm"
                  aria-label="Previous comments"
                  disabled={!commentsPage.hasPreviousPage}
                  onClick={() => setCommentPage((current) => Math.max(0, current - 1))}
                >
                  Previous
                </Button>
                <span>
                  Page {commentsPage.page + 1} of {commentsPage.pageCount}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  aria-label="Next comments"
                  disabled={!commentsPage.hasNextPage}
                  onClick={() => setCommentPage((current) => current + 1)}
                >
                  Next
                </Button>
              </div>
            ) : null}
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

type DecisionOption = {
  slug: string
  title: string
  detail: string
}

type DecisionMetadata = {
  options: DecisionOption[]
  selected: string
  reason: string
  risk?: string
  verification?: string
}

type ParsedDecision = { ok: true; metadata: DecisionMetadata } | { ok: false; error: string }

export function DecisionComment({ comment }: { comment: CommentRecord }) {
  const decision = parseDecisionMetadata(comment.metadata_json)
  if (!decision.ok) {
    return (
      <Alert className="mt-2 border-destructive/50 bg-destructive/5">
        <AlertTitle className="text-destructive">Invalid decision metadata</AlertTitle>
        <AlertDescription className="text-destructive">{decision.error}</AlertDescription>
      </Alert>
    )
  }

  const { metadata } = decision
  return (
    <div className="mt-2 space-y-2 rounded-md border border-border bg-muted/30 p-2">
      <div className="flex flex-wrap gap-1.5">
        {metadata.options.map((option) => {
          const selected = option.slug === metadata.selected
          return (
            <Collapsible key={option.slug} defaultOpen={selected}>
              <CollapsibleTrigger asChild>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className={cn(
                    "text-muted-foreground hover:bg-background",
                    selected && "border-[var(--status-ready-ring)] bg-[var(--status-ready-bg)] text-[var(--status-ready-fg)]",
                  )}
                  aria-label={`Show decision option ${option.slug}`}
                >
                  {option.slug}
                  <ChevronDown className="h-3 w-3" />
                </Button>
              </CollapsibleTrigger>
              <CollapsibleContent
                className={cn(
                  "mt-1 max-w-full rounded-md border border-border bg-background p-2 text-xs",
                  selected && "border-[var(--status-ready-ring)] bg-[var(--status-ready-bg)] text-[var(--status-ready-fg)]",
                )}
              >
                <div className="font-medium text-foreground">{option.title}</div>
                <MarkdownDescription className="mt-1 text-xs text-muted-foreground">{option.detail}</MarkdownDescription>
              </CollapsibleContent>
            </Collapsible>
          )
        })}
      </div>
      <DecisionField label="reason" value={metadata.reason} />
      {metadata.risk ? <DecisionField label="risk" value={metadata.risk} /> : null}
      {metadata.verification ? <DecisionField label="verification" value={metadata.verification} /> : null}
    </div>
  )
}

function DecisionField({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[6rem_1fr] gap-2 text-xs">
      <div className="font-medium uppercase tracking-normal text-muted-foreground">{label}</div>
      <MarkdownDescription className="mt-0 text-xs">{value}</MarkdownDescription>
    </div>
  )
}

function parseDecisionMetadata(metadataJson: string): ParsedDecision {
  let parsed: unknown
  try {
    parsed = JSON.parse(metadataJson)
  } catch {
    return { ok: false, error: "metadata_json is not valid JSON" }
  }
  if (!isObject(parsed)) return { ok: false, error: "metadata_json must be an object" }
  const options = parsed.options
  if (!Array.isArray(options) || options.length === 0) {
    return { ok: false, error: "options must be a non-empty array" }
  }

  const seen = new Set<string>()
  const decisionOptions: DecisionOption[] = []
  for (const option of options) {
    if (!isObject(option)) return { ok: false, error: "options must be objects" }
    const slug = nonEmptyRawString(option.slug)
    const title = nonEmptyString(option.title)
    const detail = nonEmptyString(option.detail)
    if (!slug || !title || !detail) return { ok: false, error: "each option needs slug, title, and detail" }
    if (!isDecisionSlug(slug)) return { ok: false, error: "option slug must be lowercase ASCII letters, digits, or hyphen" }
    if (seen.has(slug)) return { ok: false, error: "option slugs must be unique" }
    seen.add(slug)
    decisionOptions.push({ slug, title, detail })
  }

  const selected = nonEmptyRawString(parsed.selected)
  if (!selected || !seen.has(selected)) return { ok: false, error: "selected must match an option slug" }
  const reason = nonEmptyString(parsed.reason)
  if (!reason) return { ok: false, error: "reason must be a non-empty string" }
  const risk = optionalNonEmptyString(parsed.risk)
  if (risk === false) return { ok: false, error: "risk must be a non-empty string" }
  const verification = optionalNonEmptyString(parsed.verification)
  if (verification === false) return { ok: false, error: "verification must be a non-empty string" }

  return {
    ok: true,
    metadata: {
      options: decisionOptions,
      selected,
      reason,
      risk,
      verification,
    },
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function nonEmptyString(value: unknown) {
  return typeof value === "string" && value.trim() ? value.trim() : null
}

function nonEmptyRawString(value: unknown) {
  return typeof value === "string" && value.trim() ? value : null
}

function optionalNonEmptyString(value: unknown) {
  if (value === undefined) return undefined
  return nonEmptyString(value) ?? false
}

function isDecisionSlug(value: string) {
  return /^[a-z0-9][a-z0-9-]*$/.test(value)
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

export async function applySuggestedTaskLabel(
  api: Pick<KanbanApi, "addTaskLabel"> | null,
  taskId: string,
  labelName: string,
  onAction: (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null }) => Promise<unknown>,
) {
  if (!api) return undefined
  return onAction(() => api.addTaskLabel(taskId, labelName), { fallbackTaskId: taskId, label: "label" })
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

function LabelSuggestionsPanel({
  suggestions,
  requested,
  loading,
  error,
  pending,
  disabled,
  onRequest,
  onApply,
}: {
  suggestions: LabelSuggestionResult | null
  requested: boolean
  loading: boolean
  error: string | null
  pending: boolean
  disabled: boolean
  onRequest?: () => void
  onApply: (labelName: string) => void
}) {
  const requestDisabled = disabled || loading || !onRequest
  const requestButton = (
    <Button
      type="button"
      variant="outline"
      size="sm"
      disabled={requestDisabled}
      aria-label="Suggest labels"
      onClick={onRequest}
    >
      {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Sparkles className="h-3.5 w-3.5" />}
      {loading ? "Suggesting…" : requested || suggestions ? "Refresh suggestions" : "Suggest labels"}
    </Button>
  )

  if (!requested && !suggestions && !loading && !error) {
    return requestButton
  }

  const visible = suggestions ? (suggestions.selected_labels.length ? suggestions.selected_labels : suggestions.candidates) : []
  return (
    <div className="space-y-2 rounded-md border border-border p-2">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
        <span className="font-medium text-muted-foreground">Suggestions</span>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {suggestions ? (
            <span className="text-muted-foreground">
              coverage {(suggestions.coverage * 100).toFixed(0)}% / residual {suggestions.residual_norm.toFixed(3)}
            </span>
          ) : null}
          {requestButton}
        </div>
      </div>
      {loading && !suggestions ? (
        <div className="text-xs text-muted-foreground">Finding label suggestions…</div>
      ) : null}
      {error ? (
        <Alert className="border-destructive/50 bg-destructive/5 py-2">
          <AlertTitle className="text-xs text-destructive">Suggestions failed</AlertTitle>
          <AlertDescription className="text-xs text-destructive">{error}</AlertDescription>
        </Alert>
      ) : null}
      {suggestions?.needs_new_label ? (
        <div className="rounded-sm border border-amber-300 bg-amber-50 px-2 py-1 text-xs text-amber-800">
          new label may be needed
        </div>
      ) : null}
      {suggestions?.degraded ? (
        <Alert className="py-2">
          <AlertTitle className="text-xs">Degraded</AlertTitle>
          <AlertDescription className="text-xs">{suggestions.diagnostics.join(", ")}</AlertDescription>
        </Alert>
      ) : null}
      {visible.length ? (
        <div className="space-y-1.5">
          {visible.slice(0, 5).map((suggestion) => (
            <div key={suggestion.label_id} className="flex items-start justify-between gap-2">
              <div className="min-w-0 space-y-1">
                <div className="truncate text-sm font-medium">{suggestion.label_name}</div>
                <div className="text-xs text-muted-foreground">score {suggestion.score.toFixed(3)}</div>
                {suggestion.evidence_atoms.length ? (
                  <div className="space-y-0.5">
                    {suggestion.evidence_atoms.slice(0, 2).map((atom) => (
                      <div key={atom.atom_id} className="truncate text-xs text-muted-foreground">
                        {atom.text}
                      </div>
                    ))}
                  </div>
                ) : null}
                {suggestion.negative_evidence_atoms.length ? (
                  <div className="text-xs text-muted-foreground">
                    negative evidence {suggestion.negative_evidence_atoms.length}
                  </div>
                ) : null}
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={disabled || pending || suggestion.already_applied}
                onClick={() => onApply(suggestion.label_name)}
              >
                <Plus className="h-3.5 w-3.5" />
                {suggestion.already_applied ? "Applied" : "Apply"}
              </Button>
            </div>
          ))}
        </div>
      ) : !loading && !error ? (
        <div className="text-xs text-muted-foreground">No label suggestions.</div>
      ) : null}
    </div>
  )
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
    <Item className="px-0 py-0">
      <ItemContent>
        <ItemTitle className="text-sm font-normal text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        <span className="truncate font-medium">{value}</span>
      </ItemActions>
    </Item>
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
