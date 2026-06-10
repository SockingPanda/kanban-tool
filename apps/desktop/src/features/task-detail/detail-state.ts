import type { CommentRecord, Dependencies, EventRecord, Run, RunLog } from "@/lib/api"

export type DetailState = {
  dependencies: Dependencies
  runs: Run[]
  events: EventRecord[]
  comments: CommentRecord[]
  runLog: RunLog | null
}

export const emptyDetail: DetailState = {
  dependencies: { parents: [], children: [] },
  runs: [],
  events: [],
  comments: [],
  runLog: null,
}
