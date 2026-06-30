import { useMutation, useQuery } from "@tanstack/react-query"
import { Activity, DatabaseBackup, SearchCheck, Stethoscope } from "lucide-react"
import type { ElementType, ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Alert, AlertDescription } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Card } from "@/components/ui/card"
import { MetricStrip, SectionCard } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
import { useI18n } from "@/i18n"
import type { BoardStats, CheckpointReport, DoctorDerivedStore, DoctorReport, KanbanApi, SearchIndexStatus, StaleClaim } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function MaintenanceView({ api }: { api: KanbanApi | null }) {
  const { t } = useI18n()
  const statsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.stats(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.stats({ signal })
    },
  })
  const searchStatusQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.searchStatus(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.searchStatus({ signal })
    },
  })
  const doctorMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.doctor()
    },
  })
  const checkpointMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.checkpoint()
    },
  })

  return (
    <ScrollArea className="flex-1 bg-card p-4">
      <div className="grid grid-cols-2 gap-4">
        <Panel title={t("Stats")} icon={Activity}>
          <StatsGrid stats={statsQuery.data} />
        </Panel>
        <Panel title={t("Search status")} icon={SearchCheck}>
          <SearchStatus meta={searchStatusQuery.data} />
        </Panel>
        <Panel title={t("Doctor")} icon={Stethoscope}>
          <Button variant="secondary" disabled={!api || doctorMutation.isPending} onClick={() => doctorMutation.mutate()}>
            {t("Run doctor")}
          </Button>
          {doctorMutation.data ? <DoctorReportView report={doctorMutation.data} /> : null}
          {doctorMutation.error ? <ErrorText error={doctorMutation.error} /> : null}
        </Panel>
        <Panel title={t("Checkpoint")} icon={DatabaseBackup}>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="secondary" disabled={!api || checkpointMutation.isPending}>
                {t("Create checkpoint")}
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{t("Run WAL checkpoint?")}</AlertDialogTitle>
                <AlertDialogDescription>{t("Run WAL checkpoint now?")}</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
                <AlertDialogAction onClick={() => checkpointMutation.mutate()}>{t("Continue")}</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
          {checkpointMutation.data ? <CheckpointResultView result={checkpointMutation.data} /> : null}
          {checkpointMutation.error ? <ErrorText error={checkpointMutation.error} /> : null}
        </Panel>
      </div>
    </ScrollArea>
  )
}

function Panel({ title, icon: Icon, children }: { title: string; icon: ElementType; children: ReactNode }) {
  return <SectionCard title={title} icon={Icon}>{children}</SectionCard>
}

function StatsGrid({ stats }: { stats?: BoardStats }) {
  const { t } = useI18n()
  if (!stats) return <Skeleton className="h-24" />
  return (
    <div className="space-y-4 text-sm">
      <div className="grid grid-cols-2 gap-2">
        <MetricStrip
          className="contents"
          items={[
            { id: "board", label: t("board"), value: stats.board_id },
            { id: "generated", label: t("generated"), value: String(stats.generated_at) },
          ]}
        />
      </div>
      <div>
        <Subheading>{t("Status counts")}</Subheading>
        <div className="grid grid-cols-3 gap-2">
          <MetricStrip
            className="contents"
            items={stats.status_counts.map((entry) => ({ id: `status-${entry.status}`, label: entry.status, value: String(entry.count) }))}
          />
          {stats.status_counts.length === 0 ? <EmptyText>{t("No status counts returned.")}</EmptyText> : null}
        </div>
      </div>
      <div>
        <Subheading>{t("Stale claims")}</Subheading>
        <StaleClaimList claims={stats.stale_claims} />
      </div>
      <div>
        <Subheading>{t("Blocked reasons")}</Subheading>
        <div className="space-y-1">
          {stats.blocked_reasons.map((entry) => (
            <InfoRow key={entry.reason} label={entry.reason || t("unspecified")} value={String(entry.count)} />
          ))}
          {stats.blocked_reasons.length === 0 ? <EmptyText>{t("No blocked reasons returned.")}</EmptyText> : null}
        </div>
      </div>
    </div>
  )
}

function SearchStatus({ meta }: { meta?: SearchIndexStatus }) {
  const { t } = useI18n()
  if (!meta) return <Skeleton className="h-24" />
  return (
    <div className="space-y-2 text-sm">
      <InfoRow label={t("backend")} value={meta.backend} />
      <InfoRow label={t("derived index")} value={String(meta.derived_index)} />
      <InfoRow label={t("stale")} value={String(meta.stale)} />
      <InfoRow label={t("index version")} value={meta.index_version ?? "-"} />
      <InfoRow label={t("last event")} value={meta.last_event_id === null ? "-" : String(meta.last_event_id)} />
      <InfoRow label={t("lag events")} value={meta.index_lag_events === null ? "-" : String(meta.index_lag_events)} />
      <div className="text-muted-foreground">{meta.message}</div>
    </div>
  )
}

function DoctorReportView({ report }: { report: DoctorReport }) {
  const { t } = useI18n()
  const findings = [
    ["integrity", report.integrity_check],
    ["migration version", nullableNumber(report.migration_version)],
    ["user version", report.user_version],
    ["expired running", report.expired_running_tasks],
    ["running without run", report.running_tasks_without_active_run],
    ["orphan running runs", report.orphan_running_runs],
    ["dependency cycles", report.dependency_cycles],
    ["archived dependency edges", report.archived_dependency_edges],
    ["missing run logs", report.missing_run_logs],
    ["suspicious run logs", report.suspicious_run_log_paths],
    ["dependency violations", report.executable_dependency_violations],
    ["spec violations", report.executable_spec_violations],
    ["schedule violations", report.executable_schedule_violations],
    ["outbox pending", report.outbox_pending],
    ["outbox running", report.outbox_running],
    ["outbox failed", report.outbox_failed],
    ["dirty stores", report.derived_dirty_stores],
    ["error stores", report.derived_error_stores],
    ["consistency errors", report.consistency_errors],
    ["consistency warnings", report.consistency_warnings],
    ["ontology errors", report.ontology_ledger_errors],
    ["ontology warnings", report.ontology_ledger_warnings],
  ] as const
  return (
    <div className="mt-3 space-y-3 text-sm">
      <Badge variant={report.ok ? "ready" : "blocked"}>{report.ok ? t("ok") : t("findings")}</Badge>
      <div className="grid grid-cols-2 gap-2">
        <MetricStrip
          className="contents"
          items={findings.map(([label, value]) => ({ id: label.replace(/ /g, "-"), label: t(label), value: String(value) }))}
        />
      </div>
      <div>
        <Subheading>{t("Derived stores")}</Subheading>
        <DerivedStoreList stores={report.derived_stores} />
      </div>
    </div>
  )
}

function StaleClaimList({ claims }: { claims: StaleClaim[] }) {
  const { t } = useI18n()
  if (claims.length === 0) return <EmptyText>{t("No stale claims returned.")}</EmptyText>
  return (
    <div className="space-y-2">
      {claims.map((claim) => (
        <Card key={claim.task_id} className="p-2">
          <div className="flex justify-between gap-3">
            <span className="truncate font-medium">#{claim.seq} {claim.title}</span>
            <span className="shrink-0 text-muted-foreground">{claim.claim_owner ?? t("no owner")}</span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>{t("expires {value}", { value: nullableNumber(claim.claim_expires_at) })}</span>
            <span>{t("heartbeat {value}", { value: nullableNumber(claim.last_heartbeat_at) })}</span>
            <span>{t("run {value}", { value: claim.current_run_id ?? "-" })}</span>
            <span>{t("retry {current}/{max}", { current: claim.retry_count, max: nullableNumber(claim.max_retries) })}</span>
          </div>
        </Card>
      ))}
    </div>
  )
}

function DerivedStoreList({ stores }: { stores: DoctorDerivedStore[] }) {
  const { t } = useI18n()
  if (stores.length === 0) return <EmptyText>{t("No derived stores returned.")}</EmptyText>
  return (
    <div className="space-y-2">
      {stores.map((store) => (
        <Card key={store.store_name} className="p-2">
          <div className="flex justify-between gap-3">
            <span className="truncate font-medium">{store.store_name}</span>
            <span className={store.dirty || store.last_error ? "shrink-0 text-amber-700" : "shrink-0 text-emerald-700"}>
              {store.dirty ? t("dirty") : t("clean")}
            </span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>{t("schema {value}", { value: store.schema_version })}</span>
            <span>{t("event {value}", { value: store.last_event_id })}</span>
            <span>{t("pending {value}", { value: store.pending_outbox })}</span>
            <span>{t("running {value}", { value: store.running_outbox })}</span>
            <span>{t("failed {value}", { value: store.failed_outbox })}</span>
            <span className="truncate">{t("error {value}", { value: store.last_error ?? "-" })}</span>
          </div>
        </Card>
      ))}
    </div>
  )
}

function CheckpointResultView({ result }: { result: CheckpointReport }) {
  const { t } = useI18n()
  return (
    <div className="mt-3 space-y-2 text-sm">
      <Badge variant={result.busy === 0 ? "ready" : "blocked"}>{result.busy === 0 ? t("checkpointed") : t("busy")}</Badge>
      <InfoRow label={t("busy")} value={String(result.busy)} />
      <InfoRow label={t("log frames")} value={String(result.log_frames)} />
      <InfoRow label={t("checkpointed frames")} value={String(result.checkpointed_frames)} />
    </div>
  )
}

function Subheading({ children }: { children: ReactNode }) {
  return <div className="mb-2 text-xs font-semibold uppercase text-muted-foreground">{children}</div>
}

function EmptyText({ children }: { children: ReactNode }) {
  return (
    <Empty className="items-start p-0 text-left">
      <EmptyDescription>{children}</EmptyDescription>
    </Empty>
  )
}

function nullableNumber(value: number | null) {
  return value === null ? "-" : String(value)
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Item className="px-0 py-0">
      <ItemContent>
        <ItemTitle className="text-sm font-normal text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        <span className="truncate font-medium">{value}</span>
      </ItemActions>
    </Item>
  )
}

function ErrorText({ error }: { error: unknown }) {
  return (
    <Alert className="mt-3 border-destructive/50">
      <AlertDescription className="text-destructive">{error instanceof Error ? error.message : String(error)}</AlertDescription>
    </Alert>
  )
}
