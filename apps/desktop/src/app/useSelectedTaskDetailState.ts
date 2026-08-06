import { useEffect, useMemo, useState } from "react"
import { useQuery } from "@tanstack/react-query"

import { reconcileSelectedTaskId, shouldLoadTaskDetail } from "@/app/task-selection"
import { requestTaskLabelSuggestions, taskDetailOrEmpty, useTaskDetail } from "@/features/task-detail/useTaskDetail"
import { reconcileTaskDraft, type TaskDraftState } from "@/features/task-detail/task-draft"
import type { KanbanApi, Task } from "@/lib/api"
import { reconcileClaimTokenForTask, reconcileClaimTokensForTasks } from "@/lib/claim-tokens"
import { queryKeys } from "@/lib/query-keys"
import type { OperatorView } from "@/features/navigation/view-types"

export function useSelectedTaskDetailState(
  api: KanbanApi | null,
  view: OperatorView,
  tasks: Task[],
  taskCollectionEnabled: boolean,
  actor: string | null,
  reportError: (error: unknown) => void,
) {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [blockReason, setBlockReason] = useState("")
  const [dependencyInput, setDependencyInput] = useState("")
  const [commentBody, setCommentBody] = useState("")
  const [draftState, setDraftState] = useState<TaskDraftState | null>(null)
  const [claimTokens, setClaimTokens] = useState<Record<string, string>>({})
  const [labelSuggestionsRequested, setLabelSuggestionsRequested] = useState(false)
  const [taskDependenciesExpanded, setTaskDependenciesExpanded] = useState(true)
  const [taskGraphExpanded, setTaskGraphExpanded] = useState(true)
  const [taskStepsExpanded, setTaskStepsExpanded] = useState(true)
  const [taskRunsExpanded, setTaskRunsExpanded] = useState(false)
  const [taskEventsExpanded, setTaskEventsExpanded] = useState(false)
  const [taskCommentsExpanded, setTaskCommentsExpanded] = useState(true)

  const enabled = shouldLoadTaskDetail(view, selectedId)
  const detailQuery = useTaskDetail(api, selectedId, {
    enabled,
    dependenciesEnabled: enabled,
    neighborhoodEnabled: enabled,
    stepsEnabled: enabled,
    runsEnabled: view === "runs" || taskRunsExpanded,
    eventsEnabled: taskEventsExpanded,
    commentsEnabled: enabled,
    runLogEnabled: view === "runs" || taskRunsExpanded,
  })
  const labelSuggestionsQuery = useQuery({
    enabled: false,
    queryKey: selectedId ? queryKeys.taskLabelSuggestions(selectedId) : ["task-label-suggestions", "none"],
    queryFn: ({ signal }) => {
      if (!api || !selectedId) throw new Error("标签建议查询尚未就绪")
      return requestTaskLabelSuggestions(api, selectedId, signal)
    },
  })

  const boardSelectedTask = useMemo(
    () => (selectedId ? tasks.find((task) => task.id === selectedId) ?? null : null),
    [selectedId, tasks],
  )
  const selectedTask = selectedId ? detailQuery.data?.task ?? (taskCollectionEnabled ? boardSelectedTask : null) : null
  const detail = taskDetailOrEmpty(detailQuery.data)
  const dependencySnapshot = useMemo(
    () => ({
      selectedTaskId: selectedId,
      detailTaskId: detailQuery.data?.task.id ?? null,
      dependencies: detailQuery.data?.detail.dependencies ?? null,
      loading: Boolean(enabled && detailQuery.isFetching),
    }),
    [detailQuery.data?.detail.dependencies, detailQuery.data?.task.id, detailQuery.isFetching, enabled, selectedId],
  )
  const activeRun = detail.runs.find((run) => run.status === "running") ?? detail.runs[0]
  const claimToken = selectedTask ? claimTokens[selectedTask.id] ?? null : null

  useEffect(() => {
    if (!taskCollectionEnabled) return
    setClaimTokens((current) => reconcileClaimTokensForTasks(current, tasks, actor))
  }, [actor, taskCollectionEnabled, tasks])

  useEffect(() => {
    if (!taskCollectionEnabled) return
    setSelectedId((current) => reconcileSelectedTaskId(current, tasks))
  }, [taskCollectionEnabled, tasks])

  useEffect(() => {
    setDraftState((current) => reconcileTaskDraft(current, selectedTask))
  }, [selectedTask])

  useEffect(() => {
    setLabelSuggestionsRequested(false)
    setTaskDependenciesExpanded(true)
    setTaskGraphExpanded(true)
    setTaskStepsExpanded(true)
    setTaskRunsExpanded(false)
    setTaskEventsExpanded(false)
    setTaskCommentsExpanded(true)
  }, [selectedId])

  useEffect(() => {
    if (!selectedTask) return
    setClaimTokens((current) => reconcileClaimTokenForTask(current, selectedTask, actor))
  }, [actor, selectedTask])

  useEffect(() => {
    setBlockReason("")
  }, [selectedTask?.id, selectedTask?.status])

  useEffect(() => {
    if (enabled && detailQuery.error) reportError(detailQuery.error)
  }, [detailQuery.error, enabled, reportError])

  return useMemo(
    () => ({
      activeRun,
      blockReason,
      claimToken,
      claimTokens,
      commentBody,
      dependencyInput,
      dependencySnapshot,
      detail,
      detailLoading: enabled && detailQuery.isFetching,
      detailQuery,
      draftState,
      enabled,
      labelSuggestionsQuery,
      labelSuggestionsRequested:
        labelSuggestionsRequested ||
        labelSuggestionsQuery.isFetched ||
        labelSuggestionsQuery.isFetching ||
        Boolean(labelSuggestionsQuery.error),
      labelSuggestionsRequestedExplicitly: labelSuggestionsRequested,
      selectedId,
      selectedTask,
      setBlockReason,
      setClaimTokens,
      setCommentBody,
      setDependencyInput,
      setDraftState,
      setLabelSuggestionsRequested,
      setSelectedId,
      setTaskCommentsExpanded,
      setTaskDependenciesExpanded,
      setTaskEventsExpanded,
      setTaskGraphExpanded,
      setTaskRunsExpanded,
      setTaskStepsExpanded,
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
      claimTokens,
      commentBody,
      dependencyInput,
      dependencySnapshot,
      detail,
      detailQuery,
      draftState,
      enabled,
      labelSuggestionsQuery,
      labelSuggestionsRequested,
      selectedId,
      selectedTask,
      setLabelSuggestionsRequested,
      taskCommentsExpanded,
      taskDependenciesExpanded,
      taskEventsExpanded,
      taskGraphExpanded,
      taskRunsExpanded,
      taskStepsExpanded,
    ],
  )
}
