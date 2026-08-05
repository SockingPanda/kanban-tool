import { ApiTransport } from "../../transport"
import type { RequestOptions, RunLog } from "../../types"
export async function getRunLog(api: ApiTransport, runId: string, options: RequestOptions = {}) {
    return api.request<RunLog>(`/api/v1/runs/${runId}/log`, options)
  }
