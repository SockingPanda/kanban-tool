import { ApiTransport } from "../../transport"
import type { RequestOptions, Task } from "../../types"

export async function updateTask(api: ApiTransport, taskId: string, patch: Partial<Pick<Task, "title" | "description" | "assignee" | "priority" | "due_at" | "scheduled_at">>, options: RequestOptions = {}) {
    return api.request<Task>(`/api/v1/tasks/${taskId}`, {
      method: "PATCH",
      body: { ...patch, actor: api.actor },
      signal: options.signal,
    })
  }
