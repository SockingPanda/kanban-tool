import { describe, expect, test, vi } from "vitest"

import { createEventsApiClient } from "./events-api"

describe("typed list-events client", () => {
  test("validates query, serializes cursor parameters, and validates response", async () => {
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ data: [], meta: { next_after: 9 } }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    )
    const client = createEventsApiClient({ baseUrl: "http://127.0.0.1", fetcher })

    const response = await client.listEvents({ board: "board-a", after: 4, limit: 100, task_id: null })

    expect(response.meta.next_after).toBe(9)
    expect(fetcher).toHaveBeenCalledWith(
      "http://127.0.0.1/api/v1/events?board=board-a&after=4&limit=100",
      expect.objectContaining({ method: "GET" }),
    )
  })

  test("rejects malformed response before it reaches the sink", async () => {
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ data: [{ id: 1 }], meta: { next_after: 1 } }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    )
    const client = createEventsApiClient({ baseUrl: "http://127.0.0.1", fetcher })

    await expect(client.listEvents({ board: "board-a", after: 0, limit: 100 })).rejects.toThrow(/api\.list-events\.response/)
  })
})
