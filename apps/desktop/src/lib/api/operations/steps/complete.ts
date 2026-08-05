import { ApiTransport } from "../../transport"
import { parseCompleteStepEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function completeStep(api: ApiTransport, taskId: string, stepId: string, note: string, options: RequestOptions = {}) {
    return parseCompleteStepEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId + "/done", {
      method: "POST",
      body: { note, actor: api.actor },
      signal: options.signal,
    })).data
  }
