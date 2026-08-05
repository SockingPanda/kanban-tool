import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react"
import { ChevronDown, ListChecks, MessageSquare, Network } from "lucide-react"

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
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { legalActions, type LegalTaskAction } from "@/features/task-actions/legal-actions"
import { useI18n } from "@/i18n"
import type { KanbanApi, LabelSuggestionResult, Run, Task } from "@/lib/api"

import { commentPageState, type CommentSortOrder } from "./comment-list-state"
import { isLongDescription, visibleDescription } from "./description-state"
import type { DetailState } from "./detail-state"
import { MarkdownDescription } from "./markdown"
import { TaskActionPanel, taskActionView } from "./TaskActionPanel"
import { TaskCommentsPanel } from "./TaskCommentsPanel"
import { TaskDependencyPanel } from "./TaskDependencyPanel"
import { TaskExecutionPlanPanel } from "./TaskExecutionPlanPanel"
import { TaskMetadataPanel } from "./TaskMetadataPanel"
import { TaskEventsPanel, TaskRunsPanel } from "./TaskRunsEventsPanel"
import { TaskSummaryHeader } from "./TaskSummaryHeader"
import type { TaskEditDraft } from "./task-draft"
import { Section } from "./task-detail-shared"

export { DependencyGroup } from "./TaskDependencyPanel"
export { MarkdownDescription } from "./markdown"

type TaskDetailActionOptions = {
  label?: string
  fallbackTaskId?: string | null
  invalidate?: "task" | "steps" | "board-and-task"
}

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
  detailLoading,
  commentsExpanded = false,
  dependenciesExpanded = false,
  eventsExpanded = false,
  runsExpanded = false,
  stepsExpanded = false,
  pendingAction,
  onAction,
  onAddDependency,
  onRemoveDependency,
  onSelectTask,
  onAddComment,
  onCommentsExpandedChange = () => {},
  onDependenciesExpandedChange = () => {},
  onEventsExpandedChange = () => {},
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
  const [commentSortOrder, setCommentSortOrder] = useState<CommentSortOrder>("newest")
  const [commentPage, setCommentPage] = useState(0)
  const [stepTitle, setStepTitle] = useState("")
  const [attachStepId, setAttachStepId] = useState("")
  const [notRequiredReason, setNotRequiredReason] = useState("")
  const [confirmAction, setConfirmAction] = useState<LegalTaskAction | null>(null)

  useEffect(() => {
    setDescriptionExpanded(false)
    setCommentSortOrder("newest")
    setCommentPage(0)
    setStepTitle("")
    setAttachStepId("")
    setNotRequiredReason("")
    setConfirmAction(null)
  }, [task.id])

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

  const handleCreateStep = useCallback(() => void createStep(), [createStep])
  const handleAttachStep = useCallback(() => void attachStep(), [attachStep])
  const handleMarkPlanNotRequired = useCallback(() => void markPlanNotRequired(), [markPlanNotRequired])
  const handleAddDependency = useCallback(() => void onAddDependency(), [onAddDependency])
  const handleRemoveDependency = useCallback((parentTaskId: string) => void onRemoveDependency(parentTaskId), [onRemoveDependency])

  return (
    <div className="flex h-full min-h-0 flex-col">
      <TaskSummaryHeader task={task} detailLoading={detailLoading} />

      <ScrollArea className="flex-1 p-4">
        <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(260px,320px)_minmax(420px,1fr)_minmax(220px,260px)] lg:grid-cols-[minmax(260px,320px)_minmax(420px,1fr)]">
            <aside className="min-w-0 space-y-4">
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
            </aside>
          </div>
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
