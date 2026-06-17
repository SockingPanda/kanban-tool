import type { TaskDraftState } from "@/features/task-detail/task-draft"
import type { RuntimeConfig } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

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

export function createBoardSwitchInvalidationTargets({
  previousBoard,
  nextBoard,
}: {
  previousBoard: string
  nextBoard: string
}) {
  const targets = [
    queryKeys.boards(),
    queryKeys.columns(previousBoard),
    queryKeys.columns(nextBoard),
    queryKeys.boardTasksRoot(previousBoard),
    queryKeys.boardTasksRoot(nextBoard),
    queryKeys.events(previousBoard),
    queryKeys.events(nextBoard),
    queryKeys.stats(previousBoard),
    queryKeys.stats(nextBoard),
    queryKeys.searchStatus(previousBoard),
    queryKeys.searchStatus(nextBoard),
  ]

  const seen = new Set<string>()
  return targets.filter((target) => {
    const key = JSON.stringify(target)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}
