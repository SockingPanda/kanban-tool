import type { ApiListEventsQueryContract } from "../../lib/api/generated/contracts/api-list-events-query"
import type { ApiListEventsResponseContract } from "../../lib/api/generated/contracts/api-list-events-response"
import { parseApiListEventsQuery } from "../../lib/api/generated/contracts/api-list-events-query"
import { parseApiListEventsResponse } from "../../lib/api/generated/contracts/api-list-events-response"
import type { RpcTransport } from "../data/rpc-transport";

export interface EventsApiClientOptions {
  readonly transport: RpcTransport
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
      const response = await options.transport.call({ method: "ListEvents", query: validatedQuery, signal })
      if (
        response === null
        || typeof response !== "object"
        || !("payload" in response)
        || typeof response.bytes !== "number"
        || !Number.isSafeInteger(response.bytes)
        || response.bytes < 0
      ) {
        throw new EventsApiError("事件 RPC transport 返回了无效的 Protobuf 字节数")
      }
      return parseApiListEventsResponse(response.payload)
    },
  }
}
