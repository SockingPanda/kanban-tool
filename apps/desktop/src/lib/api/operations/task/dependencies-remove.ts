import { ApiTransport } from "../../transport"
import type { Dependencies, RequestOptions } from "../../types"

export async function removeDependency(api: ApiTransport, taskId: string, parentTaskId: string, options: RequestOptions = {}) {
    return api.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies/${parentTaskId}`, {
      method: "DELETE",
      actorHeader: true,
      signal: options.signal,
    })
  }
