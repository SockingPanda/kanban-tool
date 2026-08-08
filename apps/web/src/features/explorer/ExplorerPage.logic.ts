import { ExplorerReadError } from "../../lib/api/explorer-read-model"
import type { BoardRouteView } from "../../lib/router"

export function shouldClearMapTaskFromInspector(
  view: BoardRouteView,
  taskId: string | null,
  error: ExplorerReadError | Error | null,
): boolean {
  return view === "map" && Boolean(taskId) && error instanceof ExplorerReadError && error.reason === "task-not-found"
}
