import { ApiTransport } from "../../transport"
import type { BoardStats, RequestOptions } from "../../types"

export async function stats(api: ApiTransport, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: api.board })
    return api.request<BoardStats>(`/api/v1/stats?${params.toString()}`, options)
  }
