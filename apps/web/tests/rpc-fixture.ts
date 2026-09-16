import { createHash } from "node:crypto"

import { create, fromBinary, toBinary, type DescMessage } from "@bufbuild/protobuf"
import { Code, ConnectError } from "@connectrpc/connect"
import { setTrailerStatus, trailerSerialize } from "@connectrpc/connect/protocol-grpc-web"
import type { Page } from "@playwright/test"

import type { RpcMethod } from "../src/application/data/rpc-transport"
import { DtoApiErrorCode } from "../src/generated/rpc/kanban/v1/dto_pb"
import { ErrorDetailSchema } from "../src/generated/rpc/kanban/v1/kanban_pb"
import { QueryCursorSchema, QueryDefinitionSchema, QueryFrameSchema, QueryResultSchema, WatchQueriesRequestSchema, type QueryCursor, type QueryDefinition, type QueryFrame } from "../src/generated/rpc/kanban/v1/query_pb"
import { decodeRpcRequest, encodeRpcResponse } from "../src/lib/rpc/codec.generated"
import { grpcWebFrame, readyFrame, resultFrames } from "../src/lib/rpc/query-test-support"

export interface FixtureCall {
  readonly method: RpcMethod | "RecentEvents"
  readonly path: Record<string, unknown>
  readonly query: Record<string, unknown>
  readonly input: Record<string, unknown>
  readonly headers: Record<string, string>
  readonly kind: "query" | "unary"
  readonly refresh: boolean
}

type Handler = (call: FixtureCall) => unknown | Promise<unknown>
type Sample = { encoded: Uint8Array; cursor: QueryCursor }
type Subscription = { definition: QueryDefinition; call: FixtureCall; sample?: Sample }
type Connection = { id: string; subscriptions: Subscription[] }

export function unavailable(message = "暂时不可用"): ConnectError {
  return rpcFailure(Code.Unavailable, DtoApiErrorCode.SERVER_UNAVAILABLE, message)
}

export function rpcFailure(code: Code, detail: DtoApiErrorCode, message: string): ConnectError {
  return new ConnectError(message, code, undefined, [{ desc: ErrorDetailSchema, value: { code: detail, message } }])
}

/** 故意损坏的正式结果，仅用于验证浏览器拒绝无效 protobuf/DTO。 */
export class InvalidQueryResult {
  constructor(readonly encoded: Uint8Array = toBinary(QueryResultSchema, create(QueryResultSchema))) {}
}

function messageField(schema: DescMessage, name: string): DescMessage {
  const field = schema.fields.find(field => field.localName === name)
  if (field?.fieldKind !== "message") throw new Error(`缺少正式 message：${schema.typeName}.${name}`)
  return field.message
}

function queryCall(definition: QueryDefinition): FixtureCall {
  const name = definition.query.case
  if (!name) throw new Error("查询缺少 oneof")
  const method = name[0]!.toUpperCase() + name.slice(1) as FixtureCall["method"]
  if (name === "recentEvents") {
    const value = definition.query.value
    return { method, path: {}, input: {}, query: { board: value.boardId, task_id: value.taskId, limit: value.limit }, headers: {}, kind: "query", refresh: definition.refresh }
  }
  const parts = decodeRpcRequest(method as RpcMethod, toBinary(messageField(QueryDefinitionSchema, name), definition.query.value))
  return { method, path: { ...parts.path }, query: { ...parts.query }, input: {}, headers: {}, kind: "query", refresh: definition.refresh }
}

function queryBytes(definition: QueryDefinition, payload: unknown): Uint8Array {
  if (payload instanceof InvalidQueryResult) return payload.encoded
  const name = definition.query.case!
  const method = name === "recentEvents" ? "ListEvents" : name[0]!.toUpperCase() + name.slice(1) as RpcMethod
  const value = fromBinary(messageField(QueryResultSchema, name), encodeRpcResponse(method, payload))
  // 此边界由生成 descriptor 约束，业务 payload 仍使用生产的显式字段 codec。
  const result = create(QueryResultSchema)
  result.result = { case: name, value } as typeof result.result
  return toBinary(QueryResultSchema, result)
}

function identity(definition: QueryDefinition): string {
  const copy = create(QueryDefinitionSchema, { ...definition, clientQueryId: "", resume: undefined, refresh: false })
  return createHash("sha256").update(toBinary(QueryDefinitionSchema, copy)).digest("hex")
}

function sameCursor(left: QueryCursor | undefined, right: QueryCursor): boolean {
  return left?.epoch === right.epoch && left.scope === right.scope && left.revision === right.revision
}

function unframe(bytes: Uint8Array): Uint8Array {
  if (bytes.length < 5 || bytes[0] !== 0 || new DataView(bytes.buffer, bytes.byteOffset).getUint32(1) !== bytes.length - 5) throw new Error("fixture 只接受正式 binary gRPC-Web 单请求帧")
  return bytes.subarray(5)
}

export function unaryResponse(method: RpcMethod, payload: unknown): Buffer {
  const data = grpcWebFrame(encodeRpcResponse(method, payload))
  return Buffer.concat([data, trailer()])
}

export function trailer(error?: ConnectError): Buffer {
  const bytes = grpcWebFrame(trailerSerialize(setTrailerStatus(new Headers(), error)))
  bytes[0] = 128
  return Buffer.from(bytes)
}

/** 仅替换 Preview 的网络边界：生产 Connect、QueryRegistry、projection 和 React 保持原路径。 */
export class RpcFixture {
  readonly calls: FixtureCall[] = []
  readonly subscriptions: QueryDefinition[][] = []
  readonly failures: unknown[] = []
  private handlers: { method: FixtureCall["method"] | "*"; handler: Handler }[] = []
  private connections = new Map<string, Connection>()
  private samples = new Map<string, Sample>()
  constructor(private readonly page: Page) {}

  handle(method: FixtureCall["method"] | "*", handler: Handler): void { this.handlers.push({ method, handler }) }

  private async read(call: FixtureCall): Promise<unknown> {
    this.calls.push(call)
    for (const { method, handler } of [...this.handlers].reverse()) {
      if (method !== "*" && method !== call.method) continue
      const payload = await handler(call)
      if (payload !== undefined) return payload
    }
    throw rpcFailure(Code.NotFound, DtoApiErrorCode.NOT_FOUND, `fixture 未实现 ${call.method}`)
  }

  async install(): Promise<void> {
    await this.page.route("**/kanban.v1.KanbanService/*", async route => {
      const method = new URL(route.request().url()).pathname.split("/").at(-1) as RpcMethod
      try {
        const parts = decodeRpcRequest(method, unframe(route.request().postDataBuffer()!))
        const call: FixtureCall = { method, path: { ...parts.path }, query: { ...parts.query }, input: { ...parts.input }, headers: route.request().headers(), kind: "unary", refresh: false }
        const payload = await this.read(call)
        await route.fulfill({ contentType: "application/grpc-web+proto", body: unaryResponse(method, payload) })
      } catch (error) {
        if (!(error instanceof ConnectError)) this.failures.push(error)
        await route.fulfill({ contentType: "application/grpc-web+proto", body: trailer(error instanceof ConnectError ? error : unavailable(String(error))) })
      }
    })
    await this.page.exposeBinding("__kanbanQueryOpen", async (_source, id: string, bytes: number[]) => {
      const request = fromBinary(WatchQueriesRequestSchema, unframe(Uint8Array.from(bytes)))
      if (request.protocolVersion !== 1 || request.queries.length > 64) throw new Error("不支持的查询请求")
      const connection: Connection = { id, subscriptions: request.queries.map(definition => ({ definition, call: queryCall(definition) })) }
      this.connections.set(id, connection)
      this.subscriptions.push(request.queries)
      await Promise.all(connection.subscriptions.map(subscription => this.send(connection, subscription, false)))
    })
    await this.page.exposeBinding("__kanbanQueryCancel", (_source, id: string) => { this.connections.delete(id) })
    await this.page.addInitScript(() => {
      type Bridge = { __kanbanQueryOpen(id: string, bytes: number[]): Promise<void>; __kanbanQueryCancel(id: string): Promise<void> }
      const bridge = window as unknown as Bridge
      const streams = new Map<string, { serial: number; controller: ReadableStreamDefaultController<Uint8Array>; close: (error?: boolean) => void }>()
      const documentId = crypto.randomUUID()
      let count = 0
      const nativeFetch = window.fetch.bind(window)
      Object.defineProperty(window, "__kanbanQueryActive", { get: () => Array.from(streams.keys()) })
      Object.defineProperty(window, "__kanbanQueryCount", { get: () => count })
      Object.defineProperty(window, "__kanbanQueryPush", { value: (id: string, bytes: number[]) => streams.get(id)?.controller.enqueue(Uint8Array.from(bytes)) })
      Object.defineProperty(window, "__kanbanQueryClose", { value: (id?: number, error = true) => { for (const stream of streams.values()) if (id === undefined || stream.serial === id) stream.close(error) } })
      window.fetch = async (input, init) => {
        const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url, location.href)
        if (!url.pathname.endsWith("/kanban.v1.QueryService/WatchQueries")) return nativeFetch(input, init)
        if (new Headers(init?.headers).get("content-type") !== "application/grpc-web+proto") throw new Error("fixture 拒绝非 binary 查询")
        const serial = ++count
        const id = documentId + ":" + serial
        const signal = init?.signal
        if (signal?.aborted) throw signal.reason
        const cancel = (error = true) => {
          const stream = streams.get(id)
          if (!stream) return
          streams.delete(id)
          signal?.removeEventListener("abort", aborted)
          if (error) stream.controller.error(new DOMException("fixture query disconnected", "AbortError"))
          else stream.controller.close()
          void bridge.__kanbanQueryCancel(id)
        }
        const aborted = () => cancel()
        const stream = new ReadableStream<Uint8Array>({
          start(controller) { streams.set(id, { serial, controller, close: cancel }) },
          cancel() { cancel() },
        })
        signal?.addEventListener("abort", aborted, { once: true })
        const bytes = init?.body
        if (!(bytes instanceof Uint8Array)) throw new Error("fixture 请求必须是 protobuf bytes")
        void bridge.__kanbanQueryOpen(id, Array.from(bytes)).catch(() => cancel())
        const response = new Response(stream, { headers: { "content-type": "application/grpc-web+proto" } })
        Object.defineProperty(response, "url", { value: url.href })
        return response
      }
    })
  }

  private async push(connection: Connection, frames: QueryFrame[]): Promise<void> {
    if (this.connections.get(connection.id) !== connection || this.page.isClosed()) return
    const bytes = Array.from(Buffer.concat(frames.map(frame => grpcWebFrame(toBinary(QueryFrameSchema, frame)))))
    try {
      await this.page.evaluate(({ id, bytes }) => (window as unknown as { __kanbanQueryPush(id: string, bytes: number[]): void }).__kanbanQueryPush?.(id, bytes), { id: connection.id, bytes })
    } catch { /* 页面离开时旧 document 的结果不再交付。 */ }
  }

  private async send(connection: Connection, subscription: Subscription, publish: boolean): Promise<void> {
    const { definition, call } = subscription
    const key = identity(definition)
    try {
      let sample = this.samples.get(key)
      if (!sample || !definition.resume || definition.refresh || publish) {
        const encoded = queryBytes(definition, await this.read(call))
        if (!sample || !Buffer.from(sample.encoded).equals(encoded)) {
          sample = { encoded, cursor: create(QueryCursorSchema, { epoch: "fixture", scope: key, revision: (sample?.cursor.revision ?? 0n) + 1n }) }
          this.samples.set(key, sample)
        }
      }
      const previous = subscription.sample
      const unchanged = sameCursor(previous?.cursor ?? definition.resume, sample.cursor)
      const frames = unchanged ? [] : await resultFrames(definition.clientQueryId, sample.encoded, sample.cursor, previous)
      if (!publish) frames.push(readyFrame(definition.clientQueryId, sample.cursor))
      await this.push(connection, frames)
      subscription.sample = sample
    } catch (error) {
      if (!(error instanceof ConnectError)) this.failures.push(error)
      const failure = error instanceof ConnectError ? error : unavailable(String(error))
      const detail = failure.findDetails(ErrorDetailSchema)[0] ?? create(ErrorDetailSchema, { code: DtoApiErrorCode.INTERNAL, message: "fixture 编码失败" })
      await this.push(connection, [create(QueryFrameSchema, { clientQueryId: definition.clientQueryId, body: { case: "failure", value: { error: detail, retryable: false } } })])
    }
  }

  async publish(): Promise<void> {
    await Promise.all([...this.connections.values()].flatMap(connection => connection.subscriptions.map(subscription => this.send(connection, subscription, true))))
  }
  async heartbeat(): Promise<void> { await Promise.all([...this.connections.values()].map(connection => this.push(connection, [create(QueryFrameSchema, { body: { case: "heartbeat", value: {} } })]))) }
  connectionState() { return [...this.connections.values()].map(item => ({ id: item.id, methods: item.subscriptions.map(sub => sub.call.method) })) }
  activeConnectionCount(): number { return this.connections.size }
  async connectionCount(): Promise<number> { return this.page.evaluate(() => (window as unknown as { __kanbanQueryCount?: number }).__kanbanQueryCount ?? 0) }
  async waitForConnection(after: number): Promise<void> { await this.page.waitForFunction(count => ((window as unknown as { __kanbanQueryCount?: number }).__kanbanQueryCount ?? 0) > count, after) }
  async close(id?: number, error = true): Promise<void> { await this.page.evaluate(({ id, error }) => (window as unknown as { __kanbanQueryClose(id?: number, error?: boolean): void }).__kanbanQueryClose(id, error), { id, error }) }
}

const installed = new WeakMap<Page, RpcFixture>()
export async function installRpcFixture(page: Page): Promise<RpcFixture> {
  const existing = installed.get(page)
  if (existing) return existing
  const fixture = new RpcFixture(page)
  installed.set(page, fixture)
  await fixture.install()
  return fixture
}
