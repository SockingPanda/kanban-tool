import { useEffect, useMemo, useState, type ChangeEvent, type FormEvent } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"

import type {
  LabelAtomExplainRecord,
  LabelOntologyActionRecord,
  LabelOntologyReviewGroup,
  LabelOntologySignalDetail,
  LabelOntologySignalRecord,
  SignalsOntologyReadApi,
} from "../../lib/api/signals-ontology-read-model"
import { featureCopyForLocale } from "../../lib/i18n"
import type { OntologyRouteFilters } from "../../lib/router"
import { usePreferences } from "../../lib/use-preferences"
import { reconcileSelection, useReadState, type ReadPhase, type ReadState } from "../read-state"

import styles from "./OntologyScreen.module.css"

export type LifecycleAction = "confirm" | "reject" | "resolve_no_change"

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

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) return error.message
  return fallback
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
  const [atomDraft, setAtomDraft] = useState("")
  const [atomRef, setAtomRef] = useState(filters.atom ?? "")
  const [actionReason, setActionReason] = useState("")
  const [actionPending, setActionPending] = useState(false)
  const [actionError, setActionError] = useState<unknown | null>(null)
  const [listRefreshToken, setListRefreshToken] = useState(0)
  const [detailRefreshToken, setDetailRefreshToken] = useState(0)
  const [atomRefreshToken, setAtomRefreshToken] = useState(0)
  const selectedSignalId = selectedSignalIdProp === undefined ? localSelectedSignalId : selectedSignalIdProp
  useEffect(() => {
    if (selectedSignalIdProp !== undefined) setLocalSelectedSignalId(selectedSignalIdProp)
  }, [selectedSignalIdProp])
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

  useEffect(() => {
    if (signals.phase !== "success") return
    const nextId = reconcileSelection(selectedSignalId, signals.data)
    if (nextId === selectedSignalId) return
    setLocalSelectedSignalId(nextId)
    onSelectSignalProp?.(nextId)
  }, [onSelectSignalProp, selectedSignalId, signals.data, signals.phase])

  const detailRequest = useMemo(() => {
    if (!api || !selectedSignalId) return null
    return (signal: AbortSignal) => api.getLabelOntologySignal(selectedSignalId, signal)
  }, [api, selectedSignalId])
  const detail = useReadState(Boolean(api && selectedSignalId), detailRequest, `ontology-signal:${api?.cacheKey ?? "none"}:${selectedSignalId ?? "none"}:${detailRefreshToken}:${invalidationRevision}`, emptyDetail())

  const atomRequest = useMemo(() => {
    if (!api || !atomRef) return null
    return (signal: AbortSignal) => api.explainLabelAtom(atomRef, signal)
  }, [api, atomRef])
  const atom = useReadState(Boolean(api && atomRef), atomRequest, `ontology-atom:${api?.cacheKey ?? "none"}:${atomRef}:${atomRefreshToken}:${invalidationRevision}`, emptyAtom())

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
  const runLifecycleAction = async (action: LifecycleAction) => {
    if (!selectedSignalId || !actionReason.trim() || !onLifecycleAction) return
    setActionError(null)
    setActionPending(true)
    try {
      await onLifecycleAction(action, selectedSignalId, actionReason.trim())
      setActionReason("")
      setListRefreshToken((value) => value + 1)
      setDetailRefreshToken((value) => value + 1)
      setAtomRefreshToken((value) => value + 1)
    } catch (error) {
      setActionError(error)
    } finally {
      setActionPending(false)
    }
  }
  const refreshRows = () => setListRefreshToken((value) => value + 1)
  const refreshDetail = () => setDetailRefreshToken((value) => value + 1)
  const refreshAtom = () => setAtomRefreshToken((value) => value + 1)
  const refresh = () => {
    refreshRows()
    refreshDetail()
    refreshAtom()
  }
  const detailData = detail.error === null && detail.data?.signal.id === selectedSignalId ? detail.data : null

  return (
    <OntologyScreenView
      boardName={boardName ?? api?.board ?? "—"}
      copy={featureCopyForLocale(locale).ontology}
      filters={effectiveFilters}
      signals={signals}
      groups={groups}
      detail={{ phase: detail.phase, data: detailData, error: detail.error }}
      atom={atom}
      selectedSignalId={selectedSignalId}
      atomRef={atomRef}
      online={online}
      actionReason={actionReason}
      actionPending={actionPending}
      actionError={actionError}
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
  actionReason,
  actionPending,
  actionError,
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
  return (
    <main className={styles.screen} aria-labelledby="ontology-title" data-testid="ontology-screen">
      <header className={styles.hero}>
        <div>
          <Text as="p" type="supporting" className={styles.eyebrow}>{copy.eyebrow}</Text>
          <Heading level={1} id="ontology-title">{copy.heading}</Heading>
          <Text as="p" type="supporting" className={styles.lede}>{copy.lede}</Text>
          <Text as="p" type="supporting" className={styles.boardContext}>{copy.board} · <code translate="no">{boardName}</code></Text>
        </div>
        <div className={styles.heroActions}>
          <Button label={includeAll ? copy.allHistory : copy.openConfirmed} variant={includeAll ? "primary" : "secondary"} size="sm" aria-pressed={includeAll} onClick={() => onFiltersChange({ ...filters, includeAll: !includeAll })} />
          <Button label={copy.refresh} variant="secondary" size="sm" onClick={onRefresh} isLoading={signals.phase === "loading" || groups.phase === "loading" || signals.phase === "refreshing" || groups.phase === "refreshing"} />
        </div>
      </header>

      {!online ? <Banner status="warning" title={copy.offline} description={stale ? copy.offlineStale : copy.offlineConnect} /> : null}
      {signals.error ? <Banner status="error" title={copy.rowsError} description={errorMessage(signals.error, copy.unreadableResponse)} endContent={<Button label={copy.retryRows} variant="ghost" size="sm" onClick={onRefreshRows} />} /> : null}
      {groups.error ? <Banner status="error" title={copy.groupsError} description={errorMessage(groups.error, copy.unreadableResponse)} endContent={<Button label={copy.retryGroups} variant="ghost" size="sm" onClick={onRefreshGroups} />} /> : null}
      {detail.error && errorStatus(detail.error) !== 404 ? <Banner status="error" title={copy.detailError} description={errorMessage(detail.error, copy.unreadableResponse)} endContent={<Button label={copy.retryDetail} variant="ghost" size="sm" onClick={onRefreshDetail} />} /> : null}
      {atom.error ? <Banner status="error" title={copy.atomError} description={errorMessage(atom.error, copy.unreadableResponse)} endContent={<Button label={copy.retryAtom} variant="ghost" size="sm" onClick={onRefreshAtom} />} /> : null}
      {actionError ? <Banner status="error" title={copy.actionError} description={errorMessage(actionError, copy.unreadableResponse)} /> : null}

      <section className={styles.workspace}>
        <Card className={styles.signalPanel} padding={0}>
          <PanelHeader title={copy.signalRows} meta={copy.loadedCount(signals.data.length)} refreshing={signals.phase === "refreshing"} copy={copy} />
          <OntologySignalListView phase={signals.phase} signals={signals.data} selectedSignalId={selectedSignalId} onSelectSignal={onSelectSignal} copy={copy} />
        </Card>
        <Card className={styles.groupPanel} padding={0}>
          <div className={styles.panelHeader}>
            <div><Heading level={2}>{copy.groupedReview}</Heading><Text as="p" type="supporting">{copy.groupsCount(groups.data.length)}</Text></div>
            {groups.phase === "refreshing" ? <Badge variant="warning" label={copy.refreshing} /> : null}
          </div>
          <div className={styles.tabs} role="group" aria-label={copy.groupBy}>
            {(["label", "candidate_atom", "proposed_label", "cluster"] as const).map((candidate) => <Button key={candidate} label={candidate === "candidate_atom" ? copy.atom : candidate === "proposed_label" ? copy.proposal : candidate === "cluster" ? copy.cluster : copy.label} variant={groupBy === candidate ? "primary" : "ghost"} size="sm" aria-pressed={groupBy === candidate} onClick={() => onFiltersChange({ ...filters, groupBy: candidate })} />)}
          </div>
          <ReviewGroupsView phase={groups.phase} groups={groups.data} onSelectSignal={onSelectSignal} copy={copy} />
        </Card>
        <div className={styles.detailColumn}>
          <Card className={styles.detailPanel} padding={0}>
            <div className={styles.panelHeader}>
              <div><Heading level={2}>{copy.signalDetail}</Heading><Text as="p" type="supporting">{detail.data?.signal.status ?? copy.none}</Text></div>
              {selectedSignalId !== null && onCloseDetail ? <Button label={copy.closeDetail} variant="ghost" size="sm" onClick={onCloseDetail} /> : null}
            {detail.phase === "refreshing" ? <Badge variant="warning" label={copy.refreshing} /> : null}
            </div>
            <OntologySignalDetailView phase={detail.phase} detail={detail.data} error={detail.error} actionReason={actionReason} actionPending={actionPending} lifecycleEnabled={lifecycleEnabled} onActionReasonChange={onActionReasonChange} onLifecycleAction={onLifecycleAction} onExplainAtom={onExplainAtom} copy={copy} />
          </Card>
          <Card className={styles.atomPanel} padding={0}>
            <PanelHeader title={copy.atomExplain} meta={atom.phase === "refreshing" ? copy.refreshing : atomRef || copy.none} refreshing={atom.phase === "refreshing"} copy={copy} />
            <form className={styles.atomSearch} onSubmit={onAtomSearch}>
              <label>
                <span className={styles.visuallyHidden}>{copy.atomInput}</span>
                <input name="atom-ref" autoComplete="off" aria-label={copy.atomInput} value={atomDraft} onChange={(event: ChangeEvent<HTMLInputElement>) => onAtomDraftChange?.(event.currentTarget.value)} placeholder={copy.atomInput} />
              </label>
              <Button label={copy.explain} type="submit" variant="secondary" size="sm" isDisabled={!atomDraft.trim()} />
            </form>
            <AtomExplainView phase={atom.phase} explain={atom.data} copy={copy} />
          </Card>
        </div>
      </section>
    </main>
  )
}

function PanelHeader({ title, meta, refreshing, copy = featureCopyForLocale("en").ontology }: { readonly title: string; readonly meta: string; readonly refreshing: boolean; readonly copy?: OntologyCopy }) {
  return <div className={styles.panelHeader}><div><Heading level={2}>{title}</Heading><Text as="p" type="supporting">{meta}</Text></div>{refreshing ? <Badge variant="warning" label={copy.refreshing} /> : null}</div>
}

export function OntologySignalListView({ phase, signals, selectedSignalId, onSelectSignal, copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly signals: readonly LabelOntologySignalRecord[]; readonly selectedSignalId: string | null; readonly onSelectSignal: (signalId: string) => void; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && signals.length === 0) return <div className={styles.loading} role="status" aria-label={copy.signalRows}><span /><span /><span /></div>
  if (signals.length === 0) return <div className={styles.empty}>{copy.noRows}</div>
  return <div className={styles.scrollList}>{signals.map((signal) => <button key={signal.id} type="button" className={`${styles.ontologyRow} ${selectedSignalId === signal.id ? styles.selected : ""}`} aria-pressed={selectedSignalId === signal.id} onClick={() => onSelectSignal(signal.id)}><div className={styles.rowTop}><strong>{signalTitle(signal)}</strong><Badge variant={statusVariant(signal.status)} label={signal.status} /></div><span className={styles.rowMeta} translate="no">{signal.id} · {signal.kind}</span><div className={styles.rowBadges}><Badge variant="neutral" label={signal.proposed_action} />{signal.suggest_score === null ? null : <Badge variant="neutral" label={`${copy.recordedScore} ${formatScore(signal.suggest_score)}`} />}</div></button>)}</div>
}

export function ReviewGroupsView({ phase, groups, onSelectSignal, copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly groups: readonly LabelOntologyReviewGroup[]; readonly onSelectSignal: (signalId: string) => void; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && groups.length === 0) return <div className={styles.loading} role="status" aria-label={copy.groupedReview}><span /><span /><span /></div>
  if (groups.length === 0) return <div className={styles.empty}>{copy.noGroups}</div>
  return <div className={styles.scrollList}>{groups.map((group) => <article key={`${group.group_by}:${group.key}`} className={styles.group}><div className={styles.rowTop}><div><strong>{groupTitle(group)}</strong><span className={styles.rowMeta} translate="no">{group.sample_task_refs.length ? group.sample_task_refs.join(", ") : group.key}</span></div><Badge variant="warning" label={copy.sourceTasks(group.task_count)} /></div>{group.candidate_text ? <Text as="p" type="supporting">{group.candidate_text}</Text> : null}<div className={styles.metrics}><span><b>{group.signal_count}</b> {copy.signalCount(group.signal_count).replace(String(group.signal_count), "").trim()}</span><span><b>{group.open_count}</b> {copy.openCount(group.open_count).replace(String(group.open_count), "").trim()}</span><span><b>{group.confirmed_count}</b> {copy.confirmedCount(group.confirmed_count).replace(String(group.confirmed_count), "").trim()}</span><span><b>{group.action_count}</b> {copy.actionCountLabel(group.action_count).replace(String(group.action_count), "").trim()}</span></div><div className={styles.groupSignals}>{group.signal_ids.slice(0, 4).map((id) => <Button key={id} label={shortId(id)} variant="ghost" size="sm" onClick={() => onSelectSignal(id)} />)}</div></article>)}</div>
}

export function OntologySignalDetailView({ phase, detail, error, actionReason, actionPending, lifecycleEnabled = false, onActionReasonChange, onLifecycleAction, onExplainAtom, copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly detail: LabelOntologySignalDetail | null; readonly error?: unknown | null; readonly actionReason: string; readonly actionPending: boolean; readonly lifecycleEnabled?: boolean; readonly onActionReasonChange: (value: string) => void; readonly onLifecycleAction: (action: LifecycleAction) => void; readonly onExplainAtom: (atomRef: string | null | undefined) => void; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && detail === null) return <div className={styles.loading} role="status" aria-label={copy.signalDetail}><span /><span /><span /></div>
  if (errorStatus(error) === 404) return <div className={styles.empty}>{copy.unavailable}</div>
  if (detail === null) return <div className={styles.empty}>{copy.selectDetail}</div>
  const signal = detail.signal
  const ready = Boolean(actionReason.trim()) && !actionPending && lifecycleEnabled
  const canConfirm = ready && signal.status === "open"
  const canReview = ready && (signal.status === "open" || signal.status === "confirmed")
  return <article className={styles.detail}>
    <div className={styles.detailBadges}><Badge variant={statusVariant(signal.status)} label={signal.status} /><Badge variant="neutral" label={signal.kind} /><Badge variant="neutral" label={signal.proposed_action} />{detail.observation.suggest_degraded ? <Badge variant="warning" label={copy.observationDegraded} /> : null}</div>
    <Heading level={3}>{signalTitle(signal)}</Heading>
    <Text as="p" type="supporting">{signal.rationale || copy.rationaleMissing}</Text>
    <dl className={styles.facts}><Fact label={copy.sourceTask} value={detail.observation.task_ref_snapshot} /><Fact label={copy.target} value={signal.target_label_name_snapshot ?? signal.proposed_label_name ?? "—"} /><Fact label={copy.suggest} value={signal.suggest_state ?? "—"} /><Fact label={copy.recordedScore} value={formatScore(signal.suggest_score)} /><Fact label={copy.rank} value={signal.suggest_rank === null ? "—" : String(signal.suggest_rank)} /><Fact label={copy.recordedConfidence} value={formatScore(signal.confidence)} /></dl>
    {signal.candidate_text ? <div className={styles.candidate}><div className={styles.candidateHeader}><Text as="span" type="label">{copy.candidateAtom}</Text><Button label={copy.explainHash} variant="ghost" size="sm" onClick={() => onExplainAtom(signal.candidate_content_hash)} /></div><Text as="p">{signal.candidate_text}</Text><span className={styles.rowMeta} translate="no">{signal.candidate_atom_polarity ?? "—"} / {signal.candidate_atom_kind ?? "—"} / {signal.candidate_content_hash ?? "—"}</span></div> : null}
    <div className={styles.actionArea}><label><span>{copy.reviewReason}</span><textarea name="ontology-action-reason" autoComplete="off" aria-label={copy.reviewReason} value={actionReason} onChange={(event) => onActionReasonChange(event.currentTarget.value)} placeholder={copy.reasonPlaceholder} /></label><div className={styles.actionButtons}><Button label={copy.confirm} variant="primary" isDisabled={!canConfirm} isLoading={actionPending} onClick={() => onLifecycleAction("confirm")} /><Button label={copy.resolveNoChange} variant="secondary" isDisabled={!canReview || actionPending} onClick={() => onLifecycleAction("resolve_no_change")} /><Button label={copy.reject} variant="secondary" isDisabled={!canReview || actionPending} onClick={() => onLifecycleAction("reject")} /></div></div>
    <ActionHistory actions={detail.actions} onExplainAtom={onExplainAtom} copy={copy} />
  </article>
}

function ActionHistory({ actions, onExplainAtom, copy = featureCopyForLocale("en").ontology }: { readonly actions: readonly LabelOntologyActionRecord[]; readonly onExplainAtom: (atomRef: string | null | undefined) => void; readonly copy?: OntologyCopy }) {
  if (actions.length === 0) return <Text as="p" type="supporting">{copy.noActions}</Text>
  return <section className={styles.history}><Heading level={4}>{copy.actions}</Heading>{actions.map((action) => <article key={action.id} className={styles.historyRow}><div className={styles.rowBadges}><Badge variant="neutral" label={action.action_type} /><Badge variant="neutral" label={copy.requiresValidation(action.validation_requirement)} /><Badge variant={validationVariant(action.validation_effective_outcome)} label={action.validation_effective_outcome} /><span className={styles.rowMeta} translate="no">{shortId(action.id)}</span></div><Text as="p" type="supporting">{action.reason}</Text>{action.result_atom_id || action.result_atom_content_hash ? <div className={styles.groupSignals}>{action.result_atom_id ? <Button label={copy.atomRef(shortId(action.result_atom_id))} variant="ghost" size="sm" onClick={() => onExplainAtom(action.result_atom_id)} /> : null}{action.result_atom_content_hash ? <Button label={copy.hashRef(shortId(action.result_atom_content_hash))} variant="ghost" size="sm" onClick={() => onExplainAtom(action.result_atom_content_hash)} /> : null}</div> : null}</article>)}</section>
}

export function AtomExplainView({ phase, explain, copy = featureCopyForLocale("en").ontology }: { readonly phase: ReadPhase; readonly explain: LabelAtomExplainRecord | null; readonly copy?: OntologyCopy }) {
  if (phase === "loading" && explain === null) return <div className={styles.loading} role="status" aria-label={copy.atomExplain}><span /><span /></div>
  if (explain === null) return <div className={styles.empty}>{copy.enterAtom}</div>
  return <article className={styles.atomExplain}><div className={styles.detailBadges}><Badge variant={explain.legacy_untracked ? "warning" : "success"} label={explain.legacy_untracked ? copy.legacyUntracked : copy.hasProvenance} />{explain.atom ? <Badge variant="neutral" label={explain.atom.label_name} /> : null}</div>{explain.atom ? <div className={styles.atomCard}><span className={styles.rowMeta} translate="no">{explain.atom.kind}</span><Text as="p" type="supporting">{explain.atom.text}</Text><span className={styles.rowMeta} translate="no">{explain.atom.id} / {explain.atom.content_hash}</span></div> : <Text as="p" type="supporting">{copy.noCurrentAtom(explain.query)}</Text>}{explain.legacy_reason ? <Text as="p" type="supporting">{explain.legacy_reason}</Text> : null}<div className={styles.metrics}><span><b>{explain.provenance_actions.length}</b> {copy.actionCount(explain.provenance_actions.length).replace(String(explain.provenance_actions.length), "").trim()}</span><span><b>{explain.supporting_signals.length}</b> {copy.supportingCount(explain.supporting_signals.length).replace(String(explain.supporting_signals.length), "").trim()}</span><span><b>{explain.validation_history.length}</b> {copy.validationCount(explain.validation_history.length).replace(String(explain.validation_history.length), "").trim()}</span></div>{explain.provenance_actions.slice(0, 4).map((entry) => <div key={entry.action.id} className={styles.historyRow}><div className={styles.rowBadges}><Badge variant="neutral" label={entry.action.action_type} /><span className={styles.rowMeta} translate="no">{entry.matched_by}</span></div><Text as="p" type="supporting">{entry.action.reason}</Text></div>)}{explain.validation_history.slice(0, 4).map((entry) => <div key={entry.action.id} className={styles.historyRow}><div className={styles.rowBadges}><Badge variant={validationVariant(entry.validation_status)} label={entry.validation_status} /><span className={styles.rowMeta} translate="no">{copy.parentRef(shortId(entry.parent_action_id))}</span></div>{entry.warnings.length ? <Text as="p" type="supporting">{entry.warnings.join("; ")}</Text> : null}</div>)}</article>
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return <div><dt>{label}</dt><dd translate="no">{value}</dd></div>
}
