import type { ApiErrorResponseContract } from "../../lib/api/generated/contracts/api-error-response";


export const MAX_JSON_RESPONSE_BYTES = 16 * 1024 * 1024

export const MAX_BINARY_RESPONSE_BYTES = 256 * 1024 * 1024

export const MAX_PATH_DECODE_LAYERS = 64

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

export interface HttpTransportResponse {
  readonly payload: unknown
  readonly bytes: number
}

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

export interface RequestURLOptions {
  readonly opaqueAttachmentPath?: boolean
}

export interface JSONPayload {
  readonly payload: unknown
  readonly bytes: number
}

export const REQUEST_HEADER_NAMES = new Set(["Accept-Language", "Content-Type", "X-KB-Actor"])
