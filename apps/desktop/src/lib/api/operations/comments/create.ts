import { ApiTransport } from "../../transport"
import { parseCreateCommentEnvelope } from "./parsers"
import type { RequestOptions } from "../../types"
export async function createComment(api: ApiTransport, taskId: string, body: string, options: RequestOptions = {}) {
    return parseCreateCommentEnvelope(await api.requestRaw(`/api/v1/tasks/${taskId}/comments`, {
      method: "POST",
      body: {
        idempotency_key: options.idempotencyKey ?? `comment.create:c_${crypto.randomUUID()}`,
        author: api.actor,
        body,
      },
      signal: options.signal,
    })).data
  }
