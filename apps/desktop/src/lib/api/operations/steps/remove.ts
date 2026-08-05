import { ApiTransport } from "../../transport"
import { parseRemoveStepEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function removeStep(api: ApiTransport, taskId: string, stepId: string, options: RequestOptions = {}) {
    return parseRemoveStepEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId, {
      method: "DELETE",
      signal: options.signal,
    })).data
  }
