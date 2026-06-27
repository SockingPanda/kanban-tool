import type { EventPage, EventRecord } from "@/lib/api"

export const BOARD_EVENTS_LIMIT = 150

export function mergeBoardEventPage(current: EventPage | undefined, incoming: EventRecord[], limit = BOARD_EVENTS_LIMIT): EventPage {
  const byId = new Map<number, EventRecord>()
  for (const event of current?.events ?? []) byId.set(event.id, event)
  for (const event of incoming) byId.set(event.id, event)

  return {
    events: Array.from(byId.values())
      .sort((left, right) => right.id - left.id)
      .slice(0, limit),
    meta: current?.meta ?? {},
  }
}
