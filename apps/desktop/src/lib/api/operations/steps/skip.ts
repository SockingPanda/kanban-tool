import { ApiTransport } from "../../transport"
import { parseSkipStepEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function skipStep(api: ApiTransport, taskId: string, stepId: string, reason: string, options: RequestOptions = {}) {
    return parseSkipStepEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId + "/skip", {
      method: "POST",
      body: { reason, actor: api.actor },
      signal: options.signal,
    })).data
  }
