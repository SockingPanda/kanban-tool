import { parseApiHealthResponse } from "../../lib/api/generated/contracts/api-health-response";

import { ContractValidationError } from "../../lib/api/generated/runtime";

import { createRpcTransport } from "./rpc-transport";

import type { WebRuntimeConfig } from "../../lib/runtime";

import { type HealthReadDependencies, type HealthReport, HealthReadError, wrapError } from "../../application/data/health-read-model";
import type { RpcTransport } from '../../application/data/rpc-transport';

export async function readQueryHealth(transport: RpcTransport, signal?: AbortSignal): Promise<HealthReport> {
  return readHealth({ transport, signal })
}

export async function readHealth(
  options: HealthReadDependencies & { readonly runtime?: WebRuntimeConfig; readonly signal?: AbortSignal } = {},
): Promise<HealthReport> {
  const transport = options.transport ?? (options.runtime ? createRpcTransport(options.runtime, options) : null)
  if (!transport) throw new HealthReadError("offline", "Web health read 缺少 runtime transport。")

  let payload: unknown
  try {
    payload = (await transport.call({ method: "GetHealth", signal: options.signal })).payload
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
