import { translate, type DesktopLocale } from "@/i18n"

import { ApiError } from "./errors"

type Translate = (key: string, values?: Record<string, string | number>) => string

const DEFAULT_TRANSLATE: Translate = (key, values) => translate("zh-CN", key, values)

/** 将 transport/service 错误转换为可执行提示，同时保留 code、message 和 details。 */
export function presentApiError(err: unknown, t: Translate = DEFAULT_TRANSLATE) {
  const raw = rawErrorText(err)
  const lower = raw.toLowerCase()
  const code = err instanceof ApiError ? err.code.toLowerCase() : ""
  const guidance = guidanceKey(code, lower)
  return guidance ? `${t(guidance)} ${t("Original error: {error}", { error: raw })}` : raw
}

export function errorMessage(err: unknown, locale: DesktopLocale = "zh-CN") {
  return presentApiError(err, (key, values) => translate(locale, key, values))
}

function guidanceKey(code: string, lower: string) {
  if (
    code === "server_unavailable" ||
    code === "service_unavailable" ||
    code === "connection_refused" ||
    code === "http_error" && /\b5\d\d\b|unavailable|bad gateway|gateway timeout/.test(lower) ||
    /failed to fetch|fetch failed|networkerror|network request failed|load failed|econnrefused|connection refused/.test(lower)
  ) {
    return "Server unavailable. Start or check kanban serve."
  }
  if (code === "degraded" || code === "degraded_result" || /\bdegraded\b|stale/.test(lower)) {
    return "Service returned a degraded result; inspect the capability reason."
  }
  if (code === "feature_not_available" || code === "unsupported" || code === "not_implemented" || /feature not available|not supported|not implemented/.test(lower)) {
    return "The requested capability is not available from the server."
  }
  if (code === "invalid_response" || /invalid response|must be valid json|unexpected token/.test(lower)) {
    return "The desktop client received an invalid server response; check kanban serve version and logs."
  }
  return null
}

function rawErrorText(err: unknown) {
  if (err instanceof ApiError) {
    const details = err.details === undefined ? "" : ` details=${stringifyDetails(err.details)}`
    return `${err.code}: ${err.message}${details}`
  }
  if (err instanceof Error) return err.message
  return String(err)
}

function stringifyDetails(details: unknown) {
  try {
    const serialized = JSON.stringify(details)
    return serialized === undefined ? String(details) : serialized
  } catch {
    return String(details)
  }
}
