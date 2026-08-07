import { invoke } from "@tauri-apps/api/core"
import { getCurrentDesktopLocale, type DesktopLocale } from "@/i18n"
import { ApiError } from "./errors"
import { normalizeApiBaseUrl } from "../loopback-url"
import { parseJsonEnvelope } from "./parsers"
import { parseTaskReadErrorEnvelope } from "./operations/task/parsers"
import type { RequestOptions, RuntimeConfig } from "./types"

const WEB_DEV_API_BASE_URL = "/__kb_api__"
const WEB_DEV_DEFAULT_ACTOR = "desktop-dev"
const WEB_DEV_DEFAULT_BOARD = "kanban-tool"

export { ApiError }

export async function loadRuntimeConfig(): Promise<RuntimeConfig> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return invoke<RuntimeConfig>("runtime_config")
  }
  const configuredApiBaseUrl = normalizeApiBaseUrl(import.meta.env.VITE_KB_API_BASE_URL)
  const usingWebDevDefault = !configuredApiBaseUrl && import.meta.env.DEV
  const apiBaseUrl = configuredApiBaseUrl || (usingWebDevDefault ? WEB_DEV_API_BASE_URL : "")
  if (!apiBaseUrl) {
    throw new Error(
      "Tauri 外运行时必须配置 VITE_KB_API_BASE_URL；请将其设置为明确的 API 源，或设置为明确的 Vite 代理基路径，例如 /__kb_api__。",
    )
  }
  return {
    apiBaseUrl,
    actor: import.meta.env.VITE_KB_ACTOR ?? WEB_DEV_DEFAULT_ACTOR,
    board: import.meta.env.VITE_KB_BOARD ?? (usingWebDevDefault ? WEB_DEV_DEFAULT_BOARD : "default"),
  }
}

export class ApiTransport {
  constructor(
    private readonly config: RuntimeConfig,
    private readonly options: { locale?: DesktopLocale } = {},
  ) {}

  get actor() {
    return this.config.actor
  }

  get board() {
    return this.config.board
  }

  async requestRaw(path: string, init: RequestOptions = {}): Promise<unknown> {
    const headers: Record<string, string> = { "Accept-Language": this.options.locale ?? getCurrentDesktopLocale() }
    if (init.body !== undefined) headers["Content-Type"] = "application/json"
    if (init.actorHeader) headers["X-KB-Actor"] = this.actor
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, { method: init.method ?? "GET", headers, body: init.body === undefined ? undefined : JSON.stringify(init.body), signal: init.signal })
    const text = await response.text()
    let json: unknown = null
    try { json = text ? JSON.parse(text) : null } catch { throw new ApiError("invalid_response", "响应必须是有效 JSON") }
    const record = json && typeof json === "object" && !Array.isArray(json) ? json as Record<string, unknown> : null
    if (record && "error" in record) {
      const error = parseTaskReadErrorEnvelope(record)
      throw new ApiError(error.code, error.message, error.details)
    }
    if (!response.ok) throw new ApiError("http_error", `${response.status} ${response.statusText}`.trim())
    return json
  }

  async requestBytes(path: string, init: RequestOptions = {}) {
    const method = init.method ?? "GET"
    const headers: Record<string, string> = { "Accept-Language": this.options.locale ?? getCurrentDesktopLocale() }
    if (init.body !== undefined) headers["Content-Type"] = "application/json"
    if (method.toUpperCase() !== "GET" || init.actorHeader) headers["X-KB-Actor"] = this.actor
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, {
      method,
      headers,
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      signal: init.signal,
    })
    const bytes = new Uint8Array(await response.arrayBuffer())
    if (!response.ok) {
      const text = new TextDecoder().decode(bytes)
      let json: unknown = null
      try { json = text ? JSON.parse(text) : null } catch { /* 解析失败时继续使用状态错误 */ }
      const record = json && typeof json === "object" && !Array.isArray(json) ? json as Record<string, unknown> : null
      if (record && "error" in record) {
        const error = parseTaskReadErrorEnvelope(record)
        throw new ApiError(error.code, error.message, error.details)
      }
      throw new ApiError("http_error", `${response.status} ${response.statusText}`.trim())
    }
    return {
      bytes,
      contentType: response.headers.get("Content-Type"),
      attachmentId: response.headers.get("X-KB-Attachment-ID"),
      sha256: response.headers.get("X-KB-Attachment-SHA256"),
    }
  }

  async requestEnvelope<T, M = Record<string, unknown>>(path: string, init: RequestOptions = {}) {
    const method = init.method ?? "GET"
    const headers: Record<string, string> = {
      "Accept-Language": this.options.locale ?? getCurrentDesktopLocale(),
    }
    if (init.body !== undefined) headers["Content-Type"] = "application/json"
    if (method.toUpperCase() !== "GET") headers["X-KB-Actor"] = this.actor
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, {
      method,
      headers,
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      signal: init.signal,
    })
    const text = await response.text()
    const json = parseJsonEnvelope<T, M>(text)
    if (!response.ok || !json || "error" in json) {
      const error = json && "error" in json
        ? json.error
        : { code: "http_error", message: `${response.status} ${response.statusText}`.trim() }
      throw new ApiError(error.code, error.message)
    }
    return json
  }

  async request<T>(path: string, init: RequestOptions = {}) {
    const envelope = await this.requestEnvelope<T>(path, init)
    return envelope.data
  }
}

export function newClientTaskId() {
  return `t_${crypto.randomUUID().replace(/-/g, "").toUpperCase()}`
}
