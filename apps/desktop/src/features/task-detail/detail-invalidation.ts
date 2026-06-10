import type { QueryClient } from "@tanstack/react-query"

import { queryKeys } from "@/lib/query-keys"

export async function invalidateTaskDetailAndBoard(queryClient: QueryClient, board: string, taskId: string | null) {
  await queryClient.invalidateQueries({ queryKey: queryKeys.boardTasksRoot(board) })
  if (taskId) await queryClient.invalidateQueries({ queryKey: queryKeys.taskDetail(taskId) })
}
