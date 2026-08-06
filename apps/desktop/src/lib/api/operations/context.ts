import { ApiTransport } from "../transport"
import type { ContextBuildOptions, ContextPack } from "../types"

export async function buildContext(
  api: ApiTransport,
  taskId: string,
  options: ContextBuildOptions = {},
) {
  const params = new URLSearchParams({
    board: options.board ?? api.board,
    lexical_limit: String(options.lexicalLimit ?? 5),
    graph_limit: String(options.graphLimit ?? 10),
    vector_limit: String(options.vectorLimit ?? 5),
    max_items: String(options.maxItems ?? options.budget ?? 20),
    depth: String(options.depth ?? 1),
  })
  if (options.task?.trim()) params.set("task", options.task.trim())
  if (options.reference?.trim()) params.set("reference", options.reference.trim())
  if (options.query?.trim()) params.set("query", options.query.trim())
  if (typeof options.budget === "number") params.set("budget", String(options.budget))
  return api.request<ContextPack>(
    `/api/v1/tasks/${encodeURIComponent(taskId)}/context?${params.toString()}`,
    { signal: options.signal },
  )
}
