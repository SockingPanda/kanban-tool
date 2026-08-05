import { ApiTransport } from "../../transport"
import { parseListStepsEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"

export async function listSteps(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
    return parseListStepsEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps", options)).data
  }
