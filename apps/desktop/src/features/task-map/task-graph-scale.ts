const MIN_TASK_GRAPH_SCALE = 0.65
const MAX_TASK_GRAPH_SCALE = 1.5

export function clampTaskGraphScale(scale: number) {
  if (!Number.isFinite(scale)) return 1
  return Math.min(MAX_TASK_GRAPH_SCALE, Math.max(MIN_TASK_GRAPH_SCALE, scale))
}
