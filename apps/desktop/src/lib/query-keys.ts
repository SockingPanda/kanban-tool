import type { TaskStatus } from "./api"

export type BoardTaskQuery = {
  board: string
  search: string
  status: TaskStatus | "all"
  showArchived: boolean
  limit: number
  offset: number
}

export const queryKeys = {
  columns: (board: string) => ["columns", board] as const,
  boardTasksRoot: (board: string) => ["tasks", board] as const,
  boardTasks: (query: BoardTaskQuery) =>
    [
      ...queryKeys.boardTasksRoot(query.board),
      {
        search: query.search,
        status: query.status,
        showArchived: query.showArchived,
        limit: query.limit,
        offset: query.offset,
      },
    ] as const,
  taskDetail: (taskId: string) => ["task-detail", taskId] as const,
}
