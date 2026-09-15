import { createClient, ConnectError, Code } from "@connectrpc/connect"
import { createGrpcWebTransport } from "@connectrpc/connect-web"
import { WorkspaceService, RefreshReason } from "./gen/kanban/framework/v1/board_pb.js"
import { createChangeRealtime, ChangeProtocolError, type ChangeOptions, type ChangeFrame } from "./changes.js"
import { atlasRpcEndpoint } from "./endpoint.js"

/** 结构上满足 Atlas BoardRealtimeSource；application 不依赖此 client 或生成类型。 */
export function createAtlasRpcRealtime(input: string, documentUrl: string, options: ChangeOptions = {}, fetcher: typeof fetch = fetch) {
  const baseUrl = atlasRpcEndpoint(input, documentUrl)
  const transport = createGrpcWebTransport({ baseUrl, useBinaryFormat: true,
    fetch: (request, init) => {
      const requestUrl = typeof request === "string" ? request : request instanceof URL ? request.href : request.url
      if (new URL(requestUrl, baseUrl).origin !== new URL(baseUrl).origin) throw new Error("拒绝跨源 RPC 请求")
      return fetcher(request, { ...init, mode: "same-origin", credentials: "same-origin", redirect: "error", cache: "no-store" })
    } })
  const client = createClient(WorkspaceService, transport)
  return createChangeRealtime(`grpc-web:${baseUrl}:workspace-refresh:v1`, async function* (boardId, signal) {
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
  }, { ...options, terminal: (error) => options.terminal?.(error) === true || error instanceof ConnectError &&
    [Code.PermissionDenied, Code.Unauthenticated, Code.NotFound, Code.InvalidArgument, Code.FailedPrecondition, Code.Unimplemented].includes(error.code) })
}
