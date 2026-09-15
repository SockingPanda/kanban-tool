import { describe, expect, test, vi } from "vitest"
import type { RpcCall, RpcMethod } from "../../application/data/rpc-transport"
import { defaultTaskListQuery } from "../../application/data/explorer-read-model"
import { decodeRpcRequest, encodeRpcResponse } from "../../lib/rpc/codec.generated"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { createHostDataSource } from "./data-source"

const runtime: WebRuntimeConfig = {
  apiBaseUrl: "", webBasePath: "/app/", actor: "test", defaultBoard: "default", serverVersion: "3.1.0", protocolVersion: "v1", webBuildId: "test",
}
const documentBaseURI = "http://127.0.0.1:1421/app/"
const board = { id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 2, archived_at: null }

function response(url: string, body: Uint8Array): Response {
  const trailer = new TextEncoder().encode("grpc-status: 0\r\n")
  const bytes = new Uint8Array(10 + body.length + trailer.length)
  new DataView(bytes.buffer).setUint32(1, body.length)
  bytes.set(body, 5)
  bytes[5 + body.length] = 128
  new DataView(bytes.buffer).setUint32(6 + body.length, trailer.length)
  bytes.set(trailer, 10 + body.length)
  const result = new Response(bytes, { headers: { "content-type": "application/grpc-web+proto" } })
  Object.defineProperty(result, "url", { value: url })
  return result
}

describe("生产 Host 数据源", () => {
  test("所有已挂载读取共享 binary RPC transport，保留分页、筛选和现有 DTO", async () => {
    const replies: Partial<Record<RpcMethod, unknown>> = {
      ListBoards: { data: [board] },
      ListBoardColumns: { data: [] },
      ListTasks: { data: [], meta: { limit: 25, offset: 25, total: 0 } },
      ListRuns: { data: [] },
      ListEvents: { data: [], meta: { next_after: 0 } },
      BoardTaskMap: { data: { nodes: [], edges: [], meta: {
        depth: 0, context_depth: 1, generated_at: 1, node_count: 0, edge_count: 0, truncated: false,
        active_statuses: ["ready"], active_only: true, include_done_context: true,
        include_archived_context: false, hide_isolated: false, limit_nodes: 240,
      } } },
    }
    const calls: RpcCall[] = []
    const fetcher = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input)
      const method = url.slice(url.lastIndexOf("/") + 1) as RpcMethod
      expect(url).toBe(`http://127.0.0.1:1421/kanban.v1.KanbanService/${method}`)
      expect(init).toMatchObject({ method: "POST", mode: "same-origin", credentials: "same-origin", redirect: "error", cache: "no-store" })
      expect(new Headers(init?.headers).get("content-type")).toBe("application/grpc-web+proto")
      if (!(init?.body instanceof Uint8Array) || replies[method] === undefined) throw new Error(`未预期的 RPC ${method}`)
      calls.push({ method, ...decodeRpcRequest(method, init.body.subarray(5)) })
      return response(url, encodeRpcResponse(method, replies[method]))
    })
    const source = createHostDataSource(runtime, { fetcher, documentBaseURI })
    expect(await source.readBoardDirectory()).toEqual([{ id: "b_default", slug: "default", name: "Default", archived: false }])
    expect(await source.loadBoardReadModel(runtime, "default", { includeTasks: false })).toMatchObject({ identity: { canonicalBoardId: "b_default" }, columns: [] })
    expect(await source.loadTaskListPage(runtime, "default", { ...defaultTaskListQuery, status: ["ready"], search: "保留查询", page: 2, limit: 25 })).toMatchObject({ meta: { limit: 25, offset: 25, total: 0 } })
    expect(await source.loadTaskMap(runtime, "default")).toMatchObject({ map: { nodes: [] } })
    expect(await source.loadTaskRuns(runtime, "t_one")).toMatchObject({ runs: [] })
    expect(await source.loadBoardEvents(runtime, "default")).toMatchObject({ events: [] })
    expect(calls.find(call => call.method === "ListTasks")).toMatchObject({ path: { board: "default" }, query: { status: ["ready"], q: "保留查询", limit: 25, offset: 25, sort: "updated_at" } })
    expect(calls[0]).toMatchObject({ method: "ListBoards", query: { include_archived: true } })
    expect(source.streamTransport).toBeUndefined()
    expect(source.streamUrl).toBeUndefined()
  })

  test("读 RPC 失败保持错误，用户重试仍走同一 named RPC", async () => {
    let offline = true
    const fetcher = vi.fn<typeof fetch>(async input => {
      if (offline) throw new Error("离线")
      return response(String(input), encodeRpcResponse("ListBoards", { data: [board] }))
    })
    const source = createHostDataSource(runtime, { fetcher, documentBaseURI })
    await expect(source.loadExplorerBoardIdentity(runtime, "default")).rejects.toMatchObject({ name: "ExplorerReadError", kind: "offline" })
    offline = false
    await expect(source.loadExplorerBoardIdentity(runtime, "default")).resolves.toMatchObject({ id: "b_default" })
    expect(fetcher.mock.calls.map(([input]) => String(input))).toEqual(Array(2).fill("http://127.0.0.1:1421/kanban.v1.KanbanService/ListBoards"))
  })
})
