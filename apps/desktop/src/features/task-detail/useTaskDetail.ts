import { useQuery } from "@tanstack/react-query"

import { emptyDetail, type DetailState } from "@/features/task-detail/detail-state"
import type { KanbanApi, Task } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type TaskDetailData = {
  task: Task
  detail: DetailState
}

export function useTaskDetail(api: KanbanApi | null, taskId: string | null) {
  return useQuery({
    enabled: Boolean(api && taskId),
    queryKey: taskId ? queryKeys.taskDetail(taskId) : ["task-detail", "none"],
    queryFn: async ({ signal }) => {
      if (!api || !taskId) throw new Error("Task detail query is not ready")
      const [task, dependencies, runs, eventsPage, comments, labelSuggestions] = await Promise.all([
        api.getTask(taskId, { signal }),
        api.listDependencies(taskId, { signal }),
        api.listRuns(taskId, { signal }),
        api.listEvents(taskId, { signal }),
        api.listComments(taskId, { signal }),
        api.suggestTaskLabels(taskId, { signal }).catch(() => null),
      ])
      const runWithLog = runs.find((run) => Boolean(run.log_path)) ?? null
      const runLog = runWithLog
        ? await api.getRunLog(runWithLog.id, { signal }).catch(() => null)
        : null

      return {
        task,
        detail: {
          dependencies,
          runs,
          events: eventsPage.events,
          comments,
          runLog,
          labelSuggestions,
        },
      } satisfies TaskDetailData
    },
  })
}

export function taskDetailOrEmpty(data: TaskDetailData | undefined) {
  return data?.detail ?? emptyDetail
}
