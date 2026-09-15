import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { createServer as createHttpServer, request as httpRequest } from "node:http"
import type { IncomingMessage, OutgoingHttpHeaders, Server, ServerResponse } from "node:http"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"

import { createServer, preview } from "vite"
import { afterEach, describe, expect, test, vi } from "vitest"

import { resolveKanbanHostUrl } from "./host-proxy"

const configFile = fileURLToPath(new URL("../vite.config.ts", import.meta.url))
const rpcPaths = [
  "/kanban.v1.QueryService/WatchQueries",
  "/kanban.v1.KanbanService/CreateTask",
]
const strictCsp = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(accept => { resolve = accept })
  return { promise, resolve }
}

function serverOrigin(server: Server): string {
  const address = server.address()
  if (address === null || typeof address === "string") throw new Error("测试 listener 没有 TCP 端口")
  return `http://127.0.0.1:${address.port}`
}

async function closeHttpServer(server: Server): Promise<void> {
  server.closeAllConnections()
  await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
}

async function fixture(
  kind: "dev" | "preview",
  handler: (request: IncomingMessage, response: ServerResponse) => void,
) {
  const upstream = createHttpServer(handler)
  await new Promise<void>(resolve => upstream.listen(0, "127.0.0.1", resolve))
  const root = await mkdtemp(join(tmpdir(), "kanban-host-proxy-"))
  await mkdir(join(root, "dist"))
  await writeFile(join(root, "index.html"), "<!doctype html><main>本地 Vite 页面</main>")
  await writeFile(join(root, "dist/index.html"), "<!doctype html><main>本地 preview 页面</main>")
  await writeFile(join(root, "dist/manifest.json"), JSON.stringify({ owner: "local-preview" }))
  vi.stubEnv("KANBAN_HOST_URL", serverOrigin(upstream))
  const common = { configFile, root, mode: "test", logLevel: "silent" as const }
  try {
    if (kind === "dev") {
      const vite = await createServer({ ...common,
        server: { host: "127.0.0.1", port: 0, strictPort: true, watch: null, hmr: false, ws: false },
        optimizeDeps: { noDiscovery: true, include: [] },
      })
      await vite.listen()
      return { origin: serverOrigin(vite.httpServer!), upstreamOrigin: serverOrigin(upstream),
        close: async () => {
          await vite.close()
          await closeHttpServer(upstream)
          await rm(root, { recursive: true, force: true })
        } }
    }
    const vite = await preview({ ...common, preview: { host: "127.0.0.1", port: 0, strictPort: true } })
    return { origin: serverOrigin(vite.httpServer), upstreamOrigin: serverOrigin(upstream),
      close: async () => {
        await closeHttpServer(vite.httpServer)
        await closeHttpServer(upstream)
        await rm(root, { recursive: true, force: true })
      } }
  } catch (error) {
    await closeHttpServer(upstream)
    await rm(root, { recursive: true, force: true })
    throw error
  }
}

async function readBody(request: IncomingMessage): Promise<Buffer> {
  const chunks: Buffer[] = []
  for await (const chunk of request) chunks.push(Buffer.from(chunk))
  return Buffer.concat(chunks)
}

function send(origin: string, path: string, headers: OutgoingHttpHeaders | readonly string[] = {}, body?: Buffer) {
  return new Promise<{ status: number | undefined; headers: IncomingMessage["headers"]; body: Buffer }>((resolve, reject) => {
    const request = httpRequest(`${origin}${path}`, { method: body === undefined ? "GET" : "POST", headers }, response => {
      void readBody(response).then(bytes => resolve({ status: response.statusCode, headers: response.headers, body: bytes }), reject)
    })
    request.on("error", reject)
    request.end(body)
  })
}

afterEach(() => vi.unstubAllEnvs())

describe("KANBAN_HOST_URL", () => {
  test("默认端口与已支持的 loopback origin", () => {
    expect(resolveKanbanHostUrl()).toBe("http://127.0.0.1:8721")
    for (const origin of ["http://localhost:18721", "http://[::1]:18721", "https://127.0.0.1:18721"]) {
      expect(resolveKanbanHostUrl(origin + "/")).toBe(origin)
    }
  })

  test("拒绝远程、凭据、路径、query、fragment 与无效端口", () => {
    for (const origin of ["", "https://example.com", "http://0.0.0.0:8721", "http://192.168.1.2:8721",
      "http://127.0.0.1.example.com:8721", "http://user:pass@127.0.0.1:8721", "http://127.0.0.1:8721/rpc",
      "http://127.0.0.1:8721?x=1", "http://127.0.0.1:8721#x", "http://127.0.0.1:65536", "http://127.0.0.1:0",
      "http://127.1:8721", " http://127.0.0.1:8721", "http://127.0.0.1:8721\\", "ws://127.0.0.1:8721"]) {
      expect(() => resolveKanbanHostUrl(origin), origin).toThrow(/KANBAN_HOST_URL/)
    }
  })
})

describe.each(["dev", "preview"] as const)("Vite %s 实际同源代理", kind => {
  test("两个正式 RPC service使用同一个 Host，原样转发方法、Content-Type 与字节", async () => {
    const received: Array<{ url?: string; method?: string; headers: IncomingMessage["headers"]; body: Buffer }> = []
    const value = await fixture(kind, (request, response) => {
      void readBody(request).then(body => {
        received.push({ url: request.url, method: request.method, headers: request.headers, body })
        response.writeHead(200, { "Content-Type": "application/grpc-web+proto" }).end(body)
      })
    })
    const payload = Buffer.from([0, 0, 0, 0, 3, 8, 1, 0])
    try {
      for (const path of rpcPaths) {
        const result = await send(value.origin, path, {
          Origin: value.origin, "Content-Type": "application/grpc-web+proto", "X-Grpc-Web": "1",
          "X-Forwarded-Host": "attacker.invalid", Forwarded: "host=attacker.invalid",
          ...(path === rpcPaths[1] ? { Expect: "100-continue" } : {}),
        }, payload)
        expect(result.status).toBe(200)
        expect(result.headers["content-type"]).toBe("application/grpc-web+proto")
        expect(result.body).toEqual(payload)
      }
      expect(received.map(request => request.url)).toEqual(rpcPaths)
      for (const request of received) {
        expect(request.method).toBe("POST")
        expect(request.headers.host).toBe(new URL(value.upstreamOrigin).host)
        expect(request.headers.origin).toBe(value.upstreamOrigin)
        expect(request.headers["content-type"]).toBe("application/grpc-web+proto")
        expect(request.headers["x-grpc-web"]).toBe("1")
        expect(request.headers["x-forwarded-host"]).toBeUndefined()
        expect(request.headers.forwarded).toBeUndefined()
        expect(request.body).toEqual(payload)
      }
    } finally { await value.close() }
  })

  test("metadata 与健康检查共用 Host，本地 /app/ 与 strict preview CSP 保留", async () => {
    const paths: string[] = []
    const origins: Array<string | undefined> = []
    const value = await fixture(kind, (request, response) => {
      paths.push(request.url ?? "")
      origins.push(request.headers.origin)
      response.writeHead(200, { "Content-Type": "application/json" }).end(JSON.stringify({ owner: "host", path: request.url }))
    })
    try {
      for (const path of ["/app/runtime.json", "/app/manifest.json?fresh=1", "/health"]) {
        const result = await send(value.origin, path)
        expect(result.status).toBe(200)
        expect(JSON.parse(result.body.toString())).toEqual({ owner: "host", path })
      }
      expect(paths).toEqual(["/app/runtime.json", "/app/manifest.json?fresh=1", "/health"])
      expect(origins).toEqual([undefined, undefined, undefined])
      const page = await send(value.origin, "/app/")
      expect(page.status).toBe(200)
      expect(page.body.toString()).toContain(kind === "dev" ? "本地 Vite 页面" : "本地 preview 页面")
      for (const retired of ['/api/v1/boards', '/kanban.framework.v1.WorkspaceService/WatchChanges', '/kanban.v1.WorkspaceService/WatchChanges']) {
        await send(value.origin, retired)
      }
      expect(paths).toHaveLength(3)
      if (kind === "preview") expect(page.headers["content-security-policy"]).toBe(strictCsp)
    } finally { await value.close() }
  })

  test("重复或无效 Origin、不同端口 Origin 与非 loopback Host 均在上游之前拒绝", async () => {
    let calls = 0
    const value = await fixture(kind, (_request, response) => { calls += 1; response.end() })
    const host = new URL(value.origin).host
    try {
      const attempts: Array<OutgoingHttpHeaders | readonly string[]> = [
        { Host: host, Origin: "https://attacker.invalid" },
        { Host: host, Origin: value.upstreamOrigin },
        { Host: host, Origin: "null" },
        { Host: host, Origin: value.origin + "/" },
        { Host: host, Origin: value.origin + " invalid" },
        ["Host", host, "Origin", value.origin, "Origin", value.origin],
        ["Host", host, "Host", host, "Origin", value.origin],
        { Host: "192.168.1.2:1421" },
        { Host: "attacker.invalid" },
      ]
      for (const headers of attempts) {
        const result = await send(value.origin, rpcPaths[0], headers, Buffer.from([0]))
        expect(result.status).toBe(403)
      }
      expect(calls).toBe(0)
    } finally { await value.close() }
  })

  test("首个 stream chunk 在上游结束前到达，Abort 会关闭上游 stream", async () => {
    const closed = deferred<void>()
    const first = Buffer.from([0, 0, 0, 0, 2, 8, 1])
    let ended = false
    const value = await fixture(kind, (_request, response) => {
      response.on("close", () => { ended = true; closed.resolve() })
      response.writeHead(200, { "Content-Type": "application/grpc-web+proto" })
      response.write(first)
    })
    const abort = new AbortController()
    try {
      const response = await fetch(`${value.origin}${rpcPaths[0]}`, {
        method: "POST", headers: { Origin: value.origin, "Content-Type": "application/grpc-web+proto" },
        body: new Uint8Array([0, 0, 0, 0, 0]), signal: abort.signal,
      })
      const reader = response.body!.getReader()
      const chunk = await reader.read()
      expect(Buffer.from(chunk.value!)).toEqual(first)
      expect(ended).toBe(false)
      abort.abort()
      await closed.promise
      expect(ended).toBe(true)
      await reader.cancel().catch(() => undefined)
    } finally {
      abort.abort()
      await value.close()
    }
  })

  test("上游重定向不会被代理跟随到另一个 target", async () => {
    let redirectedCalls = 0
    const redirect = createHttpServer((_request, response) => { redirectedCalls += 1; response.end("不应访问") })
    await new Promise<void>(resolve => redirect.listen(0, "127.0.0.1", resolve))
    const value = await fixture(kind, (_request, response) => {
      response.writeHead(307, { Location: `${serverOrigin(redirect)}/remote` }).end()
    })
    try {
      const result = await send(value.origin, rpcPaths[0], { Origin: value.origin }, Buffer.from([0]))
      expect(result.status).toBe(307)
      expect(result.headers.location).toBe(`${serverOrigin(redirect)}/remote`)
      expect(redirectedCalls).toBe(0)
    } finally {
      await value.close()
      await closeHttpServer(redirect)
    }
  })
})
