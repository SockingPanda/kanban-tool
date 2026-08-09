import type { WebRuntimeConfig } from "../runtime"
import { parseApiErrorResponse } from "./generated/contracts/api-error-response"
import type { ApiErrorResponseContract } from "./generated/contracts/api-error-response"

/** Per-response JSON byte cap. A response must fit before it reaches a generated parser. */
export const MAX_JSON_RESPONSE_BYTES = 16 * 1024 * 1024

export type HttpTransportErrorKind =
  | "cross_origin"
  | "malformed_url"
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_content_type"
  | "response_too_large"

export class HttpTransportError extends Error {
  readonly kind: HttpTransportErrorKind
  readonly status: number | null
  readonly apiError: ApiErrorResponseContract["error"] | null

  constructor(
    kind: HttpTransportErrorKind,
    message: string,
    options: {
      status?: number
      apiError?: ApiErrorResponseContract["error"]
      cause?: unknown
    } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "HttpTransportError"
    this.kind = kind
    this.status = options.status ?? null
    this.apiError = options.apiError ?? null
  }
}

/** Decoded transport payload plus the exact raw UTF-8 byte count consumed. */
export interface HttpTransportResponse {
  readonly payload: unknown
  readonly bytes: number
}

export interface HttpTransport {
  get(path: string, signal?: AbortSignal): Promise<HttpTransportResponse>
}

export interface HttpTransportOptions {
  readonly fetcher?: typeof fetch
  readonly documentBaseURI?: string
}

function documentBaseURI(): string {
  if (typeof document !== "undefined") return document.baseURI
  return "http://127.0.0.1/app/"
}

function malformedURL(message: string, cause: unknown): HttpTransportError {
  return new HttpTransportError("malformed_url", message, { cause })
}

function sameOriginBase(runtime: WebRuntimeConfig, baseURI: string): URL {
  let documentURL: URL
  try {
    documentURL = new URL(baseURI)
  } catch (cause) {
    throw malformedURL("当前页面 base URI 不是有效 URL。", cause)
  }

  const configuredBase = runtime.apiBaseUrl.trim()
  if (configuredBase.length > 0 && hasDotSegmentOrBackslash(configuredBase)) {
    throw new HttpTransportError("cross_origin", "runtime.apiBaseUrl 不得包含 dot segment 或 backslash。")
  }
  let base: URL
  try {
    base = configuredBase.length === 0
      ? new URL("/", documentURL)
      : new URL(configuredBase, documentURL)
  } catch (cause) {
    throw malformedURL("runtime.apiBaseUrl 不是有效 URL。", cause)
  }
  if (base.origin !== documentURL.origin) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API 请求必须使用当前页面的同源 runtime.apiBaseUrl。",
    )
  }
  if (!base.pathname.endsWith("/")) base.pathname += "/"
  return base
}

function hasDotSegmentOrBackslash(path: string): boolean {
  if (path.includes("\\") || path.includes("\u0000") || /%5c/i.test(path)) return true
  const pathOnly = path.split("?", 1)[0] ?? path
  const segments = pathOnly.split("/")
  for (const segment of segments) {
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

function requestURL(base: URL, path: string): string {
  if (!path.startsWith("/") || path.startsWith("//") || /^[a-z][a-z\d+.-]*:/i.test(path)) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API path 必须是当前 origin 下的绝对路径。",
    )
  }
  if (hasDotSegmentOrBackslash(path)) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API path 不得包含 dot segment 或 backslash。",
    )
  }

  const normalizedPath = path.replace(/^\/+/, "")
  let url: URL
  try {
    url = new URL(normalizedPath, base)
  } catch (cause) {
    throw malformedURL("Web API path 不是有效 URL。", cause)
  }
  const basePrefix = base.pathname.endsWith("/") ? base.pathname : `${base.pathname}/`
  if (url.origin !== base.origin) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API 请求必须保持当前页面同源。",
    )
  }
  if (!url.pathname.startsWith(basePrefix)) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API path 不得逃逸 runtime.apiBaseUrl 前缀。",
    )
  }
  return url.toString()
}

/** Resolve an API path with the same validated origin/prefix rules as HTTP GET. */
export function resolveHttpRequestURL(
  runtime: WebRuntimeConfig,
  path: string,
  options: Pick<HttpTransportOptions, "documentBaseURI"> = {},
): string {
  const base = sameOriginBase(runtime, options.documentBaseURI ?? documentBaseURI())
  return requestURL(base, path)
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function responseContentType(response: Response): string | null {
  const value = response.headers.get("content-type")
  if (value === null) return null
  return value.split(";", 1)[0]?.trim().toLowerCase() ?? null
}

function isJSONContentType(value: string | null): boolean {
  return value === "application/json" || value?.endsWith("+json") === true
}

async function cancelResponseBody(response: Response): Promise<void> {
  try {
    await response.body?.cancel()
  } catch {
    // The caller's typed rejection remains authoritative if cancellation races the network.
  }
}

function contentLength(response: Response): number | null {
  const value = response.headers.get("content-length")
  if (value === null) return null
  if (!/^\d+$/.test(value.trim())) {
    throw new HttpTransportError(
      "invalid_json",
      "Web API 响应的 Content-Length 无效。",
      { status: response.status },
    )
  }
  const length = Number(value)
  if (!Number.isSafeInteger(length)) {
    throw new HttpTransportError(
      "response_too_large",
      "Web API 响应的 Content-Length 超出安全范围。",
      { status: response.status },
    )
  }
  return length
}

interface JSONPayload {
  readonly payload: unknown
  readonly bytes: number
}

async function readJSON(response: Response): Promise<JSONPayload> {
  let declaredLength: number | null
  try {
    declaredLength = contentLength(response)
  } catch (error) {
    await cancelResponseBody(response)
    throw error
  }
  if (declaredLength !== null && declaredLength > MAX_JSON_RESPONSE_BYTES) {
    await cancelResponseBody(response)
    throw new HttpTransportError(
      "response_too_large",
      `Web API 响应超过 ${MAX_JSON_RESPONSE_BYTES} 字节上限。`,
      { status: response.status },
    )
  }

  const reader = response.body?.getReader()
  if (reader === undefined) {
    throw new HttpTransportError(
      "invalid_json",
      "Web API 响应没有可读取的 body。",
      { status: response.status },
    )
  }

  const chunks: Uint8Array[] = []
  let total = 0
  try {
    while (true) {
      const next = await reader.read()
      if (next.done) break
      const chunk = next.value
      if (chunk === undefined) continue
      total += chunk.byteLength
      if (total > MAX_JSON_RESPONSE_BYTES) {
        try {
          await reader.cancel()
        } catch {
          // The typed size error remains authoritative even if the stream refuses cancellation.
        }
        throw new HttpTransportError(
          "response_too_large",
          `Web API 响应超过 ${MAX_JSON_RESPONSE_BYTES} 字节上限。`,
          { status: response.status },
        )
      }
      chunks.push(chunk)
    }
  } catch (cause) {
    if (cause instanceof HttpTransportError) throw cause
    if (isAbortError(cause)) throw cause
    throw new HttpTransportError(
      "invalid_json",
      "Web API 响应 body 无法读取。",
      { status: response.status, cause },
    )
  } finally {
    try {
      reader.releaseLock()
    } catch {
      // Release is best effort after cancellation or an aborted stream.
    }
  }

  const bytes = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  let text: string
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
  } catch (cause) {
    throw new HttpTransportError(
      "invalid_json",
      "Web API 响应不是有效 UTF-8 JSON。",
      { status: response.status, cause },
    )
  }
  try {
    return { payload: JSON.parse(text) as unknown, bytes: total }
  } catch (cause) {
    throw new HttpTransportError(
      "invalid_json",
      "Web API 响应不是有效 JSON。",
      { status: response.status, cause },
    )
  }
}

function validateFinalOrigin(response: Response, requestOrigin: string): void {
  if (response.url.length === 0) {
    throw new HttpTransportError("cross_origin", "Web API 响应 URL 缺失。")
  }
  let finalURL: URL
  try {
    finalURL = new URL(response.url)
  } catch (cause) {
    throw new HttpTransportError("cross_origin", "Web API 响应 URL 不是有效 URL。", { cause })
  }
  if (finalURL.origin !== requestOrigin) {
    throw new HttpTransportError("cross_origin", "Web API 响应发生了跨 origin redirect。")
  }
}

export function createHttpTransport(
  runtime: WebRuntimeConfig,
  options: HttpTransportOptions = {},
): HttpTransport {
  const fetcher = options.fetcher ?? globalThis.fetch
  const base = sameOriginBase(runtime, options.documentBaseURI ?? documentBaseURI())

  return {
    async get(path, signal) {
      const url = requestURL(base, path)
      let response: Response
      try {
        response = await fetcher(url, {
          method: "GET",
          headers: { Accept: "application/json" },
          credentials: "same-origin",
          mode: "same-origin",
          redirect: "error",
          cache: "no-store",
          signal,
        })
      } catch (cause) {
        if (isAbortError(cause)) throw cause
        throw new HttpTransportError(
          "offline",
          "无法连接 kanban serve，请确认本地服务正在运行。",
          { cause },
        )
      }

      try {
        validateFinalOrigin(response, base.origin)
      } catch (error) {
        await cancelResponseBody(response)
        throw error
      }
      const contentType = responseContentType(response)
      if (!response.ok && !isJSONContentType(contentType)) {
        try {
          await readJSON(response)
        } catch (error) {
          if (error instanceof HttpTransportError && error.kind === "invalid_json") {
            throw new HttpTransportError(
              "http",
              `Web API 请求失败：HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`,
              { status: response.status, cause: error },
            )
          }
          throw error
        }
        throw new HttpTransportError(
          "http",
          `Web API 请求失败：HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`,
          { status: response.status },
        )
      }

      if (contentType === null || !isJSONContentType(contentType)) {
        await cancelResponseBody(response)
        throw new HttpTransportError(
          "invalid_content_type",
          "Web API 成功响应必须使用 application/json 或 +json Content-Type。",
          { status: response.status },
        )
      }

      const body = await readJSON(response)
      if (!response.ok) {
        try {
          const apiError = parseApiErrorResponse(body.payload)
          throw new HttpTransportError(
            "http",
            apiError.error.message,
            { status: response.status, apiError: apiError.error },
          )
        } catch (error) {
          if (error instanceof HttpTransportError) throw error
          throw new HttpTransportError(
            "http",
            `Web API 请求失败：HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`,
            { status: response.status, cause: error },
          )
        }
      }
      return body
    },
  }
}
