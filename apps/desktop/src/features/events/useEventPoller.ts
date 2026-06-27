import { useEffect, useRef } from "react"
import { useQueryClient } from "@tanstack/react-query"

import type { EventPage, KanbanApi } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

import { mergeBoardEventPage } from "./event-cache"
import { recordEventPollerInstrumentation } from "./event-poller-instrumentation"
import { EVENT_POLL_LIMIT, planEventPollResult } from "./event-polling"

export function useEventPoller({
  api,
  enabled,
  onError,
}: {
  api: KanbanApi | null
  enabled: boolean
  onError: (error: unknown) => void
}) {
  const queryClient = useQueryClient()
  const cursorRef = useRef(0)
  const seededRef = useRef(false)

  useEffect(() => {
    cursorRef.current = 0
    seededRef.current = false
  }, [api])

  useEffect(() => {
    if (!api || !enabled) return

    let stopped = false
    let inFlight: AbortController | null = null

    const poll = async () => {
      if (inFlight) return
      const controller = new AbortController()
      inFlight = controller
      const startedAt = performance.now()
      try {
        const page = await api.listBoardEvents({ after: cursorRef.current, limit: EVENT_POLL_LIMIT, signal: controller.signal })
        if (stopped) return

        const plan = planEventPollResult({
          board: api.board,
          currentCursor: cursorRef.current,
          events: page.events,
          meta: page.meta,
          seeded: seededRef.current,
        })
        cursorRef.current = plan.nextCursor
        if (!seededRef.current) seededRef.current = true

        if (plan.eventsForCache.length) {
          queryClient.setQueryData<EventPage | undefined>(queryKeys.events(api.board), (current) =>
            mergeBoardEventPage(current, plan.eventsForCache),
          )
        }
        await Promise.all(plan.queryKeysToInvalidate.map((queryKey) => queryClient.invalidateQueries({ queryKey })))
        recordEventPollerInstrumentation({
          enabled: import.meta.env.DEV,
          board: api.board,
          receivedEvents: page.events.length,
          seedOnly: plan.seedOnly,
          setDataEvents: plan.eventsForCache.length,
          invalidatedQueryKeys: plan.queryKeysToInvalidate,
          durationMs: performance.now() - startedAt,
        })
      } catch (error) {
        if (!controller.signal.aborted) onError(error)
      } finally {
        inFlight = null
      }
    }

    const interval = window.setInterval(() => {
      void poll()
    }, 5_000)
    void poll()

    return () => {
      stopped = true
      inFlight?.abort()
      window.clearInterval(interval)
    }
  }, [api, enabled, onError, queryClient])
}
