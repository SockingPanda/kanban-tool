import { keepPreviousData, useQuery } from "@tanstack/react-query"
import { Activity, Database, RefreshCcw } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { MetricStrip, SectionCard } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Skeleton } from "@/components/ui/skeleton"
import { useI18n } from "@/i18n"
import type { HealthStatus, KanbanApi, RuntimeConfig } from "@/lib/api"
import { presentApiError } from "@/lib/api/error-presentation"

type MetricTone = "ready" | "blocked" | "secondary"

type HealthMetric = {
  id: string
  label: string
  value: string
  tone: MetricTone
}

type Translate = (key: string, values?: Record<string, string | number>) => string

const identityTranslate: Translate = (key, values = {}) =>
  key.replace(/\{([a-zA-Z0-9_]+)\}/g, (match, name) => {
    const value = values[name]
    return value === undefined ? match : String(value)
  })

export function buildHealthRuntimeModel(health: HealthStatus, t: Translate = identityTranslate) {
  const metrics: HealthMetric[] = [
    { id: "ok", label: t("ok"), value: String(health.ok), tone: health.ok ? "ready" : "blocked" },
    { id: "db", label: t("db"), value: t(health.db), tone: health.db === "ok" ? "ready" : "blocked" },
    { id: "version", label: t("version"), value: health.version, tone: "secondary" },
    { id: "db-path", label: t("db_path"), value: reportedValue(health.db_path, t), tone: "secondary" },
    { id: "db-fingerprint", label: t("db_fingerprint"), value: reportedValue(health.db_fingerprint, t), tone: "secondary" },
  ]

  return {
    metrics,
  }
}

export function HealthView({ api, config }: { api: KanbanApi | null; config: RuntimeConfig | null }) {
  const { t } = useI18n()
  const healthQuery = useQuery({
    enabled: Boolean(api),
    queryKey: ["health", config?.apiBaseUrl ?? "pending"],
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.health({ signal })
    },
    placeholderData: keepPreviousData,
  })

  const runtimeModel = healthQuery.data ? buildHealthRuntimeModel(healthQuery.data, t) : null

  return (
    <ScrollArea className="flex-1 bg-card p-4">
      <SectionCard
        title={t("Runtime health")}
        icon={Activity}
        actions={
          <Button variant="ghost" size="sm" disabled={healthQuery.isFetching} onClick={() => void healthQuery.refetch()}>
            <RefreshCcw className="h-4 w-4" />
            {t("Refresh")}
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
              <EmptyDescription>{t("No health response.")}</EmptyDescription>
            </Empty>
          )
        )}
        {healthQuery.error ? (
          <Alert className="mt-3 border-destructive/50">
            <AlertDescription className="text-destructive">{presentApiError(healthQuery.error, t)}</AlertDescription>
          </Alert>
        ) : null}
      </SectionCard>

      <SectionCard title={t("Runtime config")} icon={Database} className="mt-4">
        <div className="space-y-2 text-sm">
          <InfoRow label={t("board")} value={config?.board ?? "-"} />
          <InfoRow label={t("actor")} value={config?.actor ?? "-"} />
          <InfoRow label={t("API")} value={config?.apiBaseUrl || t("same-origin")} />
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

function reportedValue(value: string | undefined, t: Translate = identityTranslate) {
  const trimmed = value?.trim()
  return trimmed || t("not reported")
}
