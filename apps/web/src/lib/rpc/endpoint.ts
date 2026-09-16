import { MAX_RPC_MESSAGE_BYTES, RpcTransportError } from "../../application/data/rpc-transport"
import type { WebRuntimeConfig } from "../runtime"

function currentBaseURI(): string {
  return typeof document === "undefined" ? "http://127.0.0.1/app/" : document.baseURI
}

function invalidBasePath(value: string): boolean {
  if (value.includes("\\") || value.includes("\0")) return true
  for (const part of value.split("/")) {
    let decoded = part
    for (let layer = 0; layer < 64; layer += 1) {
      if (decoded === "." || decoded === ".." || decoded.includes("\\") || decoded.includes("\0") || /%(?:2f|5c|00)/i.test(decoded)) return true
      if (!decoded.includes("%")) break
      try {
        const next = decodeURIComponent(decoded)
        if (next === decoded) break
        decoded = next
      } catch {
        return true
      }
      if (layer === 63) return true
    }
  }
  return false
}

/** 使用当前页面的同源 runtime base；业务方法名由正式 service 生成。 */
export function sameOriginRpcBase(runtime: WebRuntimeConfig, baseURI = currentBaseURI()): URL {
  let documentURL: URL
  let base: URL
  const configured = runtime.apiBaseUrl.trim()
  if (invalidBasePath(configured)) {
    throw new RpcTransportError("cross_origin", "RPC 地址不得包含路径跳转、反斜杠或编码的路径分隔符。")
  }
  try {
    documentURL = new URL(baseURI)
    base = new URL(configured || "/", documentURL)
  } catch (cause) {
    throw new RpcTransportError("malformed_url", "当前页面或 runtime 的 RPC 地址无效。", { cause })
  }
  if (!/^https?:$/.test(base.protocol) || base.origin !== documentURL.origin || base.username || base.password || base.search || base.hash) {
    throw new RpcTransportError("cross_origin", "RPC 请求必须使用当前页面的同源地址，且不得包含凭据、查询或片段。")
  }
  if (!base.pathname.endsWith("/")) base.pathname += "/"
  return base
}

/** 按 gRPC frame 检查消息上限，不聚合持续更新流或复制附件内容。 */
function boundedFrames(onMessageBytes?: (bytes: number) => void): TransformStream<Uint8Array, Uint8Array> {
  const header = new Uint8Array(5)
  let headerBytes = 0
  let remaining = 0
  return new TransformStream({
    transform(chunk, controller) {
      let offset = 0
      while (offset < chunk.byteLength) {
        if (remaining > 0) {
          const consumed = Math.min(remaining, chunk.byteLength - offset)
          remaining -= consumed
          offset += consumed
          continue
        }
        const copied = Math.min(5 - headerBytes, chunk.byteLength - offset)
        header.set(chunk.subarray(offset, offset + copied), headerBytes)
        headerBytes += copied
        offset += copied
        if (headerBytes === 5) {
          remaining = new DataView(header.buffer).getUint32(1)
          const trailer = (header[0] & 128) !== 0
          const maximum = trailer ? 1024 * 1024 : MAX_RPC_MESSAGE_BYTES
          if (remaining > maximum) {
            throw new RpcTransportError("response_too_large", `RPC 响应消息超过 ${maximum} 字节上限。`)
          }
          if (!trailer) onMessageBytes?.(remaining)
          headerBytes = 0
        }
      }
      controller.enqueue(chunk)
    },
    flush() {
      if (headerBytes !== 0 || remaining !== 0) {
        throw new RpcTransportError("invalid_bytes", "RPC 响应在消息传输完成前中断。")
      }
    },
  })
}

/** 查询、命令和 QueryService 共用的 Fetch 边界。 */
export function createRpcFetch(
  baseUrl: string,
  fetcher: typeof fetch = globalThis.fetch,
  onMessageBytes?: (bytes: number) => void,
): typeof fetch {
  const base = new URL(baseUrl)
  return async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url)
    if (url.origin !== base.origin || !url.pathname.startsWith(base.pathname)) {
      throw new RpcTransportError("cross_origin", "RPC 方法地址离开了 runtime 的同源路径。")
    }
    if (init?.body instanceof Uint8Array && init.body.byteLength > MAX_RPC_MESSAGE_BYTES + 5) {
      throw new RpcTransportError("response_too_large", `RPC 请求消息超过 ${MAX_RPC_MESSAGE_BYTES} 字节上限。`)
    }
    let response: Response
    try {
      response = await fetcher(input, {
        ...init,
        credentials: "same-origin",
        mode: "same-origin",
        redirect: "error",
        referrerPolicy: "same-origin",
        cache: "no-store",
      })
    } catch (cause) {
      if (init?.signal?.aborted || (cause instanceof Error && cause.name === "AbortError")) throw cause
      throw new RpcTransportError("offline", "无法连接 kanban serve，请确认本地服务正在运行。", { cause })
    }
    const cancel = async () => { try { await response.body?.cancel() } catch { /* 保留原始 transport 错误。 */ } }
    if (response.url !== url.href || response.redirected) {
      await cancel()
      throw new RpcTransportError("cross_origin", "RPC 响应地址与请求地址不一致。")
    }
    if (!response.ok) {
      await cancel()
      throw new RpcTransportError("http", `RPC 请求失败：HTTP ${response.status}。`, { status: response.status })
    }
    if (!/^application\/grpc-web(?:\+proto)?(?:\s*;|$)/i.test(response.headers.get("content-type") ?? "")) {
      await cancel()
      throw new RpcTransportError("invalid_content_type", "RPC 响应必须使用 binary gRPC-Web Content-Type。", { status: response.status })
    }
    if (response.body === null) throw new RpcTransportError("invalid_bytes", "RPC 响应没有可读取的消息。")
    return new Response(response.body.pipeThrough(boundedFrames(onMessageBytes)), {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    })
  }
}
