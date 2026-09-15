import { Code, ConnectError } from "@connectrpc/connect"
import type { RpcTransport, RpcTransportOptions } from "../../application/data/rpc-transport"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { createRpcClients } from "../../lib/rpc/client"
import { invokeRpc } from "../../lib/rpc/codec.generated"
import { sameOriginRpcBase } from "../../lib/rpc/endpoint"
import { actorHeaders, rpcTransportError } from "../../lib/rpc/errors"

/** 业务 parts 通过字段 codec 进入具名 Connect 方法，返回页面现有 DTO。 */
export function createRpcTransport(runtime: WebRuntimeConfig, options: RpcTransportOptions = {}): RpcTransport {
  const base = sameOriginRpcBase(runtime, options.documentBaseURI)
  return {
    async call(request) {
      if (request.signal?.aborted) throw request.signal.reason ?? new DOMException("请求已取消。", "AbortError")
      let bytes = 0
      // 每次调用独立记录实际 Protobuf frame 长度，避免为计量重编码大附件。
      const client = createRpcClients(base.href, options.fetcher, (length) => { bytes += length }).business
      try {
        const payload = await invokeRpc(client, request, { signal: request.signal, headers: actorHeaders(request.actor) })
        return { payload, bytes }
      } catch (error) {
        if (request.signal?.aborted || (error instanceof ConnectError && error.code === Code.Canceled)) {
          throw request.signal?.reason ?? new DOMException("请求已取消。", "AbortError")
        }
        throw rpcTransportError(error)
      }
    },
  }
}
