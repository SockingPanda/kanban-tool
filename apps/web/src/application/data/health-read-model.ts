import type { ApiHealthResponseContract } from "../../lib/api/generated/contracts/api-health-response";

import type { ApiErrorResponseContract } from "../../lib/api/generated/contracts/api-error-response";

import { HttpTransportError, type HttpTransport, type HttpTransportOptions } from "./http-transport";

export type HealthReport = ApiHealthResponseContract["data"]

export type HealthReadErrorKind = "offline" | "http" | "invalid_json" | "invalid_contract" | "cross_origin" | "malformed_url" | "invalid_headers" | "invalid_content_type" | "invalid_bytes" | "response_too_large"

export class HealthReadError extends Error {
  readonly kind: HealthReadErrorKind
  readonly status: number | null
  readonly contractId: string | null
  readonly apiErrorCode: ApiErrorResponseContract["error"]["code"] | null
  readonly apiError: Pick<ApiErrorResponseContract["error"], "code"> | null

  constructor(
    kind: HealthReadErrorKind,
    message: string,
    options: {
      status?: number
      contractId?: string
      apiErrorCode?: ApiErrorResponseContract["error"]["code"]
      apiError?: Pick<ApiErrorResponseContract["error"], "code"> | null
      cause?: unknown
    } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "HealthReadError"
    this.kind = kind
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
    this.apiErrorCode = options.apiErrorCode ?? options.apiError?.code ?? null
    this.apiError = this.apiErrorCode === null ? null : { code: this.apiErrorCode }
  }

  /** Structured server error code, without exposing the raw response body. */
  get code(): ApiErrorResponseContract["error"]["code"] | null {
    return this.apiErrorCode
  }
}

export interface HealthReadDependencies extends HttpTransportOptions {
  readonly transport?: Pick<HttpTransport, "get">
}

export function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

export function wrapError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof HealthReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new HealthReadError(error.kind, "Web health request failed.", {
      status: error.status ?? undefined,
      apiErrorCode: error.apiError?.code,
      cause: error,
    })
  }
  throw new HealthReadError("invalid_contract", "Web health response is invalid.", { cause: error })
}
