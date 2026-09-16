import { createHash } from "node:crypto"
import { Code, ConnectError } from "@connectrpc/connect"
import { setTrailerStatus, trailerSerialize } from "@connectrpc/connect/protocol-grpc-web"
import { expect, test, vi } from "vitest"
import { createRpcTransport } from "../../adapters/host/rpc-transport"
import { MAX_RPC_MESSAGE_BYTES, RpcTransportError } from "../../application/data/rpc-transport"
import { DtoApiErrorCode } from "../../generated/rpc/kanban/v1/dto_pb"
import { ErrorDetailSchema } from "../../generated/rpc/kanban/v1/kanban_pb"
import type { WebRuntimeConfig } from "../runtime"
import { decodeRpcRequest, encodeRpcResponse } from "./codec.generated"
import { sameOriginRpcBase } from "./endpoint"
import { actorHeaders, rpcTransportError } from "./errors"

const runtime: WebRuntimeConfig = { apiBaseUrl: "", webBasePath: "/app/", actor: "local", defaultBoard: "default", serverVersion: "test", protocolVersion: "v2", webBuildId: "test" }
const baseURI = "http://127.0.0.1:8721/app/boards/default"

function frame(flag: number, data: Uint8Array): Uint8Array {
  const result = new Uint8Array(5 + data.length)
  result[0] = flag
  new DataView(result.buffer).setUint32(1, data.length)
  result.set(data, 5)
  return result
}
function response(url: string, content: Uint8Array | null, error?: ConnectError): Response {
  const chunks = content === null ? [] : [frame(0, content)]
  chunks.push(frame(128, trailerSerialize(setTrailerStatus(new Headers(), error))))
  const body = new ReadableStream({ start(controller) { for (const chunk of chunks) { controller.enqueue(chunk.subarray(0, 3)); controller.enqueue(chunk.subarray(3, 7)); controller.enqueue(chunk.subarray(7)) } controller.close() } })
  const result = new Response(body, { headers: { "content-type": "application/grpc-web+proto" } })
  Object.defineProperty(result, "url", { value: url })
  return result
}
function url(input: RequestInfo | URL): string { return typeof input === "string" ? input : input instanceof URL ? input.href : input.url }

test("正式 Connect unary 发送 binary、中文 actor，并按实际消息帧计量", async () => {
  const bytes = encodeRpcResponse("ListBoards", { data: [] })
  const fetcher = vi.fn<typeof fetch>(async (input, init) => {
    expect(url(input)).toBe("http://127.0.0.1:8721/kanban.v1.KanbanService/ListBoards")
    expect(init).toMatchObject({ method: "POST", mode: "same-origin", credentials: "same-origin", redirect: "error" })
    expect(init?.body).toBeInstanceOf(Uint8Array)
    const body = init?.body as Uint8Array
    expect(decodeRpcRequest("ListBoards", body.subarray(5))).toEqual({ query: { include_archived: false } })
    const actor = new Headers(init?.headers).get("x-kb-actor-bin")!
    expect(new TextDecoder().decode(Uint8Array.from(atob(actor), (char) => char.charCodeAt(0)))).toBe("中文执行者 🦓")
    return response(url(input), bytes)
  })
  const transport = createRpcTransport(runtime, { fetcher, documentBaseURI: baseURI })
  expect(await transport.call({ method: "ListBoards", query: { include_archived: false }, actor: "中文执行者 🦓" })).toEqual({ payload: { data: [] }, bytes: bytes.length })
  expect(fetcher).toHaveBeenCalledTimes(1)
})

test("标准 google.rpc.Status/Any 经实际 Connect Fetch 保留业务码与 UI status", async () => {
  const error = new ConnectError("claim 不匹配", Code.Aborted, undefined, [{ desc: ErrorDetailSchema, value: { code: DtoApiErrorCode.CLAIM_CONFLICT, message: "claim 不匹配" } }])
  const fetcher: typeof fetch = async (input) => response(url(input), null, error)
  const transport = createRpcTransport(runtime, { fetcher, documentBaseURI: baseURI })
  await expect(transport.call({ method: "ListBoards" })).rejects.toMatchObject({ kind: "http", status: 409, apiError: { code: "claim_conflict", message: "claim 不匹配" } })
  const mismatch = new ConnectError("wrong", Code.NotFound, undefined, [{ desc: ErrorDetailSchema, value: { code: DtoApiErrorCode.CLAIM_CONFLICT, message: "wrong" } }])
  expect(rpcTransportError(mismatch)).toMatchObject({ kind: "invalid_bytes", apiError: null })
})

test("全部 14 个稳定业务错误保持原 UI 分类和标准 detail", () => {
  const mappings: [DtoApiErrorCode, Code, number][] = [
    [DtoApiErrorCode.NOT_FOUND, Code.NotFound, 404], [DtoApiErrorCode.CONFLICT, Code.Aborted, 409],
    [DtoApiErrorCode.IDEMPOTENCY_CONFLICT, Code.Aborted, 409], [DtoApiErrorCode.DEPENDENCY_CYCLE, Code.InvalidArgument, 409],
    [DtoApiErrorCode.INVALID_INPUT, Code.InvalidArgument, 400], [DtoApiErrorCode.FEATURE_NOT_AVAILABLE, Code.Unimplemented, 501],
    [DtoApiErrorCode.SERVER_UNAVAILABLE, Code.Unavailable, 503], [DtoApiErrorCode.EXECUTION_PLAN_REQUIRED, Code.FailedPrecondition, 409],
    [DtoApiErrorCode.STEPS_INCOMPLETE, Code.FailedPrecondition, 409], [DtoApiErrorCode.CLAIM_TOKEN_MISMATCH, Code.FailedPrecondition, 403],
    [DtoApiErrorCode.DEPENDENCY_BLOCKED, Code.FailedPrecondition, 409], [DtoApiErrorCode.CLAIM_CONFLICT, Code.Aborted, 409],
    [DtoApiErrorCode.INVALID_TRANSITION, Code.FailedPrecondition, 409], [DtoApiErrorCode.INTERNAL, Code.Internal, 500],
  ]
  for (const [code, status, http] of mappings) {
    const error = new ConnectError("说明", status, undefined, [{ desc: ErrorDetailSchema, value: { code, message: "说明" } }])
    expect(rpcTransportError(error)).toMatchObject({ kind: "http", status: http, apiError: { message: "说明" } })
  }
})

test("UTF-8 actor metadata 不丢失字符，并拒绝空值、过长和非法 surrogate", () => {
  expect(actorHeaders(undefined).has("x-kb-actor-bin")).toBe(false)
  for (const actor of [" ", "中".repeat(3000), "\ud800"]) expect(() => actorHeaders(actor)).toThrow(RpcTransportError)
})

test.each(["https://other.invalid/", "//other.invalid/", "/../", "/%2e%2e/", "/%252e%252e/", "/a%2fb/", "/a\\b/", "http://user@127.0.0.1:8721/", "/?token=x", "/#fragment"]) ("runtime 拒绝越过同源或路径边界：%s", (apiBaseUrl) => {
  expect(() => sameOriginRpcBase({ ...runtime, apiBaseUrl }, baseURI)).toThrow(RpcTransportError)
})

test("相对 runtime base 保留前缀；损坏 URL 报明确错误", () => {
  expect(sameOriginRpcBase({ ...runtime, apiBaseUrl: "/rpc" }, baseURI).href).toBe("http://127.0.0.1:8721/rpc/")
  expect(() => sameOriginRpcBase(runtime, "not-a-url")).toThrow(RpcTransportError)
})

test("网络错误保留 offline，取消不变成业务失败", async () => {
  const fetcher = vi.fn<typeof fetch>(async () => { throw new TypeError("failed to fetch") })
  const transport = createRpcTransport(runtime, { fetcher, documentBaseURI: baseURI })
  await expect(transport.call({ method: "ListBoards" })).rejects.toMatchObject({ kind: "offline" })
  const controller = new AbortController()
  controller.abort()
  await expect(transport.call({ method: "ListBoards", signal: controller.signal })).rejects.toMatchObject({ name: "AbortError" })
  expect(fetcher).toHaveBeenCalledTimes(1)
})

test("响应 URL、Content-Type 和 HTTP 失败保持稳定分类", async () => {
  for (const [kind, status, contentType, finalURL] of [
    ["cross_origin", 200, "application/grpc-web+proto", "http://other/"],
    ["invalid_content_type", 200, "text/html", undefined],
    ["http", 503, "text/plain", undefined],
  ] as const) {
    const fetcher: typeof fetch = async (input) => {
      const result = new Response("error", { status, headers: { "content-type": contentType } })
      Object.defineProperty(result, "url", { value: finalURL ?? url(input) })
      return result
    }
    await expect(createRpcTransport(runtime, { fetcher, documentBaseURI: baseURI }).call({ method: "ListBoards" })).rejects.toMatchObject({ kind })
  }
})

test("超限消息从 5 字节 frame header 拒绝，截断帧保留 invalid_bytes", async () => {
  for (const [length, kind] of [[MAX_RPC_MESSAGE_BYTES + 1, "response_too_large"], [10, "invalid_bytes"]] as const) {
    const header = new Uint8Array(5)
    new DataView(header.buffer).setUint32(1, length)
    const fetcher: typeof fetch = async (input) => {
      const result = new Response(header, { headers: { "content-type": "application/grpc-web+proto" } })
      Object.defineProperty(result, "url", { value: url(input) })
      return result
    }
    await expect(createRpcTransport(runtime, { fetcher, documentBaseURI: baseURI }).call({ method: "ListBoards" })).rejects.toMatchObject({ kind })
  }
})

test("多 MiB 附件通过 binary Fetch 返回完整 metadata 和内容", async () => {
  const content = new Uint8Array(5 * 1024 * 1024).fill(127)
  const attachment = { id: "a_1", board_id: "b_1", task_id: "t_1", filename: "文件.bin", rel_path: "a_1", content_type: null, size_bytes: content.length, sha256: null, created_by: "测试", created_at: 0 }
  const encoded = encodeRpcResponse("DownloadAttachment", { attachment, content })
  const fetcher: typeof fetch = async (input) => response(url(input), encoded)
  const result = await createRpcTransport(runtime, { fetcher, documentBaseURI: baseURI }).call({ method: "DownloadAttachment", path: { task_id: "t_1", attachment_id: "a_1" } })
  expect(result.bytes).toBe(encoded.length)
  const payload = result.payload as { attachment: unknown; content: Uint8Array }
  expect(payload.attachment).toEqual(attachment)
  expect(payload.content.byteLength).toBe(content.byteLength)
  expect(createHash("sha256").update(payload.content).digest("hex")).toBe(createHash("sha256").update(content).digest("hex"))
})
