import type { TaskDraftState } from "@/features/task-detail/task-draft"
import type { RuntimeConfig } from "@/lib/api"

export type BoardSwitchState = {
  config: RuntimeConfig
  selectedId: string | null
  pageOffset: number
  newTitle: string
  newDescription: string
  blockReason: string
  dependencyInput: string
  commentBody: string
  draftState: TaskDraftState | null
  claimTokens: Record<string, string>
  lastRefreshAt: number | null
  error: string | null
}

export function createBoardSwitchReset(state: BoardSwitchState): BoardSwitchState {
  return {
    config: state.config,
    selectedId: null,
    pageOffset: 0,
    newTitle: "",
    newDescription: "",
    blockReason: "",
    dependencyInput: "",
    commentBody: "",
    draftState: null,
    claimTokens: {},
    lastRefreshAt: null,
    error: null,
  }
}
