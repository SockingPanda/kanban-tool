/** Window event used to connect host maintenance writes to the rendered health query. */
export const HEALTH_REFRESH_EVENT = "kanban:health-refresh"

/**
 * Request a health read without coupling maintenance to the HealthPage implementation.
 * The SSR guard keeps mutation seams safe in static-rendered tests and pre-rendering.
 */
export function requestHealthRefresh(): void {
  if (typeof window !== "undefined") window.dispatchEvent(new Event(HEALTH_REFRESH_EVENT))
}
