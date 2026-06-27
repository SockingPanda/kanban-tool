import { useMemo } from "react"
import { useQuery } from "@tanstack/react-query"

import { emptyDetail, type DetailState } from "@/features/task-detail/detail-state"
import type { KanbanApi, Task } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type TaskDetailData = {
  task: Task
  detail: DetailState
}

export type TaskDetailQueryOptions = {
  enabled?: boolean
  dependenciesEnabled?: boolean
  neighborhoodEnabled?: boolean
  stepsEnabled?: boolean
  runsEnabled?: boolean
  eventsEnabled?: boolean
  commentsEnabled?: boolean
  runLogEnabled?: boolean
}

export function resolveTaskDetailQueryEnablement(options: TaskDetailQueryOptions = {}) {
  return {
    task: options.enabled ?? true,
    dependencies: Boolean(options.dependenciesEnabled),
    neighborhood: Boolean(options.neighborhoodEnabled),
    steps: Boolean(options.stepsEnabled),
    runs: Boolean(options.runsEnabled),
    events: Boolean(options.eventsEnabled),
    comments: Boolean(options.commentsEnabled),
    runLog: Boolean(options.runLogEnabled),
  }
}

export async function fetchTaskDetail(api: KanbanApi, taskId: string, signal?: AbortSignal) {
  const [task, dependencies, neighborhood, steps, runs, eventsPage, comments] = await Promise.all([
    api.getTask(taskId, { signal }),
    api.listDependencies(taskId, { signal }),
    api.getTaskNeighborhood(taskId, { depth: 1, limitNodes: 40, signal }),
    api.listSteps(taskId, { signal }),
    api.listRuns(taskId, { signal }),
    api.listEvents(taskId, { signal }),
    api.listComments(taskId, { signal }),
  ])

  return {
    task,
    detail: {
      dependencies,
      steps,
      neighborhood,
      runs,
      events: eventsPage.events,
      comments,
      runLog: null,
      labelSuggestions: null,
    },
  } satisfies TaskDetailData
}

export function requestTaskLabelSuggestions(api: KanbanApi, taskId: string, signal?: AbortSignal) {
  return api.suggestTaskLabels(taskId, { signal })
}

export function useTaskDetail(
  api: KanbanApi | null,
  taskId: string | null,
  options: TaskDetailQueryOptions = {},
) {
  const queryEnablement = resolveTaskDetailQueryEnablement(options)
  const enabled = queryEnablement.task
  const ready = Boolean(enabled && api && taskId)

  const taskQuery = useQuery({
    enabled: Boolean(enabled && api && taskId),
    queryKey: taskId ? queryKeys.taskDetail(taskId) : ["task-detail", "none"],
    queryFn: async ({ signal }) => {
      if (!api || !taskId) throw new Error("Task detail query is not ready")
      return api.getTask(taskId, { signal })
    },
  })

  const dependenciesQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.dependencies),
    queryKey: taskId ? queryKeys.taskDependencies(taskId) : ["task-dependencies", "none"],
    queryFn: ({ signal }) => {
      if (!api || !taskId) throw new Error("Task dependencies query is not ready")
      return api.listDependencies(taskId, { signal })
    },
  })

  const neighborhoodQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.neighborhood),
    queryKey: taskId ? queryKeys.taskNeighborhood(taskId) : ["task-neighborhood", "none"],
    queryFn: ({ signal }) => {
      if (!api || !taskId) throw new Error("Task neighborhood query is not ready")
      return api.getTaskNeighborhood(taskId, { depth: 1, limitNodes: 40, signal })
    },
  })

  const stepsQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.steps),
    queryKey: taskId ? queryKeys.taskSteps(taskId) : ["task-steps", "none"],
    queryFn: ({ signal }) => {
      if (!api || !taskId) throw new Error("Task steps query is not ready")
      return api.listSteps(taskId, { signal })
    },
  })

  const runsQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.runs),
    queryKey: taskId ? queryKeys.taskRuns(taskId) : ["task-runs", "none"],
    queryFn: ({ signal }) => {
      if (!api || !taskId) throw new Error("Task runs query is not ready")
      return api.listRuns(taskId, { signal })
    },
  })

  const eventsQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.events),
    queryKey: taskId ? queryKeys.taskEvents(taskId) : ["task-events", "none"],
    queryFn: async ({ signal }) => {
      if (!api || !taskId) throw new Error("Task events query is not ready")
      const page = await api.listEvents(taskId, { signal })
      return page.events
    },
  })

  const commentsQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.comments),
    queryKey: taskId ? queryKeys.taskComments(taskId) : ["task-comments", "none"],
    queryFn: ({ signal }) => {
      if (!api || !taskId) throw new Error("Task comments query is not ready")
      return api.listComments(taskId, { signal })
    },
  })

  const runWithLog = runsQuery.data?.find((run) => Boolean(run.log_path)) ?? null
  const runLogQuery = useQuery({
    enabled: Boolean(ready && queryEnablement.runLog && runWithLog),
    queryKey: runWithLog ? queryKeys.taskRunLog(runWithLog.id) : ["task-run-log", "none"],
    queryFn: ({ signal }) => {
      if (!api || !runWithLog) throw new Error("Task run log query is not ready")
      return api.getRunLog(runWithLog.id, { signal })
    },
  })

  return useMemo(() => {
    const detail: DetailState = {
      dependencies: dependenciesQuery.data ?? emptyDetail.dependencies,
      steps: stepsQuery.data ?? null,
      neighborhood: neighborhoodQuery.data ?? null,
      runs: runsQuery.data ?? [],
      events: eventsQuery.data ?? [],
      comments: commentsQuery.data ?? [],
      runLog: runLogQuery.data ?? null,
      labelSuggestions: null,
    }
    const error =
      taskQuery.error ??
      dependenciesQuery.error ??
      neighborhoodQuery.error ??
      stepsQuery.error ??
      runsQuery.error ??
      eventsQuery.error ??
      commentsQuery.error ??
      runLogQuery.error ??
      null

    return {
      data: taskQuery.data ? ({ task: taskQuery.data, detail } satisfies TaskDetailData) : undefined,
      error,
      isFetching:
        taskQuery.isFetching ||
        dependenciesQuery.isFetching ||
        neighborhoodQuery.isFetching ||
        stepsQuery.isFetching ||
        runsQuery.isFetching ||
        eventsQuery.isFetching ||
        commentsQuery.isFetching ||
        runLogQuery.isFetching,
    }
  }, [
    commentsQuery.data,
    commentsQuery.error,
    commentsQuery.isFetching,
    dependenciesQuery.data,
    dependenciesQuery.error,
    dependenciesQuery.isFetching,
    eventsQuery.data,
    eventsQuery.error,
    eventsQuery.isFetching,
    neighborhoodQuery.data,
    neighborhoodQuery.error,
    neighborhoodQuery.isFetching,
    runLogQuery.data,
    runLogQuery.error,
    runLogQuery.isFetching,
    runsQuery.data,
    runsQuery.error,
    runsQuery.isFetching,
    stepsQuery.data,
    stepsQuery.error,
    stepsQuery.isFetching,
    taskQuery.data,
    taskQuery.error,
    taskQuery.isFetching,
  ])
}

export function taskDetailOrEmpty(data: TaskDetailData | undefined) {
  return data?.detail ?? emptyDetail
}
