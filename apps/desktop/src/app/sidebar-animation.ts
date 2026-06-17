export const SIDEBAR_WIDTH_TRANSITION_MS = 200

type SidebarContentEvent =
  | { type: "width-transition-start"; sidebarOpen: boolean }
  | { type: "width-transition-finish"; sidebarOpen: boolean }

export function nextSidebarContentOpen(currentContentOpen: boolean, event: SidebarContentEvent): boolean {
  if (event.type === "width-transition-start") return event.sidebarOpen || currentContentOpen
  return event.sidebarOpen
}

export function isSidebarWidthTransition(propertyName: string): boolean {
  return propertyName === "width"
}
