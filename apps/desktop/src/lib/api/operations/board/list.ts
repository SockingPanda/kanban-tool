import { ApiTransport } from "../../transport"
import { parseListBoardsEnvelope } from "./parsers"
import type { BoardListOptions } from "../../types"

export async function listBoards(api: ApiTransport, options: BoardListOptions = {}) {
    const params = new URLSearchParams({
      include_archived: String(options.includeArchived ?? false),
    })
    const envelope = parseListBoardsEnvelope(await api.requestRaw(`/api/v1/boards?${params.toString()}`, {
      signal: options.signal,
    }))
    return envelope.data
  }
