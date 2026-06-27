import { useQuery } from "@tanstack/react-query"

import { emptyDetail, type DetailState } from "@/features/task-detail/detail-state"
import type { KanbanApi, Task } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type TaskDetailData = {
  task: Task
  detail: DetailState
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
  const runWithLog = runs.find((run) => Boolean(run.log_path)) ?? null
  const runLog = runWithLog
    ? await api.getRunLog(runWithLog.id, { signal }).catch(() => null)
    : null

  return {
    task,
    detail: {
      dependencies,
      steps,
      neighborhood,
      runs,
      events: eventsPage.events,
      comments,
      runLog,
      labelSuggestions: null,
    },
  } satisfies TaskDetailData
}

export function requestTaskLabelSuggestions(api: KanbanApi, taskId: string, signal?: AbortSignal) {
  return api.suggestTaskLabels(taskId, { signal })
}

export function useTaskDetail(api: KanbanApi | null, taskId: string | null) {
  return useQuery({
    enabled: Boolean(api && taskId),
    queryKey: taskId ? queryKeys.taskDetail(taskId) : ["task-detail", "none"],
    queryFn: async ({ signal }) => {
      if (!api || !taskId) throw new Error("Task detail query is not ready")
      return fetchTaskDetail(api, taskId, signal)
    },
  })
}

export function taskDetailOrEmpty(data: TaskDetailData | undefined) {
  return data?.detail ?? emptyDetail
}
