/** Stable task opener identity used for Explorer Inspector focus return. */
export function taskOpenerKey(taskId: string): string {
  return encodeURIComponent(taskId)
}

export function taskOpenerSelector(taskId: string): string {
  return `[data-task-opener="${taskOpenerKey(taskId)}"]`
}

export type ExplorerFocusElement = {
  readonly isConnected?: boolean
  readonly disabled?: boolean
  readonly focus: () => void
}

export type ExplorerFocusDocument = {
  readonly querySelector: (selector: string) => ExplorerFocusElement | null
}

export type ExplorerFocusSnapshot = {
  readonly taskId: string
  readonly element: ExplorerFocusElement | null
}

/**
 * Return focus to the opener when it survived the route transition. If the
 * opener was removed (for example after changing view), use the page heading
 * marked by `[data-explorer-focus-fallback]`.
 */
export function restoreExplorerFocus(
  snapshot: ExplorerFocusSnapshot | null,
  documentLike: ExplorerFocusDocument,
): "opener" | "fallback" | "none" {
  if (snapshot?.element && snapshot.element.isConnected !== false && !snapshot.element.disabled) {
    snapshot.element.focus()
    return "opener"
  }
  if (snapshot) {
    const remountedOpener = documentLike.querySelector(taskOpenerSelector(snapshot.taskId))
    if (remountedOpener && remountedOpener.isConnected !== false && !remountedOpener.disabled) {
      remountedOpener.focus()
      return "opener"
    }
  }
  const fallback = documentLike.querySelector("[data-explorer-focus-fallback]")
  if (fallback && fallback.isConnected !== false && !fallback.disabled) {
    fallback.focus()
    return "fallback"
  }
  return "none"
}
