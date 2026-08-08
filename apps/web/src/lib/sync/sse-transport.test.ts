import { describe, expect, test, vi } from "vitest"

import { createFetchSseTransport } from "./sse-transport"
import type { RawSseFrame } from "./contracts"

describe("fetch SSE transport", () => {
  test("streams arbitrary named frames and sends Last-Event-ID", async () => {
    const onFrame = vi.fn<(frame: RawSseFrame) => void>()
    const onError = vi.fn()
    const onEof = vi.fn()
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode("event: future\nid: 9\ndata: {}\n\n"))
        controller.close()
      },
    })
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } }),
    )

    const transport = createFetchSseTransport({ fetcher })
    const connection = transport({
      url: "http://127.0.0.1/api/v1/stream/events?board=default&after=8",
      signal: new AbortController().signal,
      headers: { "Last-Event-ID": "8" },
      onFrame,
      onError,
      onEof,
    })

    await vi.waitFor(() => expect(onEof).toHaveBeenCalledOnce())
    expect(fetcher).toHaveBeenCalledWith(
      "http://127.0.0.1/api/v1/stream/events?board=default&after=8",
      expect.objectContaining({ headers: expect.any(Headers) }),
    )
    const requestHeaders = new Headers(fetcher.mock.calls[0]?.[1]?.headers)
    expect(requestHeaders.get("Last-Event-ID")).toBe("8")
    expect(onFrame).toHaveBeenCalledWith({ eventName: "future", id: "9", data: "{}" })
    expect(onError).not.toHaveBeenCalled()
    expect(connection.closed).toBe(false)
    connection.close()
    expect(connection.closed).toBe(true)
  })

  test("reports HTTP and parser failures through onError", async () => {
    const onError = vi.fn()
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(new Response("no", { status: 500 }))
    createFetchSseTransport({ fetcher })({
      url: "http://127.0.0.1/api/v1/stream/events",
      signal: new AbortController().signal,
      onFrame: vi.fn(),
      onError,
      onEof: vi.fn(),
    })

    await vi.waitFor(() => expect(onError).toHaveBeenCalledOnce())
    expect(onError.mock.calls[0]?.[0]).toEqual(expect.objectContaining({ message: expect.stringContaining("HTTP 500") }))
  })

  test("fails closed when the response is not text/event-stream", async () => {
    const onError = vi.fn()
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(new Response("{}", { status: 200, headers: { "content-type": "application/json" } }))
    createFetchSseTransport({ fetcher })({
      url: "http://127.0.0.1/api/v1/stream/events",
      signal: new AbortController().signal,
      onFrame: vi.fn(),
      onError,
      onEof: vi.fn(),
    })

    await vi.waitFor(() => expect(onError).toHaveBeenCalledOnce())
    expect(onError.mock.calls[0]?.[0]).toEqual(expect.objectContaining({ message: expect.stringContaining("Content-Type") }))
  })

  test("uses fatal UTF-8 decoding for stream chunks", async () => {
    const onError = vi.fn()
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new Uint8Array([0xff]))
        controller.close()
      },
    })
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } }))
    createFetchSseTransport({ fetcher })({
      url: "http://127.0.0.1/api/v1/stream/events",
      signal: new AbortController().signal,
      onFrame: vi.fn(),
      onError,
      onEof: vi.fn(),
    })

    await vi.waitFor(() => expect(onError).toHaveBeenCalledOnce())
    expect(onError.mock.calls[0]?.[0]).toBeInstanceOf(TypeError)
  })

  test("aborts an in-flight fetch when the connection closes", async () => {
    let resolveFetch: (response: Response) => void = () => undefined
    const pendingResponse = new Promise<Response>((resolve) => {
      resolveFetch = resolve
    })
    const fetcher = vi.fn<typeof fetch>().mockReturnValue(pendingResponse)
    const onError = vi.fn()
    const connection = createFetchSseTransport({ fetcher })({
      url: "http://127.0.0.1/api/v1/stream/events",
      signal: new AbortController().signal,
      onFrame: vi.fn(),
      onError,
      onEof: vi.fn(),
    })

    const request = fetcher.mock.calls[0]?.[1]
    expect(request?.signal).toBeInstanceOf(AbortSignal)
    connection.close()
    expect(connection.closed).toBe(true)
    expect(request?.signal?.aborted).toBe(true)

    resolveFetch(new Response(null, { status: 200, headers: { "content-type": "text/event-stream" } }))
    await Promise.resolve()
    expect(onError).not.toHaveBeenCalled()
  })

  test("contains a reader.cancel rejection during close", async () => {
    const onError = vi.fn()
    const body = new ReadableStream<Uint8Array>({
      cancel() {
        return Promise.reject(new Error("cancel raced with stream close"))
      },
    })
    const fetcher = vi.fn<typeof fetch>().mockResolvedValue(new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } }))
    const connection = createFetchSseTransport({ fetcher })({
      url: "http://127.0.0.1/api/v1/stream/events",
      signal: new AbortController().signal,
      onFrame: vi.fn(),
      onError,
      onEof: vi.fn(),
    })
    await vi.waitFor(() => expect(fetcher).toHaveBeenCalledOnce())
    connection.close()
    await new Promise<void>((resolve) => queueMicrotask(resolve))
    expect(onError).not.toHaveBeenCalled()
  })
})
