import { useEffect, useMemo, useState, type ReactNode } from "react"
import { useQuery } from "@tanstack/react-query"
import { Braces, CircleDashed, RefreshCcw, Search } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/i18n"
import type { KanbanApi, SignalRecord, SignalStatus } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const SIGNAL_LIMIT = 100
type StatusFilter = "review" | "all" | SignalStatus

export function SignalsWorkbench({ api }: { api: KanbanApi | null }) {
  const { t } = useI18n()
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("review")
  const [kindInput, setKindInput] = useState("")
  const [taskInput, setTaskInput] = useState("")
  const [selectedSignalId, setSelectedSignalId] = useState<string | null>(null)

  const statusQuery = useMemo(() => statusFilterToQuery(statusFilter), [statusFilter])
  const kinds = useMemo(() => parseKinds(kindInput), [kindInput])
  const task = taskInput.trim()
  const signalsQueryShape = useMemo(
    () => ({
      board: api?.board ?? "default",
      statuses: statusQuery.statuses,
      kinds,
      task,
      includeAll: statusQuery.includeAll,
      limit: SIGNAL_LIMIT,
    }),
    [api?.board, kinds, statusQuery.includeAll, statusQuery.statuses, task],
  )

  const signalsQuery = useQuery({
    queryKey: queryKeys.signals(signalsQueryShape),
    enabled: Boolean(api),
    queryFn: ({ signal }) => {
      if (!api) return []
      return api.reviewSignals({
        statuses: signalsQueryShape.statuses,
        kinds: signalsQueryShape.kinds,
        task: signalsQueryShape.task,
        includeAll: signalsQueryShape.includeAll,
        limit: SIGNAL_LIMIT,
        signal,
      })
    },
  })

  const signals = signalsQuery.data ?? []
  useEffect(() => {
    if (!signals.length) {
      setSelectedSignalId(null)
      return
    }
    if (!selectedSignalId || !signals.some((signal) => signal.id === selectedSignalId)) {
      setSelectedSignalId(signals[0]?.id ?? null)
    }
  }, [selectedSignalId, signals])

  const detailQuery = useQuery({
    queryKey: selectedSignalId ? queryKeys.signal(selectedSignalId) : ["signal", "empty"],
    enabled: Boolean(api && selectedSignalId),
    queryFn: ({ signal }) => {
      if (!api || !selectedSignalId) throw new Error("missing signal selection")
      return api.getSignal(selectedSignalId, { signal })
    },
  })

  function refresh() {
    void signalsQuery.refetch()
    if (selectedSignalId) void detailQuery.refetch()
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-md border bg-card px-4 py-3">
        <div className="min-w-0">
          <h1 className="truncate text-base font-semibold">{t("Signals")}</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            {t("Generic agent and product signals for the active board.")}
          </p>
        </div>
        <Button type="button" variant="outline" size="sm" onClick={refresh} disabled={!api || signalsQuery.isFetching}>
          <RefreshCcw className={cn("h-4 w-4", signalsQuery.isFetching && "animate-spin")} />
          {t("Refresh")}
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2 rounded-md border bg-card px-3 py-2">
        <Tabs value={statusFilter} onValueChange={(value) => setStatusFilter(value as StatusFilter)}>
          <TabsList className="h-auto flex-wrap">
            <TabsTrigger value="review">{t("Open + confirmed")}</TabsTrigger>
            <TabsTrigger value="open">{t("open")}</TabsTrigger>
            <TabsTrigger value="confirmed">{t("confirmed")}</TabsTrigger>
            <TabsTrigger value="resolved">{t("resolved")}</TabsTrigger>
            <TabsTrigger value="rejected">{t("rejected")}</TabsTrigger>
            <TabsTrigger value="superseded">{t("superseded")}</TabsTrigger>
            <TabsTrigger value="all">{t("All")}</TabsTrigger>
          </TabsList>
        </Tabs>
        <div className="ml-auto flex min-w-[280px] flex-1 flex-wrap items-center justify-end gap-2">
          <div className="relative min-w-[180px] flex-1 sm:max-w-[260px]">
            <Search className="pointer-events-none absolute left-2 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              aria-label={t("Signal kind filter")}
              className="pl-8"
              value={kindInput}
              onChange={(event) => setKindInput(event.target.value)}
              placeholder={t("kind")}
            />
          </div>
          <Input
            aria-label={t("Signal task filter")}
            className="min-w-[160px] sm:max-w-[220px]"
            value={taskInput}
            onChange={(event) => setTaskInput(event.target.value)}
            placeholder={t("task ref")}
          />
        </div>
      </div>

      <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[minmax(320px,0.85fr)_minmax(420px,1.15fr)]">
        <Panel title={t("Signal rows")} meta={t("{count} of up to {limit} loaded", { count: signals.length, limit: SIGNAL_LIMIT })}>
          <SignalList
            loading={signalsQuery.isLoading}
            signals={signals}
            selectedSignalId={selectedSignalId}
            onSelectSignal={setSelectedSignalId}
          />
        </Panel>
        <Panel title={t("Signal detail")} meta={selectedSignalId ?? t("none")}>
          <SignalDetail loading={detailQuery.isLoading} signal={detailQuery.data ?? signals.find((signal) => signal.id === selectedSignalId) ?? null} />
        </Panel>
      </div>
    </div>
  )
}

export function SignalList({
  loading,
  onSelectSignal,
  selectedSignalId,
  signals,
}: {
  loading: boolean
  signals: SignalRecord[]
  selectedSignalId: string | null
  onSelectSignal: (signalId: string) => void
}) {
  const { t } = useI18n()
  if (loading) {
    return (
      <div className="space-y-2 p-3">
        <Skeleton className="h-16 w-full" />
        <Skeleton className="h-16 w-full" />
        <Skeleton className="h-16 w-full" />
      </div>
    )
  }
  if (!signals.length) {
    return <div className="flex min-h-[180px] items-center justify-center text-sm text-muted-foreground">{t("No signals returned.")}</div>
  }
  return (
    <div className="min-h-0 overflow-auto">
      {signals.map((signal) => (
        <button
          key={signal.id}
          type="button"
          className={cn(
            "flex w-full flex-col gap-2 border-b px-3 py-3 text-left transition-colors hover:bg-muted/60",
            selectedSignalId === signal.id && "bg-muted",
          )}
          onClick={() => onSelectSignal(signal.id)}
        >
          <div className="flex min-w-0 items-center gap-2">
            <Badge variant={statusVariant(signal.status)}>{signal.status}</Badge>
            <span className="truncate text-sm font-medium">{signal.title}</span>
          </div>
          <div className="line-clamp-2 text-xs text-muted-foreground">{signal.summary}</div>
          <div className="flex flex-wrap gap-2 text-[11px] text-muted-foreground">
            <span>{signal.kind}</span>
            <span>{signal.observation.task_ref_snapshot ?? signal.observation.task_id ?? "-"}</span>
            <span>{timeLabel(signal.created_at)}</span>
          </div>
        </button>
      ))}
    </div>
  )
}

export function SignalDetail({ loading, signal }: { loading: boolean; signal: SignalRecord | null }) {
  const { t } = useI18n()
  if (loading) {
    return (
      <div className="space-y-3 p-4">
        <Skeleton className="h-8 w-2/3" />
        <Skeleton className="h-20 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    )
  }
  if (!signal) {
    return <div className="flex min-h-[220px] items-center justify-center text-sm text-muted-foreground">{t("Select a signal to inspect observation and evidence.")}</div>
  }
  const observation = signal.observation
  return (
    <div className="min-h-0 overflow-auto p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={statusVariant(signal.status)}>{signal.status}</Badge>
        <Badge variant="secondary">{signal.severity}</Badge>
        <Badge variant="secondary">{signal.kind}</Badge>
      </div>
      <h2 className="mt-3 text-base font-semibold">{signal.title}</h2>
      <p className="mt-2 whitespace-pre-wrap text-sm text-muted-foreground">{signal.summary}</p>

      <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
        <DetailItem label={t("Signal ID")} value={signal.id} />
        <DetailItem label={t("Observation ID")} value={signal.observation_id} />
        <DetailItem label={t("Task")} value={observation.task_ref_snapshot ?? observation.task_id ?? "-"} />
        <DetailItem label={t("Source")} value={observation.source ?? "-"} />
        <DetailItem label={t("Actor")} value={observation.actor} />
        <DetailItem label={t("Agent type")} value={observation.agent_type ?? "-"} />
        <DetailItem label={t("Dedupe key")} value={signal.dedupe_key ?? "-"} />
        <DetailItem label={t("Created")} value={timeLabel(signal.created_at)} />
      </dl>

      <div className="mt-4 rounded-md border bg-muted/20">
        <div className="flex items-center gap-2 border-b px-3 py-2 text-sm font-medium">
          <Braces className="h-4 w-4" />
          {t("Evidence JSON")}
        </div>
        <pre className="max-h-[340px] overflow-auto p-3 text-xs leading-relaxed">{JSON.stringify(observation.evidence, null, 2)}</pre>
      </div>
    </div>
  )
}

function Panel({ children, meta, title }: { children: ReactNode; meta: string; title: string }) {
  return (
    <section className="flex min-h-0 flex-col rounded-md border bg-card">
      <div className="flex items-center justify-between gap-2 border-b px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <CircleDashed className="h-4 w-4 text-muted-foreground" />
          <h2 className="truncate text-sm font-semibold">{title}</h2>
        </div>
        <Badge variant="secondary" className="shrink-0">
          {meta}
        </Badge>
      </div>
      <div className="min-h-0 flex-1">{children}</div>
    </section>
  )
}

function DetailItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border bg-muted/20 px-3 py-2">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="mt-1 truncate font-mono text-xs">{value}</dd>
    </div>
  )
}

function statusFilterToQuery(statusFilter: StatusFilter) {
  if (statusFilter === "review") return { statuses: [] as SignalStatus[], includeAll: false }
  if (statusFilter === "all") return { statuses: [] as SignalStatus[], includeAll: true }
  return { statuses: [statusFilter], includeAll: true }
}

function parseKinds(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
}

function statusVariant(status: SignalStatus) {
  if (status === "open") return "default"
  if (status === "confirmed") return "review"
  if (status === "resolved") return "running"
  return "blocked"
}

function timeLabel(value: number) {
  if (!Number.isFinite(value)) return "-"
  return new Date(value).toLocaleString()
}
