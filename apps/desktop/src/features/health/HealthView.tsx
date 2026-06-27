import { keepPreviousData, useQuery } from "@tanstack/react-query"
import { Activity, Database, RefreshCcw } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { MetricStrip, SectionCard } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
import type { HealthStatus, KanbanApi, RuntimeConfig } from "@/lib/api"

type MetricTone = "ready" | "blocked" | "secondary"

type HealthMetric = {
  id: string
  label: string
  value: string
  tone: MetricTone
}

type RuntimeWarning = {
  title: string
  message: string
}

export function buildHealthRuntimeModel(health: HealthStatus, config: RuntimeConfig | null) {
  const metrics: HealthMetric[] = [
    { id: "ok", label: "ok", value: String(health.ok), tone: health.ok ? "ready" : "blocked" },
    { id: "db", label: "db", value: health.db, tone: health.db === "ok" ? "ready" : "blocked" },
    { id: "version", label: "version", value: health.version, tone: "secondary" },
    { id: "db-path", label: "db_path", value: reportedValue(health.db_path), tone: "secondary" },
    { id: "db-fingerprint", label: "db_fingerprint", value: reportedValue(health.db_fingerprint), tone: "secondary" },
  ]

  return {
    metrics,
    warning: runtimeMismatchWarning(health.db_path, config?.dbPath),
  }
}

export function HealthView({ api, config }: { api: KanbanApi | null; config: RuntimeConfig | null }) {
  const healthQuery = useQuery({
    enabled: Boolean(api),
    queryKey: ["health", config?.apiBaseUrl ?? "pending", config?.dbPath ?? "pending"],
    queryFn: ({ signal }) => {
      if (!api) throw new Error("API client is not ready")
      return api.health({ signal })
    },
    placeholderData: keepPreviousData,
  })

  const runtimeModel = healthQuery.data ? buildHealthRuntimeModel(healthQuery.data, config) : null

  return (
    <ScrollArea className="flex-1 bg-card p-4">
      <SectionCard
        title="Runtime health"
        icon={Activity}
        actions={
          <Button variant="ghost" size="sm" disabled={healthQuery.isFetching} onClick={() => void healthQuery.refetch()}>
            <RefreshCcw className="h-4 w-4" />
            Refresh
          </Button>
        }
      >
        {healthQuery.data ? (
          <MetricStrip
            className="grid gap-3 text-sm md:grid-cols-3 xl:grid-cols-5"
            itemClassName="p-3"
            items={runtimeModel?.metrics ?? []}
          />
        ) : (
          healthQuery.isLoading ? <Skeleton className="h-16" /> : (
            <Empty className="p-0">
              <EmptyDescription>No health response.</EmptyDescription>
            </Empty>
          )
        )}
        {runtimeModel?.warning ? (
          <Alert className="mt-3 border-destructive/50">
            <AlertTitle className="text-destructive">{runtimeModel.warning.title}</AlertTitle>
            <AlertDescription className="text-destructive">{runtimeModel.warning.message}</AlertDescription>
          </Alert>
        ) : null}
        {healthQuery.error ? (
          <Alert className="mt-3 border-destructive/50">
            <AlertDescription className="text-destructive">{healthQuery.error.message}</AlertDescription>
          </Alert>
        ) : null}
      </SectionCard>

      <SectionCard title="Runtime config" icon={Database} className="mt-4">
        <div className="space-y-2 text-sm">
          <InfoRow label="board" value={config?.board ?? "-"} />
          <InfoRow label="actor" value={config?.actor ?? "-"} />
          <InfoRow label="api" value={config?.apiBaseUrl || "same-origin"} />
          <InfoRow label="db" value={config?.dbPath ?? "-"} />
        </div>
      </SectionCard>
    </ScrollArea>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Item className="border-border bg-card px-3 py-2">
      <ItemContent>
        <ItemTitle className="text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        <span className="truncate font-medium">{value}</span>
      </ItemActions>
    </Item>
  )
}

function reportedValue(value: string | undefined) {
  const trimmed = value?.trim()
  return trimmed || "not reported"
}

function runtimeMismatchWarning(healthDbPath: string | undefined, configDbPath: string | undefined): RuntimeWarning | null {
  const healthPath = normalizedConcretePath(healthDbPath)
  const configPath = normalizedConcretePath(configDbPath)
  if (!healthPath || !configPath || healthPath === configPath) return null

  return {
    title: "Runtime database mismatch",
    message:
      `Health is responding from ${healthPath}, but the desktop runtime is configured for ${configPath}. ` +
      "Restart kanban serve, check that this window is using the intended port, and verify VITE_KB_API_BASE_URL / VITE_KB_DEV_PROXY_TARGET.",
  }
}

function normalizedConcretePath(path: string | undefined) {
  const trimmed = path?.trim()
  if (!trimmed || trimmed === "external API" || trimmed === "not reported") return null
  return trimmed.replace(/[\\/]+$/, "")
}
