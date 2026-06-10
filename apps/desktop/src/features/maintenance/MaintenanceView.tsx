import { useMutation, useQuery } from "@tanstack/react-query"
import { Activity, DatabaseBackup, SearchCheck, Stethoscope } from "lucide-react"
import type { ElementType, ReactNode } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { BoardStats, DoctorIssue, KanbanApi, SearchMeta } from "@/lib/api"

export function MaintenanceView({ api }: { api: KanbanApi | null }) {
  const statsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: ["stats", api?.board ?? "pending"],
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.stats({ signal })
    },
  })
  const searchStatusQuery = useQuery({
    enabled: Boolean(api),
    queryKey: ["search-status", api?.board ?? "pending"],
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
    <div className="min-h-0 flex-1 overflow-auto bg-white p-4">
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
          {doctorMutation.data ? (
            <div className="mt-3 space-y-2">
              <Badge variant={doctorMutation.data.ok ? "ready" : "blocked"}>{doctorMutation.data.ok ? "ok" : "issues"}</Badge>
              <IssueList issues={[...(doctorMutation.data.issues ?? []), ...(doctorMutation.data.warnings ?? [])]} />
            </div>
          ) : null}
          {doctorMutation.error ? <ErrorText error={doctorMutation.error} /> : null}
        </Panel>
        <Panel title="Checkpoint" icon={DatabaseBackup}>
          <Button variant="secondary" disabled={!api || checkpointMutation.isPending} onClick={() => checkpointMutation.mutate()}>
            Create checkpoint
          </Button>
          {checkpointMutation.data ? (
            <div className="mt-3 space-y-1 text-sm">
              <div>status {checkpointMutation.data.ok ? "ok" : "failed"}</div>
              <div className="truncate text-neutral-500">{checkpointMutation.data.path ?? "no path returned"}</div>
            </div>
          ) : null}
          {checkpointMutation.error ? <ErrorText error={checkpointMutation.error} /> : null}
        </Panel>
      </div>
    </div>
  )
}

function Panel({ title, icon: Icon, children }: { title: string; icon: ElementType; children: ReactNode }) {
  return (
    <section className="rounded-md border border-neutral-200 bg-neutral-50 p-4">
      <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
        <Icon className="h-4 w-4 text-neutral-500" />
        {title}
      </h2>
      {children}
    </section>
  )
}

function StatsGrid({ stats }: { stats?: BoardStats }) {
  if (!stats) return <div className="text-sm text-neutral-500">Loading stats.</div>
  const entries = Object.entries(stats).filter(([, value]) => typeof value === "number" || typeof value === "string" || typeof value === "boolean")
  return (
    <div className="grid grid-cols-2 gap-2 text-sm">
      {entries.map(([key, value]) => (
        <div key={key} className="rounded border border-neutral-200 bg-white p-2">
          <div className="text-xs text-neutral-500">{key}</div>
          <div className="truncate font-medium">{String(value)}</div>
        </div>
      ))}
    </div>
  )
}

function SearchStatus({ meta }: { meta?: SearchMeta }) {
  if (!meta) return <div className="text-sm text-neutral-500">Loading search status.</div>
  return (
    <div className="space-y-2 text-sm">
      <InfoRow label="backend" value={meta.backend} />
      <InfoRow label="stale" value={String(meta.stale)} />
      <InfoRow label="index version" value={meta.index_version ?? "-"} />
      <InfoRow label="last event" value={meta.last_event_id === null ? "-" : String(meta.last_event_id)} />
      <InfoRow label="lag events" value={meta.index_lag_events === null ? "-" : String(meta.index_lag_events)} />
    </div>
  )
}

function IssueList({ issues }: { issues: DoctorIssue[] }) {
  if (!issues.length) return <div className="text-sm text-neutral-500">No issues returned.</div>
  return (
    <div className="space-y-1">
      {issues.map((issue, index) => (
        <div key={`${issue.code}-${index}`} className="rounded border border-neutral-200 bg-white p-2 text-sm">
          <div className="font-medium">{issue.code}</div>
          <div className="text-neutral-600">{issue.message}</div>
        </div>
      ))}
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-neutral-500">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}

function ErrorText({ error }: { error: unknown }) {
  return <div className="mt-3 text-sm text-red-700">{error instanceof Error ? error.message : String(error)}</div>
}
