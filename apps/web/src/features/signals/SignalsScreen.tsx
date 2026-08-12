import { useEffect, useMemo, useState, type ComponentProps } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { EmptyState } from "@astryxdesign/core/EmptyState"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { Text } from "@astryxdesign/core/Text"

import { TextInput } from "../../ui/astryx/fields"
import { PageFrame } from "../../ui/astryx/page-frame"
import {
  CodeBlock,
  Grid,
  SafeCard,
  SafeHStack,
  SafeMetadataList,
  SafeMetadataListItem,
  SafeVStack,
  Skeleton,
} from "../../ui/astryx/primitives"

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
    <PageFrame
      frame="workspace"
      data-testid="signals-screen"
      aria-labelledby="signals-title"
      header={(
        <SafeHStack gap={4} justify="between" align="start" wrap="wrap">
            <SafeVStack gap={1}>
              <Text as="p" type="supporting">{copy.eyebrow}</Text>
              <Heading level={1} id="signals-title">{copy.heading}</Heading>
              <Text as="p" type="supporting">{copy.lede}</Text>
              <Text as="p" type="supporting">{copy.board} · <code translate="no">{boardName}</code></Text>
            </SafeVStack>
            <Button label={copy.refresh} variant="secondary" size="sm" onClick={onRefresh} isLoading={list.phase === "loading" || list.phase === "refreshing"} />
        </SafeHStack>
      )}
      toolbarLabel={copy.filters}
      toolbar={(
        <SafeVStack gap={2}>
          <SafeHStack gap={1} wrap="wrap" role="group" aria-label={copy.status}>
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
          </SafeHStack>
          <SafeHStack gap={2} wrap="wrap" align="end">
            <SafeVStack className="min-w-0 flex-1">
              <TextInput
                label={copy.kind}
                value={kindsValue}
                placeholder={copy.kindPlaceholder}
                onChange={(value) => onFiltersChange({ ...filters, kinds: value.split(",").map((item) => item.trim()).filter(Boolean) })}
                htmlName="signal-kind"
              />
            </SafeVStack>
            <SafeVStack className="min-w-0 flex-1">
              <TextInput
                label={copy.taskRef}
                value={filters.task ?? ""}
                placeholder={copy.taskRefPlaceholder}
                onChange={(value) => onFiltersChange({ ...filters, task: value.trim() || undefined })}
                htmlName="signal-task-ref"
              />
            </SafeVStack>
          </SafeHStack>
        </SafeVStack>
      )}
      bodyLabel={copy.heading}
      bodyOverflow="none"
    >
      <SafeVStack gap={4} padding={4}>
            {!online ? (
              <Banner container="section" status="warning" title={copy.offline} description={stale ? copy.offlineStale : copy.offlineConnect} />
            ) : null}
            {list.error ? (
              <Banner
                container="section"
                status="error"
                title={copy.loadError}
                description={stale ? `${localizedErrorMessage(list.error, copy.unreadableResponse, locale)} · ${copy.staleRows}` : localizedErrorMessage(list.error, copy.unreadableResponse, locale)}
                endContent={<Button label={copy.retryList} variant="ghost" size="sm" onClick={onRefreshList} />}
              />
            ) : null}

            <Grid label={`${copy.signalRows} / ${copy.detail}`} columns="two" gap={4}>
              <SafeCard padding={0}>
                <SafeVStack gap={0}>
                    <SafeHStack padding={4} gap={2} justify="between" align="start" wrap="wrap">
                      <SafeVStack gap={1}>
                        <Heading level={2}>{copy.signalRows}</Heading>
                        <Text as="p" type="supporting">{copy.loadedCount(viewSignals.length)}</Text>
                      </SafeVStack>
                      {list.phase === "refreshing" ? <Badge variant="warning" label={copy.refreshing} /> : null}
                    </SafeHStack>
                    <SignalListView
                      phase={list.phase}
                      signals={viewSignals}
                      selectedSignalId={selectedSignalId}
                      onSelectSignal={onSelectSignal}
                      locale={locale}
                      copy={copy}
                    />
                </SafeVStack>
              </SafeCard>
              <SafeCard padding={0}>
                <SafeVStack gap={0}>
                    <SafeHStack padding={4} gap={2} justify="between" align="start" wrap="wrap">
                      <SafeVStack gap={1}>
                        <Heading level={2}>{copy.detail}</Heading>
                        <Text as="p" type="supporting">
                          {selectedSignalId === null ? copy.noneSelected : <span translate="no">{selectedSignalId}</span>}
                        </Text>
                      </SafeVStack>
                      <SafeHStack gap={1} wrap="wrap">
                        {selectedSignalId !== null && onCloseDetail ? <Button label={copy.closeDetail} variant="ghost" size="sm" onClick={onCloseDetail} /> : null}
                        {detail.phase === "refreshing" ? <Badge variant="warning" label={copy.refreshing} /> : null}
                      </SafeHStack>
                    </SafeHStack>
                    <SignalDetailView loading={detail.phase === "loading"} signal={detail.data} error={detail.error} onRetry={onRefreshDetail} locale={locale} copy={copy} />
                </SafeVStack>
              </SafeCard>
            </Grid>
      </SafeVStack>
    </PageFrame>
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
    return <SignalLoadingState label={copy.loading} />
  }
  if (signals.length === 0) {
    return <EmptyState title={copy.noSignals} isCompact />
  }
  return (
    <List density="compact" hasDividers data-testid="signals-list">
      {signals.map((signal) => (
        <ListItem
          key={signal.id}
          label={(
            <SafeHStack gap={1} wrap="wrap" align="center">
              <MachineBadge variant={statusVariant(signal.status)} label={signal.status} />
              <Text weight="semibold">{signal.title}</Text>
            </SafeHStack>
          )}
          description={(
            <SafeVStack gap={1}>
              <Text type="supporting">{signal.summary}</Text>
              <Text as="span" type="code"><span translate="no">{signal.kind} · {signalTask(signal)} · {timestamp(signal.created_at, locale)}</span></Text>
            </SafeVStack>
          )}
          isSelected={signal.id === selectedSignalId}
          onClick={() => onSelectSignal(signal.id)}
        />
      ))}
    </List>
  )
}

export function SignalDetailView({ loading, signal, error, onRetry, locale = "en", copy = featureCopyForLocale("en").signals }: { readonly loading: boolean; readonly signal: SignalRecord | null; readonly error?: unknown | null; readonly onRetry?: () => void; readonly locale?: Locale; readonly copy?: SignalsCopy }) {
  if (loading && signal === null) {
    return <SignalLoadingState label={copy.loading} />
  }
  if (errorStatus(error) === 404) {
    return <EmptyState title={copy.unavailable} isCompact />
  }
  if (signal === null) {
    if (error) {
      return <SafeVStack padding={4}><Banner container="section" status="error" title={copy.detailError} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryDetail} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /></SafeVStack>
    }
    return <EmptyState title={copy.selectDetail} isCompact />
  }
  return (
    <SafeVStack as="article" gap={4} padding={4} aria-label={copy.detail}>
      {error ? <Banner container="section" status="warning" title={copy.staleDetail} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryDetail} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /> : null}
      <SafeHStack gap={1} wrap="wrap">
        <MachineBadge variant={statusVariant(signal.status)} label={signal.status} />
        <MachineBadge variant="neutral" label={signal.severity} />
        <MachineBadge variant="neutral" label={signal.kind} />
      </SafeHStack>
      <Heading level={3}>{signal.title}</Heading>
      <Text as="p" type="supporting">{signal.summary}</Text>
      <SafeMetadataList columns="multi" label={{ position: "top" }}>
        <Fact label={copy.signalId} value={signal.id} />
        <Fact label={copy.observationId} value={signal.observation_id} />
        <Fact label={copy.task} value={signalTask(signal)} />
        <Fact label={copy.source} value={signal.observation.source ?? "—"} />
        <Fact label={copy.actor} value={signal.observation.actor} />
        <Fact label={copy.agentType} value={signal.observation.agent_type ?? "—"} />
        <Fact label={copy.dedupeKey} value={signal.dedupe_key ?? "—"} />
        <Fact label={copy.created} value={timestamp(signal.created_at, locale)} />
      </SafeMetadataList>
      <SafeVStack as="section" gap={2}>
        <Heading level={4}>{copy.evidence}</Heading>
        <CodeBlock
          code={JSON.stringify(signal.observation.evidence, null, 2)}
          language="json"
          label={copy.evidence}
          hasCopy={false}
          copyLabel={copy.evidence}
          copiedLabel={copy.evidence}
          errorLabel={copy.evidence}
          isWrapped
          maxHeight="evidence"
          container="section"
        />
      </SafeVStack>
    </SafeVStack>
  )
}

function SignalLoadingState({ label }: { readonly label: string }) {
  return (
    <SafeVStack gap={2} padding={4} role="status" aria-label={label}>
      <Text type="supporting">{label}</Text>
      <Skeleton size="row" />
    </SafeVStack>
  )
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return <SafeMetadataListItem label={label}><Text as="span" type="code"><span translate="no">{value}</span></Text></SafeMetadataListItem>
}
