import { useQuery } from "@tanstack/react-query"
import { Activity, Database, RefreshCcw } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
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
    <ScrollArea className="flex-1 bg-card p-4">
      <Card className="p-4">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-sm font-semibold">
            <Activity className="h-4 w-4 text-muted-foreground" />
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
          healthQuery.isLoading ? <Skeleton className="h-16" /> : <div className="text-sm text-muted-foreground">No health response.</div>
        )}
        {healthQuery.error ? (
          <Alert className="mt-3 border-destructive/50">
            <AlertDescription className="text-destructive">{healthQuery.error.message}</AlertDescription>
          </Alert>
        ) : null}
      </Card>

      <Card className="mt-4 p-4">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
          <Database className="h-4 w-4 text-muted-foreground" />
          Runtime config
        </h2>
        <div className="space-y-2 text-sm">
          <InfoRow label="board" value={config?.board ?? "-"} />
          <InfoRow label="actor" value={config?.actor ?? "-"} />
          <InfoRow label="api" value={config?.apiBaseUrl || "same-origin"} />
          <InfoRow label="db" value={config?.dbPath ?? "-"} />
        </div>
      </Card>
    </ScrollArea>
  )
}

function Metric({ label, value, tone = "secondary" }: { label: string; value: string; tone?: "ready" | "blocked" | "secondary" }) {
  return (
    <Card className="p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1"><Badge variant={tone}>{value}</Badge></div>
    </Card>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Card className="flex justify-between gap-3 px-3 py-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </Card>
  )
}
