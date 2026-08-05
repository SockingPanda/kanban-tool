import { ApiTransport } from "../../transport"
import type { RequestOptions, Task } from "../../types"

export async function getTask(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
    return api.request<Task>(`/api/v1/tasks/${taskId}`, options)
  }
