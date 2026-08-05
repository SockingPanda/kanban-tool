import { ApiTransport } from "../../transport"
import type { RequestOptions, SearchIndexStatus } from "../../types"

export async function searchStatus(api: ApiTransport, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: api.board })
    return api.request<SearchIndexStatus>(`/api/v1/search/status?${params.toString()}`, options)
  }
