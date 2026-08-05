import { ApiTransport } from "../../transport"
import type { BoardColumn, RequestOptions } from "../../types"

export async function listBoardColumns(api: ApiTransport, options: RequestOptions = {}) {
    return api.request<BoardColumn[]>(`/api/v1/boards/${api.board}/columns`, options)
  }
