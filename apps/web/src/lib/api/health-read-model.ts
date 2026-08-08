import type { ApiHealthResponseContract } from "./generated/contracts/api-health-response"
import { parseApiHealthResponse } from "./generated/contracts/api-health-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpTransport,
  type HttpTransportOptions,
} from "./http-transport"
import type { WebRuntimeConfig } from "../runtime"

export type HealthReport = ApiHealthResponseContract["data"]

export type HealthReadErrorKind = "offline" | "http" | "invalid_json" | "invalid_contract" | "cross_origin" | "malformed_url" | "invalid_content_type" | "response_too_large"

export class HealthReadError extends Error {
  readonly kind: HealthReadErrorKind
  readonly status: number | null
  readonly contractId: string | null

  constructor(
    kind: HealthReadErrorKind,
    message: string,
    options: { status?: number; contractId?: string; cause?: unknown } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "HealthReadError"
    this.kind = kind
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
  }
}

export interface HealthReadDependencies extends HttpTransportOptions {
  readonly transport?: HttpTransport
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function wrapError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof HealthReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new HealthReadError(error.kind, error.message, { status: error.status ?? undefined, cause: error })
  }
  throw error
}

/** Read `/health` through the same-origin typed transport and generated contract. */
export async function readHealth(
  options: HealthReadDependencies & { readonly runtime?: WebRuntimeConfig; readonly signal?: AbortSignal } = {},
): Promise<HealthReport> {
  const transport = options.transport ?? (options.runtime ? createHttpTransport(options.runtime, options) : null)
  if (!transport) throw new HealthReadError("offline", "Web health read 缺少 runtime transport。")

  let payload: unknown
  try {
    payload = (await transport.get("/health", options.signal)).payload
  } catch (error) {
    return wrapError(error)
  }

  try {
    return parseApiHealthResponse(payload).data
  } catch (error) {
    if (error instanceof ContractValidationError) {
      throw new HealthReadError("invalid_contract", "Web health 响应不符合当前协议。", {
        contractId: "api.health.response",
        cause: error,
      })
    }
    throw error
  }
}
