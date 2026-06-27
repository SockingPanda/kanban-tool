import type { QueryClient } from "@tanstack/react-query"

import { queryKeys } from "@/lib/query-keys"

export async function invalidateTaskDetailAndBoard(queryClient: QueryClient, board: string, taskId: string | null) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.boardTasksRoot(board) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.stats(board) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.boardTaskMapRoot(board) }),
    taskId ? invalidateTaskDetailQueries(queryClient, taskId) : Promise.resolve(),
  ])
}

export async function invalidateTaskDetailQueries(queryClient: QueryClient, taskId: string) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.taskDetail(taskId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.taskDependencies(taskId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.taskSteps(taskId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.taskNeighborhood(taskId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.taskRuns(taskId) }),
    invalidateTaskTimelineQueries(queryClient, taskId),
  ])
}

export async function invalidateTaskTimelineQueries(queryClient: QueryClient, taskId: string) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.taskEvents(taskId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.taskComments(taskId) }),
  ])
}
