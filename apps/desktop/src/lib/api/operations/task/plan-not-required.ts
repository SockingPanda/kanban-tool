import { ApiTransport } from "../../transport"
import type { RequestOptions, TaskExecutionPlan } from "../../types"

export async function markExecutionPlanNotRequired(api: ApiTransport, taskId: string, reason: string, options: RequestOptions = {}) {
    return api.request<TaskExecutionPlan>("/api/v1/tasks/" + taskId + "/execution-plan/not-required", {
      method: "POST",
      body: { reason, actor: api.actor },
      signal: options.signal,
    })
  }
