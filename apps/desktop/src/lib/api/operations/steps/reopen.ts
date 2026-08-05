import { ApiTransport } from "../../transport"
import { parseReopenStepEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function reopenStep(api: ApiTransport, taskId: string, stepId: string, reason: string, options: RequestOptions = {}) {
    return parseReopenStepEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId + "/reopen", {
      method: "POST",
      body: { reason, actor: api.actor },
      signal: options.signal,
    })).data
  }
