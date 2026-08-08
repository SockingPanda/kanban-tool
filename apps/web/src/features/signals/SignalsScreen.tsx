import { useEffect, useMemo, useState, type ChangeEvent } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"

import type {
  SignalListQuery,
  SignalRecord,
  SignalsOntologyReadApi,
} from "../../lib/api/signals-ontology-read-model"
import type { SignalsRouteFilters } from "../../lib/router"

import styles from "./SignalsScreen.module.css"

const SIGNAL_STATUSES = ["review", "open", "confirmed", "resolved", "rejected", "superseded", "all"] as const

type SignalStatusFilter = (typeof SIGNAL_STATUSES)[number]

export type ReadPhase = "idle" | "loading" | "refreshing" | "success" | "error"

export interface ReadState<T> {
  readonly phase: ReadPhase
  readonly data: T
  readonly error: unknown | null
}

export interface SignalsScreenProps {
  readonly api: SignalsOntologyReadApi | null
  readonly boardName?: string
  readonly filters?: SignalsRouteFilters
  readonly selectedSignalId?: string | null
  /** Incremented by the sync sink when a matching `signals`/`signal` target is invalidated. */
  readonly invalidationRevision?: number
  readonly online?: boolean
  readonly onFiltersChange?: (filters: SignalsRouteFilters) => void
  readonly onSelectSignal?: (signalId: string | null) => void
  /** Parent-owned history seam for closing the selected detail. */
  readonly onCloseDetail?: () => void
}

function emptyListState(): ReadState<readonly SignalRecord[]> {
  return { phase: "idle", data: [], error: null }
}

function emptyDetailState(): ReadState<SignalRecord | null> {
  return { phase: "idle", data: null, error: null }
}

function hasPriorData<T>(state: ReadState<T>): boolean {
  return state.phase === "success" || state.phase === "refreshing" || state.phase === "error"
}

export function useReadState<T>(
  enabled: boolean,
  request: ((signal: AbortSignal) => Promise<T>) | null,
  key: string,
  initial: ReadState<T>,
): ReadState<T> {
  const [state, setState] = useState<ReadState<T>>(initial)

  useEffect(() => {
    if (!enabled || request === null) {
      setState(initial)
      return
    }
    const controller = new AbortController()
    setState((previous) => ({
      phase: hasPriorData(previous) ? "refreshing" : "loading",
      data: previous.data,
      error: null,
    }))
    void request(controller.signal).then(
      (data) => {
        if (controller.signal.aborted) return
        setState({ phase: "success", data, error: null })
      },
      (error: unknown) => {
        if (controller.signal.aborted) return
        setState((previous) => ({ phase: "error", data: previous.data, error }))
      },
    )
    return () => controller.abort()
    // `request` is recreated by callers only when this stable key changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, key])

  return state
}

function queryFromFilters(filters: SignalsRouteFilters): SignalListQuery {
  const status = filters.status ?? "review"
  return {
    statuses: status === "review" || status === "all" ? [] : [status],
    kinds: filters.kinds ?? [],
    task: filters.task,
    includeAll: status === "all" || status !== "review",
    limit: 100,
  }
}

function errorStatus(error: unknown): number | null {
  if (typeof error !== "object" || error === null || !("status" in error)) return null
  const value = (error as { status?: unknown }).status
  return typeof value === "number" ? value : null
}

function statusVariant(status: string): "neutral" | "info" | "success" | "warning" | "error" {
  switch (status) {
    case "open": return "info"
    case "confirmed": return "warning"
    case "resolved": return "success"
    case "rejected":
    case "superseded": return "error"
    default: return "neutral"
  }
}

function timestamp(value: number): string {
  if (!Number.isFinite(value)) return "—"
  return new Date(value).toLocaleString()
}

function signalTask(signal: SignalRecord): string {
  return signal.observation.task_ref_snapshot ?? signal.observation.task_id ?? "—"
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message
  return "The local service returned an unreadable response."
}

export function SignalsScreen({
  api,
  boardName,
  filters = {},
  selectedSignalId: selectedSignalIdProp,
  invalidationRevision = 0,
  online = typeof navigator === "undefined" || navigator.onLine,
  onFiltersChange,
  onSelectSignal: onSelectSignalProp,
  onCloseDetail: onCloseDetailProp,
}: SignalsScreenProps) {
  const [localFilters, setLocalFilters] = useState<SignalsRouteFilters>(filters)
  const [localSelectedSignalId, setLocalSelectedSignalId] = useState<string | null>(selectedSignalIdProp ?? null)
  const [refreshToken, setRefreshToken] = useState(0)
  const effectiveFilters = onFiltersChange ? filters : localFilters
  const selectedSignalId = selectedSignalIdProp === undefined ? localSelectedSignalId : selectedSignalIdProp
  const filterKey = JSON.stringify(effectiveFilters)
  const listRequest = useMemo(() => {
    if (!api) return null
    const query = queryFromFilters(effectiveFilters)
    return (signal: AbortSignal) => api.reviewSignals(query, signal)
  }, [api, filterKey])
  const list = useReadState(Boolean(api), listRequest, `signals:${api?.board ?? "none"}:${filterKey}:${refreshToken}:${invalidationRevision}`, emptyListState())

  const visibleSignals = list.data
  useEffect(() => {
    if (selectedSignalId !== null && visibleSignals.some((signal) => signal.id === selectedSignalId)) return
    const nextId = visibleSignals[0]?.id ?? null
    setLocalSelectedSignalId(nextId)
    onSelectSignalProp?.(nextId)
  }, [onSelectSignalProp, selectedSignalId, visibleSignals])

  const detailRequest = useMemo(() => {
    if (!api || !selectedSignalId) return null
    return (signal: AbortSignal) => api.getSignal(selectedSignalId, signal)
  }, [api, selectedSignalId])
  const detail = useReadState(Boolean(api && selectedSignalId), detailRequest, `signal:${selectedSignalId ?? "none"}:${refreshToken}:${invalidationRevision}`, emptyDetailState())

  const updateFilters = (next: SignalsRouteFilters) => {
    setLocalFilters(next)
    onFiltersChange?.(next)
  }
  const selectSignal = (signalId: string | null) => {
    setLocalSelectedSignalId(signalId)
    onSelectSignalProp?.(signalId)
  }
  const refresh = () => setRefreshToken((value) => value + 1)
  const closeDetail = () => {
    selectSignal(null)
    onCloseDetailProp?.()
  }
  const selectedFromList = selectedSignalId === null ? null : visibleSignals.find((signal) => signal.id === selectedSignalId) ?? null

  return (
    <SignalsScreenView
      boardName={boardName ?? api?.board ?? "—"}
      filters={effectiveFilters}
      list={list}
      detail={{
        phase: detail.phase,
        data: detail.data ?? selectedFromList,
        error: detail.error,
      }}
      selectedSignalId={selectedSignalId}
      online={online}
      onRefresh={refresh}
      onFiltersChange={updateFilters}
      onSelectSignal={selectSignal}
      onCloseDetail={closeDetail}
    />
  )
}

export interface SignalsScreenViewProps {
  readonly boardName: string
  readonly filters: SignalsRouteFilters
  readonly list: ReadState<readonly SignalRecord[]>
  readonly detail: ReadState<SignalRecord | null>
  readonly selectedSignalId: string | null
  readonly online: boolean
  readonly onRefresh: () => void
  readonly onFiltersChange: (filters: SignalsRouteFilters) => void
  readonly onSelectSignal: (signalId: string | null) => void
  readonly onCloseDetail?: () => void
}

export function SignalsScreenView({
  boardName,
  filters,
  list,
  detail,
  selectedSignalId,
  online,
  onRefresh,
  onFiltersChange,
  onSelectSignal,
  onCloseDetail,
}: SignalsScreenViewProps) {
  const stale = list.phase === "error" && list.data.length > 0
  const status = (filters.status ?? "review") as SignalStatusFilter
  const kindsValue = (filters.kinds ?? []).join(", ")
  return (
    <main className={styles.screen} aria-labelledby="signals-title" data-testid="signals-screen">
      <header className={styles.hero}>
        <div>
          <Text as="p" type="supporting" className={styles.eyebrow}>KANBAN TOOL / SIGNALS</Text>
          <Heading level={1} id="signals-title">Signals</Heading>
          <Text as="p" type="supporting" className={styles.lede}>Generic agent and product signals for the active board.</Text>
          <Text as="p" type="supporting" className={styles.boardContext}>Board · <code translate="no">{boardName}</code></Text>
        </div>
        <Button label="Refresh" variant="secondary" size="sm" onClick={onRefresh} isLoading={list.phase === "loading" || list.phase === "refreshing"} />
      </header>

      {!online ? (
        <Banner status="warning" title="You are offline" description={stale ? "Showing stale signal rows until the local service reconnects." : "Connect to kanban serve to load signals."} />
      ) : null}
      {list.error ? (
        <Banner
          status="error"
          title="Unable to load signals"
          description={stale ? `${errorMessage(list.error)} · stale rows are still visible.` : errorMessage(list.error)}
          endContent={<Button label="Retry" variant="ghost" size="sm" onClick={onRefresh} />}
        />
      ) : null}

      <section className={styles.filterBar} aria-label="Signal filters">
        <div className={styles.statusFilters} role="group" aria-label="Signal status">
          {SIGNAL_STATUSES.map((candidate) => (
            <Button
              key={candidate}
              label={candidate === "review" ? "Open + confirmed" : candidate}
              variant={status === candidate ? "primary" : "ghost"}
              size="sm"
              aria-pressed={status === candidate}
              onClick={() => onFiltersChange({ ...filters, status: candidate })}
            />
          ))}
        </div>
        <label className={styles.filterField}>
          <span>Signal kind</span>
          <input
            aria-label="Signal kind filter"
            value={kindsValue}
            placeholder="kind, kind"
            onChange={(event: ChangeEvent<HTMLInputElement>) => onFiltersChange({ ...filters, kinds: event.currentTarget.value.split(",").map((value) => value.trim()).filter(Boolean) })}
          />
        </label>
        <label className={styles.filterField}>
          <span>Task ref</span>
          <input
            aria-label="Signal task filter"
            value={filters.task ?? ""}
            placeholder="default#123"
            onChange={(event: ChangeEvent<HTMLInputElement>) => onFiltersChange({ ...filters, task: event.currentTarget.value.trim() || undefined })}
          />
        </label>
      </section>

      <section className={styles.workspace}>
        <Card className={styles.listPanel} padding={0}>
          <div className={styles.panelHeader}>
            <div>
              <Heading level={2}>Signal rows</Heading>
              <Text as="p" type="supporting">{list.data.length} of up to 100 loaded</Text>
            </div>
            {list.phase === "refreshing" ? <Badge variant="warning" label="refreshing" /> : null}
          </div>
          <SignalListView
            phase={list.phase}
            signals={list.data}
            selectedSignalId={selectedSignalId}
            onSelectSignal={onSelectSignal}
          />
        </Card>

        <Card className={styles.detailPanel} padding={0}>
          <div className={styles.panelHeader}>
            <div>
              <Heading level={2}>Signal detail</Heading>
              <Text as="p" type="supporting">{selectedSignalId ?? "none selected"}</Text>
            </div>
            {selectedSignalId !== null && onCloseDetail ? <Button label="Close detail" variant="ghost" size="sm" onClick={onCloseDetail} /> : null}
            {detail.phase === "refreshing" ? <Badge variant="warning" label="refreshing" /> : null}
          </div>
          <SignalDetailView loading={detail.phase === "loading"} signal={detail.data} error={detail.error} />
        </Card>
      </section>
    </main>
  )
}

export function SignalListView({
  phase,
  signals,
  selectedSignalId,
  onSelectSignal,
}: {
  readonly phase: ReadPhase
  readonly signals: readonly SignalRecord[]
  readonly selectedSignalId: string | null
  readonly onSelectSignal: (signalId: string) => void
}) {
  if (phase === "loading" && signals.length === 0) {
    return <div className={styles.loadingList} role="status" aria-live="polite"><span /><span /><span /></div>
  }
  if (signals.length === 0) {
    return <div className={styles.emptyState}>No signals returned.</div>
  }
  return (
    <div className={styles.signalList} aria-live="polite">
      {signals.map((signal) => (
        <button
          key={signal.id}
          type="button"
          className={`${styles.signalRow} ${signal.id === selectedSignalId ? styles.signalRowSelected : ""}`}
          aria-current={signal.id === selectedSignalId ? "true" : undefined}
          onClick={() => onSelectSignal(signal.id)}
        >
          <span className={styles.rowTitle}>
            <Badge variant={statusVariant(signal.status)} label={signal.status} />
            <strong>{signal.title}</strong>
          </span>
          <span className={styles.rowSummary}>{signal.summary}</span>
          <span className={styles.rowMeta}>{signal.kind} · {signalTask(signal)} · {timestamp(signal.created_at)}</span>
        </button>
      ))}
    </div>
  )
}

export function SignalDetailView({ loading, signal, error }: { readonly loading: boolean; readonly signal: SignalRecord | null; readonly error?: unknown | null }) {
  if (loading && signal === null) {
    return <div className={styles.detailLoading} role="status" aria-live="polite"><span /><span /><span /></div>
  }
  if (errorStatus(error) === 404 && signal === null) {
    return <div className={styles.emptyState}>Signal is no longer available.</div>
  }
  if (signal === null) {
    return <div className={styles.emptyState}>Select a signal to inspect observation and evidence.</div>
  }
  return (
    <article className={styles.detail} aria-label="Signal detail">
      {error ? <Banner status="warning" title="Showing stale detail" description={errorMessage(error)} /> : null}
      <div className={styles.detailBadges}>
        <Badge variant={statusVariant(signal.status)} label={signal.status} />
        <Badge variant="neutral" label={signal.severity} />
        <Badge variant="neutral" label={signal.kind} />
      </div>
      <Heading level={3}>{signal.title}</Heading>
      <Text as="p" type="supporting" className={styles.detailSummary}>{signal.summary}</Text>
      <dl className={styles.facts}>
        <Fact label="Signal ID" value={signal.id} />
        <Fact label="Observation ID" value={signal.observation_id} />
        <Fact label="Task" value={signalTask(signal)} />
        <Fact label="Source" value={signal.observation.source ?? "—"} />
        <Fact label="Actor" value={signal.observation.actor} />
        <Fact label="Agent type" value={signal.observation.agent_type ?? "—"} />
        <Fact label="Dedupe key" value={signal.dedupe_key ?? "—"} />
        <Fact label="Created" value={timestamp(signal.created_at)} />
      </dl>
      <div className={styles.evidence}>
        <Heading level={4}>Evidence JSON</Heading>
        <pre>{JSON.stringify(signal.observation.evidence, null, 2)}</pre>
      </div>
    </article>
  )
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return <div><dt>{label}</dt><dd translate="no">{value}</dd></div>
}
