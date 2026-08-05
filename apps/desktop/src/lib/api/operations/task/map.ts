import { ApiTransport } from "../../transport"
import type { BoardTaskMap, RequestOptions } from "../../types"

export async function getBoardTaskMap(api: ApiTransport,
    board = api.board,
    options: RequestOptions & {
      activeOnly?: boolean
      contextDepth?: number
      includeDoneContext?: boolean
      includeArchivedContext?: boolean
      hideIsolated?: boolean
      limitNodes?: number
    } = {},
  ) {
    const params = new URLSearchParams({
      active_only: String(options.activeOnly ?? true),
      context_depth: String(options.contextDepth ?? 1),
    })
    if (typeof options.includeDoneContext === "boolean") {
      params.set("include_done_context", String(options.includeDoneContext))
    }
    if (typeof options.includeArchivedContext === "boolean") {
      params.set("include_archived_context", String(options.includeArchivedContext))
    }
    if (typeof options.hideIsolated === "boolean") {
      params.set("hide_isolated", String(options.hideIsolated))
    }
    if (typeof options.limitNodes === "number") params.set("limit_nodes", String(options.limitNodes))
    return api.request<BoardTaskMap>("/api/v1/boards/" + board + "/task-map?" + params.toString(), {
      signal: options.signal,
    })
  }
