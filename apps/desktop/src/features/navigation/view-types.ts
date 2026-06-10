export type OperatorView = "board" | "list" | "events" | "runs" | "maintenance" | "health" | "settings"

export const primaryViews: OperatorView[] = ["board", "list", "events"]

export const sidebarViews: OperatorView[] = [
  "board",
  "list",
  "runs",
  "events",
  "maintenance",
  "health",
  "settings",
]
