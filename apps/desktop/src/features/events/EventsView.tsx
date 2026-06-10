import { useQuery } from "@tanstack/react-query"
import { CircleDot, RefreshCcw } from "lucide-react"

import { Button } from "@/components/ui/button"
import type { EventRecord, KanbanApi } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { formatRelativeTime, shortId } from "@/lib/utils"

export function EventsView({ api }: { api: KanbanApi | null }) {
  const eventsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.events(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.listBoardEvents({ limit: 150, signal })
    },
  })
  const events = eventsQuery.data?.events ?? []

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-white">
      <div className="flex h-10 items-center justify-between border-b border-neutral-200 px-4 text-sm">
        <span className="font-medium">Board events</span>
        <Button variant="ghost" size="sm" disabled={eventsQuery.isFetching} onClick={() => void eventsQuery.refetch()}>
          <RefreshCcw className="h-4 w-4" />
          Refresh
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-4">
        {events.length ? (
          <div className="space-y-2">
            {events.map((event) => <EventRow key={event.id} event={event} />)}
          </div>
        ) : (
          <div className="text-sm text-neutral-500">{eventsQuery.isLoading ? "Loading events." : "No events returned."}</div>
        )}
      </div>
    </div>
  )
}

function EventRow({ event }: { event: EventRecord }) {
  return (
    <div className="grid grid-cols-[auto_1fr_auto] gap-3 rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-sm">
      <CircleDot className="mt-0.5 h-4 w-4 text-neutral-400" />
      <div className="min-w-0">
        <div className="font-medium">{event.kind}</div>
        <div className="truncate text-xs text-neutral-500">
          {event.task_id ? shortId(event.task_id) : "board"} {event.run_id ? ` · ${shortId(event.run_id)}` : ""}
        </div>
      </div>
      <div className="text-right text-xs text-neutral-500">
        <div>{formatRelativeTime(event.created_at)}</div>
        <div>{event.actor ?? "system"}</div>
      </div>
    </div>
  )
}
