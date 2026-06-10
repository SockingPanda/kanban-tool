import type { BoardColumn, TaskStatus } from "@/lib/api"

export const fallbackColumns: BoardColumn[] = [
  boardColumn("triage", "Triage", 10),
  boardColumn("todo", "Todo", 20),
  boardColumn("scheduled", "Scheduled", 30),
  boardColumn("ready", "Ready", 40),
  boardColumn("running", "Running", 50),
  boardColumn("blocked", "Blocked", 60),
  boardColumn("review", "Review", 70),
  boardColumn("done", "Done", 80),
]

export const columnHints: Record<TaskStatus, string> = {
  triage: "needs spec",
  todo: "waiting deps",
  scheduled: "future work",
  ready: "claimable",
  running: "active runs",
  blocked: "needs input",
  review: "manual check",
  done: "finished",
  archived: "hidden",
}

export const filterStatuses: TaskStatus[] = [
  "triage",
  "todo",
  "scheduled",
  "ready",
  "running",
  "blocked",
  "review",
  "done",
]

export const statusAccent: Record<TaskStatus, string> = {
  triage: "bg-neutral-400",
  todo: "bg-stone-500",
  scheduled: "bg-indigo-500",
  ready: "bg-emerald-500",
  running: "bg-sky-500",
  blocked: "bg-red-500",
  review: "bg-amber-500",
  done: "bg-lime-600",
  archived: "bg-neutral-300",
}

function boardColumn(status: TaskStatus, title: string, position: number): BoardColumn {
  return {
    id: `col_${status}`,
    board_id: "b_local",
    status,
    title,
    position,
    hidden: false,
    wip_limit: null,
    created_at: 0,
    updated_at: 0,
  }
}
