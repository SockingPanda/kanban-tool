export function healthMetricTone(ok: boolean): "ready" | "degraded" {
  return ok ? "ready" : "degraded"
}
