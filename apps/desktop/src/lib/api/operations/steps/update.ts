import { ApiTransport } from "../../transport"
import { parseUpdateStepEnvelope } from "./parsers"
import type { RequestOptions, UpdateStepInput } from "../../types"

export async function updateStep(api: ApiTransport,
    taskId: string,
    stepId: string,
    input: UpdateStepInput,
    options: RequestOptions = {},
  ) {
    return parseUpdateStepEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId, {
      method: "PATCH",
      body: { ...input, actor: api.actor },
      signal: options.signal,
    })).data
  }
