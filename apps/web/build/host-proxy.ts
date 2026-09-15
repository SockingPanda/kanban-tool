import type { IncomingMessage } from "node:http"
import type { ProxyOptions } from "vite"

const DEFAULT_KANBAN_HOST_URL = "http://127.0.0.1:8721"
const LOOPBACK_ORIGIN = /^https?:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::[1-9]\d{0,4})?\/?$/

/** 开发代理只连接显式 loopback origin，不接受路径、凭据或远程 target。 */
export function resolveKanbanHostUrl(input = DEFAULT_KANBAN_HOST_URL): string {
  if (!LOOPBACK_ORIGIN.test(input)) throw new Error("KANBAN_HOST_URL 必须是 loopback HTTP(S) origin，例如 http://127.0.0.1:8721")
  try {
    return new URL(input).origin
  } catch {
    throw new Error("KANBAN_HOST_URL 的地址或端口无效")
  }
}

function headerCount(request: IncomingMessage, name: string): number {
  let count = 0
  for (let index = 0; index < request.rawHeaders.length; index += 2) {
    if (request.rawHeaders[index]?.toLowerCase() === name) count += 1
  }
  return count
}

function hasSameLocalOrigin(request: IncomingMessage): boolean {
  if (headerCount(request, "host") !== 1 || headerCount(request, "origin") > 1) return false
  const host = request.headers.host
  if (host === undefined || host.includes("/")) return false
  const protocol = "encrypted" in request.socket && request.socket.encrypted ? "https:" : "http:"
  try {
    const incomingOrigin = resolveKanbanHostUrl(`${protocol}//${host}`)
    const origin = request.headers.origin
    // 无 Origin 的本机工具可以访问；浏览器 Origin 必须是严格序列化的当前 origin。
    return origin === undefined || origin === incomingOrigin
  } catch {
    return false
  }
}

/** Vite 直接流式转发 body；不解码 Protobuf、不缓冲响应、不创建第二条业务路径。 */
export function createKanbanHostProxy(input?: string): Record<string, ProxyOptions> {
  const target = resolveKanbanHostUrl(input)
  const options: ProxyOptions = {
    target,
    changeOrigin: true,
    followRedirects: false,
    ws: false,
    bypass(request, response) {
      if (!hasSameLocalOrigin(request)) {
        response?.writeHead(403, { "Content-Type": "text/plain; charset=utf-8" }).end("Kanban 开发代理拒绝非同源或无效请求")
        // Vite 在 string bypass 且响应已经结束时停止处理，不向上游发送请求。
        return request.url ?? "/"
      }
      // 在创建 upstream request 前统一身份，包含使用 Expect: 100-continue 的请求。
      if (request.headers.origin !== undefined) request.headers.origin = target
      for (const header of ["forwarded", "x-forwarded-host", "x-forwarded-proto", "x-forwarded-port", "x-forwarded-for"]) {
        delete request.headers[header]
      }
    },
  }
  return {
    "^/kanban(?:\\.framework)?\\.v1\\.": options,
    "^/app/(?:runtime|manifest)\\.json(?:\\?|$)": options,
    "^/api/v1(?:/|\\?|$)": options,
    "^/health(?:\\?|$)": options,
  }
}
