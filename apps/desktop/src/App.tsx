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

type PlannedDragTransition = {
  task: Task
  plan: Extract<ReturnType<typeof planDragTransition>, { ok: true }>
}

function App() {
  const queryClient = useQueryClient()
  const [view, setView] = useState<OperatorView>("board")
  const [dragReasonRequest, setDragReasonRequest] = useState<PlannedDragTransition | null>(null)
  const [dragConfirmRequest, setDragConfirmRequest] = useState<PlannedDragTransition | null>(null)
  const [dragReasonDraft, setDragReasonDraft] = useState("")

  const runtimeState = useRuntimeConfigState()
  const { api, config, error, setConfig, setError } = runtimeState
  const reportError = useCallback((err: unknown) => setError(errorMessage(err)), [setError])

  useEventPoller({
    api,
    enabled: Boolean(api),
    onError: reportError,
  })

  const taskCollectionState = useTaskCollectionState(api, view, reportError)
  const taskDetailState = useSelectedTaskDetailState(
    api,
    view,
    taskCollectionState.tasks,
    taskCollectionState.enabled,
    config?.actor ?? null,
    reportError,
  )
  const creationDialogState = useTaskCreationDialogState()
  const taskMutations = useTaskMutations({
    api,
    commentBody: taskDetailState.commentBody,
    config,
    creation: creationDialogState,
    dependencyInput: taskDetailState.dependencyInput,
    draftState: taskDetailState.draftState,
    queryClient,
    selectedId: taskDetailState.selectedId,
    selectedTask: taskDetailState.selectedTask,
    setClaimTokens: taskDetailState.setClaimTokens,
    setCommentBody: taskDetailState.setCommentBody,
    setDependencyInput: taskDetailState.setDependencyInput,
    setDraftState: taskDetailState.setDraftState,
    setError,
    setLabelSuggestionsRequested: taskDetailState.setLabelSuggestionsRequested,
    setSelectedId: taskDetailState.setSelectedId,
  })

  const tasksById = useMemo(
    () => new Map(taskCollectionState.tasks.map((task) => [task.id, task])),
    [taskCollectionState.tasks],
  )

  const requestLabelSuggestions = useCallback(async () => {
    if (!api || !taskDetailState.selectedId) return
    taskDetailState.setLabelSuggestionsRequested(true)
    await taskDetailState.labelSuggestionsQuery.refetch()
  }, [api, taskDetailState])

  const dropTask = useCallback(
    (taskId: string, targetStatus: TaskStatus) => {
      if (!api) return
      const task = tasksById.get(taskId)
      if (!task) return
      const token = taskDetailState.claimTokens[task.id] ?? null
      const plan = planDragTransition(task, targetStatus, token)
      if (!plan.ok) {
        setError(plan.reason)
        return
      }
      if (plan.promptReason) {
        setDragReasonDraft("")
        setDragReasonRequest({ task, plan })
        return
      }
      requestDragExecution({ task, plan })
    },
    [api, taskDetailState.claimTokens, tasksById, setError],
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
        setError("A block reason is required.")
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
    [dragReasonDraft, dragReasonRequest, requestDragExecution, setError],
  )

  const switchBoard = useCallback(
    async (board: string) => {
      if (!config || board === config.board) return
      taskMutations.runAction(async () => {
        const nextConfig = await switchRuntimeBoard(config, board)
        const reset = createBoardSwitchReset({
          config: nextConfig,
          selectedId: taskDetailState.selectedId,
          pageOffset: taskCollectionState.pageOffset,
          newTitle: creationDialogState.title,
          newDescription: creationDialogState.description,
          blockReason: taskDetailState.blockReason,
          dependencyInput: taskDetailState.dependencyInput,
          commentBody: taskDetailState.commentBody,
          draftState: taskDetailState.draftState,
          claimTokens: taskDetailState.claimTokens,
          lastRefreshAt: taskCollectionState.lastRefreshAt,
          error,
        })
        setConfig(reset.config)
        taskDetailState.setSelectedId(reset.selectedId)
        taskCollectionState.setPageOffset(reset.pageOffset)
        creationDialogState.setTitle(reset.newTitle)
        creationDialogState.setDescription(reset.newDescription)
        taskDetailState.setBlockReason(reset.blockReason)
        taskDetailState.setDependencyInput(reset.dependencyInput)
        taskDetailState.setCommentBody(reset.commentBody)
        taskDetailState.setDraftState(reset.draftState)
        taskDetailState.setClaimTokens(reset.claimTokens)
        taskCollectionState.setLastRefreshAt(reset.lastRefreshAt)
        setError(reset.error)
        await Promise.all(
          createBoardSwitchInvalidationTargets({
            previousBoard: config.board,
            nextBoard: reset.config.board,
          }).map((queryKey) => queryClient.invalidateQueries({ queryKey })),
        )
        return reset.config
      }, { label: "board", fallbackTaskId: taskDetailState.selectedId, invalidate: "none" })
    },
    [
      config,
      creationDialogState,
      error,
      queryClient,
      setConfig,
      setError,
      taskCollectionState,
      taskDetailState,
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
      boards: taskCollectionState.boardsQuery.data ?? [],
      boardsError: taskCollectionState.boardsQuery.error ? errorMessage(taskCollectionState.boardsQuery.error) : null,
      boardsLoading: taskCollectionState.boardsQuery.isLoading,
      config,
      error,
      pendingAction: taskMutations.pendingAction,
      queueCounts: taskCollectionState.queueCounts,
      themeMode: runtimeState.themeMode,
      lastRefreshAt: taskCollectionState.lastRefreshAt,
    }),
    [
      api,
      config,
      error,
      runtimeState.themeMode,
      taskCollectionState.boardsQuery.data,
      taskCollectionState.boardsQuery.error,
      taskCollectionState.boardsQuery.isLoading,
      taskCollectionState.lastRefreshAt,
      taskCollectionState.queueCounts,
      taskMutations.pendingAction,
    ],
  )

  const taskCollection = useMemo(
    () => ({
      canGoLastPage: taskCollectionState.lastOffset !== null && taskCollectionState.lastOffset !== taskCollectionState.page.offset,
      columns: taskCollectionState.columns,
      debouncedSearch: taskCollectionState.debouncedSearch,
      groupedTasks: taskCollectionState.groupedTasks,
      hasNextPage: taskCollectionState.hasNext,
      hasPreviousPage: taskCollectionState.hasPrevious,
      listSort: taskCollectionState.listSort,
      page: taskCollectionState.page,
      planFilters: taskCollectionState.planFilters,
      priorityFilters: taskCollectionState.priorityFilters,
      rowsPerPage: taskCollectionState.rowsPerPage,
      search: taskCollectionState.search,
      searchMeta: taskCollectionState.searchMeta,
      showArchived: taskCollectionState.showArchived,
      statusFilter: taskCollectionState.statusFilter,
      tasks: taskCollectionState.tasks,
      tasksRefreshing: taskCollectionState.enabled && taskCollectionState.tasksQuery.isFetching,
    }),
    [taskCollectionState],
  )

  const taskDetail = useMemo(
    () => ({
      activeRun: taskDetailState.activeRun,
      blockReason: taskDetailState.blockReason,
      claimToken: taskDetailState.claimToken,
      commentBody: taskDetailState.commentBody,
      dependencyInput: taskDetailState.dependencyInput,
      dependencySnapshot: taskDetailState.dependencySnapshot,
      detail: taskDetailState.detail,
      detailLoading: taskDetailState.detailLoading,
      draftDirty: taskDetailState.draftState?.dirty ?? false,
      editDraft: taskDetailState.draftState?.draft ?? null,
      labelSuggestions: taskDetailState.labelSuggestionsQuery.data ?? null,
      labelSuggestionsError: taskDetailState.labelSuggestionsQuery.error
        ? errorMessage(taskDetailState.labelSuggestionsQuery.error)
        : null,
      labelSuggestionsLoading: taskDetailState.labelSuggestionsQuery.isFetching,
      labelSuggestionsRequested: taskDetailState.labelSuggestionsRequested,
      selectedId: taskDetailState.selectedId,
      selectedTask: taskDetailState.selectedTask,
      taskRunsExpanded: taskDetailState.taskRunsExpanded,
    }),
    [taskDetailState],
  )

  const commands = useMemo(
    () => ({
      addComment: taskMutations.addComment,
      addDependency: taskMutations.addDependency,
      cancelTaskEdit: taskMutations.cancelTaskEdit,
      changeBoard: (board: string) => void switchBoard(board),
      changeThemeMode: runtimeState.setThemeMode,
      closeTaskDetail: () => taskDetailState.setSelectedId(null),
      createTask: taskMutations.createTask,
      cycleThemeMode: runtimeState.cycleThemeMode,
      dropTask: (taskId: string, status: TaskStatus) => dropTask(taskId, status),
      firstPage: () => taskCollectionState.setPageOffset(0),
      lastPage: () => taskCollectionState.setPageOffset(taskCollectionState.lastOffset ?? taskCollectionState.pageOffset),
      nextPage: () => taskCollectionState.setPageOffset((current) => current + taskCollectionState.rowsPerPage),
      previousPage: () =>
        taskCollectionState.setPageOffset((current) => Math.max(0, current - taskCollectionState.rowsPerPage)),
      refreshTasks: () => {
        const refreshes: Promise<unknown>[] = [taskCollectionState.statsQuery.refetch()]
        if (taskCollectionState.enabled) refreshes.push(taskCollectionState.tasksQuery.refetch())
        void Promise.all(refreshes)
      },
      removeDependency: taskMutations.removeDependency,
      requestLabelSuggestions: () => void requestLabelSuggestions(),
      resetListFilters: () => {
        taskCollectionState.setStatusFilter("all")
        taskCollectionState.setPriorityFilters([])
        taskCollectionState.setPlanFilters([])
        taskCollectionState.setPageOffset(0)
      },
      saveTask: taskMutations.saveTask,
      selectTask: taskDetailState.setSelectedId,
      setBlockReason: taskDetailState.setBlockReason,
      setCommentBody: taskDetailState.setCommentBody,
      setDependencyInput: taskDetailState.setDependencyInput,
      setEditDraft: taskMutations.updateDraft,
      setListSort: taskCollectionState.setListSort,
      setPlanFilters: taskCollectionState.setPlanFilters,
      setPriorityFilters: taskCollectionState.setPriorityFilters,
      setRowsPerPage: (value: number) => {
        taskCollectionState.setRowsPerPage(value)
        taskCollectionState.setPageOffset(0)
      },
      setSearch: taskCollectionState.setSearch,
      setShowArchived: taskCollectionState.setShowArchived,
      setSidebarOpen: runtimeState.setSidebarOpen,
      setStatusFilter: taskCollectionState.setStatusFilter,
      setTaskCreationDescription: creationDialogState.setDescription,
      setTaskCreationFirstStepTitle: creationDialogState.setFirstStepTitle,
      setTaskCreationOpen: creationDialogState.setOpen,
      setTaskCreationTitle: creationDialogState.setTitle,
      setTaskRunsExpanded: taskDetailState.setTaskRunsExpanded,
      setView,
      runAction: taskMutations.runAction,
    }),
    [
      creationDialogState,
      dropTask,
      requestLabelSuggestions,
      runtimeState.cycleThemeMode,
      runtimeState.setSidebarOpen,
      runtimeState.setThemeMode,
      switchBoard,
      taskCollectionState,
      taskDetailState,
      taskMutations,
    ],
  )

  const taskCreation = useMemo(
    () => ({
      description: creationDialogState.description,
      firstStepTitle: creationDialogState.firstStepTitle,
      open: creationDialogState.open,
      title: creationDialogState.title,
    }),
    [creationDialogState.description, creationDialogState.firstStepTitle, creationDialogState.open, creationDialogState.title],
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
              <DialogTitle>Block reason</DialogTitle>
              <DialogDescription>Record why this task is being moved to blocked.</DialogDescription>
            </DialogHeader>
            <Textarea
              aria-label="Block reason"
              name="block-reason"
              autoComplete="off"
              value={dragReasonDraft}
              onChange={(event) => setDragReasonDraft(event.target.value)}
              placeholder="Block reason"
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
                Cancel
              </Button>
              <Button type="submit" disabled={!dragReasonDraft.trim()}>
                Continue
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
            <AlertDialogTitle>Confirm transition</AlertDialogTitle>
            <AlertDialogDescription>{dragConfirmRequest?.plan.confirm}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                const request = dragConfirmRequest
                setDragConfirmRequest(null)
                if (request) void executePlannedDrag(request)
              }}
            >
              Continue
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

export default App
