import type { TaskStatus } from "@/lib/api"

import type { TaskGraphEdgeKind } from "./task-graph-types"

const statusNodeClasses: Record<TaskStatus, string> = {
  triage: "border-neutral-300 bg-neutral-50 text-neutral-950 dark:border-neutral-800 dark:bg-neutral-950/45 dark:text-neutral-100",
  todo: "border-stone-300 bg-stone-50 text-stone-950 dark:border-stone-800 dark:bg-stone-950/45 dark:text-stone-100",
  scheduled: "border-indigo-300 bg-indigo-50 text-indigo-950 dark:border-indigo-900 dark:bg-indigo-950/45 dark:text-indigo-100",
  ready: "border-emerald-300 bg-emerald-50 text-emerald-950 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-100",
  running: "border-sky-300 bg-sky-50 text-sky-950 dark:border-sky-900 dark:bg-sky-950/45 dark:text-sky-100",
  blocked: "border-red-300 bg-red-50 text-red-950 dark:border-red-900 dark:bg-red-950/45 dark:text-red-100",
  review: "border-amber-300 bg-amber-50 text-amber-950 dark:border-amber-900 dark:bg-amber-950/45 dark:text-amber-100",
  done: "border-lime-300 bg-lime-50 text-lime-950 dark:border-lime-900 dark:bg-lime-950/40 dark:text-lime-100",
  archived: "border-neutral-200 bg-neutral-100 text-neutral-500 dark:border-neutral-800 dark:bg-neutral-950/30 dark:text-neutral-400",
}

const miniMapStatusColors: Record<TaskStatus, string> = {
  triage: "#737373",
  todo: "#78716c",
  scheduled: "#6366f1",
  ready: "#059669",
  running: "#0284c7",
  blocked: "#dc2626",
  review: "#d97706",
  done: "#65a30d",
  archived: "#9ca3af",
}

export function graphNodeStatusClass(status: TaskStatus | undefined, selected: boolean, contextOnly: boolean | undefined) {
  return [
    status ? statusNodeClasses[status] : "border-border bg-card text-card-foreground",
    contextOnly ? "border-dashed opacity-75 saturate-75" : "",
    selected ? "shadow-sm ring-2 ring-ring ring-offset-1 ring-offset-background" : "",
  ].filter(Boolean).join(" ")
}

export function graphNodeStatusMiniMapColor(status: TaskStatus | undefined) {
  if (!status) return "#64748b"
  return miniMapStatusColors[status]
}

export function graphEdgeClass(kind: TaskGraphEdgeKind, blocking?: boolean) {
  if (kind === "step") return "stroke-violet-500"
  return blocking ? "stroke-red-500" : "stroke-emerald-500"
}
