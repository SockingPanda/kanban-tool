import type { RuntimeWebConfigOutputContract } from "./api/generated/contracts/runtime-web-config-output"
import runtimeWebConfigValidator from "virtual:kanban-runtime-validator"

/** `/app/runtime.json` 的已验证 host metadata。 */
export type WebRuntimeConfig = RuntimeWebConfigOutputContract

export type RuntimeBootstrapErrorKind = "network" | "http" | "invalid_json" | "invalid_contract"

/** runtime bootstrap 失败时保留可诊断但不泄漏 payload 的错误。 */
export class RuntimeBootstrapError extends Error {
  readonly kind: RuntimeBootstrapErrorKind
  readonly status: number | null

  constructor(kind: RuntimeBootstrapErrorKind, message: string, options: { status?: number; cause?: unknown } = {}) {
    super(message, { cause: options.cause })
    this.name = "RuntimeBootstrapError"
    this.kind = kind
    this.status = options.status ?? null
  }
}

export type RuntimeBootstrapOptions = {
  fetch?: typeof globalThis.fetch
  documentBaseURI?: string
  webBasePath?: string
}

/**
 * 从当前 document 的同源 base 推导 runtime endpoint。
 * `webBasePath` 来自 Vite 的 `/app/` base，深层 SPA URL 也始终回到 `/app/runtime.json`。
 */
export function runtimeEndpointUrl(
  documentBaseURI: string,
  webBasePath = "/app/",
): string {
  const documentUrl = new URL(documentBaseURI)
  const base = new URL(webBasePath, documentBaseURI)
  if (!base.pathname.endsWith("/")) base.pathname += "/"
  const endpoint = new URL("runtime.json", base)
  if (endpoint.origin !== documentUrl.origin) {
    throw new RuntimeBootstrapError(
      "network",
      "Web runtime 配置必须通过当前页面的同源 `/app/runtime.json` 加载。",
    )
  }
  return endpoint.toString()
}

function runtimeDocumentBaseURI() {
  if (typeof document === "undefined") return "http://127.0.0.1/app/"
  return document.baseURI
}

function runtimeWebBasePath() {
  const basePath = import.meta.env.BASE_URL
  return typeof basePath === "string" && basePath.length > 0 ? basePath : "/app/"
}

/** 在 React mount 前读取并验证同源 `/app/runtime.json`。 */
export async function loadWebRuntimeConfig(options: RuntimeBootstrapOptions = {}): Promise<WebRuntimeConfig> {
  const fetcher = options.fetch ?? globalThis.fetch
  const endpoint = runtimeEndpointUrl(
    options.documentBaseURI ?? runtimeDocumentBaseURI(),
    options.webBasePath ?? runtimeWebBasePath(),
  )

  let response: Response
  try {
    response = await fetcher(endpoint, {
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    })
  } catch (cause) {
    throw new RuntimeBootstrapError(
      "network",
      "无法加载 Web runtime 配置，请确认 kanban serve 正在运行。",
      { cause },
    )
  }

  if (!response.ok) {
    throw new RuntimeBootstrapError(
      "http",
      `Web runtime 配置请求失败：HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`,
      { status: response.status },
    )
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch (cause) {
    throw new RuntimeBootstrapError(
      "invalid_json",
      "Web runtime 配置不是有效 JSON，请检查 kanban serve 版本与日志。",
      { cause },
    )
  }

  if (!runtimeWebConfigValidator(payload)) {
    throw new RuntimeBootstrapError(
      "invalid_contract",
      "Web runtime 配置不符合当前协议，请升级 kanban serve 与 Web artifact。",
      {
        cause: {
          contractId: "runtime.web-config.output",
          errors: runtimeWebConfigValidator.errors,
        },
      },
    )
  }
  return payload
}

export function runtimeErrorMessage(error: unknown): string {
  if (error instanceof RuntimeBootstrapError) return error.message
  if (error instanceof Error) return error.message
  return String(error)
}
