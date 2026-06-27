import { useQuery } from "@tanstack/react-query"

import type { KanbanApi } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type BoardTaskMapOptions = {
  includeDoneContext: boolean
}

export function useBoardTaskMap(api: KanbanApi | null, options: BoardTaskMapOptions) {
  const board = api?.board ?? "pending"
  return useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.boardTaskMap(board, { includeDoneContext: options.includeDoneContext }),
    queryFn: ({ signal }) => {
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
  })
}
