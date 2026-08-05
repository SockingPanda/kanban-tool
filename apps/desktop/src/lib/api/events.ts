import { ApiTransport } from "./transport"
import type { EventMeta, EventPage, EventRecord, RequestOptions } from "./types"

export async function listEvents(api: ApiTransport, taskId: string, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: api.board, task_id: taskId, limit: "50" })
    const envelope = await api.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      options,
    )
    return { events: envelope.data, meta: envelope.meta ?? {} } satisfies EventPage
  }


export async function listBoardEvents(api: ApiTransport, options: { after?: number; limit?: number; signal?: AbortSignal } = {}) {
    const params = new URLSearchParams({ board: api.board, limit: String(options.limit ?? 100) })
    if (typeof options.after === "number") params.set("after", String(options.after))
    const envelope = await api.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      { signal: options.signal },
    )
    return { events: envelope.data, meta: envelope.meta ?? {} } satisfies EventPage
  }


export async function listEventsAfter(api: ApiTransport, after: number, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: api.board, after: String(after), limit: "100" })
    const envelope = await api.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      options,
    )
    return { events: envelope.data, meta: envelope.meta ?? {} } satisfies EventPage
  }
