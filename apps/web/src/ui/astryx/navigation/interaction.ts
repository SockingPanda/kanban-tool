import type { MouseEvent } from "react"

/** Only an unmodified primary click may enter an SPA navigation callback. */
export function isPrimaryNavigationClick(
  event: Pick<MouseEvent<HTMLElement>, "button" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey" | "defaultPrevented">,
): boolean {
  return !event.defaultPrevented && event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
}
