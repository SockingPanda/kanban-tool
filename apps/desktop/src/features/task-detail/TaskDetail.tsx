import { CircleDot, FileText, GitBranch, MessageSquare, Save } from "lucide-react"
import type { ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import { legalActions } from "@/features/task-actions/legal-actions"
import type { KanbanApi, Run, Task, TaskStatus } from "@/lib/api"
import { formatRelativeTime, shortId } from "@/lib/utils"

import type { DetailState } from "./detail-state"
import type { TaskEditDraft } from "./task-draft"

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
  onSaveTask,
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
  onAction: (action: () => Promise<unknown>) => Promise<unknown>
  onAddDependency: () => Promise<void>
  onSaveTask: () => Promise<void>
  onAddComment: () => Promise<void>
}) {
  if (!task) {
    return <aside className="w-[420px] border-l border-neutral-200 bg-white p-4 text-sm text-neutral-500">No task selected.</aside>
  }

  const actions = legalActions(task, claimToken, blockReason)

  return (
    <aside className="flex w-[420px] shrink-0 flex-col border-l border-neutral-200 bg-white">
      <div className="border-b border-neutral-200 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-xs text-neutral-500">#{task.seq} {shortId(task.id)}</div>
            <h2 className="mt-1 text-lg font-semibold leading-snug">{task.title}</h2>
          </div>
          <div className="flex shrink-0 flex-col items-end gap-1">
            <Badge variant={badgeVariant(task.status)}>{task.status}</Badge>
            {detailLoading ? <span className="text-xs text-neutral-500">refreshing</span> : null}
          </div>
        </div>
        <p className="mt-2 text-sm text-neutral-600">{task.description || "No description yet."}</p>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {editDraft ? (
          <>
            <Section title="Task detail">
              <div className="space-y-2">
                {draftDirty ? <div className="text-xs font-medium text-amber-700">Unsaved changes</div> : null}
                <Input
                  value={editDraft.title}
                  onChange={(event) => setEditDraft({ ...editDraft, title: event.target.value })}
                />
                <Textarea
                  className="min-h-28"
                  value={editDraft.description}
                  onChange={(event) => setEditDraft({ ...editDraft, description: event.target.value })}
                  placeholder="Description"
                />
                <div className="grid grid-cols-2 gap-2">
                  <Input
                    value={editDraft.assignee}
                    onChange={(event) => setEditDraft({ ...editDraft, assignee: event.target.value })}
                    placeholder="Assignee"
                  />
                  <Input
                    type="number"
                    value={editDraft.priority}
                    onChange={(event) => setEditDraft({ ...editDraft, priority: event.target.value })}
                    placeholder="Priority"
                  />
                  <Input
                    type="datetime-local"
                    value={editDraft.scheduledAt}
                    onChange={(event) => setEditDraft({ ...editDraft, scheduledAt: event.target.value })}
                  />
                  <Input
                    type="datetime-local"
                    value={editDraft.dueAt}
                    onChange={(event) => setEditDraft({ ...editDraft, dueAt: event.target.value })}
                  />
                </div>
                <Button disabled={!api || pendingAction === "save" || !editDraft.title.trim()} onClick={() => void onSaveTask()}>
                  <Save className="h-4 w-4" />
                  {pendingAction === "save" ? "Saving" : "Save"}
                </Button>
              </div>
            </Section>

            <Separator className="my-4" />
          </>
        ) : null}

        <Section title="Legal transitions">
          <div className="grid grid-cols-2 gap-2">
            {actions.map((action) => (
              <Button
                key={action.label}
                variant={action.danger ? "destructive" : "secondary"}
                disabled={!api || Boolean(pendingAction) || !action.enabled}
                onClick={() => {
                  if (!api) return
                  void onAction(() => action.run(api, task))
                }}
              >
                <action.icon className="h-4 w-4" />
                {action.label}
              </Button>
            ))}
          </div>
          {task.status === "blocked" ? (
            <div className="mt-2 text-xs text-neutral-500">Unblock asks the service to recompute schedule and dependency state.</div>
          ) : null}
          {task.status === "running" ? (
            <Textarea
              className="mt-3"
              placeholder="Block reason"
              value={blockReason}
              onChange={(event) => setBlockReason(event.target.value)}
            />
          ) : null}
        </Section>

        <Separator className="my-4" />

        <Section title="Dependencies">
          <div className="space-y-3">
            <DependencyGroup title="Parents" tasks={detail.dependencies.parents} />
            <DependencyGroup title="Children" tasks={detail.dependencies.children} />
            <div className="flex gap-2">
              <Input
                value={dependencyInput}
                onChange={(event) => setDependencyInput(event.target.value)}
                placeholder="Parent task id"
              />
              <Button variant="outline" disabled={!dependencyInput.trim() || pendingAction === "dependency"} onClick={() => void onAddDependency()}>
                <GitBranch className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </Section>

        <Separator className="my-4" />

        <Section title="Comments">
          <div className="space-y-3">
            <div className="space-y-2">
              {detail.comments.length ? (
                detail.comments.slice(-4).map((comment) => (
                  <div key={comment.id} className="rounded-md border border-neutral-200 bg-neutral-50 p-2 text-sm">
                    <div className="mb-1 flex items-center justify-between text-xs text-neutral-500">
                      <span>{comment.author}</span>
                      <span>{formatRelativeTime(comment.created_at)}</span>
                    </div>
                    <div className="whitespace-pre-wrap text-neutral-800">{comment.body}</div>
                  </div>
                ))
              ) : (
                <div className="text-sm text-neutral-500">No comments yet.</div>
              )}
            </div>
            <div className="flex gap-2">
              <Input
                value={commentBody}
                onChange={(event) => setCommentBody(event.target.value)}
                placeholder="Add handoff note"
              />
              <Button variant="outline" disabled={!commentBody.trim() || pendingAction === "comment"} onClick={() => void onAddComment()}>
                <MessageSquare className="h-4 w-4" />
              </Button>
            </div>
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
                <div className="mt-3 rounded-md border border-neutral-200 bg-neutral-950 p-2 text-xs text-neutral-50">
                  <div className="mb-2 flex items-center justify-between text-neutral-400">
                    <span className="flex items-center gap-1">
                      <FileText className="h-3.5 w-3.5" />
                      log
                    </span>
                    {detail.runLog.truncated ? <span>truncated</span> : null}
                  </div>
                  <pre className="max-h-40 overflow-auto whitespace-pre-wrap font-mono leading-relaxed">
                    {detail.runLog.content || "(empty)"}
                  </pre>
                </div>
              ) : null}
            </div>
          ) : (
            <div className="text-sm text-neutral-500">No runs yet.</div>
          )}
        </Section>

        <Separator className="my-4" />

        <Section title="Event timeline">
          <div className="space-y-2">
            {detail.events.slice().reverse().map((event) => (
              <div key={event.id} className="grid grid-cols-[auto_1fr] gap-2 text-sm">
                <CircleDot className="mt-0.5 h-4 w-4 text-neutral-400" />
                <div>
                  <div className="font-medium">{event.kind}</div>
                  <div className="text-xs text-neutral-500">
                    {formatRelativeTime(event.created_at)} by {event.actor ?? "system"}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Section>
      </div>
    </aside>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-normal text-neutral-500">{title}</h3>
      {children}
    </section>
  )
}

function DependencyGroup({ title, tasks }: { title: string; tasks: Task[] }) {
  return (
    <div>
      <div className="mb-1 text-xs text-neutral-500">{title}</div>
      <div className="flex flex-wrap gap-1">
        {tasks.length ? (
          tasks.map((task) => (
            <Badge key={task.id} variant={task.status === "done" ? "ready" : task.status === "blocked" ? "blocked" : "secondary"}>
              #{task.seq} {task.status}
            </Badge>
          ))
        ) : (
          <span className="text-sm text-neutral-400">none</span>
        )}
      </div>
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-neutral-500">{label}</span>
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
