import type { TaskListSort, TaskStatus } from "./api"

export type BoardTaskQuery = {
  board: string
  search: string
  status: TaskStatus | "all"
  priorities: number[]
  sort: TaskListSort
  mode: "board" | "list"
  showArchived: boolean
  limit: number
  offset: number
}

export const queryKeys = {
  boards: () => ["boards"] as const,
  columns: (board: string) => ["columns", board] as const,
  events: (board: string) => ["events", board] as const,
  stats: (board: string) => ["stats", board] as const,
  searchStatus: (board: string) => ["search-status", board] as const,
  boardTasksRoot: (board: string) => ["tasks", board] as const,
  boardTasks: (query: BoardTaskQuery) =>
    [
      ...queryKeys.boardTasksRoot(query.board),
      {
        search: query.search,
        status: query.status,
        priorities: query.priorities,
        sort: query.sort,
        mode: query.mode,
        showArchived: query.showArchived,
        limit: query.limit,
        offset: query.offset,
      },
    ] as const,
  taskDetail: (taskId: string) => ["task-detail", taskId] as const,
  taskLabelSuggestions: (taskId: string) => ["task-label-suggestions", taskId] as const,
}
