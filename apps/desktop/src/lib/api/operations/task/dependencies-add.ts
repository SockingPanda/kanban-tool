import { ApiTransport } from "../../transport"
import type { Dependencies, RequestOptions } from "../../types"

export async function addDependency(api: ApiTransport, taskId: string, parentTaskId: string, options: RequestOptions = {}) {
    return api.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies`, {
      method: "POST",
      body: { parent_task_id: parentTaskId, actor: api.actor },
      signal: options.signal,
    })
  }
