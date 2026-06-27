export type EventPollerInstrumentation = {
  board: string
  receivedEvents: number
  seedOnly: boolean
  setDataEvents: number
  invalidatedQueryKeys: readonly (readonly unknown[])[]
  durationMs: number
}

export function recordEventPollerInstrumentation({
  enabled,
  logger = console.debug,
  ...entry
}: EventPollerInstrumentation & {
  enabled: boolean
  logger?: (message?: unknown, ...optionalParams: unknown[]) => void
}) {
  if (!enabled) return
  logger("[kanban-desktop:event-poller]", entry)
}
