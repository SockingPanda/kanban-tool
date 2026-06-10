import { useEffect, useRef } from "react"
import { useQueryClient } from "@tanstack/react-query"

import type { KanbanApi } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

import { affectedQueriesForEvents, nextEventCursor } from "./event-invalidation"

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
        if (affected.invalidateBoardTasks) {
          await queryClient.invalidateQueries({ queryKey: queryKeys.boardTasksRoot(api.board) })
        }
        for (const taskId of affected.taskIds) {
          await queryClient.invalidateQueries({ queryKey: queryKeys.taskDetail(taskId) })
        }
        const currentSelectedTaskId = selectedTaskIdRef.current
        if (currentSelectedTaskId && !affected.taskIds.has(currentSelectedTaskId) && affected.invalidateEvents) {
          await queryClient.invalidateQueries({ queryKey: queryKeys.taskDetail(currentSelectedTaskId) })
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
