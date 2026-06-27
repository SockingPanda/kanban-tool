import type { EventMeta, EventRecord } from "@/lib/api"

import { affectedQueriesForEvents, nextEventCursor, queryKeysForAffectedEvents } from "./event-invalidation"

export const EVENT_POLL_LIMIT = 100

export type EventPollPlan = {
  nextCursor: number
  seedOnly: boolean
  eventsForCache: EventRecord[]
  queryKeysToInvalidate: readonly (readonly unknown[])[]
}

export function planEventPollResult({
  board,
  currentCursor,
  events,
  meta,
  seeded,
  pollLimit = EVENT_POLL_LIMIT,
}: {
  board: string
  currentCursor: number
  events: EventRecord[]
  meta: EventMeta
  seeded: boolean
  pollLimit?: number
}): EventPollPlan {
  const nextCursor = nextEventCursor(currentCursor, events, meta)
  if (!seeded) {
    return {
      nextCursor,
      seedOnly: true,
      eventsForCache: [],
      queryKeysToInvalidate: [],
    }
  }
  if (!events.length) {
    return {
      nextCursor,
      seedOnly: false,
      eventsForCache: [],
      queryKeysToInvalidate: [],
    }
  }

  const affected = affectedQueriesForEvents(events)
  const queryKeysToInvalidate = [...uniqueQueryKeys(
    queryKeysForAffectedEvents({
      affected,
      board,
    }).filter((queryKey) => queryKey[0] !== "events"),
  )]
  if (events.length >= pollLimit) queryKeysToInvalidate.push(["events", board])

  return {
    nextCursor,
    seedOnly: false,
    eventsForCache: events,
    queryKeysToInvalidate,
  }
}

function uniqueQueryKeys(queryKeys: readonly (readonly unknown[])[]) {
  return Array.from(new Map(queryKeys.map((queryKey) => [JSON.stringify(queryKey), queryKey])).values())
}
