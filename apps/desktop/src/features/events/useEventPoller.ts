import { useEffect, useRef } from "react"
import { useQueryClient } from "@tanstack/react-query"

import type { KanbanApi } from "@/lib/api"

import { affectedQueriesForEvents, nextEventCursor, queryKeysForAffectedEvents } from "./event-invalidation"

export function useEventPoller({
  api,
  enabled,
  selectedTaskId,
  onError,
}: {
  api: KanbanApi | null
  enabled: boolean
  selectedTaskId: string | null
  onError: (error: unknown) => void
}) {
  const queryClient = useQueryClient()
  const cursorRef = useRef(0)
  const selectedTaskIdRef = useRef(selectedTaskId)

  useEffect(() => {
    selectedTaskIdRef.current = selectedTaskId
  }, [selectedTaskId])

  useEffect(() => {
    cursorRef.current = 0
  }, [api])

  useEffect(() => {
    if (!api || !enabled) return

    let stopped = false
    let inFlight: AbortController | null = null

    const poll = async () => {
      if (inFlight) return
      const controller = new AbortController()
      inFlight = controller
      try {
        const page = await api.listEventsAfter(cursorRef.current, { signal: controller.signal })
        if (stopped || !page.events.length) return

        cursorRef.current = nextEventCursor(cursorRef.current, page.events, page.meta)
        const affected = affectedQueriesForEvents(page.events)
        const queryKeysToInvalidate = queryKeysForAffectedEvents({
          affected,
          board: api.board,
          selectedTaskId: selectedTaskIdRef.current,
        })
        for (const queryKey of queryKeysToInvalidate) {
          await queryClient.invalidateQueries({ queryKey })
        }
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
