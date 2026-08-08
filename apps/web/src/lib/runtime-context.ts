import { createContext, useContext } from "react"

import type { WebRuntimeConfig } from "./runtime"

export const WebRuntimeContext = createContext<WebRuntimeConfig | null>(null)

/** 读取已通过 `/app/runtime.json` generated contract 校验的 host metadata。 */
export function useWebRuntime(): WebRuntimeConfig {
  const runtime = useContext(WebRuntimeContext)
  if (!runtime) throw new Error("Web runtime context is unavailable before bootstrap")
  return runtime
}
