import type { ReactNode } from "react"

import { WebRuntimeContext } from "./runtime-context"
import type { WebRuntimeConfig } from "./runtime"

export function WebRuntimeProvider({ runtime, children }: { runtime: WebRuntimeConfig; children: ReactNode }) {
  return <WebRuntimeContext.Provider value={runtime}>{children}</WebRuntimeContext.Provider>
}
