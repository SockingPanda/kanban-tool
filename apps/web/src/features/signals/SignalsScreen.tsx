import { useEffect, useMemo, useState, type ChangeEvent, type ComponentProps } from "react"

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
import { featureCopyForLocale } from "../../lib/i18n"
import type { Locale } from "../../lib/preferences"
import type { SignalsRouteFilters } from "../../lib/router"
import { usePreferences } from "../../lib/use-preferences"
import { reconcileSelection, useReadState, type ReadPhase, type ReadState } from "../read-state"
import { localizedErrorMessage } from "../safe-error"

import styles from "./SignalsScreen.module.css"

const SIGNAL_STATUSES = ["review", "open", "confirmed", "resolved", "rejected", "superseded", "all"] as const

type SignalStatusFilter = (typeof SIGNAL_STATUSES)[number]

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

type SignalsCopy = ReturnType<typeof featureCopyForLocale>["signals"]

function emptyListState(): ReadState<readonly SignalRecord[]> {
  return { phase: "idle", data: [], error: null }
}

function emptyDetailState(): ReadState<SignalRecord | null> {
  return { phase: "idle", data: null, error: null }
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

function timestamp(value: number, locale: Locale): string {
  if (!Number.isFinite(value)) return "—"
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return "—"
  return date.toLocaleString(locale === "zh" ? "zh-CN" : "en-US")
}

function signalTask(signal: SignalRecord): string {
  return signal.observation.task_ref_snapshot ?? signal.observation.task_id ?? "—"
}

function MachineBadge(props: ComponentProps<typeof Badge>) {
  return <span translate="no"><Badge {...props} /></span>
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
  const { locale } = usePreferences()
  const [localFilters, setLocalFilters] = useState<SignalsRouteFilters>(filters)
  const [localSelectedSignalId, setLocalSelectedSignalId] = useState<string | null>(selectedSignalIdProp ?? null)
  const [listRefreshToken, setListRefreshToken] = useState(0)
  const [detailRefreshToken, setDetailRefreshToken] = useState(0)
  const effectiveFilters = onFiltersChange ? filters : localFilters
  const selectedSignalId = selectedSignalIdProp === undefined ? localSelectedSignalId : selectedSignalIdProp
  useEffect(() => {
    if (selectedSignalIdProp !== undefined) setLocalSelectedSignalId(selectedSignalIdProp)
  }, [selectedSignalIdProp])
  const filterKey = JSON.stringify(effectiveFilters)
  const listRequest = useMemo(() => {
    if (!api) return null
    const query = queryFromFilters(effectiveFilters)
    return (signal: AbortSignal) => api.reviewSignals(query, signal)
  }, [api, effectiveFilters])
  const list = useReadState(Boolean(api), listRequest, `signals:${api?.cacheKey ?? "none"}:${filterKey}:${listRefreshToken}:${invalidationRevision}`, emptyListState())

  const detailRequest = useMemo(() => {
    if (!api || !selectedSignalId) return null
    return (signal: AbortSignal) => api.getSignal(selectedSignalId, signal)
  }, [api, selectedSignalId])
  const detail = useReadState(Boolean(api && selectedSignalId), detailRequest, `signal:${api?.cacheKey ?? "none"}:${selectedSignalId ?? "none"}:${detailRefreshToken}:${invalidationRevision}`, emptyDetailState())
  const visibleSignals = useMemo(
    () => errorStatus(detail.error) === 404 && selectedSignalId !== null
      ? list.data.filter((signal) => signal.id !== selectedSignalId)
      : list.data,
    [detail.error, list.data, selectedSignalId],
  )
  useEffect(() => {
    // A filtered list cannot prove that a deep-linked signal is invalid. Only
    // an authoritative detail 404 permits replacing the URL selection.
    if (list.phase !== "success" || selectedSignalId === null || errorStatus(detail.error) !== 404) return
    const nextId = reconcileSelection(selectedSignalId, list.data.filter((signal) => signal.id !== selectedSignalId))
    if (nextId === selectedSignalId) return
    setLocalSelectedSignalId(nextId)
    onSelectSignalProp?.(nextId)
  }, [detail.error, list.data, list.phase, onSelectSignalProp, selectedSignalId])

  const updateFilters = (next: SignalsRouteFilters) => {
    setLocalFilters(next)
    onFiltersChange?.(next)
  }
  const selectSignal = (signalId: string | null) => {
    setLocalSelectedSignalId(signalId)
    onSelectSignalProp?.(signalId)
  }
  const refreshList = () => setListRefreshToken((value) => value + 1)
  const refreshDetail = () => setDetailRefreshToken((value) => value + 1)
  const refresh = () => {
    refreshList()
    refreshDetail()
  }
  const closeDetail = () => {
    if (onCloseDetailProp) {
      onCloseDetailProp()
      return
    }
    selectSignal(null)
  }
  const selectedFromList = selectedSignalId === null || detail.error !== null
    ? null
    : visibleSignals.find((signal) => signal.id === selectedSignalId) ?? null
  const detailData = detail.error === null && detail.data?.id === selectedSignalId
    ? detail.data
    : selectedFromList

  return (
    <SignalsScreenView
      boardName={boardName ?? api?.board ?? "—"}
      copy={featureCopyForLocale(locale).signals}
      locale={locale}
      filters={effectiveFilters}
      list={{ ...list, data: visibleSignals }}
      detail={{
        phase: detail.phase,
        data: detailData,
        error: detail.error,
      }}
      selectedSignalId={selectedSignalId}
      online={online}
      onRefresh={refresh}
      onRefreshList={refreshList}
      onRefreshDetail={refreshDetail}
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
  readonly locale?: Locale
  readonly onRefresh: () => void
  readonly onRefreshList?: () => void
  readonly onRefreshDetail?: () => void
  readonly onFiltersChange: (filters: SignalsRouteFilters) => void
  readonly onSelectSignal: (signalId: string | null) => void
  readonly onCloseDetail?: () => void
  readonly copy?: SignalsCopy
}

export function SignalsScreenView({
  boardName,
  filters,
  list,
  detail,
  selectedSignalId,
  online,
  locale = "en",
  onRefresh,
  onRefreshList = onRefresh,
  onRefreshDetail = onRefresh,
  onFiltersChange,
  onSelectSignal,
  onCloseDetail,
  copy = featureCopyForLocale("en").signals,
}: SignalsScreenViewProps) {
  const stale = list.phase === "error" && list.data.length > 0
  const status = (filters.status ?? "review") as SignalStatusFilter
  const kindsValue = (filters.kinds ?? []).join(", ")
  const viewSignals = errorStatus(detail.error) === 404 && selectedSignalId !== null
    ? list.data.filter((signal) => signal.id !== selectedSignalId)
    : list.data
  return (
      <main className={styles.screen} aria-labelledby="signals-title" data-testid="signals-screen">
      <header className={styles.hero}>
        <div>
          <Text as="p" type="supporting" className={styles.eyebrow}>{copy.eyebrow}</Text>
          <Heading level={1} id="signals-title">{copy.heading}</Heading>
          <Text as="p" type="supporting" className={styles.lede}>{copy.lede}</Text>
          <Text as="p" type="supporting" className={styles.boardContext}>{copy.board} · <code translate="no">{boardName}</code></Text>
        </div>
        <Button label={copy.refresh} variant="secondary" size="sm" onClick={onRefresh} isLoading={list.phase === "loading" || list.phase === "refreshing"} />
      </header>

      {!online ? (
        <Banner status="warning" title={copy.offline} description={stale ? copy.offlineStale : copy.offlineConnect} />
      ) : null}
      {list.error ? (
        <Banner
          status="error"
          title={copy.loadError}
          description={stale ? `${localizedErrorMessage(list.error, copy.unreadableResponse, locale)} · ${copy.staleRows}` : localizedErrorMessage(list.error, copy.unreadableResponse, locale)}
          endContent={<Button label={copy.retryList} variant="ghost" size="sm" onClick={onRefreshList} />}
        />
      ) : null}

      <section className={styles.filterBar} aria-label={copy.filters}>
        <div className={styles.statusFilters} role="group" aria-label={copy.status}>
          {SIGNAL_STATUSES.map((candidate) => (
            <Button
              key={candidate}
              label={candidate === "review" ? copy.openConfirmed : candidate === "all" ? copy.all : copy.statusLabels[candidate]}
              variant={status === candidate ? "primary" : "ghost"}
              size="sm"
              aria-pressed={status === candidate}
              onClick={() => onFiltersChange({ ...filters, status: candidate })}
            />
          ))}
        </div>
        <label className={styles.filterField}>
          <span>{copy.kind}</span>
          <input
            name="signal-kind"
            autoComplete="off"
            aria-label={copy.kind}
            value={kindsValue}
            placeholder={copy.kindPlaceholder}
            onChange={(event: ChangeEvent<HTMLInputElement>) => onFiltersChange({ ...filters, kinds: event.currentTarget.value.split(",").map((value) => value.trim()).filter(Boolean) })}
          />
        </label>
        <label className={styles.filterField}>
          <span>{copy.taskRef}</span>
          <input
            name="signal-task-ref"
            autoComplete="off"
            aria-label={copy.taskRef}
            value={filters.task ?? ""}
            placeholder={copy.taskRefPlaceholder}
            onChange={(event: ChangeEvent<HTMLInputElement>) => onFiltersChange({ ...filters, task: event.currentTarget.value.trim() || undefined })}
          />
        </label>
      </section>

      <section className={styles.workspace}>
        <Card className={styles.listPanel} padding={0}>
          <div className={styles.panelHeader}>
            <div>
              <Heading level={2}>{copy.signalRows}</Heading>
              <Text as="p" type="supporting">{copy.loadedCount(viewSignals.length)}</Text>
            </div>
            {list.phase === "refreshing" ? <Badge variant="warning" label={copy.refreshing} /> : null}
          </div>
          <SignalListView
            phase={list.phase}
            signals={viewSignals}
            selectedSignalId={selectedSignalId}
            onSelectSignal={onSelectSignal}
            locale={locale}
            copy={copy}
          />
        </Card>

        <Card className={styles.detailPanel} padding={0}>
          <div className={styles.panelHeader}>
            <div>
              <Heading level={2}>{copy.detail}</Heading>
              <Text as="p" type="supporting">{selectedSignalId === null ? copy.noneSelected : <span translate="no">{selectedSignalId}</span>}</Text>
            </div>
            {selectedSignalId !== null && onCloseDetail ? <Button label={copy.closeDetail} variant="ghost" size="sm" onClick={onCloseDetail} /> : null}
            {detail.phase === "refreshing" ? <Badge variant="warning" label={copy.refreshing} /> : null}
          </div>
          <SignalDetailView loading={detail.phase === "loading"} signal={detail.data} error={detail.error} onRetry={onRefreshDetail} locale={locale} copy={copy} />
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
  locale = "en",
  copy = featureCopyForLocale("en").signals,
}: {
  readonly phase: ReadPhase
  readonly signals: readonly SignalRecord[]
  readonly selectedSignalId: string | null
  readonly onSelectSignal: (signalId: string) => void
  readonly locale?: Locale
  readonly copy?: SignalsCopy
}) {
  if (phase === "loading" && signals.length === 0) {
    return <div className={styles.loadingList} role="status" aria-label={copy.loading}><span /><span /><span /></div>
  }
  if (signals.length === 0) {
    return <div className={styles.emptyState}>{copy.noSignals}</div>
  }
  return (
    <div className={styles.signalList}>
      {signals.map((signal) => (
        <button
          key={signal.id}
          type="button"
          className={`${styles.signalRow} ${signal.id === selectedSignalId ? styles.signalRowSelected : ""}`}
          aria-pressed={signal.id === selectedSignalId}
          onClick={() => onSelectSignal(signal.id)}
        >
          <span className={styles.rowTitle}>
            <MachineBadge variant={statusVariant(signal.status)} label={signal.status} />
            <strong>{signal.title}</strong>
          </span>
          <span className={styles.rowSummary}>{signal.summary}</span>
          <span className={styles.rowMeta} translate="no">{signal.kind} · {signalTask(signal)} · {timestamp(signal.created_at, locale)}</span>
        </button>
      ))}
    </div>
  )
}

export function SignalDetailView({ loading, signal, error, onRetry, locale = "en", copy = featureCopyForLocale("en").signals }: { readonly loading: boolean; readonly signal: SignalRecord | null; readonly error?: unknown | null; readonly onRetry?: () => void; readonly locale?: Locale; readonly copy?: SignalsCopy }) {
  if (loading && signal === null) {
    return <div className={styles.detailLoading} role="status" aria-label={copy.loading}><span /><span /><span /></div>
  }
  if (errorStatus(error) === 404) {
    return <div className={styles.emptyState}>{copy.unavailable}</div>
  }
  if (signal === null) {
    if (error) return <div className={styles.emptyState}><Banner status="error" title={copy.detailError} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryDetail} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /></div>
    return <div className={styles.emptyState}>{copy.selectDetail}</div>
  }
  return (
    <article className={styles.detail} aria-label={copy.detail}>
      {error ? <Banner status="warning" title={copy.staleDetail} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryDetail} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /> : null}
      <div className={styles.detailBadges}>
        <MachineBadge variant={statusVariant(signal.status)} label={signal.status} />
        <MachineBadge variant="neutral" label={signal.severity} />
        <MachineBadge variant="neutral" label={signal.kind} />
      </div>
      <Heading level={3}>{signal.title}</Heading>
      <Text as="p" type="supporting" className={styles.detailSummary}>{signal.summary}</Text>
      <dl className={styles.facts}>
        <Fact label={copy.signalId} value={signal.id} />
        <Fact label={copy.observationId} value={signal.observation_id} />
        <Fact label={copy.task} value={signalTask(signal)} />
        <Fact label={copy.source} value={signal.observation.source ?? "—"} />
        <Fact label={copy.actor} value={signal.observation.actor} />
        <Fact label={copy.agentType} value={signal.observation.agent_type ?? "—"} />
        <Fact label={copy.dedupeKey} value={signal.dedupe_key ?? "—"} />
        <Fact label={copy.created} value={timestamp(signal.created_at, locale)} />
      </dl>
      <div className={styles.evidence}>
        <Heading level={4}>{copy.evidence}</Heading>
        <pre>{JSON.stringify(signal.observation.evidence, null, 2)}</pre>
      </div>
    </article>
  )
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return <div><dt>{label}</dt><dd translate="no">{value}</dd></div>
}
