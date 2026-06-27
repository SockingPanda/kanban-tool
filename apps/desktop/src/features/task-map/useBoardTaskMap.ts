import { keepPreviousData, useQuery } from "@tanstack/react-query"

import type { KanbanApi } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type BoardTaskMapOptions = {
  includeDoneContext: boolean
}

export function useBoardTaskMap(api: KanbanApi | null, options: BoardTaskMapOptions) {
  return useQuery(boardTaskMapQueryOptions(api, options))
}

export function boardTaskMapQueryOptions(api: KanbanApi | null, options: BoardTaskMapOptions) {
  const board = api?.board ?? "pending"
  return {
    enabled: Boolean(api),
    queryKey: queryKeys.boardTaskMap(board, { includeDoneContext: options.includeDoneContext }),
    queryFn: ({ signal }: { signal?: AbortSignal }) => {
      if (!api) throw new Error("Board task map query is not ready")
      return api.getBoardTaskMap(api.board, {
        activeOnly: true,
        contextDepth: 1,
        includeDoneContext: options.includeDoneContext,
        includeArchivedContext: false,
        limitNodes: 240,
        signal,
      })
    },
    placeholderData: keepPreviousData,
  }
}
