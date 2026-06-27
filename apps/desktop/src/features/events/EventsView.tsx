import { useQuery } from "@tanstack/react-query"
import { CircleDot, RefreshCcw } from "lucide-react"
import { memo } from "react"

import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { PageToolbar } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
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
    <div className="flex min-h-0 flex-1 flex-col bg-card">
      <PageToolbar title="Board events" className="py-2" meta={
        <Button variant="ghost" size="sm" disabled={eventsQuery.isFetching} onClick={() => void eventsQuery.refetch()}>
          <RefreshCcw className="h-4 w-4" />
          Refresh
        </Button>
      } />
      <ScrollArea className="flex-1 p-4">
        {events.length ? (
          <div className="space-y-2">
            {events.map((event) => <EventRow key={event.id} event={event} />)}
          </div>
        ) : eventsQuery.isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        ) : (
          <Empty>
            <EmptyDescription>No events returned.</EmptyDescription>
          </Empty>
        )}
      </ScrollArea>
    </div>
  )
}

const EventRow = memo(function EventRow({ event }: { event: EventRecord }) {
  return (
    <Card className="grid grid-cols-[auto_1fr_auto] gap-3 px-3 py-2 text-sm">
      <CircleDot className="mt-0.5 h-4 w-4 text-muted-foreground" />
      <div className="min-w-0">
        <div className="font-medium">{event.kind}</div>
        <div className="truncate text-xs text-muted-foreground">
          {event.task_id ? shortId(event.task_id) : "board"} {event.run_id ? ` · ${shortId(event.run_id)}` : ""}
        </div>
      </div>
      <div className="text-right text-xs text-muted-foreground">
        <div>{formatRelativeTime(event.created_at)}</div>
        <div>{event.actor ?? "system"}</div>
      </div>
    </Card>
  )
})
