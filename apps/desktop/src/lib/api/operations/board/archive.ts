import { ApiTransport } from "../../transport"
import { parseArchiveBoardEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function archiveBoard(api: ApiTransport, board: string, options: RequestOptions = {}) {
    return parseArchiveBoardEnvelope(await api.requestRaw(
      `/api/v1/boards/${encodeURIComponent(board)}/archive`,
      {
        method: "POST",
        body: { actor: api.actor },
        actorHeader: true,
        signal: options.signal,
      },
    )).data
  }
