import { ChevronDown, CircleDot, FileText, Route } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { useI18n } from "@/i18n"
import type { Run } from "@/lib/api"
import { formatRelativeTime, shortId } from "@/lib/utils"

import type { DetailState } from "./detail-state"
import { InfoRow, Section } from "./task-detail-shared"

export function TaskRunsPanel({
  activeRun,
  detail,
  open,
  onOpenChange,
}: {
  activeRun?: Run
  detail: DetailState
  open: boolean
  onOpenChange: (value: boolean) => void
}) {
  const { t } = useI18n()
  return (
    <Collapsible open={open} onOpenChange={onOpenChange}>
      <Section title={t("Runs")}>
        <CollapsibleTrigger asChild>
          <Button variant="outline" size="sm" className="mb-3">
            <Route className="h-4 w-4" />
            {t(detail.runs.length === 1 ? "{count} run" : "{count} runs", { count: detail.runs.length })}
            <ChevronDown className="h-4 w-4" />
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <RunSummary activeRun={activeRun} detail={detail} />
        </CollapsibleContent>
      </Section>
    </Collapsible>
  )
}

export function TaskEventsPanel({
  events,
  open,
  onOpenChange,
}: {
  events: DetailState["events"]
  open: boolean
  onOpenChange: (value: boolean) => void
}) {
  const { t } = useI18n()
  return (
    <Collapsible open={open} onOpenChange={onOpenChange}>
      <Section title={t("Events")}>
        <CollapsibleTrigger asChild>
          <Button variant="outline" size="sm" className="mb-3">
            <CircleDot className="h-4 w-4" />
            {t(events.length === 1 ? "{count} event" : "{count} events", { count: events.length })}
            <ChevronDown className="h-4 w-4" />
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <EventTimeline events={events} />
        </CollapsibleContent>
      </Section>
    </Collapsible>
  )
}

function RunSummary({ activeRun, detail }: { activeRun?: Run; detail: DetailState }) {
  const { t } = useI18n()
  if (!activeRun) {
    return (
      <Empty className="items-start p-0 text-left">
        <EmptyDescription>{t("No runs yet.")}</EmptyDescription>
      </Empty>
    )
  }

  return (
    <div className="space-y-2 text-sm">
      <InfoRow label={t("run")} value={shortId(activeRun.id)} />
      <InfoRow label={t("status")} value={activeRun.status} />
      <InfoRow label={t("worker")} value={activeRun.worker_profile ?? t("manual")} />
      <InfoRow label={t("owner")} value={activeRun.claim_owner} />
      <InfoRow label={t("started")} value={formatRelativeTime(activeRun.started_at)} />
      <InfoRow label={t("log")} value={activeRun.log_path ?? "-"} />
      {detail.runLog ? (
        <div className="mt-3 rounded-md border border-border bg-terminal-bg p-2 text-xs text-terminal-fg">
          <div className="mb-2 flex items-center justify-between text-terminal-muted-foreground">
            <span className="flex items-center gap-1">
              <FileText className="h-3.5 w-3.5" />
              {t("log")}
            </span>
            {detail.runLog.truncated ? <span>{t("truncated")}</span> : null}
          </div>
          <pre className="whitespace-pre-wrap font-mono leading-relaxed">{detail.runLog.content || t("(empty)")}</pre>
        </div>
      ) : null}
    </div>
  )
}

function EventTimeline({ events }: { events: DetailState["events"] }) {
  const { t } = useI18n()
  return (
    <div className="space-y-2">
      {events.length ? (
        events
          .slice()
          .reverse()
          .map((event) => (
            <div key={event.id} className="grid grid-cols-[auto_1fr] gap-2 text-sm">
              <CircleDot className="mt-0.5 h-4 w-4 text-muted-foreground" />
              <div>
                <div className="font-medium">{event.kind}</div>
                <div className="text-xs text-muted-foreground">
                  {t("{time} by {actor}", { time: formatRelativeTime(event.created_at), actor: event.actor ?? t("system") })}
                </div>
              </div>
            </div>
          ))
      ) : (
        <Empty className="items-start p-0 text-left">
          <EmptyDescription>{t("No events yet.")}</EmptyDescription>
        </Empty>
      )}
    </div>
  )
}
