import { invoke } from "@tauri-apps/api/core"

import type { RuntimeConfig } from "./api"

export type RuntimeInvoker = (command: string, args?: Record<string, unknown>) => Promise<RuntimeConfig>

function defaultRuntimeInvoker(): RuntimeInvoker | null {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) return invoke
  return null
}

export async function switchRuntimeBoard(
  config: RuntimeConfig,
  board: string,
  invokeRuntime: RuntimeInvoker | null = defaultRuntimeInvoker(),
): Promise<RuntimeConfig> {
  if (board === config.board) return config
  if (invokeRuntime) {
    return invokeRuntime("set_runtime_board", { board })
  }
  return { ...config, board }
}
