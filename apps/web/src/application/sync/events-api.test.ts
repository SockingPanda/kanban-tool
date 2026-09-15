import { describe, expect, test, vi } from "vitest"

import type { RpcTransport } from "../data/rpc-transport";
import { createEventsApiClient } from "./events-api"

describe("typed list-events client", () => {
  test("validates query, serializes cursor parameters, and validates response", async () => {
    const transport: RpcTransport = {
      call: vi.fn<RpcTransport["call"]>().mockResolvedValue({
        payload: { data: [], meta: { next_after: 9 } },
        bytes: 29,
      }),
    }
    const client = createEventsApiClient({ transport })

    const response = await client.listEvents({ board: "board-a", after: 4, limit: 100, task_id: null })

    expect(response.meta.next_after).toBe(9)
    expect(transport.call).toHaveBeenCalledWith({ method: "ListEvents", query: { board: "board-a", after: 4, limit: 100, task_id: null }, signal: undefined })
  })

  test("rejects malformed response before it reaches the sink", async () => {
    const transport: RpcTransport = {
      call: vi.fn<RpcTransport["call"]>().mockResolvedValue({
        payload: { data: [{ id: 1 }], meta: { next_after: 1 } },
        bytes: 23,
      }),
    }
    const client = createEventsApiClient({ transport })

    await expect(client.listEvents({ board: "board-a", after: 0, limit: 100 })).rejects.toThrow(/api\.list-events\.response/)
  })

  test("preserves an AbortError from the injected RPC read seam", async () => {
    const abortError = new Error("request aborted")
    abortError.name = "AbortError"
    const call = vi.fn<RpcTransport["call"]>().mockRejectedValue(abortError)
    const client = createEventsApiClient({ transport: { call } })
    const signal = new AbortController().signal

    await expect(client.listEvents({ board: "board-a", after: 0, limit: 100 }, signal)).rejects.toBe(abortError)
    expect(call).toHaveBeenCalledWith({ method: "ListEvents", query: { board: "board-a", after: 0, limit: 100 }, signal })
  })

  test("rejects an invalid raw byte count from the injected seam", async () => {
    const transport: RpcTransport = {
      call: vi.fn<RpcTransport["call"]>().mockResolvedValue({
        payload: { data: [], meta: { next_after: 0 } },
        bytes: Number.NaN,
      }),
    }
    const client = createEventsApiClient({ transport })

    await expect(client.listEvents({ board: "board-a", after: 0, limit: 100 })).rejects.toMatchObject({ kind: "anomaly" })
  })
})
