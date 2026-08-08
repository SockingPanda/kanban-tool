import { createSseParser } from "./sse-parser"
import type { SseTransport, SseTransportConnection, SseTransportRequest } from "./contracts"

export interface FetchSseTransportOptions {
  readonly fetcher?: typeof fetch
  readonly parser?: Parameters<typeof createSseParser>[0]
}

/**
 * A small fetch + ReadableStream transport. Native EventSource cannot expose
 * unknown named events or attach the required Last-Event-ID header, so every
 * frame is forwarded to the sync controller as a raw frame.
 */
export function createFetchSseTransport(options: FetchSseTransportOptions = {}): SseTransport {
  const fetcher = options.fetcher ?? fetch

  return (request: SseTransportRequest): SseTransportConnection => {
    let closed = false
    let reader: ReadableStreamDefaultReader<Uint8Array> | null = null
    const parser = createSseParser(options.parser)
    const internalAbort = new AbortController()
    const onExternalAbort = (): void => {
      closeConnection()
    }

    function cleanupExternalAbort(): void {
      request.signal.removeEventListener("abort", onExternalAbort)
    }

    function closeConnection(): void {
      if (closed) return
      closed = true
      cleanupExternalAbort()
      internalAbort.abort()
      void reader?.cancel()
    }

    if (request.signal.aborted) {
      closeConnection()
    } else {
      request.signal.addEventListener("abort", onExternalAbort, { once: true })
    }

    const connection: SseTransportConnection = {
      get closed() {
        return closed
      },
      close() {
        closeConnection()
      },
    }

    void (async () => {
      try {
        if (closed) return
        const headers = new Headers(request.headers)
        headers.set("Accept", "text/event-stream")
        const response = await fetcher(request.url, {
          method: "GET",
          headers,
          signal: internalAbort.signal,
        })
        if (!response.ok) throw new Error(`SSE HTTP ${response.status}`)
        const contentType = response.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase()
        if (contentType !== "text/event-stream") throw new Error("SSE response has invalid Content-Type")
        if (!response.body) throw new Error("SSE response has no body")

        reader = response.body.getReader()
        const decoder = new TextDecoder("utf-8", { fatal: true })
        while (!closed) {
          const chunk = await reader.read()
          if (chunk.done) break
          if (!chunk.value) continue
          for (const frame of parser.push(decoder.decode(chunk.value, { stream: true }))) {
            if (!closed) request.onFrame(frame)
          }
        }
        if (closed) return
        const tail = decoder.decode()
        if (tail !== "") {
          for (const frame of parser.push(tail)) request.onFrame(frame)
        }
        parser.finish()
        if (!closed) request.onEof()
      } catch (error) {
        if (!closed) request.onError(error)
      } finally {
        cleanupExternalAbort()
      }
    })()

    return connection
  }
}
