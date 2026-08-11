import type { HealthReport } from "../../lib/api/health-read-model"
import type { WebRuntimeConfig } from "../../lib/runtime"

function runtimeBaseURI(baseURI?: string): string {
  if (baseURI) return baseURI
  if (typeof document !== "undefined") return document.baseURI
  return "http://127.0.0.1/app/"
}

/** Resolve only the current page origin; a remote runtime origin is never surfaced as a target. */
export function apiOriginForRuntime(runtime: WebRuntimeConfig, baseURI?: string): string {
  try {
    const documentURL = new URL(runtimeBaseURI(baseURI))
    const configured = runtime.apiBaseUrl.trim()
    const apiURL = new URL(configured.length > 0 ? configured : "/", documentURL)
    return apiURL.origin === documentURL.origin ? apiURL.origin : "invalid/untrusted"
  } catch {
    return "invalid/untrusted"
  }
}

function reported(value: string | null | undefined, fallback: string): string {
  return value?.trim() || fallback
}

/** A stable, server-safe diagnostics payload. Database paths and raw error text are intentionally omitted. */
export function diagnosticsText(runtime: WebRuntimeConfig, health: HealthReport | null, baseURI?: string): string {
  const lines = [
    "kanban-tool Web diagnostics",
    `apiOrigin=${apiOriginForRuntime(runtime, baseURI)}`,
    `serverVersion=${reported(runtime.serverVersion, "not-reported")}`,
    `protocolVersion=${reported(runtime.protocolVersion, "not-reported")}`,
    `webBuildId=${reported(runtime.webBuildId, "not-reported")}`,
    `health=${health?.ok === true ? "ok" : health === null ? "unavailable" : "degraded"}`,
  ]
  return lines.join("\n")
}
