import { useEffect, useMemo, useRef, useState, type ComponentProps, type FormEvent, type ReactNode } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { ButtonGroup } from "@astryxdesign/core/ButtonGroup"
import { EmptyState } from "@astryxdesign/core/EmptyState"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { Text } from "@astryxdesign/core/Text"

import { TextArea, TextInput } from "../../ui/astryx/fields"
import { PageFrame } from "../../ui/astryx/page-frame"
import {
  Grid,
  SafeCard,
  SafeHStack,
  SafeMetadataList,
  SafeMetadataListItem,
  SafeSection,
  SafeVStack,
  Skeleton,
} from "../../ui/astryx/primitives"

import type {
  LabelAtomExplainRecord,
  LabelOntologyActionRecord,
  LabelOntologyReviewGroup,
  LabelOntologySignalDetail,
  LabelOntologySignalRecord,
  SignalsOntologyReadApi,
} from "../../lib/api/signals-ontology-read-model"
import { featureCopyForLocale } from "../../lib/i18n"
import type { Locale } from "../../lib/preferences"
import type { OntologyRouteFilters } from "../../lib/router"
import { usePreferences } from "../../lib/use-preferences"
import { reconcileSelection, useReadState, type ReadPhase, type ReadState } from "../read-state"
import { localizedErrorMessage } from "../safe-error"
import {
  createOntologyLifecycleAttempt,
  executeOntologyLifecycleAttempt,
  sameOntologyLifecycleIdentity,
  type LifecycleAction,
  type OntologyLifecycleAttempt,
  type OntologyLifecycleIdentity,
} from "./ontology-lifecycle"
import { ONTOLOGY_SIGNAL_SCOPE_LIMIT, scopeReviewGroupsToKnownSignals } from "./ontology-review-scope"

export type { LifecycleAction } from "./ontology-lifecycle"

export interface OntologyScreenProps {
  readonly api: SignalsOntologyReadApi | null
  readonly boardName?: string
  readonly filters?: OntologyRouteFilters
  readonly selectedSignalId?: string | null
  /** Incremented by the sync sink when a matching ontology target is invalidated. */
  readonly invalidationRevision?: number
  readonly online?: boolean
  readonly onFiltersChange?: (filters: OntologyRouteFilters) => void
  readonly onSelectSignal?: (signalId: string | null) => void
  readonly onCloseDetail?: () => void
  readonly onLifecycleAction?: (action: LifecycleAction, signalId: string, reason: string) => void | Promise<void>
}

type OntologyCopy = ReturnType<typeof featureCopyForLocale>["ontology"]

function emptySignals(): ReadState<readonly LabelOntologySignalRecord[]> {
  return { phase: "idle", data: [], error: null }
}

function emptyGroups(): ReadState<readonly LabelOntologyReviewGroup[]> {
  return { phase: "idle", data: [], error: null }
}

function emptyDetail(): ReadState<LabelOntologySignalDetail | null> {
  return { phase: "idle", data: null, error: null }
}

function emptyAtom(): ReadState<LabelAtomExplainRecord | null> {
  return { phase: "idle", data: null, error: null }
}

function MachineBadge(props: ComponentProps<typeof Badge>) {
  return <span translate="no"><Badge {...props} /></span>
}

function errorStatus(error: unknown): number | null {
  if (typeof error !== "object" || error === null || !("status" in error)) return null
  const value = (error as { status?: unknown }).status
  return typeof value === "number" ? value : null
}

function statusVariant(status: string): "neutral" | "info" | "success" | "warning" | "error" {
  if (status === "confirmed") return "warning"
  if (status === "resolved") return "success"
  if (status === "rejected" || status === "superseded") return "error"
  return status === "open" ? "info" : "neutral"
}

function localizedSignalStatus(status: string | undefined, copy: OntologyCopy): string {
  if (status === undefined) return copy.none
  return copy.statusLabels[status as keyof typeof copy.statusLabels] ?? status
}

function validationVariant(status: string): "neutral" | "info" | "success" | "warning" | "error" {
  if (status === "passed" || status === "not_required") return "success"
  if (status === "pending" || status === "partial") return "warning"
  if (status === "failed") return "error"
  return "neutral"
}

function signalTitle(signal: LabelOntologySignalRecord): string {
  return signal.target_label_name_snapshot ?? signal.proposed_label_name ?? signal.candidate_text ?? signal.kind
}

function groupTitle(group: LabelOntologyReviewGroup): string {
  return group.label_name ?? group.proposed_label_name ?? group.candidate_text ?? group.key
}

function shortId(value: string | null | undefined): string {
  if (!value) return "—"
  return value.length > 12 ? `${value.slice(0, 10)}…` : value
}

function formatScore(value: number | null): string {
  return value === null ? "—" : value.toFixed(2)
}

function useFilterState(filters: OntologyRouteFilters, onFiltersChange?: (filters: OntologyRouteFilters) => void) {
  const [localFilters, setLocalFilters] = useState<OntologyRouteFilters>(filters)
  const effectiveFilters = onFiltersChange ? filters : localFilters
  const update = (next: OntologyRouteFilters) => {
    setLocalFilters(next)
    onFiltersChange?.(next)
  }
  return { effectiveFilters, update }
}

export function OntologyScreen({
  api,
  boardName,
  filters = {},
  selectedSignalId: selectedSignalIdProp,
  invalidationRevision = 0,
  online = typeof navigator === "undefined" || navigator.onLine,
  onFiltersChange,
  onSelectSignal: onSelectSignalProp,
  onCloseDetail,
  onLifecycleAction,
}: OntologyScreenProps) {
  const { locale } = usePreferences()
  const { effectiveFilters, update: updateFilters } = useFilterState(filters, onFiltersChange)
  const [localSelectedSignalId, setLocalSelectedSignalId] = useState<string | null>(selectedSignalIdProp ?? null)
  const selectedSignalId = selectedSignalIdProp === undefined ? localSelectedSignalId : selectedSignalIdProp
  const [atomDraft, setAtomDraft] = useState("")
  const [atomRef, setAtomRef] = useState(filters.atom ?? "")
  const [actionReason, setActionReason] = useState("")
  const lifecycleIdentity = useMemo<OntologyLifecycleIdentity>(() => ({ api, signalId: selectedSignalId }), [api, selectedSignalId])
  const lifecycleMountedRef = useRef(false)
  const [lifecycleState, setLifecycleState] = useState<{
    readonly identity: OntologyLifecycleIdentity
    readonly attempt: OntologyLifecycleAttempt | null
    readonly pending: boolean
    readonly error: unknown | null
    readonly succeeded: boolean
  }>(() => ({ identity: lifecycleIdentity, attempt: null, pending: false, error: null, succeeded: false }))
  const [listRefreshToken, setListRefreshToken] = useState(0)
  const [detailRefreshToken, setDetailRefreshToken] = useState(0)
  const [atomRefreshToken, setAtomRefreshToken] = useState(0)
  useEffect(() => {
    if (selectedSignalIdProp !== undefined) setLocalSelectedSignalId(selectedSignalIdProp)
  }, [selectedSignalIdProp])
  useEffect(() => {
    lifecycleMountedRef.current = true
    return () => {
      lifecycleMountedRef.current = false
    }
  }, [])
  useEffect(() => {
    setLifecycleState((previous) => sameOntologyLifecycleIdentity(previous.identity, lifecycleIdentity)
      ? previous
      : { ...previous, identity: lifecycleIdentity, pending: false, error: null, succeeded: false })
  }, [lifecycleIdentity])
  useEffect(() => {
    setActionReason("")
  }, [selectedSignalId])
  useEffect(() => {
    setAtomRef(filters.atom ?? "")
    setAtomDraft(filters.atom ?? "")
  }, [filters.atom])
  const includeAll = effectiveFilters.includeAll === true
  const groupBy = effectiveFilters.groupBy ?? "label"
  const filterKey = JSON.stringify({ includeAll, groupBy })

  const signalsRequest = useMemo(() => {
    if (!api) return null
    return (signal: AbortSignal) => api.listLabelOntologySignals({ statuses: includeAll ? [] : ["open", "confirmed"], kinds: [], includeAll, limit: 100 }, signal)
  }, [api, includeAll])
  const signals = useReadState(Boolean(api), signalsRequest, `ontology-signals:${api?.cacheKey ?? "none"}:${filterKey}:${listRefreshToken}:${invalidationRevision}`, emptySignals())

  const reviewRequest = useMemo(() => {
    if (!api) return null
    return (signal: AbortSignal) => api.reviewLabelOntology({ groupBy, includeAll, limit: 100 }, signal)
  }, [api, groupBy, includeAll])
  const groups = useReadState(Boolean(api), reviewRequest, `ontology-review:${api?.cacheKey ?? "none"}:${filterKey}:${listRefreshToken}:${invalidationRevision}`, emptyGroups())
  const scopedGroups = useMemo(
    () => scopeReviewGroupsToKnownSignals(groups.data, signals.data, signals.phase === "success" && signals.data.length < ONTOLOGY_SIGNAL_SCOPE_LIMIT),
    [groups.data, signals.data, signals.phase],
  )

  const detailRequest = useMemo(() => {
    if (!api || !selectedSignalId) return null
    return (signal: AbortSignal) => api.getLabelOntologySignal(selectedSignalId, signal)
  }, [api, selectedSignalId])
  const detail = useReadState(Boolean(api && selectedSignalId), detailRequest, `ontology-signal:${api?.cacheKey ?? "none"}:${selectedSignalId ?? "none"}:${detailRefreshToken}:${invalidationRevision}`, emptyDetail())
  const visibleSignals = useMemo(
    () => errorStatus(detail.error) === 404 && selectedSignalId !== null
      ? signals.data.filter((signal) => signal.id !== selectedSignalId)
      : signals.data,
    [detail.error, selectedSignalId, signals.data],
  )

  useEffect(() => {
    // Filtered rows do not prove a deep-linked signal is invalid. Reconcile
    // only after the authoritative detail endpoint returns 404.
    if (signals.phase !== "success" || selectedSignalId === null || errorStatus(detail.error) !== 404) return
    const nextId = reconcileSelection(selectedSignalId, signals.data.filter((signal) => signal.id !== selectedSignalId))
    if (nextId === selectedSignalId) return
    setLocalSelectedSignalId(nextId)
    onSelectSignalProp?.(nextId)
  }, [detail.error, onSelectSignalProp, selectedSignalId, signals.data, signals.phase])

  const atomRequest = useMemo(() => {
    if (!api || !atomRef) return null
    return (signal: AbortSignal) => api.explainLabelAtom(atomRef, signal)
  }, [api, atomRef])
  const atom = useReadState(Boolean(api && atomRef), atomRequest, `ontology-atom:${api?.cacheKey ?? "none"}:${atomRef}:${atomRefreshToken}:${invalidationRevision}`, emptyAtom())

  useEffect(() => {
    if (!lifecycleState.succeeded || !sameOntologyLifecycleIdentity(lifecycleState.identity, lifecycleIdentity)) return
    setActionReason("")
    setListRefreshToken((value) => value + 1)
    setDetailRefreshToken((value) => value + 1)
    setAtomRefreshToken((value) => value + 1)
    setLifecycleState((previous) => previous.succeeded && sameOntologyLifecycleIdentity(previous.identity, lifecycleIdentity)
      ? { ...previous, succeeded: false }
      : previous)
  }, [lifecycleIdentity, lifecycleState.identity, lifecycleState.succeeded])

  const selectSignal = (signalId: string | null) => {
    setLocalSelectedSignalId(signalId)
    onSelectSignalProp?.(signalId)
  }
  const submitAtomSearch = (event: FormEvent) => {
    event.preventDefault()
    const value = atomDraft.trim()
    if (value) {
      setAtomRef(value)
      updateFilters({ ...effectiveFilters, atom: value })
    }
  }
  const selectAtom = (value: string | null | undefined) => {
    const next = value?.trim()
    if (!next) return
    setAtomDraft(next)
    setAtomRef(next)
    updateFilters({ ...effectiveFilters, atom: next })
  }
  const runLifecycleAttempt = async (attempt: OntologyLifecycleAttempt) => {
    if (!onLifecycleAction) return
    setLifecycleState((previous) => ({
      ...previous,
      identity: attempt.identity,
      attempt,
      pending: true,
      error: null,
      succeeded: false,
    }))
    const result = await executeOntologyLifecycleAttempt(attempt, onLifecycleAction, () => lifecycleMountedRef.current)
    if (result.kind === "stale" || !lifecycleMountedRef.current) return
    setLifecycleState((previous) => {
      if (!sameOntologyLifecycleIdentity(previous.identity, attempt.identity) || previous.attempt !== attempt) return previous
      return result.kind === "success"
        ? { ...previous, pending: false, error: null, succeeded: true }
        : { ...previous, pending: false, error: result.error, succeeded: false }
    })
  }
  const runLifecycleAction = (action: LifecycleAction) => {
    const signalId = selectedSignalId?.trim()
    const reason = actionReason.trim()
    if (!signalId || !reason || !onLifecycleAction) return
    void runLifecycleAttempt(createOntologyLifecycleAttempt(action, signalId, reason, lifecycleIdentity))
  }
  const refreshRows = () => setListRefreshToken((value) => value + 1)
  const refreshDetail = () => setDetailRefreshToken((value) => value + 1)
  const refreshAtom = () => setAtomRefreshToken((value) => value + 1)
  const refresh = () => {
    refreshRows()
    refreshDetail()
    refreshAtom()
  }
  const retryLifecycleAction = () => {
    if (lifecycleState.attempt !== null) void runLifecycleAttempt(lifecycleState.attempt)
  }
  const lifecycleVisible = sameOntologyLifecycleIdentity(lifecycleState.identity, lifecycleIdentity)
  const actionPending = lifecycleVisible && lifecycleState.pending
  const actionError = lifecycleVisible ? lifecycleState.error : null
  const detailData = detail.error === null && detail.data?.signal.id === selectedSignalId ? detail.data : null

  return (
    <OntologyScreenView
      boardName={boardName ?? api?.board ?? "—"}
      copy={featureCopyForLocale(locale).ontology}
      locale={locale}
      filters={effectiveFilters}
      signals={{ ...signals, data: visibleSignals }}
      groups={{ ...groups, data: scopedGroups }}
      detail={{ phase: detail.phase, data: detailData, error: detail.error }}
      atom={atom}
      selectedSignalId={selectedSignalId}
      atomRef={atomRef}
      online={online}
      actionReason={actionReason}
      actionPending={actionPending}
      actionError={actionError}
      onRetryAction={retryLifecycleAction}
      onRefresh={refresh}
      onRefreshRows={refreshRows}
      onRefreshGroups={refreshRows}
      onRefreshDetail={refreshDetail}
      onRefreshAtom={refreshAtom}
      onFiltersChange={updateFilters}
      onSelectSignal={selectSignal}
      onCloseDetail={onCloseDetail}
      onActionReasonChange={setActionReason}
      onLifecycleAction={(action) => void runLifecycleAction(action)}
      lifecycleEnabled={onLifecycleAction !== undefined}
      onExplainAtom={selectAtom}
      atomDraft={atomDraft}
      onAtomDraftChange={setAtomDraft}
      onAtomSearch={submitAtomSearch}
    />
  )
}

export interface OntologyScreenViewProps {
  readonly boardName: string
  readonly filters: OntologyRouteFilters
  readonly signals: ReadState<readonly LabelOntologySignalRecord[]>
  readonly groups: ReadState<readonly LabelOntologyReviewGroup[]>
  readonly detail: ReadState<LabelOntologySignalDetail | null>
  readonly atom: ReadState<LabelAtomExplainRecord | null>
  readonly selectedSignalId: string | null
  readonly atomRef: string
  readonly online: boolean
  readonly locale?: Locale
  readonly actionReason: string
  readonly actionPending: boolean
  readonly actionError?: unknown | null
  readonly atomDraft?: string
  readonly onRefresh: () => void
  readonly onRefreshRows?: () => void
  readonly onRefreshGroups?: () => void
  readonly onRefreshDetail?: () => void
  readonly onRefreshAtom?: () => void
  readonly onFiltersChange: (filters: OntologyRouteFilters) => void
  readonly onSelectSignal: (signalId: string | null) => void
  readonly onCloseDetail?: () => void
  readonly onActionReasonChange: (value: string) => void
  readonly onLifecycleAction: (action: LifecycleAction) => void
  readonly lifecycleEnabled?: boolean
  readonly onRetryAction?: () => void
  readonly onExplainAtom: (atomRef: string | null | undefined) => void
  readonly onAtomDraftChange?: (value: string) => void
  readonly onAtomSearch: (event: FormEvent) => void
  readonly copy?: OntologyCopy
}

export function OntologyScreenView({
  boardName,
  filters,
  signals,
  groups,
  detail,
  atom,
  selectedSignalId,
  atomRef,
  online,
  locale = "en",
  actionReason,
  actionPending,
  actionError,
  onRetryAction,
  atomDraft = "",
  onRefresh,
  onRefreshRows = onRefresh,
  onRefreshGroups = onRefresh,
  onRefreshDetail = onRefresh,
  onRefreshAtom = onRefresh,
  onFiltersChange,
  onSelectSignal,
  onCloseDetail,
  onActionReasonChange,
  onLifecycleAction,
  lifecycleEnabled = false,
  onExplainAtom,
  onAtomDraftChange,
  onAtomSearch,
  copy = featureCopyForLocale("en").ontology,
}: OntologyScreenViewProps) {
  const stale = signals.data.length > 0 && signals.phase === "error"
  const includeAll = filters.includeAll === true
  const groupBy = filters.groupBy ?? "label"
  const viewSignals = detail.error && errorStatus(detail.error) === 404 && selectedSignalId !== null
    ? signals.data.filter((signal) => signal.id !== selectedSignalId)
    : signals.data
  return (
    <PageFrame
      frame="workspace"
      aria-labelledby="ontology-title"
      data-testid="ontology-screen"
      bodyLabel={copy.heading}
      bodyOverflow="none"
      header={(
        <SafeHStack gap={4} justify="between" align="start" wrap="wrap">
          <SafeVStack gap={1}>
            <Heading level={1} id="ontology-title">{copy.heading}</Heading>
            <Text as="p" type="supporting">{copy.lede}</Text>
            <Text as="p" type="supporting">{copy.board} · <MachineText>{boardName}</MachineText></Text>
          </SafeVStack>
          <SafeHStack gap={1} wrap="wrap">
            <Button label={includeAll ? copy.allHistory : copy.openConfirmed} variant={includeAll ? "primary" : "secondary"} size="sm" aria-pressed={includeAll} onClick={() => onFiltersChange({ ...filters, includeAll: !includeAll })} />
            <Button label={copy.refresh} variant="secondary" size="sm" onClick={onRefresh} isLoading={signals.phase === "loading" || groups.phase === "loading" || signals.phase === "refreshing" || groups.phase === "refreshing"} />
          </SafeHStack>
        </SafeHStack>
      )}
    >
      <SafeVStack gap={4} padding={4}>
        {!online ? <Banner status="warning" title={copy.offline} description={stale ? copy.offlineStale : copy.offlineConnect} /> : null}
        {signals.error ? <Banner status="error" title={copy.rowsError} description={localizedErrorMessage(signals.error, copy.unreadableResponse, locale)} endContent={<Button label={copy.retryRows} variant="ghost" size="sm" onClick={onRefreshRows} />} /> : null}
        {groups.error ? <Banner status="error" title={copy.groupsError} description={localizedErrorMessage(groups.error, copy.unreadableResponse, locale)} endContent={<Button label={copy.retryGroups} variant="ghost" size="sm" onClick={onRefreshGroups} />} /> : null}

        <Grid label={`${copy.signalRows} / ${copy.groupedReview} / ${copy.signalDetail}`} columns="auto-lg" gap={4}>
        <SafeCard padding={0}>
          <SafeVStack gap={0}>
            <PanelHeader title={copy.signalRows} meta={copy.loadedCount(viewSignals.length)} refreshing={signals.phase === "refreshing"} copy={copy} />
            <OntologySignalListView phase={signals.phase} signals={viewSignals} selectedSignalId={selectedSignalId} onSelectSignal={onSelectSignal} copy={copy} />
          </SafeVStack>
        </SafeCard>
        <SafeCard padding={0}>
          <SafeVStack gap={0}>
            <PanelHeader title={copy.groupedReview} meta={copy.groupsCount(groups.data.length)} refreshing={groups.phase === "refreshing"} copy={copy} />
            <SafeHStack gap={1} padding={3} wrap="wrap" role="group" aria-label={copy.groupBy}>
              {(["label", "candidate_atom", "proposed_label"] as const).map((candidate) => <Button key={candidate} label={candidate === "candidate_atom" ? copy.atom : candidate === "proposed_label" ? copy.proposal : copy.label} variant={groupBy === candidate ? "primary" : "ghost"} size="sm" aria-pressed={groupBy === candidate} onClick={() => onFiltersChange({ ...filters, groupBy: candidate })} />)}
            </SafeHStack>
            <ReviewGroupsView phase={groups.phase} groups={groups.data} onSelectSignal={onSelectSignal} copy={copy} />
          </SafeVStack>
        </SafeCard>
        <SafeVStack gap={4}>
          <SafeCard padding={0}>
            <PanelHeader title={copy.signalDetail} meta={localizedSignalStatus(detail.data?.signal.status, copy)} refreshing={detail.phase === "refreshing"} copy={copy} endContent={selectedSignalId !== null && onCloseDetail ? <Button label={copy.closeDetail} variant="ghost" size="sm" onClick={onCloseDetail} /> : null} />
            <OntologySignalDetailView phase={detail.phase} detail={detail.data} error={detail.error} actionError={actionError} onRetryAction={onRetryAction} actionReason={actionReason} actionPending={actionPending} lifecycleEnabled={lifecycleEnabled} onActionReasonChange={onActionReasonChange} onLifecycleAction={onLifecycleAction} onExplainAtom={onExplainAtom} locale={locale} copy={copy} onRetry={onRefreshDetail} />
          </SafeCard>
          <SafeCard padding={0}>
            <PanelHeader title={copy.atomExplain} meta={atom.phase === "refreshing" ? copy.refreshing : atomRef || copy.none} refreshing={atom.phase === "refreshing"} copy={copy} />
            <form onSubmit={onAtomSearch}>
              <SafeHStack gap={2} padding={4} align="end" wrap="wrap">
                <SafeVStack className="min-w-0 flex-1">
                  <TextInput label={copy.atomInput} isLabelHidden value={atomDraft} onChange={(value) => onAtomDraftChange?.(value)} placeholder={copy.atomInput} htmlName="atom-ref" />
                </SafeVStack>
                <Button label={copy.explain} type="submit" variant="secondary" size="sm" isDisabled={!atomDraft.trim()} />
              </SafeHStack>
            </form>
            <AtomExplainView phase={atom.phase} explain={atom.data} error={atom.error} onRetry={onRefreshAtom} locale={locale} copy={copy} />
          </SafeCard>
        </SafeVStack>
        </Grid>
      </SafeVStack>
    </PageFrame>
  )
}

function PanelHeader({ title, meta, refreshing, endContent, copy = featureCopyForLocale("en").ontology }: { readonly title: string; readonly meta: string; readonly refreshing: boolean; readonly endContent?: ReactNode; readonly copy?: OntologyCopy }) {
  return <SafeHStack padding={4} gap={2} justify="between" align="start" wrap="wrap"><SafeVStack gap={1}><Heading level={2}>{title}</Heading><Text type="supporting">{meta}</Text></SafeVStack><SafeHStack gap={1} wrap="wrap">{endContent}{refreshing ? <Badge variant="warning" label={copy.refreshing} /> : null}</SafeHStack></SafeHStack>
}

export function OntologySignalListView({ phase, signals, selectedSignalId, onSelectSignal, copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly signals: readonly LabelOntologySignalRecord[]; readonly selectedSignalId: string | null; readonly onSelectSignal: (signalId: string) => void; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && signals.length === 0) return <LoadingState label={copy.signalRows} />
  if (signals.length === 0) return <EmptyState title={copy.noRows} isCompact />
  return <List density="compact" hasDividers>
    {signals.map((signal) => (
      <ListItem
        key={signal.id}
        label={<SafeHStack gap={1} wrap="wrap"><Text weight="semibold">{signalTitle(signal)}</Text><MachineBadge variant={statusVariant(signal.status)} label={signal.status} /></SafeHStack>}
        description={<SafeVStack gap={1} align="start"><Text type="code"><MachineText>{signal.id} · {signal.kind}</MachineText></Text><SafeHStack gap={1} wrap="wrap"><MachineBadge variant="neutral" label={signal.proposed_action} />{signal.suggest_score === null ? null : <Badge variant="neutral" label={`${copy.recordedScore} ${formatScore(signal.suggest_score)}`} />}</SafeHStack></SafeVStack>}
        isSelected={selectedSignalId === signal.id}
        onClick={() => onSelectSignal(signal.id)}
      />
    ))}
  </List>
}

export function ReviewGroupsView({ phase, groups, onSelectSignal, copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly groups: readonly LabelOntologyReviewGroup[]; readonly onSelectSignal: (signalId: string) => void; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && groups.length === 0) return <LoadingState label={copy.groupedReview} />
  if (groups.length === 0) return <EmptyState title={copy.noGroups} isCompact />
  return <List density="spacious" hasDividers>
    {groups.map((group) => (
      <ListItem
        key={`${group.group_by}:${group.key}`}
        label={groupTitle(group)}
        endContent={<Badge variant="warning" label={copy.sourceTasks(group.task_count)} />}
        description={(
          <SafeVStack gap={3} paddingBlock={2}>
            <Text type="code"><MachineText>{group.sample_task_refs.length ? group.sample_task_refs.join(", ") : group.key}</MachineText></Text>
            {group.candidate_text ? <Text type="supporting">{group.candidate_text}</Text> : null}
            <SafeMetadataList columns="multi" label={{ position: "top" }}>
              <SafeMetadataListItem label={copy.signalCount(group.signal_count).replace(String(group.signal_count), "").trim()}>{group.signal_count}</SafeMetadataListItem>
              <SafeMetadataListItem label={copy.openCount(group.open_count).replace(String(group.open_count), "").trim()}>{group.open_count}</SafeMetadataListItem>
              <SafeMetadataListItem label={copy.confirmedCount(group.confirmed_count).replace(String(group.confirmed_count), "").trim()}>{group.confirmed_count}</SafeMetadataListItem>
              <SafeMetadataListItem label={copy.actionCountLabel(group.action_count).replace(String(group.action_count), "").trim()}>{group.action_count}</SafeMetadataListItem>
            </SafeMetadataList>
            <SafeHStack gap={1} wrap="wrap">
              {group.signal_ids.slice(0, 4).map((id) => <Button key={id} label={shortId(id)} variant="ghost" size="sm" onClick={() => onSelectSignal(id)} />)}
            </SafeHStack>
          </SafeVStack>
        )}
      />
    ))}
  </List>
}

export function OntologySignalDetailView({ phase, detail, error, actionError, onRetry, onRetryAction, actionReason, actionPending, lifecycleEnabled = false, onActionReasonChange, onLifecycleAction, onExplainAtom, locale = "en", copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly detail: LabelOntologySignalDetail | null; readonly error?: unknown | null; readonly actionError?: unknown | null; readonly onRetry?: () => void; readonly onRetryAction?: () => void; readonly actionReason: string; readonly actionPending: boolean; readonly lifecycleEnabled?: boolean; readonly onActionReasonChange: (value: string) => void; readonly onLifecycleAction: (action: LifecycleAction) => void; readonly onExplainAtom: (atomRef: string | null | undefined) => void; readonly locale?: Locale; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && detail === null) return <LoadingState label={copy.signalDetail} />
  if (errorStatus(error) === 404) return <EmptyState title={copy.unavailable} isCompact />
  if (detail === null) {
    if (error) return <SafeVStack padding={4}><Banner status="error" title={copy.detailError} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryDetail} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /></SafeVStack>
    return <EmptyState title={copy.selectDetail} isCompact />
  }
  const signal = detail.signal
  const ready = Boolean(actionReason.trim()) && !actionPending && lifecycleEnabled
  const canConfirm = ready && signal.status === "open"
  const canReview = ready && (signal.status === "open" || signal.status === "confirmed")
  return <SafeVStack as="article" gap={3} padding={4} aria-label={copy.signalDetail}>
    <SafeHStack gap={1} wrap="wrap"><MachineBadge variant={statusVariant(signal.status)} label={signal.status} /><MachineBadge variant="neutral" label={signal.kind} /><MachineBadge variant="neutral" label={signal.proposed_action} />{detail.observation.suggest_degraded ? <Badge variant="warning" label={copy.observationDegraded} /> : null}</SafeHStack>
    <Heading level={3}>{signalTitle(signal)}</Heading>
    <Text as="p" type="supporting">{signal.rationale || copy.rationaleMissing}</Text>
    <SafeMetadataList columns="multi" label={{ position: "top" }}>
      <Fact label={copy.sourceTask} value={detail.observation.task_ref_snapshot} />
      <Fact label={copy.target} value={signal.target_label_name_snapshot ?? signal.proposed_label_name ?? "—"} />
      <Fact label={copy.suggest} value={signal.suggest_state ?? "—"} />
      <Fact label={copy.recordedScore} value={formatScore(signal.suggest_score)} />
      <Fact label={copy.rank} value={signal.suggest_rank === null ? "—" : String(signal.suggest_rank)} />
      <Fact label={copy.recordedConfidence} value={formatScore(signal.confidence)} />
    </SafeMetadataList>
    {signal.candidate_text ? <SafeSection variant="muted" padding={3}><SafeVStack gap={2}><SafeHStack gap={2} justify="between" align="center" wrap="wrap"><Text type="label">{copy.candidateAtom}</Text><Button label={copy.explainHash} variant="ghost" size="sm" onClick={() => onExplainAtom(signal.candidate_content_hash)} /></SafeHStack><Text as="p">{signal.candidate_text}</Text><MachineText>{signal.candidate_atom_polarity ?? "—"} / {signal.candidate_atom_kind ?? "—"} / {signal.candidate_content_hash ?? "—"}</MachineText></SafeVStack></SafeSection> : null}
    <SafeVStack gap={3}>
      {actionError ? <Banner status="error" title={copy.actionError} description={`${localizedErrorMessage(actionError, copy.unreadableResponse, locale)} ${copy.actionNextStep}`} endContent={onRetryAction ? <Button label={copy.retryAction} variant="ghost" size="sm" onClick={onRetryAction} /> : undefined} /> : null}
      <TextArea label={copy.reviewReason} value={actionReason} onChange={(value) => onActionReasonChange(value)} placeholder={copy.reasonPlaceholder} rows={3} htmlName="ontology-action-reason" />
      <ButtonGroup label={copy.reviewReason} size="sm">
        <Button label={copy.confirm} variant="primary" isDisabled={!canConfirm} isLoading={actionPending} onClick={() => onLifecycleAction("confirm")} />
        <Button label={copy.resolveNoChange} variant="secondary" isDisabled={!canReview || actionPending} onClick={() => onLifecycleAction("resolve_no_change")} />
        <Button label={copy.reject} variant="secondary" isDisabled={!canReview || actionPending} onClick={() => onLifecycleAction("reject")} />
      </ButtonGroup>
    </SafeVStack>
    <ActionHistory actions={detail.actions} onExplainAtom={onExplainAtom} copy={copy} />
  </SafeVStack>
}

function ActionHistory({ actions, onExplainAtom, copy = featureCopyForLocale("en").ontology }: { readonly actions: readonly LabelOntologyActionRecord[]; readonly onExplainAtom: (atomRef: string | null | undefined) => void; readonly copy?: OntologyCopy }) {
  if (actions.length === 0) return <Text as="p" type="supporting">{copy.noActions}</Text>
  return <SafeSection variant="transparent" dividers={["top"]} padding={4}><SafeVStack gap={2}><Heading level={4}>{copy.actions}</Heading><List density="compact" hasDividers>{actions.map((action) => <ListItem key={action.id} label={<SafeHStack gap={1} wrap="wrap"><MachineBadge variant="neutral" label={action.action_type} /><MachineBadge variant="neutral" label={copy.requiresValidation(action.validation_requirement)} /><MachineBadge variant={validationVariant(action.validation_effective_outcome)} label={action.validation_effective_outcome} /><MachineText>{shortId(action.id)}</MachineText></SafeHStack>} description={<SafeVStack gap={1}><Text as="p" type="supporting">{action.reason}</Text>{action.result_atom_id || action.result_atom_content_hash ? <SafeHStack gap={1} wrap="wrap">{action.result_atom_id ? <Button label={copy.atomRef(shortId(action.result_atom_id))} variant="ghost" size="sm" onClick={() => onExplainAtom(action.result_atom_id)} /> : null}{action.result_atom_content_hash ? <Button label={copy.hashRef(shortId(action.result_atom_content_hash))} variant="ghost" size="sm" onClick={() => onExplainAtom(action.result_atom_content_hash)} /> : null}</SafeHStack> : null}</SafeVStack>} />)}</List></SafeVStack></SafeSection>
}

export function AtomExplainView({ phase, explain, error, onRetry, locale = "en", copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly explain: LabelAtomExplainRecord | null; readonly error?: unknown | null; readonly onRetry?: () => void; readonly locale?: Locale; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && explain === null) return <LoadingState label={copy.atomExplain} count={2} />
  if (explain === null) {
    if (error) return <SafeVStack padding={4}><Banner status="error" title={copy.atomError} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryAtom} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /></SafeVStack>
    return <EmptyState title={copy.enterAtom} isCompact />
  }
  if (error) return <SafeVStack as="article" padding={4}><Banner status="warning" title={copy.atomError} description={localizedErrorMessage(error, copy.unreadableResponse, locale)} endContent={onRetry ? <Button label={copy.retryAtom} variant="ghost" size="sm" onClick={onRetry} /> : undefined} /></SafeVStack>
  return <SafeVStack as="article" gap={3} padding={4}>
    <SafeHStack gap={1} wrap="wrap"><Badge variant={explain.legacy_untracked ? "warning" : "success"} label={explain.legacy_untracked ? copy.legacyUntracked : copy.hasProvenance} />{explain.atom ? <Badge variant="neutral" label={explain.atom.label_name} /> : null}</SafeHStack>
    {explain.atom ? <SafeSection variant="muted" padding={3}><SafeVStack gap={1}><MachineText>{explain.atom.kind}</MachineText><Text as="p" type="supporting">{explain.atom.text}</Text><MachineText>{explain.atom.id} / {explain.atom.content_hash}</MachineText></SafeVStack></SafeSection> : <Text as="p" type="supporting">{copy.noCurrentAtom(explain.query)}</Text>}
    {explain.legacy_reason ? <Text as="p" type="supporting">{explain.legacy_reason}</Text> : null}
    <SafeMetadataList columns="multi" label={{ position: "top" }}>
      <SafeMetadataListItem label={copy.actionCount(explain.provenance_actions.length).replace(String(explain.provenance_actions.length), "").trim()}>{explain.provenance_actions.length}</SafeMetadataListItem>
      <SafeMetadataListItem label={copy.supportingCount(explain.supporting_signals.length).replace(String(explain.supporting_signals.length), "").trim()}>{explain.supporting_signals.length}</SafeMetadataListItem>
      <SafeMetadataListItem label={copy.validationCount(explain.validation_history.length).replace(String(explain.validation_history.length), "").trim()}>{explain.validation_history.length}</SafeMetadataListItem>
    </SafeMetadataList>
    {explain.provenance_actions.length > 0 ? <List density="compact" hasDividers>{explain.provenance_actions.slice(0, 4).map((entry) => <ListItem key={entry.action.id} label={<SafeHStack gap={1} wrap="wrap"><MachineBadge variant="neutral" label={entry.action.action_type} /><MachineText>{entry.matched_by}</MachineText></SafeHStack>} description={<Text as="p" type="supporting">{entry.action.reason}</Text>} />)}</List> : null}
    {explain.validation_history.length > 0 ? <List density="compact" hasDividers>{explain.validation_history.slice(0, 4).map((entry) => <ListItem key={entry.action.id} label={<SafeHStack gap={1} wrap="wrap"><MachineBadge variant={validationVariant(entry.validation_status)} label={entry.validation_status} /><MachineText>{copy.parentRef(shortId(entry.parent_action_id))}</MachineText></SafeHStack>} description={entry.warnings.length ? <Text as="p" type="supporting">{entry.warnings.join("; ")}</Text> : undefined} />)}</List> : null}
  </SafeVStack>
}

function LoadingState({ label, count = 3 }: { readonly label: string; readonly count?: number }) {
  return <SafeVStack gap={2} padding={4} role="status" aria-label={label}>{Array.from({ length: count }, (_, index) => <Skeleton key={index} size="row" />)}</SafeVStack>
}

function MachineText({ children }: { readonly children: ReactNode }) {
  return <span translate="no">{children}</span>
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return <SafeMetadataListItem label={label}><MachineText>{value}</MachineText></SafeMetadataListItem>
}
