import type { ApiListEventsQueryContract } from "../api/generated/contracts/api-list-events-query"
import type { ApiListEventsResponseContract } from "../api/generated/contracts/api-list-events-response"
import { parseApiListEventsQuery } from "../api/generated/contracts/api-list-events-query"
import { parseApiListEventsResponse } from "../api/generated/contracts/api-list-events-response"

export interface EventsApiClientOptions {
  readonly baseUrl?: string
  readonly fetcher?: typeof fetch
}

export interface EventsApiClient {
  listEvents(query: ApiListEventsQueryContract, signal?: AbortSignal): Promise<ApiListEventsResponseContract>
}

function apiUrl(baseUrl: string | undefined): URL | string {
  if (baseUrl !== undefined && baseUrl !== "") return new URL("/api/v1/events", baseUrl)
  if (typeof globalThis.location !== "undefined") return new URL("/api/v1/events", globalThis.location.origin)
  return "/api/v1/events"
}

export function createEventsApiClient(options: EventsApiClientOptions = {}): EventsApiClient {
  const fetcher = options.fetcher ?? fetch

  return {
    async listEvents(query, signal) {
      const validatedQuery = parseApiListEventsQuery(query)
      const endpoint = apiUrl(options.baseUrl)
      const params = new URLSearchParams()
      if (validatedQuery.board !== undefined) params.set("board", validatedQuery.board)
      if (validatedQuery.after !== undefined) params.set("after", String(validatedQuery.after))
      if (validatedQuery.limit !== undefined) params.set("limit", String(validatedQuery.limit))
      if (validatedQuery.task_id !== undefined && validatedQuery.task_id !== null) params.set("task_id", validatedQuery.task_id)
      const url = `${endpoint.toString()}?${params.toString()}`
      const response = await fetcher(url, {
        method: "GET",
        headers: { Accept: "application/json" },
        signal,
      })
      if (!response.ok) throw new Error(`api.list-events HTTP ${response.status}`)
      const payload: unknown = await response.json()
      return parseApiListEventsResponse(payload)
    },
  }
}
