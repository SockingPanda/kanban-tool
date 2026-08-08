import type { WebRuntimeConfig } from "../runtime"
import { parseApiErrorResponse } from "./generated/contracts/api-error-response"
import type { ApiErrorResponseContract } from "./generated/contracts/api-error-response"

/** Per-response JSON byte cap. A response must fit before it reaches a generated parser. */
export const MAX_JSON_RESPONSE_BYTES = 16 * 1024 * 1024
/** Maximum attachment body size accepted by the browser transport. */
export const MAX_BINARY_RESPONSE_BYTES = 256 * 1024 * 1024
const MAX_PATH_DECODE_LAYERS = 64

export type HttpTransportErrorKind =
  | "cross_origin"
  | "malformed_url"
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_headers"
  | "invalid_content_type"
  | "invalid_bytes"
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

/** Raw attachment bytes plus the response metadata allowed by the web API. */
export interface HttpTransportBytesResponse {
  readonly bytes: Uint8Array
  readonly contentType: string | null
  readonly attachmentId: string | null
  readonly sha256: string | null
}

export interface HttpReadTransport {
  get(path: string, signal?: AbortSignal): Promise<HttpTransportResponse>
}

export type HttpRequestMethod = "GET" | "POST" | "PATCH" | "DELETE"

export interface HttpTransport extends HttpReadTransport {
  /** Raw, non-generic JSON request used by generated operation adapters. */
  request(options: {
    readonly method: HttpRequestMethod
    readonly path: string
    readonly body?: unknown
    readonly headers?: Readonly<Record<string, string | null>>
    readonly signal?: AbortSignal
  }): Promise<HttpTransportResponse>
  /** Download a bounded raw byte response without converting it through JSON. */
  requestBytes(options: {
    readonly method: "GET"
    readonly path: string
    readonly headers?: Readonly<Record<string, string | null>>
    readonly signal?: AbortSignal
  }): Promise<HttpTransportBytesResponse>
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
    if (hasMalformedPercent(segment)) return true

    let layer = segment
    for (let unwrapLayers = 0; ; unwrapLayers += 1) {
      if (
        layer.includes("\\")
        || layer.includes("/")
        || layer.includes("\u0000")
        || /%(?:2f|5c|00)/i.test(layer)
      ) return true
      const dotView = layer.replace(/%2e/gi, ".")
      if (dotView === "." || dotView === "..") return true
      if (!layer.includes("%25")) break
      if (unwrapLayers >= MAX_PATH_DECODE_LAYERS) return true
      layer = layer.replace(/%25/g, "%")
    }
  }
  return false
}

function hasMalformedPercent(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] !== "%") continue
    const first = value[index + 1]
    const second = value[index + 2]
    if (first === undefined || second === undefined || !isHex(first) || !isHex(second)) return true
    index += 2
  }
  return false
}

function isHex(value: string): boolean {
  return /^[0-9a-f]$/i.test(value)
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

function binaryContentLength(response: Response): number {
  const value = response.headers.get("content-length")
  if (value === null) {
    throw new HttpTransportError(
      "invalid_bytes",
      "Web API 二进制响应必须声明 Content-Length。",
      { status: response.status },
    )
  }
  const normalized = value.trim()
  if (!/^\d+$/.test(normalized)) {
    throw new HttpTransportError(
      "invalid_bytes",
      "Web API 二进制响应的 Content-Length 无效或重复。",
      { status: response.status },
    )
  }
  const length = Number(normalized)
  if (!Number.isSafeInteger(length) || length > MAX_BINARY_RESPONSE_BYTES) {
    throw new HttpTransportError(
      "response_too_large",
      `Web API 二进制响应超过 ${MAX_BINARY_RESPONSE_BYTES} 字节上限。`,
      { status: response.status },
    )
  }
  return length
}

async function cancelReader(reader: ReadableStreamDefaultReader<Uint8Array>): Promise<void> {
  try {
    await reader.cancel()
  } catch {
    // The typed bytes error remains authoritative if cancellation races the network.
  }
}

async function readBytes(response: Response, declaredLength: number): Promise<Uint8Array> {
  if (response.body === null) {
    if (declaredLength === 0) return new Uint8Array(0)
    throw new HttpTransportError(
      "invalid_bytes",
      "Web API 二进制响应没有可读取的 body。",
      { status: response.status },
    )
  }

  let reader: ReadableStreamDefaultReader<Uint8Array>
  try {
    reader = response.body.getReader()
  } catch (cause) {
    await cancelResponseBody(response)
    throw new HttpTransportError(
      "invalid_bytes",
      "Web API 二进制响应没有可读取的 body。",
      { status: response.status, cause },
    )
  }

  let bytes: Uint8Array
  try {
    bytes = new Uint8Array(declaredLength)
  } catch (cause) {
    await cancelReader(reader)
    try {
      reader.releaseLock()
    } catch {
      // Release is best effort after an allocation failure.
    }
    throw new HttpTransportError(
      "response_too_large",
      `Web API 二进制响应超过 ${MAX_BINARY_RESPONSE_BYTES} 字节上限。`,
      { status: response.status, cause },
    )
  }

  let offset = 0
  try {
    while (true) {
      const next = await reader.read()
      if (next.done) {
        if (offset !== declaredLength) {
          throw new HttpTransportError(
            "invalid_bytes",
            "Web API 二进制响应的实际长度与 Content-Length 不一致。",
            { status: response.status },
          )
        }
        return bytes
      }

      const chunk = next.value
      if (!(chunk instanceof Uint8Array)) {
        throw new HttpTransportError(
          "invalid_bytes",
          "Web API 二进制响应包含无效 body chunk。",
          { status: response.status },
        )
      }
      if (chunk.byteLength > MAX_BINARY_RESPONSE_BYTES - offset) {
        throw new HttpTransportError(
          "response_too_large",
          `Web API 二进制响应超过 ${MAX_BINARY_RESPONSE_BYTES} 字节上限。`,
          { status: response.status },
        )
      }
      if (chunk.byteLength > declaredLength - offset) {
        throw new HttpTransportError(
          "invalid_bytes",
          "Web API 二进制响应的实际长度与 Content-Length 不一致。",
          { status: response.status },
        )
      }
      bytes.set(chunk, offset)
      offset += chunk.byteLength
    }
  } catch (cause) {
    await cancelReader(reader)
    if (cause instanceof HttpTransportError || isAbortError(cause)) throw cause
    throw new HttpTransportError(
      "invalid_bytes",
      "Web API 二进制响应 body 无法读取。",
      { status: response.status, cause },
    )
  } finally {
    try {
      reader.releaseLock()
    } catch {
      // Release is best effort after cancellation or an aborted stream.
    }
  }
}

function genericHTTPError(response: Response, cause?: unknown): HttpTransportError {
  return new HttpTransportError(
    "http",
    `Web API 请求失败：HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`,
    { status: response.status, cause },
  )
}

async function throwByteHTTPError(response: Response): Promise<never> {
  const contentType = responseContentType(response)
  if (contentType === null || !isJSONContentType(contentType)) {
    await cancelResponseBody(response)
    throw genericHTTPError(
      response,
      new HttpTransportError(
        "invalid_content_type",
        "Web API 错误响应必须使用 application/json 或 +json Content-Type。",
        { status: response.status },
      ),
    )
  }

  let responseBody: JSONPayload
  try {
    responseBody = await readJSON(response)
  } catch (cause) {
    if (cause instanceof HttpTransportError && cause.kind === "response_too_large") throw cause
    throw genericHTTPError(response, cause)
  }

  try {
    const apiError = parseApiErrorResponse(responseBody.payload)
    throw new HttpTransportError(
      "http",
      apiError.error.message,
      { status: response.status, apiError: apiError.error },
    )
  } catch (cause) {
    if (cause instanceof HttpTransportError) throw cause
    throw genericHTTPError(response, cause)
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

const REQUEST_HEADER_NAMES = new Set(["Accept-Language", "Content-Type", "X-KB-Actor"])

function requestHeaders(
  provided: Readonly<Record<string, string | null>> | undefined,
  hasBody: boolean,
  accept = "application/json",
): Record<string, string> {
  const headers: Record<string, string> = { Accept: accept }
  if (provided === undefined) {
    if (hasBody) headers["Content-Type"] = "application/json"
    return headers
  }

  const seen = new Set<string>()
  for (const [name, value] of Object.entries(provided)) {
    if (!REQUEST_HEADER_NAMES.has(name)) {
      throw new HttpTransportError("invalid_headers", `Web API 请求包含不支持的 header：${name}。`)
    }
    const canonicalName = name
    if (seen.has(canonicalName)) {
      throw new HttpTransportError("invalid_headers", `Web API 请求重复设置 header：${canonicalName}。`)
    }
    seen.add(canonicalName)
    if (value === null) continue
    if (typeof value !== "string") {
      throw new HttpTransportError("invalid_headers", `Web API 请求 header ${canonicalName} 必须是字符串或 null。`)
    }
    if (canonicalName === "Content-Type" && value !== "application/json") {
      throw new HttpTransportError("invalid_headers", "Web API JSON 请求的 Content-Type 必须是 application/json。")
    }
    headers[canonicalName] = value
  }

  if (hasBody) {
    if (Object.hasOwn(provided, "Content-Type") && (provided["Content-Type"] === null || headers["Content-Type"] !== "application/json")) {
      throw new HttpTransportError("invalid_headers", "带 body 的 Web API 请求必须使用 application/json Content-Type。")
    }
    headers["Content-Type"] = "application/json"
  } else if (provided["Content-Type"] !== undefined && provided["Content-Type"] !== null) {
    throw new HttpTransportError("invalid_headers", "无 body 的 Web API 请求不得设置 Content-Type。")
  }

  return headers
}

export function createHttpTransport(
  runtime: WebRuntimeConfig,
  options: HttpTransportOptions = {},
): HttpTransport {
  const fetcher = options.fetcher ?? globalThis.fetch
  const base = sameOriginBase(runtime, options.documentBaseURI ?? documentBaseURI())

  async function request(options: {
    readonly method: HttpRequestMethod
    readonly path: string
    readonly body?: unknown
    readonly headers?: Readonly<Record<string, string | null>>
    readonly signal?: AbortSignal
  }): Promise<HttpTransportResponse> {
    const { method, path, body: bodyValue, headers: requestHeaderValues, signal } = options
    const url = requestURL(base, path)
    const headers = requestHeaders(requestHeaderValues, bodyValue !== undefined)
    let body: string | undefined
    if (bodyValue !== undefined) {
      try {
        body = JSON.stringify(bodyValue)
      } catch (cause) {
        throw new HttpTransportError("invalid_json", "Web API request body 无法编码为 JSON。", { cause })
      }
      if (body === undefined) {
        throw new HttpTransportError("invalid_json", "Web API request body 无法编码为 JSON。")
      }
      headers["Content-Type"] = "application/json"
    }

    let response: Response
    try {
      const init: RequestInit = {
        method,
        headers,
        credentials: "same-origin",
        mode: "same-origin",
        redirect: "error",
        cache: "no-store",
        signal,
      }
      if (body !== undefined) init.body = body
      response = await fetcher(url, init)
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

    const responseBody = await readJSON(response)
    if (!response.ok) {
      try {
        const apiError = parseApiErrorResponse(responseBody.payload)
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
    return responseBody
  }

  async function requestBytes(options: {
    readonly method: "GET"
    readonly path: string
    readonly headers?: Readonly<Record<string, string | null>>
    readonly signal?: AbortSignal
  }): Promise<HttpTransportBytesResponse> {
    const { path, headers: requestHeaderValues, signal } = options
    const url = requestURL(base, path)
    const headers = requestHeaders(requestHeaderValues, false, "application/octet-stream")

    let response: Response
    try {
      response = await fetcher(url, {
        method: "GET",
        headers,
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

    if (!response.ok) {
      return throwByteHTTPError(response)
    }

    let declaredLength: number
    try {
      declaredLength = binaryContentLength(response)
    } catch (error) {
      await cancelResponseBody(response)
      throw error
    }
    const bytes = await readBytes(response, declaredLength)
    return {
      bytes,
      contentType: response.headers.get("content-type"),
      attachmentId: response.headers.get("x-kb-attachment-id"),
      sha256: response.headers.get("x-kb-attachment-sha256"),
    }
  }

  return {
    get: (path, signal) => request({ method: "GET", path, signal }),
    request,
    requestBytes,
  }
}
