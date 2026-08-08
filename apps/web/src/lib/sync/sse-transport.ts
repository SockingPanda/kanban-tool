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
    let readerCleanup: Promise<void> | null = null
    const parser = createSseParser(options.parser)
    const internalAbort = new AbortController()
    const onExternalAbort = (): void => {
      closeConnection()
    }

    function cleanupExternalAbort(): void {
      request.signal.removeEventListener("abort", onExternalAbort)
    }

    function reportError(error: unknown): void {
      try {
        request.onError(error)
      } catch {
        // Consumer callbacks must never create an unhandled rejection in the
        // detached transport task.
      }
    }

    function cleanupReader(cancel: boolean): Promise<void> {
      if (readerCleanup !== null) return readerCleanup
      const current = reader
      if (current === null) return Promise.resolve()
      readerCleanup = (async () => {
        if (cancel) {
          try {
            await current.cancel()
          } catch {
            // Cancellation races with an already-closed stream are expected.
          }
        }
        try {
          current.releaseLock()
        } catch {
          // The stream may have released the lock as part of its own close.
        }
        if (reader === current) reader = null
      })()
      return readerCleanup
    }

    function closeConnection(): void {
      if (closed) return
      closed = true
      cleanupExternalAbort()
      internalAbort.abort()
      void cleanupReader(true).catch(() => undefined)
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

    const run = async (): Promise<void> => {
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
        if (closed) {
          await cleanupReader(true)
          return
        }
        const decoder = new TextDecoder("utf-8", { fatal: true })
        while (!closed) {
          const currentReader = reader
          if (currentReader === null) return
          const chunk = await currentReader.read()
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
        if (!closed) reportError(error)
      } finally {
        cleanupExternalAbort()
        await cleanupReader(false)
      }
    }
    void run().catch((error) => {
      if (!closed) reportError(error)
    })

    return connection
  }
}
