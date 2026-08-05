import { ApiTransport } from "../../transport"
import { parseGetBoardEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function getBoard(api: ApiTransport, board: string, options: RequestOptions = {}) {
    return parseGetBoardEnvelope(await api.requestRaw(
      `/api/v1/boards/${encodeURIComponent(board)}`,
      { signal: options.signal },
    )).data
  }
