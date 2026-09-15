import { Code, ConnectError } from "@connectrpc/connect"
import { RefreshReason } from "../../generated/rpc/kanban/v1/workspace_pb"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { createRpcClients } from "../../lib/rpc/client"
import { sameOriginRpcBase } from "../../lib/rpc/endpoint"
import type { RpcTransportOptions } from "../../application/data/rpc-transport"
import { ChangeProtocolError, createChangeRealtime, type ChangeOptions, type ChangeFrame } from "../../application/realtime/changes"

/** 正式 WorkspaceService 只使读取失效；连接序号不参与审计事件续读。 */
export function createHostRealtime(runtime: WebRuntimeConfig, options: RpcTransportOptions & ChangeOptions = {}) {
  const baseUrl = sameOriginRpcBase(runtime, options.documentBaseURI).href
  const client = createRpcClients(baseUrl, options.fetcher).workspace
  return createChangeRealtime(`grpc-web:${baseUrl}:kanban.v1.WorkspaceService:1`, async function* (boardId, signal) {
    for await (const value of client.watchChanges({ boardId, protocolVersion: 1 }, { signal, timeoutMs: 0 })) {
      const head = { boardId: value.boardId, epoch: value.epoch, sequence: value.sequence }
      let frame: ChangeFrame
      if (value.body.case === "heartbeat") frame = { ...head, kind: "heartbeat" }
      else if (value.body.case === "invalidated") {
        const reason = value.body.value.reason
        if (reason !== RefreshReason.ATTACHED && reason !== RefreshReason.WRITE_HINT) throw new ChangeProtocolError("未知刷新原因")
        frame = { ...head, kind: "refresh", reason: reason === RefreshReason.ATTACHED ? "attached" : "write_hint" }
      } else throw new ChangeProtocolError("缺少 WorkspaceChangeFrame body")
      yield frame
    }
  }, { ...options, terminal: error => options.terminal?.(error) === true || error instanceof ConnectError &&
    [Code.PermissionDenied, Code.Unauthenticated, Code.NotFound, Code.InvalidArgument, Code.FailedPrecondition, Code.Unimplemented].includes(error.code) })
}
