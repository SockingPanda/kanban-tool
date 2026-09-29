import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { createServer as createHttpServer, request as httpRequest } from "node:http"
import type { IncomingMessage, OutgoingHttpHeaders, Server, ServerResponse } from "node:http"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"

import { createServer, preview } from "vite"
import { afterAll, afterEach, beforeAll, describe, expect, test, vi } from "vitest"

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

// 复用 listener，但每例都等待连接和异步 handler 结束；不能只替换 handler 指针。
function requestDrainer(server: Server) {
  const sockets = new Set<import("node:net").Socket>()
  server.on("connection", socket => {
    sockets.add(socket)
    socket.once("close", () => sockets.delete(socket))
  })
  return async () => {
    await Promise.all([...sockets].map(socket => new Promise<void>(resolve => {
      socket.once("close", () => resolve())
      socket.destroy()
    })))
  }
}

async function fixture(
  kind: "dev" | "preview",
) {
  type Handler = (request: IncomingMessage, response: ServerResponse) => void | Promise<void>
  const unset: Handler = (_request, response) => {
    response.writeHead(500).end("没有为该用例设置上游 handler")
  }
  let handler = unset
  let resetting = false
  const pending = new Set<Promise<void>>()
  const failures: unknown[] = []
  const upstream = createHttpServer((request, response) => {
    if (resetting) { response.destroy(); return }
    const current = handler
    const work: Promise<void> = Promise.resolve().then(() => current(request, response)).catch(error => {
      const interrupted = resetting && error instanceof Error && "code" in error
        && (error.code === "ECONNRESET" || error.code === "ERR_STREAM_PREMATURE_CLOSE")
      if (!interrupted) failures.push(error)
      response.destroy()
    }).finally(() => pending.delete(work))
    pending.add(work)
  })
  const drainUpstream = requestDrainer(upstream)
  await new Promise<void>(resolve => upstream.listen(0, "127.0.0.1", resolve))
  const root = await mkdtemp(join(tmpdir(), "kanban-host-proxy-"))
  await mkdir(join(root, "dist"))
  await writeFile(join(root, "index.html"), "<!doctype html><main>本地 Vite 页面</main>")
  await writeFile(join(root, "dist/index.html"), "<!doctype html><main>本地 preview 页面</main>")
  await writeFile(join(root, "dist/manifest.json"), JSON.stringify({ owner: "local-preview" }))
  vi.stubEnv("KANBAN_HOST_URL", serverOrigin(upstream))
  const common = { configFile, root, mode: "test", logLevel: "silent" as const }
  function wrap(host: Server, stop: () => Promise<void>) {
    const drainHost = requestDrainer(host)
    async function resetRequests() {
      resetting = true
      handler = unset
      try {
        await drainHost()
        await drainUpstream()
        await Promise.all([...pending])
        if (failures.length) throw new AggregateError(failures.splice(0), "上游 handler 失败")
      } finally {
        resetting = false
      }
    }
    return {
      origin: serverOrigin(host), upstreamOrigin: serverOrigin(upstream),
      setHandler: (next: Handler) => { handler = next },
      resetRequests,
      close: async () => {
        try { await resetRequests() } finally {
          try { await stop() } finally {
            try { await closeHttpServer(upstream) } finally {
              await rm(root, { recursive: true, force: true })
            }
          }
        }
      },
    }
  }
  let stop: (() => Promise<void>) | undefined
  try {
    if (kind === "dev") {
      const vite = await createServer({ ...common,
        server: { host: "127.0.0.1", port: 0, strictPort: true, watch: null, hmr: false, ws: false },
        optimizeDeps: { noDiscovery: true, include: [] },
      })
      stop = () => vite.close()
      await vite.listen()
      return wrap(vite.httpServer!, stop)
    }
    const vite = await preview({ ...common, preview: { host: "127.0.0.1", port: 0, strictPort: true } })
    stop = () => closeHttpServer(vite.httpServer)
    return wrap(vite.httpServer, stop)
  } catch (error) {
    try { await stop?.() } finally {
      try { await closeHttpServer(upstream) } finally {
        await rm(root, { recursive: true, force: true })
      }
    }
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
    // 每例会主动关闭连接；避免全局 Agent 在相邻用例复用刚被关闭的空闲 socket。
    const request = httpRequest(`${origin}${path}`, { method: body === undefined ? "GET" : "POST", headers, agent: false }, response => {
      void readBody(response).then(bytes => resolve({ status: response.statusCode, headers: response.headers, body: bytes }), reject)
    })
    request.on("error", reject)
    request.end(body)
  })
}

describe.sequential("KANBAN_HOST_URL", () => {
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

describe.sequential.each(["dev", "preview"] as const)("Vite %s 实际同源代理", kind => {
  let value: Awaited<ReturnType<typeof fixture>> | undefined

  function currentFixture(): Awaited<ReturnType<typeof fixture>> {
    if (!value) throw new Error("Vite proxy fixture was not initialized")
    return value
  }

  beforeAll(async () => {
    value = await fixture(kind)
  })

  afterEach(async () => {
    await value?.resetRequests()
  })

  afterAll(async () => {
    try {
      await value?.close()
    } finally {
      vi.unstubAllEnvs()
    }
  })

  test("两个正式 RPC service使用同一个 Host，原样转发方法、Content-Type 与字节", async () => {
    const value = currentFixture()
    const received: Array<{ url?: string; method?: string; headers: IncomingMessage["headers"]; body: Buffer }> = []
    value.setHandler((request, response) => {
      return readBody(request).then(body => {
        received.push({ url: request.url, method: request.method, headers: request.headers, body })
        response.writeHead(200, { "Content-Type": "application/grpc-web+proto" }).end(body)
      })
    })
    const payload = Buffer.from([0, 0, 0, 0, 3, 8, 1, 0])
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
  })

  test("metadata 与健康检查共用 Host，本地 /app/ 与 strict preview CSP 保留", async () => {
    const value = currentFixture()
    const paths: string[] = []
    const origins: Array<string | undefined> = []
    value.setHandler((request, response) => {
      paths.push(request.url ?? "")
      origins.push(request.headers.origin)
      response.writeHead(200, { "Content-Type": "application/json" }).end(JSON.stringify({ owner: "host", path: request.url }))
    })
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
  })

  test("重复或无效 Origin、不同端口 Origin 与非 loopback Host 均在上游之前拒绝", async () => {
    const value = currentFixture()
    let calls = 0
    value.setHandler((_request, response) => { calls += 1; response.end() })
    const host = new URL(value.origin).host
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
  })

  test("首个 stream chunk 在上游结束前到达，Abort 会关闭上游 stream", async () => {
    const value = currentFixture()
    const closed = deferred<void>()
    const first = Buffer.from([0, 0, 0, 0, 2, 8, 1])
    let ended = false
    value.setHandler((_request, response) => {
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
    }
  })

  test("上游重定向不会被代理跟随到另一个 target", async () => {
    const value = currentFixture()
    let redirectedCalls = 0
    const redirect = createHttpServer((_request, response) => { redirectedCalls += 1; response.end("不应访问") })
    await new Promise<void>(resolve => redirect.listen(0, "127.0.0.1", resolve))
    value.setHandler((_request, response) => {
      response.writeHead(307, { Location: `${serverOrigin(redirect)}/remote` }).end()
    })
    try {
      const result = await send(value.origin, rpcPaths[0], { Origin: value.origin }, Buffer.from([0]))
      expect(result.status).toBe(307)
      expect(result.headers.location).toBe(`${serverOrigin(redirect)}/remote`)
      expect(redirectedCalls).toBe(0)
    } finally {
      await closeHttpServer(redirect)
    }
  })
})
