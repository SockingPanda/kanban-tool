import type { QueryClient } from "@tanstack/react-query"

import { queryKeys } from "@/lib/query-keys"

export type TaskMutationInvalidationScope =
  | "none"
  | "task"
  | "timeline"
  | "dependencies"
  | "steps"
  | "runs"
  | "board"
  | "board-and-task"

export async function invalidateTaskMutationScope({
  board,
  queryClient,
  scope,
  selectedTaskId,
  taskId,
}: {
  board: string
  queryClient: QueryClient
  scope: TaskMutationInvalidationScope
  selectedTaskId: string | null
  taskId: string | null
}) {
  const targetTaskId = taskId ?? selectedTaskId
  const invalidations: Promise<unknown>[] = []

  const invalidate = (queryKey: readonly unknown[]) => {
    invalidations.push(queryClient.invalidateQueries({ queryKey }))
  }
  const invalidateBoard = () => {
    invalidate(queryKeys.boardTasksRoot(board))
    invalidate(queryKeys.stats(board))
    invalidate(queryKeys.boardTaskMapRoot(board))
  }
  const invalidateBoardRowsAndMap = () => {
    invalidate(queryKeys.boardTasksRoot(board))
    invalidate(queryKeys.boardTaskMapRoot(board))
  }
  const invalidateTimeline = () => {
    if (!targetTaskId) return
    invalidate(queryKeys.taskEvents(targetTaskId))
    invalidate(queryKeys.taskComments(targetTaskId))
  }
  const invalidateTask = () => {
    if (!targetTaskId) return
    invalidate(queryKeys.taskDetail(targetTaskId))
    invalidate(queryKeys.taskDependencies(targetTaskId))
    invalidate(queryKeys.taskSteps(targetTaskId))
    invalidate(queryKeys.taskNeighborhood(targetTaskId))
    invalidate(queryKeys.taskRuns(targetTaskId))
    invalidateTimeline()
  }

  switch (scope) {
    case "none":
      break
    case "task":
      invalidateTask()
      break
    case "timeline":
      invalidateTimeline()
      break
    case "dependencies":
      if (targetTaskId) {
        invalidate(queryKeys.taskDependencies(targetTaskId))
        invalidate(queryKeys.taskNeighborhood(targetTaskId))
        invalidate(queryKeys.taskEvents(targetTaskId))
      }
      invalidateBoardRowsAndMap()
      break
    case "steps":
      if (targetTaskId) {
        invalidate(queryKeys.taskSteps(targetTaskId))
        invalidate(queryKeys.taskNeighborhood(targetTaskId))
        invalidate(queryKeys.taskEvents(targetTaskId))
      }
      invalidateBoardRowsAndMap()
      break
    case "runs":
      if (targetTaskId) {
        invalidate(queryKeys.taskRuns(targetTaskId))
        invalidate(queryKeys.taskEvents(targetTaskId))
      }
      invalidateBoard()
      break
    case "board":
      invalidateBoard()
      break
    case "board-and-task":
      invalidateBoard()
      invalidateTask()
      break
  }

  await Promise.all(invalidations)
}
