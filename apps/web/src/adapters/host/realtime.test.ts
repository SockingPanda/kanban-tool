import { createServer, type IncomingMessage, type ServerResponse } from "node:http"
import { afterEach, describe, expect, test, vi } from "vitest"
import { create, fromBinary, toBinary } from "@bufbuild/protobuf"
import { RefreshReason, WatchChangesRequestSchema, WorkspaceChangeFrameSchema } from "../../generated/rpc/kanban/v1/workspace_pb"
import type { BoardRealtimeController } from "../../application/realtime/source"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { createHostRealtime } from "./realtime"

const active: BoardRealtimeController[] = []
const cleanup: Array<() => Promise<void>> = []
afterEach(async () => {
  active.splice(0).forEach(controller => controller.stop())
  for (const close of cleanup.splice(0)) await close()
})

function envelope(body: Uint8Array, flags = 0): Uint8Array {
  const data = new Uint8Array(body.length + 5)
  data[0] = flags
  new DataView(data.buffer).setUint32(1, body.length)
  data.set(body, 5)
  return data
}
function refresh(boardId = "b_one", sequence = 1n, reason = RefreshReason.ATTACHED): Uint8Array {
  return envelope(toBinary(WorkspaceChangeFrameSchema, create(WorkspaceChangeFrameSchema, {
    boardId, epoch: "fixture", sequence, body: { case: "invalidated", value: { reason } },
  })))
}
function trailer(status: number): Uint8Array {
  return envelope(new TextEncoder().encode(`grpc-status: ${status}\r\n`), 128)
}
async function fixture(handle: (request: IncomingMessage, response: ServerResponse, body: Uint8Array) => void) {
  const server = createServer((request, response) => {
    const chunks: Buffer[] = []
    request.on("data", chunk => chunks.push(Buffer.from(chunk)))
    request.on("end", () => handle(request, response, Buffer.concat(chunks)))
  })
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve))
  const address = server.address()
  if (!address || typeof address === "string") throw new Error("缺少临时 Host listener")
  cleanup.push(async () => {
    server.closeAllConnections()
    await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
  })
  const base = `http://127.0.0.1:${address.port}`
  const runtime: WebRuntimeConfig = {
    apiBaseUrl: "", webBasePath: "/app/", actor: "test", defaultBoard: "one", serverVersion: "3.1.0", protocolVersion: "v1", webBuildId: "test",
  }
  return { runtime, documentBaseURI: `${base}/app/` }
}

describe("正式 WorkspaceService binary gRPC-Web", () => {
  test("真实 Fetch 持续接收 attached/write hint，heartbeat 不刷新，stop 关闭 upstream", async () => {
    const requests: Array<{ path: string | undefined; type: string | undefined; method: string | undefined; boardId: string; protocolVersion: number }> = []
    let upstream: ServerResponse | undefined
    let closed = false
    const host = await fixture((request, response, body) => {
      const parsed = fromBinary(WatchChangesRequestSchema, body.subarray(5))
      requests.push({ path: request.url, type: request.headers["content-type"], method: request.method, boardId: parsed.boardId, protocolVersion: parsed.protocolVersion })
      response.writeHead(200, { "content-type": "application/grpc-web+proto" })
      response.on("close", () => { closed = true })
      response.write(refresh())
      response.write(envelope(toBinary(WorkspaceChangeFrameSchema, create(WorkspaceChangeFrameSchema, {
        boardId: "b_one", epoch: "fixture", sequence: 1n, body: { case: "heartbeat", value: {} },
      }))))
      upstream = response
    })
    const onRefresh = vi.fn()
    const controller = createHostRealtime(host.runtime, host).create({ boardId: "b_one", boardSelector: "one", onRefresh, onState: vi.fn() })
    active.push(controller)
    controller.start()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("live"))
    expect(onRefresh).toHaveBeenCalledOnce()
    expect(closed).toBe(false)
    expect(requests).toEqual([{ path: "/kanban.v1.WorkspaceService/WatchChanges", type: "application/grpc-web+proto", method: "POST", boardId: "b_one", protocolVersion: 1 }])
    upstream?.write(refresh("b_one", 2n, RefreshReason.WRITE_HINT))
    await vi.waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(2))
    controller.stop()
    await vi.waitFor(() => expect(closed).toBe(true))
    expect(controller.snapshot().state).toBe("stopped")
  })

  test("失败只重试正式 RPC，预算耗尽后手动 retry 可以恢复", async () => {
    const paths: string[] = []
    let unavailable = true
    const host = await fixture((request, response) => {
      paths.push(request.url ?? "")
      response.writeHead(200, { "content-type": "application/grpc-web+proto" })
      if (unavailable) response.end(trailer(14))
      else response.write(refresh())
    })
    const onRefresh = vi.fn()
    const controller = createHostRealtime(host.runtime, { ...host, maxFailures: 3, delay: async () => undefined }).create({
      boardId: "b_one", boardSelector: "one", onRefresh, onState: vi.fn(),
    })
    active.push(controller)
    controller.start()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("failed"))
    expect(paths).toHaveLength(3)
    expect(onRefresh).not.toHaveBeenCalled()
    unavailable = false
    controller.retry()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("live"))
    expect(paths).toHaveLength(4)
    expect(paths.every(path => path === "/kanban.v1.WorkspaceService/WatchChanges")).toBe(true)
    expect(onRefresh).toHaveBeenCalledOnce()
  })

  test.each(["foreign-board", "missing-body", "unimplemented"])("%s 不触发刷新或 HTTP/SSE fallback", async fault => {
    const paths: string[] = []
    const host = await fixture((request, response) => {
      paths.push(request.url ?? "")
      response.writeHead(200, { "content-type": "application/grpc-web+proto" })
      if (fault === "unimplemented") response.end(trailer(12))
      else {
        response.write(fault === "foreign-board" ? refresh("b_other") : envelope(toBinary(WorkspaceChangeFrameSchema, create(WorkspaceChangeFrameSchema, { boardId: "b_one", epoch: "fixture", sequence: 1n }))))
        response.end(trailer(0))
      }
    })
    const onRefresh = vi.fn()
    const controller = createHostRealtime(host.runtime, { ...host, maxFailures: fault === "unimplemented" ? 8 : 1 }).create({
      boardId: "b_one", boardSelector: "one", onRefresh, onState: vi.fn(),
    })
    active.push(controller)
    controller.start()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("failed"))
    expect(onRefresh).not.toHaveBeenCalled()
    expect(paths).toEqual(["/kanban.v1.WorkspaceService/WatchChanges"])
  })
})
