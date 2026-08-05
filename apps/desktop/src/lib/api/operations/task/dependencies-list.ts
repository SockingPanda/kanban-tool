import { ApiTransport } from "../../transport"
import type { Dependencies, RequestOptions } from "../../types"

export async function listDependencies(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
    return api.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies`, options)
  }
