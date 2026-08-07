import { FormEvent, useCallback, useMemo, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"

import { AppShell } from "@/app/AppShell"
import { createBoardSwitchInvalidationTargets, createBoardSwitchReset } from "@/app/board-switch-state"
import { useRuntimeConfigState, errorMessage } from "@/app/useRuntimeConfigState"
import { useSelectedTaskDetailState } from "@/app/useSelectedTaskDetailState"
import { useTaskCollectionState } from "@/app/useTaskCollectionState"
import { useTaskCreationDialogState } from "@/app/useTaskCreationDialogState"
import { useTaskMutations } from "@/app/useTaskMutations"
import { executeDragTransition, planDragTransition } from "@/features/board/drag-policy"
import { useEventPoller } from "@/features/events/useEventPoller"
import type { OperatorView } from "@/features/navigation/view-types"
import { Task, TaskStatus } from "@/lib/api"
import { switchRuntimeBoard } from "@/lib/runtime-board"
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Textarea } from "@/components/ui/textarea"
import { useI18n } from "@/i18n"

type PlannedDragTransition = {
  task: Task
  plan: Extract<ReturnType<typeof planDragTransition>, { ok: true }>
}

function App() {
  const queryClient = useQueryClient()
  const { locale, t } = useI18n()
  const [view, setView] = useState<OperatorView>("board")
  const [dragReasonRequest, setDragReasonRequest] = useState<PlannedDragTransition | null>(null)
  const [dragConfirmRequest, setDragConfirmRequest] = useState<PlannedDragTransition | null>(null)
  const [dragReasonDraft, setDragReasonDraft] = useState("")

  const runtimeState = useRuntimeConfigState(locale)
  const { api, config, error, setConfig, setError } = runtimeState
  const reportError = useCallback((err: unknown) => setError(errorMessage(err, locale)), [locale, setError])

  useEventPoller({
    api,
    enabled: Boolean(api),
    onError: reportError,
  })

  const taskCollectionState = useTaskCollectionState(api, view, reportError)
  const {
    boardsQuery,
    columns,
    debouncedSearch,
    enabled: taskCollectionEnabled,
    groupedTasks,
    hasNext,
    hasPrevious,
    lastOffset,
    lastRefreshAt,
    listSort,
    page,
    pageOffset,
    planFilters,
    priorityFilters,
    queueCounts,
    rowsPerPage,
    search,
    searchMeta,
    setLastRefreshAt,
    setListSort,
    setPageOffset,
    setPlanFilters,
    setPriorityFilters,
    setRowsPerPage,
    setSearch,
    setShowArchived,
    setStatusFilter,
    showArchived,
    statsQuery,
    statusFilter,
    tasks,
    tasksQuery,
  } = taskCollectionState
  const taskDetailState = useSelectedTaskDetailState(
    api,
    view,
    tasks,
    taskCollectionEnabled,
    config?.actor ?? null,
    reportError,
  )
  const {
    activeRun,
    blockReason,
    claimToken,
    claimTokens,
    commentBody,
    dependencyInput,
    dependencySnapshot,
    detail,
    detailLoading,
    draftState,
    labelSuggestionsQuery,
    labelSuggestionsRequested,
    selectedId,
    selectedTask,
    setBlockReason,
    setClaimTokens,
    setCommentBody,
    setDependencyInput,
    setDraftState,
    setLabelSuggestionsRequested,
    setSelectedId,
    taskCommentsExpanded,
    taskDependenciesExpanded,
    taskEventsExpanded,
    taskGraphExpanded,
    taskRunsExpanded,
    taskStepsExpanded,
    setTaskCommentsExpanded,
    setTaskDependenciesExpanded,
    setTaskEventsExpanded,
    setTaskGraphExpanded,
    setTaskRunsExpanded,
    setTaskStepsExpanded,
  } = taskDetailState
  const creationDialogState = useTaskCreationDialogState()
  const {
    description: newDescription,
    firstStepTitle: newFirstStepTitle,
    open: taskCreationOpen,
    setDescription: setTaskCreationDescription,
    setFirstStepTitle: setTaskCreationFirstStepTitle,
    setOpen: setTaskCreationOpen,
    setTitle: setTaskCreationTitle,
    title: newTitle,
  } = creationDialogState
  const taskMutations = useTaskMutations({
    api,
    commentBody,
    config,
    creation: creationDialogState,
    dependencyInput,
    draftState,
    queryClient,
    selectedId,
    selectedTask,
    setClaimTokens,
    setCommentBody,
    setDependencyInput,
    setDraftState,
    setError,
    setLabelSuggestionsRequested,
    setSelectedId,
  })

  const tasksById = useMemo(
    () => new Map(tasks.map((task) => [task.id, task])),
    [tasks],
  )

  const requestLabelSuggestions = useCallback(async () => {
    if (!api || !selectedId) return
    setLabelSuggestionsRequested(true)
    await labelSuggestionsQuery.refetch()
  }, [api, labelSuggestionsQuery, selectedId, setLabelSuggestionsRequested])

  const dropTask = useCallback(
    (taskId: string, targetStatus: TaskStatus) => {
      if (!api) return
      const task = tasksById.get(taskId)
      if (!task) return
      const token = claimTokens[task.id] ?? null
      const plan = planDragTransition(task, targetStatus, token)
      if (!plan.ok) {
        setError(t(plan.reason.key, plan.reason.values))
        return
      }
      if (plan.promptReason) {
        setDragReasonDraft("")
        setDragReasonRequest({ task, plan })
        return
      }
      requestDragExecution({ task, plan })
    },
    [api, claimTokens, tasksById, setError, t],
  )

  const executePlannedDrag = useCallback(
    async ({ task, plan }: PlannedDragTransition) => {
      if (!api) return
      await taskMutations.runAction(() => executeDragTransition(api, task, plan), {
        label: "transition",
        fallbackTaskId: task.id,
        invalidate: "board-and-task",
      })
    },
    [api, taskMutations],
  )

  const requestDragExecution = useCallback(
    (request: PlannedDragTransition) => {
      if (request.plan.confirm) {
        setDragConfirmRequest(request)
        return
      }
      void executePlannedDrag(request)
    },
    [executePlannedDrag],
  )

  const submitDragReason = useCallback(
    (event: FormEvent) => {
      event.preventDefault()
      if (!dragReasonRequest) return
      const reason = dragReasonDraft.trim()
      if (!reason) {
        setError(t("A block reason is required."))
        return
      }
      const request = {
        task: dragReasonRequest.task,
        plan: {
          ...dragReasonRequest.plan,
          body: { ...dragReasonRequest.plan.body, reason },
          promptReason: false,
        },
      }
      setDragReasonRequest(null)
      setDragReasonDraft("")
      requestDragExecution(request)
    },
    [dragReasonDraft, dragReasonRequest, requestDragExecution, setError, t],
  )

  const switchBoard = useCallback(
    async (board: string) => {
      if (!config || board === config.board) return
      taskMutations.runAction(async () => {
        const nextConfig = await switchRuntimeBoard(config, board)
        const reset = createBoardSwitchReset({
          config: nextConfig,
          selectedId,
          pageOffset,
          newTitle,
          newDescription,
          blockReason,
          dependencyInput,
          commentBody,
          draftState,
          claimTokens,
          lastRefreshAt,
          error,
        })
        setConfig(reset.config)
        setSelectedId(reset.selectedId)
        setPageOffset(reset.pageOffset)
        setTaskCreationTitle(reset.newTitle)
        setTaskCreationDescription(reset.newDescription)
        setBlockReason(reset.blockReason)
        setDependencyInput(reset.dependencyInput)
        setCommentBody(reset.commentBody)
        setDraftState(reset.draftState)
        setClaimTokens(reset.claimTokens)
        setLastRefreshAt(reset.lastRefreshAt)
        setError(reset.error)
        await Promise.all(
          createBoardSwitchInvalidationTargets({
            previousBoard: config.board,
            nextBoard: reset.config.board,
          }).map((queryKey) => queryClient.invalidateQueries({ queryKey })),
        )
        return reset.config
      }, { label: "board", fallbackTaskId: selectedId, invalidate: "none" })
    },
    [
      blockReason,
      claimTokens,
      commentBody,
      config,
      dependencyInput,
      draftState,
      error,
      lastRefreshAt,
      newDescription,
      newTitle,
      pageOffset,
      queryClient,
      selectedId,
      setBlockReason,
      setClaimTokens,
      setCommentBody,
      setConfig,
      setDependencyInput,
      setDraftState,
      setError,
      setLastRefreshAt,
      setPageOffset,
      setSelectedId,
      setTaskCreationDescription,
      setTaskCreationTitle,
      taskMutations,
    ],
  )

  const navigation = useMemo(
    () => ({
      view,
      sidebarOpen: runtimeState.sidebarOpen,
      setView,
      setSidebarOpen: runtimeState.setSidebarOpen,
    }),
    [runtimeState.setSidebarOpen, runtimeState.sidebarOpen, view],
  )

  const runtime = useMemo(
    () => ({
      api,
      boards: boardsQuery.data ?? [],
      boardsError: boardsQuery.error ? errorMessage(boardsQuery.error, locale) : null,
      boardsLoading: boardsQuery.isLoading,
      config,
      error,
      pendingAction: taskMutations.pendingAction,
      queueCounts,
      themeMode: runtimeState.themeMode,
      lastRefreshAt,
    }),
    [
      api,
      config,
      error,
      runtimeState.themeMode,
      boardsQuery.data,
      boardsQuery.error,
      boardsQuery.isLoading,
      lastRefreshAt,
      queueCounts,
      taskMutations.pendingAction,
    ],
  )

  const taskCollection = useMemo(
    () => ({
      canGoLastPage: lastOffset !== null && lastOffset !== page.offset,
      columns,
      debouncedSearch,
      groupedTasks,
      hasNextPage: hasNext,
      hasPreviousPage: hasPrevious,
      listSort,
      page,
      planFilters,
      priorityFilters,
      rowsPerPage,
      search,
      searchMeta,
      showArchived,
      statusFilter,
      tasks,
      tasksRefreshing: taskCollectionEnabled && tasksQuery.isFetching,
    }),
    [
      columns,
      debouncedSearch,
      groupedTasks,
      hasNext,
      hasPrevious,
      lastOffset,
      listSort,
      page,
      planFilters,
      priorityFilters,
      rowsPerPage,
      search,
      searchMeta,
      showArchived,
      statusFilter,
      taskCollectionEnabled,
      tasks,
      tasksQuery.isFetching,
    ],
  )

  const taskDetail = useMemo(
    () => ({
      activeRun,
      blockReason,
      claimToken,
      commentBody,
      dependencyInput,
      dependencySnapshot,
      detail,
      detailLoading,
      draftDirty: draftState?.dirty ?? false,
      editDraft: draftState?.draft ?? null,
      labelSuggestions: labelSuggestionsQuery.data ?? null,
      labelSuggestionsError: labelSuggestionsQuery.error
        ? errorMessage(labelSuggestionsQuery.error, locale)
        : null,
      labelSuggestionsLoading: labelSuggestionsQuery.isFetching,
      labelSuggestionsRequested,
      selectedId,
      selectedTask,
      taskCommentsExpanded,
      taskDependenciesExpanded,
      taskEventsExpanded,
      taskGraphExpanded,
      taskRunsExpanded,
      taskStepsExpanded,
    }),
    [
      activeRun,
      blockReason,
      claimToken,
      commentBody,
      dependencyInput,
      dependencySnapshot,
      detail,
      detailLoading,
      draftState,
      labelSuggestionsQuery.data,
      labelSuggestionsQuery.error,
      labelSuggestionsQuery.isFetching,
      labelSuggestionsRequested,
      selectedId,
      selectedTask,
      taskCommentsExpanded,
      taskDependenciesExpanded,
      taskEventsExpanded,
      taskGraphExpanded,
      taskRunsExpanded,
      taskStepsExpanded,
    ],
  )

  const changeBoard = useCallback((board: string) => void switchBoard(board), [switchBoard])
  const closeTaskDetail = useCallback(() => setSelectedId(null), [setSelectedId])
  const firstPage = useCallback(() => setPageOffset(0), [setPageOffset])
  const lastPage = useCallback(() => setPageOffset(lastOffset ?? pageOffset), [lastOffset, pageOffset, setPageOffset])
  const nextPage = useCallback(() => setPageOffset((current) => current + rowsPerPage), [rowsPerPage, setPageOffset])
  const previousPage = useCallback(
    () => setPageOffset((current) => Math.max(0, current - rowsPerPage)),
    [rowsPerPage, setPageOffset],
  )
  const refreshTasks = useCallback(() => {
    const refreshes: Promise<unknown>[] = [statsQuery.refetch()]
    if (taskCollectionEnabled) refreshes.push(tasksQuery.refetch())
    void Promise.all(refreshes)
  }, [statsQuery, taskCollectionEnabled, tasksQuery])
  const resetListFilters = useCallback(() => {
    setStatusFilter("all")
    setPriorityFilters([])
    setPlanFilters([])
    setPageOffset(0)
  }, [setPageOffset, setPlanFilters, setPriorityFilters, setStatusFilter])
  const requestLabelSuggestionsCommand = useCallback(() => void requestLabelSuggestions(), [requestLabelSuggestions])
  const setRowsPerPageCommand = useCallback(
    (value: number) => {
      setRowsPerPage(value)
      setPageOffset(0)
    },
    [setPageOffset, setRowsPerPage],
  )

  const commands = useMemo(
    () => ({
      addComment: taskMutations.addComment,
      addDependency: taskMutations.addDependency,
      cancelTaskEdit: taskMutations.cancelTaskEdit,
      changeBoard,
      closeTaskDetail,
      createTask: taskMutations.createTask,
      cycleThemeMode: runtimeState.cycleThemeMode,
      dropTask,
      firstPage,
      lastPage,
      nextPage,
      previousPage,
      refreshTasks,
      removeDependency: taskMutations.removeDependency,
      requestLabelSuggestions: requestLabelSuggestionsCommand,
      resetListFilters,
      saveTask: taskMutations.saveTask,
      selectTask: setSelectedId,
      setBlockReason,
      setCommentBody,
      setDependencyInput,
      setEditDraft: taskMutations.updateDraft,
      setListSort,
      setPlanFilters,
      setPriorityFilters,
      setRowsPerPage: setRowsPerPageCommand,
      setSearch,
      setShowArchived,
      setSidebarOpen: runtimeState.setSidebarOpen,
      setStatusFilter,
      setTaskCommentsExpanded,
      setTaskCreationDescription,
      setTaskCreationFirstStepTitle,
      setTaskCreationOpen,
      setTaskCreationTitle,
      setTaskDependenciesExpanded,
      setTaskEventsExpanded,
      setTaskGraphExpanded,
      setTaskRunsExpanded,
      setTaskStepsExpanded,
      setView,
      runAction: taskMutations.runAction,
    }),
    [
      changeBoard,
      closeTaskDetail,
      dropTask,
      firstPage,
      lastPage,
      nextPage,
      previousPage,
      refreshTasks,
      requestLabelSuggestionsCommand,
      resetListFilters,
      runtimeState.cycleThemeMode,
      runtimeState.setSidebarOpen,
      setBlockReason,
      setCommentBody,
      setDependencyInput,
      setListSort,
      setPlanFilters,
      setPriorityFilters,
      setRowsPerPageCommand,
      setSearch,
      setSelectedId,
      setShowArchived,
      setStatusFilter,
      setTaskCommentsExpanded,
      setTaskCreationDescription,
      setTaskCreationFirstStepTitle,
      setTaskCreationOpen,
      setTaskCreationTitle,
      setTaskDependenciesExpanded,
      setTaskEventsExpanded,
      setTaskGraphExpanded,
      setTaskRunsExpanded,
      setTaskStepsExpanded,
      taskMutations,
    ],
  )

  const taskCreation = useMemo(
    () => ({
      description: newDescription,
      firstStepTitle: newFirstStepTitle,
      open: taskCreationOpen,
      title: newTitle,
    }),
    [newDescription, newFirstStepTitle, newTitle, taskCreationOpen],
  )

  return (
    <>
      <AppShell
        runtime={runtime}
        navigation={navigation}
        taskCollection={taskCollection}
        taskDetail={taskDetail}
        taskCreation={taskCreation}
        commands={commands}
      />
      <Dialog
        open={Boolean(dragReasonRequest)}
        onOpenChange={(open) => {
          if (!open) {
            setDragReasonRequest(null)
            setDragReasonDraft("")
          }
        }}
      >
        <DialogContent>
          <form onSubmit={submitDragReason} className="space-y-4">
            <DialogHeader>
              <DialogTitle>{t("Block reason")}</DialogTitle>
              <DialogDescription>{t("Record why this task is being moved to blocked.")}</DialogDescription>
            </DialogHeader>
            <Textarea
              aria-label={t("Block reason")}
              name="block-reason"
              autoComplete="off"
              value={dragReasonDraft}
              onChange={(event) => setDragReasonDraft(event.target.value)}
              placeholder={t("Block reason")}
            />
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => {
                  setDragReasonRequest(null)
                  setDragReasonDraft("")
                }}
              >
                {t("Cancel")}
              </Button>
              <Button type="submit" disabled={!dragReasonDraft.trim()}>
                {t("Continue")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <AlertDialog
        open={Boolean(dragConfirmRequest)}
        onOpenChange={(open) => {
          if (!open) setDragConfirmRequest(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("Confirm transition")}</AlertDialogTitle>
            <AlertDialogDescription>
              {dragConfirmRequest?.plan.confirm ? t(dragConfirmRequest.plan.confirm.key, dragConfirmRequest.plan.confirm.values) : null}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                const request = dragConfirmRequest
                setDragConfirmRequest(null)
                if (request) void executePlannedDrag(request)
              }}
            >
              {t("Continue")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

export default App
