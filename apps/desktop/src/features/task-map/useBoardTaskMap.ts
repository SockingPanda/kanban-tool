import { keepPreviousData, useQuery } from "@tanstack/react-query"

import type { KanbanApi } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type BoardTaskMapOptions = {
  includeDoneContext: boolean
  hideIsolated?: boolean
}

export function useBoardTaskMap(api: KanbanApi | null, options: BoardTaskMapOptions) {
  return useQuery(boardTaskMapQueryOptions(api, options))
}

export function boardTaskMapQueryOptions(api: KanbanApi | null, options: BoardTaskMapOptions) {
  const board = api?.board ?? "pending"
  return {
    enabled: Boolean(api),
    queryKey: queryKeys.boardTaskMap(board, {
      includeDoneContext: options.includeDoneContext,
      hideIsolated: Boolean(options.hideIsolated),
    }),
    queryFn: ({ signal }: { signal?: AbortSignal }) => {
      if (!api) throw new Error("任务关系图查询尚未就绪")
      return api.getBoardTaskMap(api.board, {
        activeOnly: true,
        contextDepth: 1,
        includeDoneContext: options.includeDoneContext,
        includeArchivedContext: false,
        hideIsolated: Boolean(options.hideIsolated),
        limitNodes: 240,
        signal,
      })
    },
    placeholderData: keepPreviousData,
  }
}
