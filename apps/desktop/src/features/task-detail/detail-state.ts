import type {
  CommentRecord,
  Dependencies,
  EventRecord,
  LabelSuggestionResult,
  Run,
  RunLog,
  TaskNeighborhood,
  TaskSubtasks,
} from "@/lib/api"

export type DetailState = {
  dependencies: Dependencies
  subtasks: TaskSubtasks | null
  neighborhood: TaskNeighborhood | null
  runs: Run[]
  events: EventRecord[]
  comments: CommentRecord[]
  runLog: RunLog | null
  labelSuggestions: LabelSuggestionResult | null
}

export const emptyDetail: DetailState = {
  dependencies: { parents: [], children: [] },
  subtasks: null,
  neighborhood: null,
  runs: [],
  events: [],
  comments: [],
  runLog: null,
  labelSuggestions: null,
}
