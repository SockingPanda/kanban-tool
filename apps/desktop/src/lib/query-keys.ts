import type {
  LabelOntologyReviewGroupBy,
  LabelOntologySignalKind,
  LabelOntologySignalStatus,
  TaskListSort,
  TaskStatus,
} from "./api"

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

export type LabelOntologySignalsQuery = {
  board: string
  statuses: LabelOntologySignalStatus[]
  kinds: LabelOntologySignalKind[]
  includeAll: boolean
  limit: number
}

export type LabelOntologyReviewQuery = {
  board: string
  groupBy: LabelOntologyReviewGroupBy
  includeAll: boolean
  limit: number
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
  ontologyRoot: (board: string) => ["label-ontology", board] as const,
  ontologySignals: (query: LabelOntologySignalsQuery) =>
    [
      ...queryKeys.ontologyRoot(query.board),
      "signals",
      {
        statuses: query.statuses,
        kinds: query.kinds,
        includeAll: query.includeAll,
        limit: query.limit,
      },
    ] as const,
  ontologyReview: (query: LabelOntologyReviewQuery) =>
    [
      ...queryKeys.ontologyRoot(query.board),
      "review",
      {
        groupBy: query.groupBy,
        includeAll: query.includeAll,
        limit: query.limit,
      },
    ] as const,
  ontologySignal: (signalId: string) => ["label-ontology-signal", signalId] as const,
  ontologyAtomExplain: (board: string, atomRef: string) =>
    [...queryKeys.ontologyRoot(board), "atom-explain", atomRef] as const,
}
