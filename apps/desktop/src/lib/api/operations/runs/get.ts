import { ApiTransport } from "../../transport"
import { parseGetRunEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"
export async function getRun(api: ApiTransport, runId: string, options: RequestOptions = {}) {
    return parseGetRunEnvelope(await api.requestRaw(`/api/v1/runs/${runId}`, options)).data
  }
