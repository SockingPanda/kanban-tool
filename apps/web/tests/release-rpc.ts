import type { APIResponse } from "@playwright/test"

import { createRpcTransport } from "../src/adapters/host/rpc-transport"
import { RpcTransportError, type RpcCall, type RpcMethod } from "../src/application/data/rpc-transport"

/** 真实 Host 验收的具名 RPC 客户端；保留既有状态/业务 DTO 断言，网络只使用 binary gRPC-Web。 */
export async function rpcRequest(method: RpcMethod, parts: Omit<RpcCall, "method"> = {}) {
  const baseURL = process.env.KANBAN_RELEASE_BASE_URL
  if (!baseURL) throw new Error("真实 RPC 验收需要 KANBAN_RELEASE_BASE_URL")
  const transport = createRpcTransport({ apiBaseUrl: "", webBasePath: "/app/", actor: "v4-proof", defaultBoard: "default", serverVersion: "proof", protocolVersion: "v4", webBuildId: "proof" }, { documentBaseURI: `${baseURL}/app/`, fetcher: globalThis.fetch })
  let status = 200
  let payload: unknown
  try { payload = (await transport.call({ method, ...parts })).payload }
  catch (error) {
    if (!(error instanceof RpcTransportError) || error.status === null) throw error
    status = error.status
    payload = { error: error.apiError }
  }
  return {
    ok: () => status >= 200 && status < 300,
    status: () => status,
    json: (): ReturnType<APIResponse["json"]> => Promise.resolve(payload),
    text: async () => JSON.stringify(payload),
  }
}
