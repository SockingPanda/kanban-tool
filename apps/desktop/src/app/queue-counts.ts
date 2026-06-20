import type { BoardStats, Task } from "@/lib/api"

export type QueueCounts = { ready: number; running: number; blocked: number }

export function queueCountsFromTasks(tasks: Pick<Task, "status">[]): QueueCounts {
  return {
    ready: tasks.filter((task) => task.status === "ready").length,
    running: tasks.filter((task) => task.status === "running").length,
    blocked: tasks.filter((task) => task.status === "blocked").length,
  }
}

export function queueCountsFromStats(statusCounts: BoardStats["status_counts"] | undefined, fallback: QueueCounts): QueueCounts {
  if (!statusCounts) return fallback
  const counts = new Map(statusCounts.map((entry) => [entry.status, entry.count]))
  return {
    ready: counts.get("ready") ?? 0,
    running: counts.get("running") ?? 0,
    blocked: counts.get("blocked") ?? 0,
  }
}
