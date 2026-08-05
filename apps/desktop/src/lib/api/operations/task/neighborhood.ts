import { ApiTransport } from "../../transport"
import type { RequestOptions, TaskNeighborhood } from "../../types"

export async function getTaskNeighborhood(api: ApiTransport,
    taskId: string,
    options: RequestOptions & { depth?: number; limitNodes?: number } = {},
  ) {
    const params = new URLSearchParams({ depth: String(options.depth ?? 1) })
    if (typeof options.limitNodes === "number") params.set("limit_nodes", String(options.limitNodes))
    return api.request<TaskNeighborhood>("/api/v1/tasks/" + taskId + "/neighborhood?" + params.toString(), {
      signal: options.signal,
    })
  }
