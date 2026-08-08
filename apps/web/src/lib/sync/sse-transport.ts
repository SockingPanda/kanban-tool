import { createSseParser } from "./sse-parser"
import type { SseTransport, SseTransportConnection, SseTransportRequest } from "./contracts"

export interface FetchSseTransportOptions {
  readonly fetcher?: typeof fetch
  readonly parser?: Parameters<typeof createSseParser>[0]
  /** Test seam for resolving relative stream URLs without a browser document. */
  readonly documentBaseURI?: string
}

export type SseTransportErrorKind = "cross_origin" | "malformed_url" | "http" | "invalid_content_type" | "empty_body"

export class SseTransportError extends Error {
  readonly kind: SseTransportErrorKind
  readonly status: number | null

  constructor(kind: SseTransportErrorKind, message: string, status?: number, options: { cause?: unknown } = {}) {
    super(message, { cause: options.cause })
    this.name = "SseTransportError"
    this.kind = kind
    this.status = status ?? null
  }
}

function defaultDocumentBaseURI(): string {
  if (typeof document !== "undefined") return document.baseURI
  if (typeof globalThis.location !== "undefined") return globalThis.location.href
  return "http://127.0.0.1/app/"
}

function hasDotSegmentOrBackslash(path: string): boolean {
  if (path.includes("\\") || path.includes("\u0000") || /%5c/i.test(path)) return true
  const pathOnly = path.split("?", 1)[0] ?? path
  for (const segment of pathOnly.split("/")) {
    if (segment === "." || segment === "..") return true
    try {
      let decoded = segment
      for (let pass = 0; pass < 64; pass += 1) {
        if (
          decoded === "."
          || decoded === ".."
          || decoded.includes("\\")
          || decoded.includes("/")
          || decoded.includes("\u0000")
        ) return true
        const next = decodeURIComponent(decoded)
        if (next === decoded) break
        decoded = next
      }
      if (
        decoded === "."
        || decoded === ".."
        || decoded.includes("\\")
        || decoded.includes("/")
        || decoded.includes("\u0000")
        || decoded.includes("%")
      ) return true
    } catch {
      return true
    }
  }
  return false
}

function resolveStreamURL(input: string, baseURI: string): string {
  let base: URL
  try {
    base = new URL(baseURI)
  } catch (cause) {
    throw new SseTransportError("malformed_url", "SSE document base URI 不是有效 URL。", undefined, { cause })
  }
  if (hasDotSegmentOrBackslash(input)) {
    throw new SseTransportError("cross_origin", "SSE URL 不得包含 dot segment 或 backslash。")
  }
  let url: URL
  try {
    url = new URL(input, base)
  } catch (cause) {
    throw new SseTransportError("malformed_url", "SSE URL 不是有效 URL。", undefined, { cause })
  }
  if (url.origin !== base.origin) {
    throw new SseTransportError("cross_origin", "SSE 请求必须使用当前页面的同源 origin。")
  }
  return url.toString()
}

function responseContentType(response: Response): string | null {
  return response.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase() ?? null
}

async function cancelResponseBody(response: Response): Promise<void> {
  try {
    await response.body?.cancel()
  } catch {
    // The typed transport error remains authoritative if cancellation races the network.
  }
}

function validateFinalOrigin(response: Response, origin: string): void {
  if (response.url.length === 0) {
    throw new SseTransportError("cross_origin", "SSE 响应 URL 缺失。")
  }
  let finalURL: URL
  try {
    finalURL = new URL(response.url)
  } catch {
    throw new SseTransportError("cross_origin", "SSE 响应 URL 不是有效 URL。")
  }
  if (finalURL.origin !== origin) {
    throw new SseTransportError("cross_origin", "SSE 响应发生了跨 origin redirect。")
  }
}

/**
 * A small fetch + ReadableStream transport. Native EventSource cannot expose
 * unknown named events or attach the required Last-Event-ID header, so every
 * frame is forwarded to the sync controller as a raw frame.
 */
export function createFetchSseTransport(options: FetchSseTransportOptions = {}): SseTransport {
  const fetcher = options.fetcher ?? fetch
  const baseURI = options.documentBaseURI ?? defaultDocumentBaseURI()

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
        // Consumer callbacks must never create an unhandled rejection in the detached task.
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

    async function cancelClosedResponse(response: Response): Promise<void> {
      await cancelResponseBody(response)
      await cleanupReader(true)
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
        const url = resolveStreamURL(request.url, baseURI)
        const headers = new Headers(request.headers)
        headers.set("Accept", "text/event-stream")
        const response = await fetcher(url, {
          method: "GET",
          headers,
          credentials: "same-origin",
          mode: "same-origin",
          redirect: "error",
          cache: "no-store",
          signal: internalAbort.signal,
        })
        if (closed) {
          await cancelClosedResponse(response)
          return
        }
        try {
          validateFinalOrigin(response, new URL(baseURI).origin)
        } catch (error) {
          await cancelResponseBody(response)
          throw error
        }
        if (!response.ok) {
          await cancelResponseBody(response)
          throw new SseTransportError("http", `SSE HTTP ${response.status}`, response.status)
        }
        if (responseContentType(response) !== "text/event-stream") {
          await cancelResponseBody(response)
          throw new SseTransportError("invalid_content_type", "SSE response has invalid Content-Type", response.status)
        }
        if (!response.body) throw new SseTransportError("empty_body", "SSE response has no body", response.status)

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
          for (const frame of parser.push(tail)) {
            if (!closed) request.onFrame(frame)
          }
        }
        parser.finish()
        if (!closed) request.onEof()
      } catch (error) {
        if (!closed) {
          await cleanupReader(true)
          reportError(error)
        }
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
