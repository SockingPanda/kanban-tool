import { Code, ConnectError } from "@connectrpc/connect"
import { RpcTransportError } from "../../application/data/rpc-transport"
import { DtoApiErrorCode } from "../../generated/rpc/kanban/v1/dto_pb"
import { ErrorDetailSchema, type ErrorDetail } from "../../generated/rpc/kanban/v1/kanban_pb"
import { RpcCodecError } from "./value-codec"

type ApiError = NonNullable<RpcTransportError["apiError"]>
const businessErrors: Partial<Record<DtoApiErrorCode, readonly [ApiError["code"], Code, number]>> = {
  [DtoApiErrorCode.NOT_FOUND]: ["not_found", Code.NotFound, 404],
  [DtoApiErrorCode.CONFLICT]: ["conflict", Code.Aborted, 409],
  [DtoApiErrorCode.IDEMPOTENCY_CONFLICT]: ["idempotency_conflict", Code.Aborted, 409],
  [DtoApiErrorCode.DEPENDENCY_CYCLE]: ["dependency_cycle", Code.InvalidArgument, 409],
  [DtoApiErrorCode.INVALID_INPUT]: ["invalid_input", Code.InvalidArgument, 400],
  [DtoApiErrorCode.FEATURE_NOT_AVAILABLE]: ["feature_not_available", Code.Unimplemented, 501],
  [DtoApiErrorCode.SERVER_UNAVAILABLE]: ["server_unavailable", Code.Unavailable, 503],
  [DtoApiErrorCode.EXECUTION_PLAN_REQUIRED]: ["execution_plan_required", Code.FailedPrecondition, 409],
  [DtoApiErrorCode.STEPS_INCOMPLETE]: ["steps_incomplete", Code.FailedPrecondition, 409],
  [DtoApiErrorCode.CLAIM_TOKEN_MISMATCH]: ["claim_token_mismatch", Code.FailedPrecondition, 403],
  [DtoApiErrorCode.DEPENDENCY_BLOCKED]: ["dependency_blocked", Code.FailedPrecondition, 409],
  [DtoApiErrorCode.CLAIM_CONFLICT]: ["claim_conflict", Code.Aborted, 409],
  [DtoApiErrorCode.INVALID_TRANSITION]: ["invalid_transition", Code.FailedPrecondition, 409],
  [DtoApiErrorCode.INTERNAL]: ["internal", Code.Internal, 500],
}

export function queryFailureError(detail: ErrorDetail | undefined): RpcTransportError {
  const mapping = detail && businessErrors[detail.code]
  if (!detail || !mapping) return new RpcTransportError('invalid_bytes', '查询失败帧缺少正式业务错误。')
  return new RpcTransportError('http', detail.message, { status: mapping[2], apiError: { code: mapping[0], message: detail.message } })
}

/** 标准 google.rpc.Status/Any 由 Connect 解包，本层只消费正式 ErrorDetail。 */
export function rpcTransportError(error: unknown): RpcTransportError {
  const visited = new Set<unknown>()
  for (let cause = error; cause instanceof Error && !visited.has(cause); cause = cause.cause) {
    visited.add(cause)
    if (cause instanceof RpcTransportError) return cause
    if (cause instanceof RpcCodecError) return new RpcTransportError("invalid_json", cause.message, { cause })
  }
  if (error instanceof ConnectError) {
    const details = error.findDetails(ErrorDetailSchema)
    if (details.length > 0) {
      const detail = details[0]
      const mapping = businessErrors[detail.code]
      if (details.length !== 1 || mapping === undefined || mapping[1] !== error.code) {
        return new RpcTransportError("invalid_bytes", "RPC 错误细节与状态码不一致。", { cause: error })
      }
      return new RpcTransportError("http", detail.message, { status: mapping[2], apiError: { code: mapping[0], message: detail.message }, cause: error })
    }
    if (error.code === Code.Unavailable) return new RpcTransportError("offline", "无法连接 kanban serve，请确认本地服务正在运行。", { cause: error })
    if (error.code === Code.ResourceExhausted) return new RpcTransportError("response_too_large", "RPC 消息超过服务端允许的大小。", { cause: error })
    return new RpcTransportError("invalid_bytes", "RPC 响应无效或缺少正式错误细节。", { cause: error })
  }
  return new RpcTransportError("invalid_bytes", "无法读取 RPC 响应。", { cause: error })
}

/** binary metadata 保留中文 actor，不把 UTF-8 字节塞进 ASCII header。 */
export function actorHeaders(actor: string | undefined): Headers {
  const headers = new Headers()
  if (actor === undefined) return headers
  const bytes = new TextEncoder().encode(actor)
  if (actor.trim().length === 0 || bytes.length > 8192 || new TextDecoder().decode(bytes) !== actor) throw new RpcTransportError("invalid_headers", "actor 必须是非空文本，且 UTF-8 长度不超过 8192 字节。")
  headers.set("x-kb-actor-bin", btoa(String.fromCharCode(...bytes)))
  return headers
}
