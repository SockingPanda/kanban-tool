import { parseApiHealthResponse } from "../../lib/api/generated/contracts/api-health-response";

import { ContractValidationError } from "../../lib/api/generated/runtime";

import { createHttpTransport } from "./http-transport";

import type { WebRuntimeConfig } from "../../lib/runtime";

import { type HealthReadDependencies, type HealthReport, HealthReadError, wrapError } from "../../application/data/health-read-model";

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
