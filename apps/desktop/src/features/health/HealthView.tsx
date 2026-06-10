import { useQuery } from "@tanstack/react-query"
import { Activity, Database, RefreshCcw } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { KanbanApi, RuntimeConfig } from "@/lib/api"

export function HealthView({ api, config }: { api: KanbanApi | null; config: RuntimeConfig | null }) {
  const healthQuery = useQuery({
    enabled: Boolean(api),
    queryKey: ["health", api?.board ?? "pending"],
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.health({ signal })
    },
  })

  return (
    <div className="min-h-0 flex-1 overflow-auto bg-white p-4">
      <section className="rounded-md border border-neutral-200 bg-neutral-50 p-4">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-sm font-semibold">
            <Activity className="h-4 w-4 text-neutral-500" />
            Runtime health
          </h2>
          <Button variant="ghost" size="sm" disabled={healthQuery.isFetching} onClick={() => void healthQuery.refetch()}>
            <RefreshCcw className="h-4 w-4" />
            Refresh
          </Button>
        </div>
        {healthQuery.data ? (
          <div className="grid grid-cols-3 gap-3 text-sm">
            <Metric label="ok" value={String(healthQuery.data.ok)} tone={healthQuery.data.ok ? "ready" : "blocked"} />
            <Metric label="db" value={healthQuery.data.db} tone={healthQuery.data.db === "ok" ? "ready" : "blocked"} />
            <Metric label="version" value={healthQuery.data.version} />
          </div>
        ) : (
          <div className="text-sm text-neutral-500">{healthQuery.isLoading ? "Loading health." : "No health response."}</div>
        )}
        {healthQuery.error ? <div className="mt-3 text-sm text-red-700">{healthQuery.error.message}</div> : null}
      </section>

      <section className="mt-4 rounded-md border border-neutral-200 bg-neutral-50 p-4">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
          <Database className="h-4 w-4 text-neutral-500" />
          Runtime config
        </h2>
        <div className="space-y-2 text-sm">
          <InfoRow label="board" value={config?.board ?? "-"} />
          <InfoRow label="actor" value={config?.actor ?? "-"} />
          <InfoRow label="api" value={config?.apiBaseUrl || "same-origin"} />
          <InfoRow label="db" value={config?.dbPath ?? "-"} />
        </div>
      </section>
    </div>
  )
}

function Metric({ label, value, tone = "secondary" }: { label: string; value: string; tone?: "ready" | "blocked" | "secondary" }) {
  return (
    <div className="rounded border border-neutral-200 bg-white p-3">
      <div className="text-xs text-neutral-500">{label}</div>
      <div className="mt-1"><Badge variant={tone}>{value}</Badge></div>
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3 rounded border border-neutral-200 bg-white px-3 py-2">
      <span className="text-neutral-500">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}
