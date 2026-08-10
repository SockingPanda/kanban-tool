import type { ApiListEventsQueryContract } from "../api/generated/contracts/api-list-events-query"
import type { ApiListEventsResponseContract } from "../api/generated/contracts/api-list-events-response"
import { parseApiListEventsQuery } from "../api/generated/contracts/api-list-events-query"
import { parseApiListEventsResponse } from "../api/generated/contracts/api-list-events-response"
import type { HttpTransport } from "../api/http-transport"

export interface EventsApiClientOptions {
  readonly transport: Pick<HttpTransport, "get">
}

export interface EventsApiClient {
  listEvents(query: ApiListEventsQueryContract, signal?: AbortSignal): Promise<ApiListEventsResponseContract>
}

export class EventsApiError extends Error {
  readonly kind = "anomaly" as const

  constructor(message: string) {
    super(message)
    this.name = "EventsApiError"
  }
}

export function createEventsApiClient(options: EventsApiClientOptions): EventsApiClient {
  return {
    async listEvents(query, signal) {
      const validatedQuery = parseApiListEventsQuery(query)
      const params = new URLSearchParams()
      if (validatedQuery.board !== undefined) params.set("board", validatedQuery.board)
      if (validatedQuery.after !== undefined) params.set("after", String(validatedQuery.after))
      if (validatedQuery.limit !== undefined) params.set("limit", String(validatedQuery.limit))
      if (validatedQuery.task_id !== undefined && validatedQuery.task_id !== null) params.set("task_id", validatedQuery.task_id)
      const path = `/api/v1/events?${params.toString()}`
      const response = await options.transport.get(path, signal)
      if (
        response === null
        || typeof response !== "object"
        || !("payload" in response)
        || typeof response.bytes !== "number"
        || !Number.isSafeInteger(response.bytes)
        || response.bytes < 0
      ) {
        throw new EventsApiError("events API transport returned an invalid raw JSON byte count")
      }
      return parseApiListEventsResponse(response.payload)
    },
  }
}
