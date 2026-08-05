import { lazy, Suspense, useCallback, useEffect, useMemo, useState, type ReactNode } from "react"
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
import { useI18n } from "@/i18n"
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
import { TaskAttachmentsPanel } from "./TaskAttachmentsPanel"
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
  invalidate?: "none" | "task" | "attachments" | "steps" | "board-and-task"
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
  task: Task
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
  const { t } = useI18n()
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
  }, [task.id])

  const graph = useMemo(() => apiTaskGraphToCanvasGraph(detail.neighborhood), [detail.neighborhood])
  const actions = useMemo(() => legalActions(task, claimToken, blockReason), [blockReason, claimToken, task])
  const longDescription = useMemo(() => isLongDescription(task.description), [task.description])
  const renderedDescription = useMemo(
    () => visibleDescription(task.description, descriptionExpanded),
    [descriptionExpanded, task.description],
  )
  const commentsPage = useMemo(
    () => commentPageState({ comments: detail.comments, page: commentPage, sortOrder: commentSortOrder }),
    [commentPage, commentSortOrder, detail.comments],
  )
  const actionView = useMemo(() => taskActionView(task, actions), [actions, task])
  const currentTask = task

  const saveAndClose = useCallback(async () => {
    const saved = await onSaveTask()
    if (saved) setEditing(false)
  }, [onSaveTask])

  const cancelEdit = useCallback(() => {
    onCancelEdit()
    setEditing(false)
  }, [onCancelEdit])

  const addLabel = useCallback(async () => {
    if (!api || !labelInput.trim()) return
    const name = labelInput.trim()
    await onAction(async () => {
      const updated = await api.addTaskLabel(currentTask.id, name)
      setLabelInput("")
      return updated
    }, { fallbackTaskId: currentTask.id, label: "label", invalidate: "task" })
  }, [api, currentTask.id, labelInput, onAction])

  const removeLabel = useCallback(async (labelId: string) => {
    if (!api) return
    await onAction(() => api.removeTaskLabel(currentTask.id, labelId), { fallbackTaskId: currentTask.id, label: "label", invalidate: "task" })
  }, [api, currentTask.id, onAction])

  const applySuggestedLabel = useCallback(async (labelName: string) => {
    await applySuggestedTaskLabel(api, currentTask.id, labelName, onAction)
  }, [api, currentTask.id, onAction])

  const createStep = useCallback(async () => {
    if (!api || !stepTitle.trim()) return
    const title = stepTitle.trim()
    await onAction(async () => {
      const result = await api.createStep(currentTask.id, { title, required: true })
      setStepTitle("")
      return result
    }, { fallbackTaskId: currentTask.id, label: "step", invalidate: "steps" })
  }, [api, currentTask.id, onAction, stepTitle])

  const attachStep = useCallback(async () => {
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
  }, [api, attachStepId, currentTask.id, onAction])

  const markPlanNotRequired = useCallback(async () => {
    if (!api || !notRequiredReason.trim()) return
    const reason = notRequiredReason.trim()
    await onAction(async () => {
      const result = await api.markExecutionPlanNotRequired(currentTask.id, reason)
      setNotRequiredReason("")
      return result
    }, { fallbackTaskId: currentTask.id, label: "step", invalidate: "steps" })
  }, [api, currentTask.id, notRequiredReason, onAction])

  const runAction = useCallback((action: LegalTaskAction) => {
    if (!api) return
    void onAction(() => action.run(api, currentTask), {
      fallbackTaskId: currentTask.id,
      label: action.label.toLowerCase(),
      invalidate: "board-and-task",
    })
  }, [api, currentTask, onAction])

  const handleSaveTask = useCallback(() => void saveAndClose(), [saveAndClose])
  const handleCreateStep = useCallback(() => void createStep(), [createStep])
  const handleAttachStep = useCallback(() => void attachStep(), [attachStep])
  const handleMarkPlanNotRequired = useCallback(() => void markPlanNotRequired(), [markPlanNotRequired])
  const handleAddDependency = useCallback(() => void onAddDependency(), [onAddDependency])
  const handleRemoveDependency = useCallback((parentTaskId: string) => void onRemoveDependency(parentTaskId), [onRemoveDependency])
  const handleAddLabel = useCallback(() => void addLabel(), [addLabel])
  const handleRemoveLabel = useCallback((labelId: string) => void removeLabel(labelId), [removeLabel])
  const handleApplySuggestedLabel = useCallback((labelName: string) => void applySuggestedLabel(labelName), [applySuggestedLabel])

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
            onSave={handleSaveTask}
            onCancel={cancelEdit}
          />
        ) : (
          <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(260px,320px)_minmax(420px,1fr)_minmax(220px,260px)] lg:grid-cols-[minmax(260px,320px)_minmax(420px,1fr)]">
            <aside className="min-w-0 space-y-4">
              <TaskDetailPanel
                title={t("One-hop map")}
                icon={<Map className="h-4 w-4" />}
                open={graphExpanded}
                summary={graphExpanded ? t(graph.nodes.length === 1 ? "{count} node" : "{count} nodes", { count: graph.nodes.length }) : t("Load map")}
                onOpenChange={onGraphExpandedChange}
              >
                {graph.nodes.length ? (
                  <Suspense fallback={<TaskDetailGraphSkeleton />}>
                    <TaskGraphCanvas graph={graph} selectedTaskId={task.id} onSelectTask={onSelectTask} className="h-[420px] min-h-[320px]" />
                  </Suspense>
                ) : (
                  <Empty className="items-start rounded-md border border-border bg-muted/20 p-3 text-left">
                    <EmptyDescription>{t("No task context graph yet.")}</EmptyDescription>
                  </Empty>
                )}
              </TaskDetailPanel>
              <TaskDetailPanel
                title={t("Dependencies")}
                icon={<Network className="h-4 w-4" />}
                open={dependenciesExpanded}
                summary={
                  dependenciesExpanded
                    ? t(
                        detail.dependencies.parents.length + detail.dependencies.children.length === 1 ? "{count} link" : "{count} links",
                        { count: detail.dependencies.parents.length + detail.dependencies.children.length },
                      )
                    : t("Load dependencies")
                }
                onOpenChange={onDependenciesExpandedChange}
              >
                <TaskDependencyPanel
                  parents={detail.dependencies.parents}
                  children={detail.dependencies.children}
                  dependencyInput={dependencyInput}
                  pending={pendingAction === "dependency"}
                  setDependencyInput={setDependencyInput}
                  onAddDependency={handleAddDependency}
                  onRemoveDependency={handleRemoveDependency}
                  onSelectTask={onSelectTask}
                />
              </TaskDetailPanel>
            </aside>

            <main className="min-w-0 space-y-4">
              <Section title={t("Description")}>
                <MarkdownDescription>{renderedDescription}</MarkdownDescription>
                {longDescription ? (
                  <Button className="mt-2 px-0" variant="ghost" size="sm" onClick={() => setDescriptionExpanded((current) => !current)}>
                    {descriptionExpanded ? t("Show less") : t("Show more")}
                  </Button>
                ) : null}
              </Section>
              <Separator />
              <TaskDetailPanel
                title={t("Execution plan")}
                icon={<ListChecks className="h-4 w-4" />}
                open={stepsExpanded}
                summary={
                  stepsExpanded
                    ? t((detail.steps?.steps.length ?? 0) === 1 ? "{count} step" : "{count} steps", { count: detail.steps?.steps.length ?? 0 })
                    : t("Load steps")
                }
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
                  onCreateStep={handleCreateStep}
                  onAttachStep={handleAttachStep}
                  onMarkNotRequired={handleMarkPlanNotRequired}
                  onSelectTask={onSelectTask}
                />
              </TaskDetailPanel>
              <Separator />
              <Section title={t("Primary action")}>
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
                title={t("Discussion")}
                icon={<MessageSquare className="h-4 w-4" />}
                open={commentsExpanded}
                summary={commentsExpanded ? t(commentsPage.total === 1 ? "{count} comment" : "{count} comments", { count: commentsPage.total }) : t("Load comments")}
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
              <Section title={t("Labels")}>
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
                  onAddLabel={handleAddLabel}
                  onRemoveLabel={handleRemoveLabel}
                  onRequestLabelSuggestions={onRequestLabelSuggestions}
                  onApplySuggestedLabel={handleApplySuggestedLabel}
                />
              </Section>
              <Section title={t("Attachments")}>
                <TaskAttachmentsPanel
                  api={api}
                  task={task}
                  pending={pendingAction === "attachment"}
                  onAction={onAction}
                />
              </Section>
            </aside>
          </div>
        )}
      </ScrollArea>

      <AlertDialog open={Boolean(confirmAction)} onOpenChange={(open) => !open && setConfirmAction(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("Confirm action")}</AlertDialogTitle>
            <AlertDialogDescription>
              {confirmAction?.confirmation ? t(confirmAction.confirmation.key, confirmAction.confirmation.values) : null}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant={confirmAction?.danger ? "destructive" : "default"}
              onClick={() => {
                if (confirmAction) runAction(confirmAction)
                setConfirmAction(null)
              }}
            >
              {t("Continue")}
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
