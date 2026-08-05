import { ApiTransport } from "../../transport"
import { parseListRunsEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"
export async function listRuns(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
    return parseListRunsEnvelope(await api.requestRaw(`/api/v1/tasks/${taskId}/runs`, options)).data
  }
