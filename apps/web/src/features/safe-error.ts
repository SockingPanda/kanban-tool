import type { Locale } from "../lib/preferences"
import { HttpTransportError } from "../lib/api/http-transport"
import { SignalsOntologyReadError } from "../lib/api/signals-ontology-read-model"

type ErrorCopy = {
  readonly invalidIdentity: string
  readonly wrongBoard: string
  readonly offline: string
  readonly conflict: string
  readonly notFound: string
  readonly invalidInput: string
  readonly unreadable: string
}

const copies: Record<Locale, ErrorCopy> = {
  zh: {
    invalidIdentity: "无法解析当前看板身份，请刷新后重试。",
    wrongBoard: "服务返回了不属于当前看板的数据，请刷新后重试。",
    offline: "无法连接本地服务，请确认 kanban serve 正在运行。",
    conflict: "操作发生冲突，请刷新后重试。",
    notFound: "请求的记录已不可用，请刷新后重试。",
    invalidInput: "请求内容不符合当前操作要求。",
    unreadable: "本地服务返回了无法读取的响应。",
  },
  en: {
    invalidIdentity: "The current board identity could not be resolved. Refresh and try again.",
    wrongBoard: "The service returned data from another board. Refresh and try again.",
    offline: "The local service is unreachable. Confirm kanban serve is running.",
    conflict: "The operation conflicted with a newer state. Refresh and try again.",
    notFound: "The requested record is no longer available. Refresh and try again.",
    invalidInput: "The request does not satisfy this operation's requirements.",
    unreadable: "The local service returned an unreadable response.",
  },
}

function apiCode(error: HttpTransportError): string | null {
  return error.apiError?.code ?? null
}

function transportMessage(error: HttpTransportError, copy: ErrorCopy): string {
  switch (apiCode(error)) {
    case "not_found": return copy.notFound
    case "conflict":
    case "idempotency_conflict":
    case "claim_conflict":
    case "claim_token_mismatch":
    case "invalid_transition": return copy.conflict
    case "invalid_input": return copy.invalidInput
    case "server_unavailable": return copy.offline
    default: break
  }
  if (error.kind === "offline") return copy.offline
  if (error.kind === "cross_origin" || error.kind === "malformed_url") return copy.invalidIdentity
  return copy.unreadable
}

/** Keep server internals and arbitrary Error messages out of rendered product copy. */
export function localizedErrorMessage(error: unknown, fallback: string, locale: Locale): string {
  const copy = copies[locale]
  if (error instanceof SignalsOntologyReadError) {
    if (error.kind === "identity") return copy.invalidIdentity
    if (error.kind === "board_scope") return copy.wrongBoard
    if (error.kind === "http" && error.cause instanceof HttpTransportError) return transportMessage(error.cause, copy)
    return copy.unreadable || fallback
  }
  if (error instanceof HttpTransportError) return transportMessage(error, copy)
  return fallback
}
