export type OperatorView = "board" | "list" | "map" | "events" | "runs" | "signals" | "ontology" | "maintenance" | "health" | "settings"

export type NavigableOperatorView = Exclude<OperatorView, "map" | "signals" | "ontology" | "maintenance">

export const primaryViews: NavigableOperatorView[] = ["board", "list", "events"]

export const sidebarViews: NavigableOperatorView[] = [
  "board",
  "list",
  "runs",
  "events",
  "health",
  "settings",
]
