import type {
  LabelOntologyReviewGroupBy,
  LabelOntologySignalKind,
  LabelOntologySignalStatus,
  TaskListSort,
  TaskPlanFilter,
  TaskStatus,
} from "./api"

export type BoardTaskQuery = {
  board: string
  search: string
  status: TaskStatus | "all"
  priorities: number[]
  planFilters: TaskPlanFilter[]
  sort: TaskListSort
  mode: "board" | "list"
  statuses: TaskStatus[]
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
        planFilters: query.planFilters,
        sort: query.sort,
        mode: query.mode,
        statuses: query.statuses,
        showArchived: query.showArchived,
        limit: query.limit,
        offset: query.offset,
      },
    ] as const,
  taskDetail: (taskId: string) => ["task-detail", taskId] as const,
  taskDependencies: (taskId: string) => ["task-dependencies", taskId] as const,
  taskSteps: (taskId: string) => ["task-steps", taskId] as const,
  taskNeighborhood: (taskId: string) => ["task-neighborhood", taskId] as const,
  taskRuns: (taskId: string) => ["task-runs", taskId] as const,
  taskRunLog: (runId: string) => ["task-run-log", runId] as const,
  taskEvents: (taskId: string) => ["task-events", taskId] as const,
  taskComments: (taskId: string) => ["task-comments", taskId] as const,
  boardTaskMapRoot: (board: string) => ["board-task-map", board] as const,
  boardTaskMap: (board: string, options?: { includeDoneContext?: boolean }) =>
    [...queryKeys.boardTaskMapRoot(board), options ?? {}] as const,
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
