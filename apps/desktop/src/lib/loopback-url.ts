/**
 * 校验桌面端和 Web 开发代理使用的 API 地址。
 *
 * kanban serve 只监听 loopback；客户端不能因为配置错误而把业务请求发往
 * 外部主机。相对路径仅用于同一个 Vite host 上的显式代理基路径。
 */
export function normalizeApiBaseUrl(value: string | undefined) {
  const trimmed = value?.trim()
  if (!trimmed) return ""

  if (trimmed.startsWith("/")) {
    if (trimmed === "/" || trimmed.startsWith("//") || trimmed.includes("?") || trimmed.includes("#") || trimmed.includes("\\")) {
      throw invalidLoopbackUrl("VITE_KB_API_BASE_URL")
    }
    const normalized = trimmed.replace(/\/+$/, "")
    return normalized
  }

  return normalizeLoopbackHttpUrl(trimmed, "VITE_KB_API_BASE_URL")
}

export function normalizeLoopbackHttpUrl(value: string, variableName: string) {
  const trimmed = value.trim().replace(/\/+$/, "")
  let parsed: URL
  try {
    parsed = new URL(trimmed)
  } catch {
    throw invalidLoopbackUrl(variableName)
  }

  const hostname = parsed.hostname.replace(/^\[|\]$/g, "").toLowerCase()
  const isLoopback = hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1"
  if (
    !isLoopback ||
    (parsed.protocol !== "http:" && parsed.protocol !== "https:") ||
    parsed.username ||
    parsed.password ||
    parsed.pathname !== "/" ||
    parsed.search ||
    parsed.hash
  ) {
    throw invalidLoopbackUrl(variableName)
  }

  return trimmed
}

export function invalidLoopbackUrl(variableName: string) {
  return new Error(
    `${variableName} 必须使用 localhost、127.0.0.1 或 [::1] 上的 http(s) 地址（例如 http://127.0.0.1:8721）；请修正配置后重启。`,
  )
}
