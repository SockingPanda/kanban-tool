import { lazy, Suspense, useEffect, useMemo, useState, type ReactNode } from "react"
import { ChevronDown, ListChecks, Map, MessageSquare, Network } from "lucide-react"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { legalActions, type LegalTaskAction } from "@/features/task-actions/legal-actions"
import { apiTaskGraphToCanvasGraph } from "@/features/task-map/task-graph-adapter"
import type { KanbanApi, LabelSuggestionResult, Run, Task } from "@/lib/api"

import { commentPageState, type CommentSortOrder } from "./comment-list-state"
import { isLongDescription, visibleDescription } from "./description-state"
import type { DetailState } from "./detail-state"
import { MarkdownDescription } from "./markdown"
import { TaskActionPanel, taskActionView } from "./TaskActionPanel"
import { TaskCommentsPanel } from "./TaskCommentsPanel"
import { TaskDependencyPanel } from "./TaskDependencyPanel"
import { TaskEditForm } from "./TaskEditForm"
import { TaskExecutionPlanPanel } from "./TaskExecutionPlanPanel"
import { applySuggestedTaskLabel, TaskLabelsPanel } from "./TaskLabelsPanel"
import { TaskMetadataPanel } from "./TaskMetadataPanel"
import { TaskEventsPanel, TaskRunsPanel } from "./TaskRunsEventsPanel"
import { TaskSummaryHeader } from "./TaskSummaryHeader"
import type { TaskEditDraft } from "./task-draft"
import { Section } from "./task-detail-shared"

export { applySuggestedTaskLabel } from "./TaskLabelsPanel"
export { DependencyGroup } from "./TaskDependencyPanel"
export { MarkdownDescription } from "./markdown"

const TaskGraphCanvas = lazy(() => import("@/features/task-map/TaskGraphCanvas").then((module) => ({ default: module.TaskGraphCanvas })))

type TaskDetailActionOptions = {
  label?: string
  fallbackTaskId?: string | null
  invalidate?: "task" | "steps" | "board-and-task"
}

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
  commentsExpanded = false,
  dependenciesExpanded = false,
  eventsExpanded = false,
  graphExpanded = false,
  runsExpanded = false,
  stepsExpanded = false,
  pendingAction,
  onAction,
  onAddDependency,
  onRemoveDependency,
  onRequestLabelSuggestions,
  onSelectTask,
  onSaveTask,
  onCancelEdit,
  onAddComment,
  onCommentsExpandedChange = () => {},
  onDependenciesExpandedChange = () => {},
  onEventsExpandedChange = () => {},
  onGraphExpandedChange = () => {},
  onRunsExpandedChange = () => {},
  onStepsExpandedChange = () => {},
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
  commentsExpanded?: boolean
  dependenciesExpanded?: boolean
  eventsExpanded?: boolean
  graphExpanded?: boolean
  runsExpanded?: boolean
  stepsExpanded?: boolean
  pendingAction: string | null
  onAction: (action: () => Promise<unknown>, options?: TaskDetailActionOptions) => Promise<unknown>
  onAddDependency: () => Promise<void>
  onRemoveDependency: (parentTaskId: string) => Promise<void>
  onRequestLabelSuggestions?: () => void
  onSelectTask: (taskId: string) => void
  onSaveTask: () => Promise<boolean>
  onCancelEdit: () => void
  onAddComment: () => Promise<void>
  onCommentsExpandedChange?: (value: boolean) => void
  onDependenciesExpandedChange?: (value: boolean) => void
  onEventsExpandedChange?: (value: boolean) => void
  onGraphExpandedChange?: (value: boolean) => void
  onRunsExpandedChange?: (value: boolean) => void
  onStepsExpandedChange?: (value: boolean) => void
}) {
  const [descriptionExpanded, setDescriptionExpanded] = useState(false)
  const [editing, setEditing] = useState(false)
  const [labelInput, setLabelInput] = useState("")
  const [commentSortOrder, setCommentSortOrder] = useState<CommentSortOrder>("newest")
  const [commentPage, setCommentPage] = useState(0)
  const [stepTitle, setStepTitle] = useState("")
  const [attachStepId, setAttachStepId] = useState("")
  const [notRequiredReason, setNotRequiredReason] = useState("")
  const [confirmAction, setConfirmAction] = useState<LegalTaskAction | null>(null)

  useEffect(() => {
    setDescriptionExpanded(false)
    setEditing(false)
    setLabelInput("")
    setCommentSortOrder("newest")
    setCommentPage(0)
    setStepTitle("")
    setAttachStepId("")
    setNotRequiredReason("")
    setConfirmAction(null)
  }, [task?.id])

  const graph = useMemo(() => apiTaskGraphToCanvasGraph(detail.neighborhood), [detail.neighborhood])
  const actions = useMemo(() => (task ? legalActions(task, claimToken, blockReason) : []), [blockReason, claimToken, task])
  const longDescription = useMemo(() => Boolean(task && isLongDescription(task.description)), [task])
  const renderedDescription = useMemo(
    () => (task ? visibleDescription(task.description, descriptionExpanded) : ""),
    [descriptionExpanded, task],
  )
  const commentsPage = useMemo(
    () => commentPageState({ comments: detail.comments, page: commentPage, sortOrder: commentSortOrder }),
    [commentPage, commentSortOrder, detail.comments],
  )
  const actionView = useMemo(() => (task ? taskActionView(task, actions) : null), [actions, task])

  if (!task || !actionView) return null
  const currentTask = task

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
    }, { fallbackTaskId: currentTask.id, label: "label", invalidate: "task" })
  }

  async function removeLabel(labelId: string) {
    if (!api) return
    await onAction(() => api.removeTaskLabel(currentTask.id, labelId), { fallbackTaskId: currentTask.id, label: "label", invalidate: "task" })
  }

  async function applySuggestedLabel(labelName: string) {
    await applySuggestedTaskLabel(api, currentTask.id, labelName, onAction)
  }

  async function createStep() {
    if (!api || !stepTitle.trim()) return
    const title = stepTitle.trim()
    await onAction(async () => {
      const result = await api.createStep(currentTask.id, { title, required: true })
      setStepTitle("")
      return result
    }, { fallbackTaskId: currentTask.id, label: "step", invalidate: "steps" })
  }

  async function attachStep() {
    if (!api || !attachStepId.trim()) return
    const linkedTaskRef = attachStepId.trim()
    await onAction(async () => {
      const result = await api.createStep(currentTask.id, {
        title: "Review linked task " + linkedTaskRef,
        linked_task_ref: linkedTaskRef,
        required: true,
      })
      setAttachStepId("")
      return result
    }, { fallbackTaskId: currentTask.id, label: "step", invalidate: "steps" })
  }

  async function markPlanNotRequired() {
    if (!api || !notRequiredReason.trim()) return
    const reason = notRequiredReason.trim()
    await onAction(async () => {
      const result = await api.markExecutionPlanNotRequired(currentTask.id, reason)
      setNotRequiredReason("")
      return result
    }, { fallbackTaskId: currentTask.id, label: "step", invalidate: "steps" })
  }

  function runAction(action: LegalTaskAction) {
    if (!api) return
    void onAction(() => action.run(api, currentTask), {
      fallbackTaskId: currentTask.id,
      label: action.label.toLowerCase(),
      invalidate: "board-and-task",
    })
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <TaskSummaryHeader task={task} editing={editing} editEnabled={Boolean(editDraft)} detailLoading={detailLoading} onEdit={() => setEditing(true)} />

      <ScrollArea className="flex-1 p-4">
        {editing && editDraft ? (
          <TaskEditForm
            api={api}
            editDraft={editDraft}
            draftDirty={draftDirty}
            pendingAction={pendingAction}
            setEditDraft={setEditDraft}
            onSave={() => void saveAndClose()}
            onCancel={cancelEdit}
          />
        ) : (
          <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(260px,320px)_minmax(420px,1fr)_minmax(220px,260px)] lg:grid-cols-[minmax(260px,320px)_minmax(420px,1fr)]">
            <aside className="min-w-0 space-y-4">
              <TaskDetailPanel
                title="One-hop map"
                icon={<Map className="h-4 w-4" />}
                open={graphExpanded}
                summary={graphExpanded ? `${graph.nodes.length} node${graph.nodes.length === 1 ? "" : "s"}` : "Load map"}
                onOpenChange={onGraphExpandedChange}
              >
                {graph.nodes.length ? (
                  <Suspense fallback={<TaskDetailGraphSkeleton />}>
                    <TaskGraphCanvas graph={graph} selectedTaskId={task.id} onSelectTask={onSelectTask} className="h-[420px] min-h-[320px]" />
                  </Suspense>
                ) : (
                  <Empty className="items-start rounded-md border border-border bg-muted/20 p-3 text-left">
                    <EmptyDescription>No task context graph yet.</EmptyDescription>
                  </Empty>
                )}
              </TaskDetailPanel>
              <TaskDetailPanel
                title="Dependencies"
                icon={<Network className="h-4 w-4" />}
                open={dependenciesExpanded}
                summary={
                  dependenciesExpanded
                    ? `${detail.dependencies.parents.length + detail.dependencies.children.length} link${
                        detail.dependencies.parents.length + detail.dependencies.children.length === 1 ? "" : "s"
                      }`
                    : "Load dependencies"
                }
                onOpenChange={onDependenciesExpandedChange}
              >
                <TaskDependencyPanel
                  parents={detail.dependencies.parents}
                  children={detail.dependencies.children}
                  dependencyInput={dependencyInput}
                  pending={pendingAction === "dependency"}
                  setDependencyInput={setDependencyInput}
                  onAddDependency={() => void onAddDependency()}
                  onRemoveDependency={(parentTaskId) => void onRemoveDependency(parentTaskId)}
                  onSelectTask={onSelectTask}
                />
              </TaskDetailPanel>
            </aside>

            <main className="min-w-0 space-y-4">
              <Section title="Description">
                <MarkdownDescription>{renderedDescription}</MarkdownDescription>
                {longDescription ? (
                  <Button className="mt-2 px-0" variant="ghost" size="sm" onClick={() => setDescriptionExpanded((current) => !current)}>
                    {descriptionExpanded ? "Show less" : "Show more"}
                  </Button>
                ) : null}
              </Section>
              <Separator />
              <TaskDetailPanel
                title="Execution plan"
                icon={<ListChecks className="h-4 w-4" />}
                open={stepsExpanded}
                summary={stepsExpanded ? `${detail.steps?.steps.length ?? 0} step${detail.steps?.steps.length === 1 ? "" : "s"}` : "Load steps"}
                onOpenChange={onStepsExpandedChange}
              >
                <TaskExecutionPlanPanel
                  task={task}
                  steps={detail.steps}
                  pending={pendingAction === "step"}
                  stepTitle={stepTitle}
                  attachStepId={attachStepId}
                  notRequiredReason={notRequiredReason}
                  setStepTitle={setStepTitle}
                  setAttachStepId={setAttachStepId}
                  setNotRequiredReason={setNotRequiredReason}
                  onCreateStep={() => void createStep()}
                  onAttachStep={() => void attachStep()}
                  onMarkNotRequired={() => void markPlanNotRequired()}
                  onSelectTask={onSelectTask}
                />
              </TaskDetailPanel>
              <Separator />
              <Section title="Primary action">
                <TaskActionPanel
                  api={api}
                  task={task}
                  pendingAction={pendingAction}
                  blockReason={blockReason}
                  setBlockReason={setBlockReason}
                  actionView={actionView}
                  onRun={runAction}
                  onConfirm={setConfirmAction}
                />
              </Section>
              <Separator />
              <TaskDetailPanel
                title="Discussion"
                icon={<MessageSquare className="h-4 w-4" />}
                open={commentsExpanded}
                summary={commentsExpanded ? `${commentsPage.total} comment${commentsPage.total === 1 ? "" : "s"}` : "Load comments"}
                onOpenChange={onCommentsExpandedChange}
              >
                <TaskCommentsPanel
                  commentsPage={commentsPage}
                  commentSortOrder={commentSortOrder}
                  setCommentSortOrder={setCommentSortOrder}
                  setCommentPage={setCommentPage}
                  commentBody={commentBody}
                  setCommentBody={setCommentBody}
                  pendingAction={pendingAction}
                  onAddComment={onAddComment}
                />
              </TaskDetailPanel>
              <Separator />
              <TaskRunsPanel activeRun={activeRun} detail={detail} open={runsExpanded} onOpenChange={onRunsExpandedChange} />
              <TaskEventsPanel events={detail.events} open={eventsExpanded} onOpenChange={onEventsExpandedChange} />
            </main>

            <aside className="min-w-0 space-y-4 lg:col-span-2 xl:col-span-1">
              <TaskMetadataPanel task={task} />
              <Section title="Labels">
                <TaskLabelsPanel
                  api={api}
                  task={task}
                  labelInput={labelInput}
                  setLabelInput={setLabelInput}
                  suggestions={labelSuggestions}
                  suggestionsRequested={labelSuggestionsRequested}
                  suggestionsLoading={labelSuggestionsLoading}
                  suggestionsError={labelSuggestionsError}
                  pending={pendingAction === "label"}
                  onAddLabel={() => void addLabel()}
                  onRemoveLabel={(labelId) => void removeLabel(labelId)}
                  onRequestLabelSuggestions={onRequestLabelSuggestions}
                  onApplySuggestedLabel={(labelName) => void applySuggestedLabel(labelName)}
                />
              </Section>
            </aside>
          </div>
        )}
      </ScrollArea>

      <AlertDialog open={Boolean(confirmAction)} onOpenChange={(open) => !open && setConfirmAction(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Confirm action</AlertDialogTitle>
            <AlertDialogDescription>{confirmAction?.confirmation}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant={confirmAction?.danger ? "destructive" : "default"}
              onClick={() => {
                if (confirmAction) runAction(confirmAction)
                setConfirmAction(null)
              }}
            >
              Continue
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

function TaskDetailGraphSkeleton() {
  return (
    <div className="space-y-3 rounded-md border border-border bg-muted/20 p-3">
      <Skeleton className="h-20 w-2/3" />
      <Skeleton className="ml-auto h-20 w-3/4" />
      <Skeleton className="h-20 w-1/2" />
    </div>
  )
}

function TaskDetailPanel({
  children,
  icon,
  open,
  summary,
  title,
  onOpenChange,
}: {
  children: ReactNode
  icon: ReactNode
  open: boolean
  summary: string
  title: string
  onOpenChange: (value: boolean) => void
}) {
  return (
    <Collapsible open={open} onOpenChange={onOpenChange}>
      <Section title={title}>
        <CollapsibleTrigger asChild>
          <Button variant="outline" size="sm" className="mb-3">
            {icon}
            {summary}
            <ChevronDown className="h-4 w-4" />
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent>{open ? children : null}</CollapsibleContent>
      </Section>
    </Collapsible>
  )
}
