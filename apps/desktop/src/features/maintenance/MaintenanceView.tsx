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
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
import type { BoardStats, CheckpointReport, DoctorDerivedStore, DoctorReport, KanbanApi, SearchIndexStatus, StaleClaim } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export function MaintenanceView({ api }: { api: KanbanApi | null }) {
  const statsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.stats(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.stats({ signal })
    },
  })
  const searchStatusQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.searchStatus(api?.board ?? "pending"),
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.searchStatus({ signal })
    },
  })
  const doctorMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error("API client is not ready")
      return api.doctor()
    },
  })
  const checkpointMutation = useMutation({
    mutationFn: () => {
      if (!api) throw new Error("API client is not ready")
      return api.checkpoint()
    },
  })

  return (
    <ScrollArea className="flex-1 bg-card p-4">
      <div className="grid grid-cols-2 gap-4">
        <Panel title="Stats" icon={Activity}>
          <StatsGrid stats={statsQuery.data} />
        </Panel>
        <Panel title="Search status" icon={SearchCheck}>
          <SearchStatus meta={searchStatusQuery.data} />
        </Panel>
        <Panel title="Doctor" icon={Stethoscope}>
          <Button variant="secondary" disabled={!api || doctorMutation.isPending} onClick={() => doctorMutation.mutate()}>
            Run doctor
          </Button>
          {doctorMutation.data ? <DoctorReportView report={doctorMutation.data} /> : null}
          {doctorMutation.error ? <ErrorText error={doctorMutation.error} /> : null}
        </Panel>
        <Panel title="Checkpoint" icon={DatabaseBackup}>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="secondary" disabled={!api || checkpointMutation.isPending}>
                Create checkpoint
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Run WAL checkpoint?</AlertDialogTitle>
                <AlertDialogDescription>Run WAL checkpoint now?</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction onClick={() => checkpointMutation.mutate()}>Continue</AlertDialogAction>
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
  return (
    <Card className="p-4">
      <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
        <Icon className="h-4 w-4 text-muted-foreground" />
        {title}
      </h2>
      {children}
    </Card>
  )
}

function StatsGrid({ stats }: { stats?: BoardStats }) {
  if (!stats) return <Skeleton className="h-24" />
  return (
    <div className="space-y-4 text-sm">
      <div className="grid grid-cols-2 gap-2">
        <Metric label="board" value={stats.board_id} />
        <Metric label="generated" value={String(stats.generated_at)} />
      </div>
      <div>
        <Subheading>Status counts</Subheading>
        <div className="grid grid-cols-3 gap-2">
          {stats.status_counts.map((entry) => (
            <Metric key={entry.status} label={entry.status} value={String(entry.count)} />
          ))}
          {stats.status_counts.length === 0 ? <EmptyText>No status counts returned.</EmptyText> : null}
        </div>
      </div>
      <div>
        <Subheading>Stale claims</Subheading>
        <StaleClaimList claims={stats.stale_claims} />
      </div>
      <div>
        <Subheading>Blocked reasons</Subheading>
        <div className="space-y-1">
          {stats.blocked_reasons.map((entry) => (
            <InfoRow key={entry.reason} label={entry.reason || "unspecified"} value={String(entry.count)} />
          ))}
          {stats.blocked_reasons.length === 0 ? <EmptyText>No blocked reasons returned.</EmptyText> : null}
        </div>
      </div>
    </div>
  )
}

function SearchStatus({ meta }: { meta?: SearchIndexStatus }) {
  if (!meta) return <Skeleton className="h-24" />
  return (
    <div className="space-y-2 text-sm">
      <InfoRow label="backend" value={meta.backend} />
      <InfoRow label="derived index" value={String(meta.derived_index)} />
      <InfoRow label="stale" value={String(meta.stale)} />
      <InfoRow label="index version" value={meta.index_version ?? "-"} />
      <InfoRow label="last event" value={meta.last_event_id === null ? "-" : String(meta.last_event_id)} />
      <InfoRow label="lag events" value={meta.index_lag_events === null ? "-" : String(meta.index_lag_events)} />
      <div className="text-muted-foreground">{meta.message}</div>
    </div>
  )
}

function DoctorReportView({ report }: { report: DoctorReport }) {
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
  ] as const
  return (
    <div className="mt-3 space-y-3 text-sm">
      <Badge variant={report.ok ? "ready" : "blocked"}>{report.ok ? "ok" : "findings"}</Badge>
      <div className="grid grid-cols-2 gap-2">
        {findings.map(([label, value]) => (
          <Metric key={label} label={label} value={String(value)} />
        ))}
      </div>
      <div>
        <Subheading>Derived stores</Subheading>
        <DerivedStoreList stores={report.derived_stores} />
      </div>
    </div>
  )
}

function StaleClaimList({ claims }: { claims: StaleClaim[] }) {
  if (claims.length === 0) return <EmptyText>No stale claims returned.</EmptyText>
  return (
    <div className="space-y-2">
      {claims.map((claim) => (
        <Card key={claim.task_id} className="p-2">
          <div className="flex justify-between gap-3">
            <span className="truncate font-medium">#{claim.seq} {claim.title}</span>
            <span className="shrink-0 text-muted-foreground">{claim.claim_owner ?? "no owner"}</span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>expires {nullableNumber(claim.claim_expires_at)}</span>
            <span>heartbeat {nullableNumber(claim.last_heartbeat_at)}</span>
            <span>run {claim.current_run_id ?? "-"}</span>
            <span>retry {claim.retry_count}/{nullableNumber(claim.max_retries)}</span>
          </div>
        </Card>
      ))}
    </div>
  )
}

function DerivedStoreList({ stores }: { stores: DoctorDerivedStore[] }) {
  if (stores.length === 0) return <EmptyText>No derived stores returned.</EmptyText>
  return (
    <div className="space-y-2">
      {stores.map((store) => (
        <Card key={store.store_name} className="p-2">
          <div className="flex justify-between gap-3">
            <span className="truncate font-medium">{store.store_name}</span>
            <span className={store.dirty || store.last_error ? "shrink-0 text-amber-700" : "shrink-0 text-emerald-700"}>
              {store.dirty ? "dirty" : "clean"}
            </span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
            <span>schema {store.schema_version}</span>
            <span>event {store.last_event_id}</span>
            <span>pending {store.pending_outbox}</span>
            <span>running {store.running_outbox}</span>
            <span>failed {store.failed_outbox}</span>
            <span className="truncate">error {store.last_error ?? "-"}</span>
          </div>
        </Card>
      ))}
    </div>
  )
}

function CheckpointResultView({ result }: { result: CheckpointReport }) {
  return (
    <div className="mt-3 space-y-2 text-sm">
      <Badge variant={result.busy === 0 ? "ready" : "blocked"}>{result.busy === 0 ? "checkpointed" : "busy"}</Badge>
      <InfoRow label="busy" value={String(result.busy)} />
      <InfoRow label="log frames" value={String(result.log_frames)} />
      <InfoRow label="checkpointed frames" value={String(result.checkpointed_frames)} />
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <Card className="p-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="truncate font-medium">{value}</div>
    </Card>
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
    <div className="flex justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}

function ErrorText({ error }: { error: unknown }) {
  return (
    <Alert className="mt-3 border-destructive/50">
      <AlertDescription className="text-destructive">{error instanceof Error ? error.message : String(error)}</AlertDescription>
    </Alert>
  )
}
