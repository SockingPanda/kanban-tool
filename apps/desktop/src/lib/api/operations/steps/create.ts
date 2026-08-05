import { ApiTransport } from "../../transport"
import { parseCreateStepEnvelope } from "./parsers"
import type { CreateStepInput, RequestOptions } from "../../types"

export async function createStep(api: ApiTransport, taskId: string, input: CreateStepInput, options: RequestOptions = {}) {
    return parseCreateStepEnvelope(await api.requestRaw("/api/v1/tasks/" + taskId + "/steps", {
      method: "POST",
      body: {
        ...input,
        idempotency_key: input.idempotency_key ?? `step.create:step_${crypto.randomUUID()}`,
        actor: api.actor,
      },
      signal: options.signal,
    })).data
  }
