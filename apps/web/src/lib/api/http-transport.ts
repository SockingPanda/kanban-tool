import type { WebRuntimeConfig } from "../runtime"
import { parseApiErrorResponse } from "./generated/contracts/api-error-response"
import type { ApiErrorResponseContract } from "./generated/contracts/api-error-response"

export type HttpTransportErrorKind = "cross_origin" | "offline" | "http" | "invalid_json"

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

export interface HttpTransport {
  get(path: string, signal?: AbortSignal): Promise<unknown>
}

export interface HttpTransportOptions {
  readonly fetcher?: typeof fetch
  readonly fetch?: typeof fetch
  readonly documentBaseURI?: string
}

function documentBaseURI(): string {
  if (typeof document !== "undefined") return document.baseURI
  return "http://127.0.0.1/app/"
}

function sameOriginBase(runtime: WebRuntimeConfig, baseURI: string): URL {
  const documentURL = new URL(baseURI)
  const configuredBase = runtime.apiBaseUrl.trim()
  const base = configuredBase.length === 0
    ? new URL("/", documentURL)
    : new URL(configuredBase, documentURL)
  if (base.origin !== documentURL.origin) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API 请求必须使用当前页面的同源 runtime.apiBaseUrl。",
    )
  }
  if (!base.pathname.endsWith("/")) base.pathname += "/"
  return base
}

function requestURL(base: URL, path: string): string {
  if (!path.startsWith("/") || path.startsWith("//") || /^[a-z][a-z\d+.-]*:/i.test(path)) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API path 必须是当前 origin 下的绝对路径。",
    )
  }
  const normalizedPath = path.replace(/^\/+/, "")
  const url = new URL(normalizedPath, base)
  if (url.origin !== base.origin) {
    throw new HttpTransportError(
      "cross_origin",
      "Web API 请求必须保持当前页面同源。",
    )
  }
  return url.toString()
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

async function readJSON(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch (cause) {
    throw new HttpTransportError(
      "invalid_json",
      "Web API 响应不是有效 JSON。",
      { status: response.status, cause },
    )
  }
}

export function createHttpTransport(
  runtime: WebRuntimeConfig,
  options: HttpTransportOptions = {},
): HttpTransport {
  const fetcher = options.fetcher ?? options.fetch ?? globalThis.fetch
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

      if (!response.ok) {
        let payload: unknown
        try {
          payload = await readJSON(response)
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
        try {
          const apiError = parseApiErrorResponse(payload)
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

      return readJSON(response)
    },
  }
}

export const createSameOriginHttpTransport = createHttpTransport
