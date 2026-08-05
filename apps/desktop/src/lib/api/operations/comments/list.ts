import { ApiTransport } from "../../transport"
import { parseListCommentsEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"
export async function listComments(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
    return parseListCommentsEnvelope(await api.requestRaw(`/api/v1/tasks/${taskId}/comments`, options)).data
  }
