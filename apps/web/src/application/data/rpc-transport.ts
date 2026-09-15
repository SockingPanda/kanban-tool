import type { ApiErrorResponseContract } from "../../lib/api/generated/contracts/api-error-response"
import type { RpcMethod } from "../../lib/rpc/methods.generated"

export type { RpcMethod } from "../../lib/rpc/methods.generated"

/** parts 使用现有业务 DTO 的 snake_case 字段，不包含 HTTP path 或 method。 */
export interface RpcCall {
  readonly method: RpcMethod
  readonly path?: object
  readonly query?: object
  readonly input?: object
  readonly actor?: string
  readonly signal?: AbortSignal
}

export interface RpcTransportResponse {
  /** 页面 DTO；附件下载为 { attachment, content: Uint8Array }。 */
  readonly payload: unknown
  /** 收到的业务响应 Protobuf 长度，不计 gRPC framing 和 metadata。 */
  readonly bytes: number
}

export interface RpcTransport {
  call(request: RpcCall): Promise<RpcTransportResponse>
  /** 已挂载 Events 消费完整有界窗口；旧测试 adapter 可继续独立验证向前分页。 */
  recentEvents?(query: { readonly board: string; readonly task_id?: string; readonly limit: number }, signal?: AbortSignal): Promise<RpcTransportResponse>
}

export interface RpcTransportOptions {
  readonly fetcher?: typeof fetch
  readonly documentBaseURI?: string
}

export type RpcTransportErrorKind =
  | "cross_origin"
  | "malformed_url"
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_headers"
  | "invalid_content_type"
  | "invalid_bytes"
  | "response_too_large"

/** 保留页面既有错误分类；invalid_json 表示业务 DTO 无法与正式消息互转。 */
export class RpcTransportError extends Error {
  readonly kind: RpcTransportErrorKind
  readonly status: number | null
  readonly apiError: ApiErrorResponseContract["error"] | null

  constructor(
    kind: RpcTransportErrorKind,
    message: string,
    options: {
      status?: number
      apiError?: ApiErrorResponseContract["error"]
      cause?: unknown
    } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "RpcTransportError"
    this.kind = kind
    this.status = options.status ?? null
    this.apiError = options.apiError ?? null
  }
}

/** 256 MiB 附件加 1 MiB envelope；附件 service 仍限制实际 content。 */
export const MAX_RPC_MESSAGE_BYTES = 257 * 1024 * 1024
