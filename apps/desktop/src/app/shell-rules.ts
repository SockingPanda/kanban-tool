import type { OperatorView } from "@/features/navigation/view-types"

const taskExplorerToolbarViews = new Set<OperatorView>(["board", "list"])

export function shouldShowTaskExplorerToolbar(view: OperatorView): boolean {
  return taskExplorerToolbarViews.has(view)
}

export function apiEndpointLabel(apiBaseUrl: string): string {
  if (!apiBaseUrl || apiBaseUrl.startsWith("/")) return "same-origin"
  try {
    return new URL(apiBaseUrl).port || "default"
  } catch {
    return "same-origin"
  }
}
