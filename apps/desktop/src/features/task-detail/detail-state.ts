import type {
  CommentRecord,
  Dependencies,
  EventRecord,
  LabelSuggestionResult,
  Run,
  RunLog,
  TaskNeighborhood,
  TaskSteps,
} from "@/lib/api"

export type DetailState = {
  dependencies: Dependencies
  steps: TaskSteps | null
  neighborhood: TaskNeighborhood | null
  runs: Run[]
  events: EventRecord[]
  comments: CommentRecord[]
  runLog: RunLog | null
  labelSuggestions: LabelSuggestionResult | null
}

export const emptyDetail: DetailState = {
  dependencies: { parents: [], children: [] },
  steps: null,
  neighborhood: null,
  runs: [],
  events: [],
  comments: [],
  runLog: null,
  labelSuggestions: null,
}
