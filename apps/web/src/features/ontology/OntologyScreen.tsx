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
import type { OntologyRouteFilters } from "../../lib/router"
import { useReadState, type ReadPhase, type ReadState } from "../signals/SignalsScreen"

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
  readonly onLifecycleAction?: (action: LifecycleAction, signalId: string, reason: string) => void | Promise<void>
}

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

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message
  return "The local service returned an unreadable response."
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
  onLifecycleAction,
}: OntologyScreenProps) {
  const { effectiveFilters, update: updateFilters } = useFilterState(filters, onFiltersChange)
  const [localSelectedSignalId, setLocalSelectedSignalId] = useState<string | null>(selectedSignalIdProp ?? null)
  const [atomDraft, setAtomDraft] = useState("")
  const [atomRef, setAtomRef] = useState("")
  const [actionReason, setActionReason] = useState("")
  const [actionPending, setActionPending] = useState(false)
  const [actionError, setActionError] = useState<unknown | null>(null)
  const [refreshToken, setRefreshToken] = useState(0)
  const selectedSignalId = selectedSignalIdProp === undefined ? localSelectedSignalId : selectedSignalIdProp
  const includeAll = effectiveFilters.includeAll === true
  const groupBy = effectiveFilters.groupBy ?? "label"
  const filterKey = JSON.stringify({ includeAll, groupBy })

  const signalsRequest = useMemo(() => {
    if (!api) return null
    return (signal: AbortSignal) => api.listLabelOntologySignals({ statuses: includeAll ? [] : ["open", "confirmed"], kinds: [], includeAll, limit: 100 }, signal)
  }, [api, includeAll])
  const signals = useReadState(Boolean(api), signalsRequest, `ontology-signals:${api?.board ?? "none"}:${filterKey}:${refreshToken}:${invalidationRevision}`, emptySignals())

  const reviewRequest = useMemo(() => {
    if (!api) return null
    return (signal: AbortSignal) => api.reviewLabelOntology({ groupBy, includeAll, limit: 100 }, signal)
  }, [api, groupBy, includeAll])
  const groups = useReadState(Boolean(api), reviewRequest, `ontology-review:${api?.board ?? "none"}:${filterKey}:${refreshToken}:${invalidationRevision}`, emptyGroups())

  useEffect(() => {
    if (selectedSignalId !== null && signals.data.some((signal) => signal.id === selectedSignalId)) return
    const nextId = signals.data[0]?.id ?? null
    setLocalSelectedSignalId(nextId)
    onSelectSignalProp?.(nextId)
  }, [onSelectSignalProp, selectedSignalId, signals.data])

  const detailRequest = useMemo(() => {
    if (!api || !selectedSignalId) return null
    return (signal: AbortSignal) => api.getLabelOntologySignal(selectedSignalId, signal)
  }, [api, selectedSignalId])
  const detail = useReadState(Boolean(api && selectedSignalId), detailRequest, `ontology-signal:${selectedSignalId ?? "none"}:${refreshToken}:${invalidationRevision}`, emptyDetail())

  const atomRequest = useMemo(() => {
    if (!api || !atomRef) return null
    return (signal: AbortSignal) => api.explainLabelAtom(atomRef, signal)
  }, [api, atomRef])
  const atom = useReadState(Boolean(api && atomRef), atomRequest, `ontology-atom:${atomRef}:${refreshToken}:${invalidationRevision}`, emptyAtom())

  const selectSignal = (signalId: string | null) => {
    setLocalSelectedSignalId(signalId)
    onSelectSignalProp?.(signalId)
  }
  const submitAtomSearch = (event: FormEvent) => {
    event.preventDefault()
    const value = atomDraft.trim()
    if (value) setAtomRef(value)
  }
  const selectAtom = (value: string | null | undefined) => {
    const next = value?.trim()
    if (!next) return
    setAtomDraft(next)
    setAtomRef(next)
  }
  const runLifecycleAction = async (action: LifecycleAction) => {
    if (!selectedSignalId || !actionReason.trim() || !onLifecycleAction) return
    setActionError(null)
    setActionPending(true)
    try {
      await onLifecycleAction(action, selectedSignalId, actionReason.trim())
      setActionReason("")
      setRefreshToken((value) => value + 1)
    } catch (error) {
      setActionError(error)
    } finally {
      setActionPending(false)
    }
  }
  const refresh = () => setRefreshToken((value) => value + 1)

  return (
    <OntologyScreenView
      boardName={boardName ?? api?.board ?? "—"}
      filters={effectiveFilters}
      signals={signals}
      groups={groups}
      detail={detail}
      atom={atom}
      selectedSignalId={selectedSignalId}
      atomRef={atomRef}
      online={online}
      actionReason={actionReason}
      actionPending={actionPending}
      actionError={actionError}
      onRefresh={refresh}
      onFiltersChange={updateFilters}
      onSelectSignal={selectSignal}
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
  readonly onFiltersChange: (filters: OntologyRouteFilters) => void
  readonly onSelectSignal: (signalId: string | null) => void
  readonly onActionReasonChange: (value: string) => void
  readonly onLifecycleAction: (action: LifecycleAction) => void
  readonly lifecycleEnabled?: boolean
  readonly onExplainAtom: (atomRef: string | null | undefined) => void
  readonly onAtomDraftChange?: (value: string) => void
  readonly onAtomSearch: (event: FormEvent) => void
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
  onFiltersChange,
  onSelectSignal,
  onActionReasonChange,
  onLifecycleAction,
  lifecycleEnabled = true,
  onExplainAtom,
  onAtomDraftChange,
  onAtomSearch,
}: OntologyScreenViewProps) {
  const stale = signals.data.length > 0 && signals.phase === "error"
  const includeAll = filters.includeAll === true
  const groupBy = filters.groupBy ?? "label"
  return (
    <main className={styles.screen} aria-labelledby="ontology-title" data-testid="ontology-screen">
      <header className={styles.hero}>
        <div>
          <Text as="p" type="supporting" className={styles.eyebrow}>KANBAN TOOL / ONTOLOGY</Text>
          <Heading level={1} id="ontology-title">Ontology review</Heading>
          <Text as="p" type="supporting" className={styles.lede}>Review aid; does not modify canonical semantics. Lifecycle actions do not modify canonical label semantics.</Text>
          <Text as="p" type="supporting" className={styles.boardContext}>Board · <code translate="no">{boardName}</code></Text>
        </div>
        <div className={styles.heroActions}>
          <Button label={includeAll ? "All history" : "Open + confirmed"} variant={includeAll ? "primary" : "secondary"} size="sm" aria-pressed={includeAll} onClick={() => onFiltersChange({ ...filters, includeAll: !includeAll })} />
          <Button label="Refresh" variant="secondary" size="sm" onClick={onRefresh} isLoading={signals.phase === "loading" || groups.phase === "loading" || signals.phase === "refreshing" || groups.phase === "refreshing"} />
        </div>
      </header>

      {!online ? <Banner status="warning" title="You are offline" description={stale ? "Showing stale ontology projections until the local service reconnects." : "Connect to kanban serve to load ontology projections."} /> : null}
      {signals.error || groups.error || detail.error || atom.error || actionError ? (
        <Banner status="error" title="Unable to load ontology" description={[signals.error, groups.error, detail.error, atom.error, actionError].filter(Boolean).map(errorMessage).join(" · ")} endContent={<Button label="Retry" variant="ghost" size="sm" onClick={onRefresh} />} />
      ) : null}

      <section className={styles.workspace}>
        <Card className={styles.signalPanel} padding={0}>
          <PanelHeader title="Signal rows" meta={`${signals.data.length} of up to 100 loaded`} refreshing={signals.phase === "refreshing"} />
          <OntologySignalListView phase={signals.phase} signals={signals.data} selectedSignalId={selectedSignalId} onSelectSignal={onSelectSignal} />
        </Card>
        <Card className={styles.groupPanel} padding={0}>
          <div className={styles.panelHeader}>
            <div><Heading level={2}>Grouped review</Heading><Text as="p" type="supporting">{groups.data.length} of up to 100 groups loaded</Text></div>
            {groups.phase === "refreshing" ? <Badge variant="warning" label="refreshing" /> : null}
          </div>
          <div className={styles.tabs} role="group" aria-label="Group by">
            {(["label", "candidate_atom", "proposed_label"] as const).map((candidate) => <Button key={candidate} label={candidate === "candidate_atom" ? "Atom" : candidate === "proposed_label" ? "Proposal" : "Label"} variant={groupBy === candidate ? "primary" : "ghost"} size="sm" aria-pressed={groupBy === candidate} onClick={() => onFiltersChange({ ...filters, groupBy: candidate })} />)}
          </div>
          <ReviewGroupsView phase={groups.phase} groups={groups.data} onSelectSignal={onSelectSignal} />
        </Card>
        <div className={styles.detailColumn}>
          <Card className={styles.detailPanel} padding={0}>
            <PanelHeader title="Signal detail" meta={detail.data?.signal.status ?? "none"} refreshing={detail.phase === "refreshing"} />
            <OntologySignalDetailView phase={detail.phase} detail={detail.data} error={detail.error} actionReason={actionReason} actionPending={actionPending} lifecycleEnabled={lifecycleEnabled} onActionReasonChange={onActionReasonChange} onLifecycleAction={onLifecycleAction} onExplainAtom={onExplainAtom} />
          </Card>
          <Card className={styles.atomPanel} padding={0}>
            <PanelHeader title="Atom explain" meta={atom.phase === "refreshing" ? "refreshing" : atomRef || "idle"} refreshing={atom.phase === "refreshing"} />
            <form className={styles.atomSearch} onSubmit={onAtomSearch}>
              <label>
                <span className={styles.visuallyHidden}>Atom id or content hash</span>
                <input aria-label="Atom id or content hash" value={atomDraft} onChange={(event: ChangeEvent<HTMLInputElement>) => onAtomDraftChange?.(event.currentTarget.value)} placeholder="Atom id or content hash" />
              </label>
              <Button label="Explain" type="submit" variant="secondary" size="sm" isDisabled={!atomDraft.trim()} />
            </form>
            <AtomExplainView phase={atom.phase} explain={atom.data} />
          </Card>
        </div>
      </section>
    </main>
  )
}

function PanelHeader({ title, meta, refreshing }: { readonly title: string; readonly meta: string; readonly refreshing: boolean }) {
  return <div className={styles.panelHeader}><div><Heading level={2}>{title}</Heading><Text as="p" type="supporting">{meta}</Text></div>{refreshing ? <Badge variant="warning" label="refreshing" /> : null}</div>
}

export function OntologySignalListView({ phase, signals, selectedSignalId, onSelectSignal }: { readonly phase: ReadPhase; readonly signals: readonly LabelOntologySignalRecord[]; readonly selectedSignalId: string | null; readonly onSelectSignal: (signalId: string) => void }) {
  if (phase === "loading" && signals.length === 0) return <div className={styles.loading} role="status"><span /><span /><span /></div>
  if (signals.length === 0) return <div className={styles.empty}>No ontology signal rows returned.</div>
  return <div className={styles.scrollList}>{signals.map((signal) => <button key={signal.id} type="button" className={`${styles.ontologyRow} ${selectedSignalId === signal.id ? styles.selected : ""}`} onClick={() => onSelectSignal(signal.id)}><div className={styles.rowTop}><strong>{signalTitle(signal)}</strong><Badge variant={statusVariant(signal.status)} label={signal.status} /></div><div className={styles.rowMeta}>{signal.id} · {signal.kind}</div><div className={styles.rowBadges}><Badge variant="neutral" label={signal.proposed_action} />{signal.suggest_score === null ? null : <Badge variant="neutral" label={`recorded score ${formatScore(signal.suggest_score)}`} />}</div></button>)}</div>
}

export function ReviewGroupsView({ phase, groups, onSelectSignal }: { readonly phase: ReadPhase; readonly groups: readonly LabelOntologyReviewGroup[]; readonly onSelectSignal: (signalId: string) => void }) {
  if (phase === "loading" && groups.length === 0) return <div className={styles.loading} role="status"><span /><span /><span /></div>
  if (groups.length === 0) return <div className={styles.empty}>No review groups returned.</div>
  return <div className={styles.scrollList}>{groups.map((group) => <article key={`${group.group_by}:${group.key}`} className={styles.group}><div className={styles.rowTop}><div><strong>{groupTitle(group)}</strong><Text as="p" type="supporting">{group.sample_task_refs.length ? group.sample_task_refs.join(", ") : group.key}</Text></div><Badge variant="warning" label={`${group.task_count} source tasks`} /></div>{group.candidate_text ? <Text as="p" type="supporting">{group.candidate_text}</Text> : null}<div className={styles.metrics}><span><b>{group.signal_count}</b> signal rows</span><span><b>{group.open_count}</b> open</span><span><b>{group.confirmed_count}</b> confirmed</span><span><b>{group.action_count}</b> actions</span></div><div className={styles.groupSignals}>{group.signal_ids.slice(0, 4).map((id) => <Button key={id} label={shortId(id)} variant="ghost" size="sm" onClick={() => onSelectSignal(id)} />)}</div></article>)}</div>
}

export function OntologySignalDetailView({ phase, detail, error, actionReason, actionPending, lifecycleEnabled = true, onActionReasonChange, onLifecycleAction, onExplainAtom }: { readonly phase: ReadPhase; readonly detail: LabelOntologySignalDetail | null; readonly error?: unknown | null; readonly actionReason: string; readonly actionPending: boolean; readonly lifecycleEnabled?: boolean; readonly onActionReasonChange: (value: string) => void; readonly onLifecycleAction: (action: LifecycleAction) => void; readonly onExplainAtom: (atomRef: string | null | undefined) => void }) {
  if (phase === "loading" && detail === null) return <div className={styles.loading} role="status"><span /><span /><span /></div>
  if (errorStatus(error) === 404 && detail === null) return <div className={styles.empty}>Ontology signal is no longer available.</div>
  if (detail === null) return <div className={styles.empty}>Select a signal to inspect its observation and actions.</div>
  const signal = detail.signal
  const ready = Boolean(actionReason.trim()) && !actionPending && lifecycleEnabled
  const canConfirm = ready && signal.status === "open"
  const canReview = ready && (signal.status === "open" || signal.status === "confirmed")
  return <article className={styles.detail}>
    <div className={styles.detailBadges}><Badge variant={statusVariant(signal.status)} label={signal.status} /><Badge variant="neutral" label={signal.kind} /><Badge variant="neutral" label={signal.proposed_action} />{detail.observation.suggest_degraded ? <Badge variant="warning" label="observation degraded" /> : null}</div>
    <Heading level={3}>{signalTitle(signal)}</Heading>
    <Text as="p" type="supporting">{signal.rationale || "No rationale recorded."}</Text>
    <dl className={styles.facts}><Fact label="source task" value={detail.observation.task_ref_snapshot} /><Fact label="target" value={signal.target_label_name_snapshot ?? signal.proposed_label_name ?? "—"} /><Fact label="suggest" value={signal.suggest_state ?? "—"} /><Fact label="recorded score" value={formatScore(signal.suggest_score)} /><Fact label="rank" value={signal.suggest_rank === null ? "—" : String(signal.suggest_rank)} /><Fact label="recorded confidence" value={formatScore(signal.confidence)} /></dl>
    {signal.candidate_text ? <div className={styles.candidate}><div className={styles.candidateHeader}><Text as="span" type="label">Candidate atom</Text><Button label="Explain hash" variant="ghost" size="sm" onClick={() => onExplainAtom(signal.candidate_content_hash)} /></div><Text as="p">{signal.candidate_text}</Text><Text as="p" type="supporting">{signal.candidate_atom_polarity ?? "—"} / {signal.candidate_atom_kind ?? "—"} / {signal.candidate_content_hash ?? "—"}</Text></div> : null}
    <div className={styles.actionArea}><label><span>Review action reason</span><textarea aria-label="Review action reason" value={actionReason} onChange={(event) => onActionReasonChange(event.currentTarget.value)} placeholder="Reason for lifecycle action" /></label><div className={styles.actionButtons}><Button label="Confirm signal" variant="primary" isDisabled={!canConfirm} isLoading={actionPending} onClick={() => onLifecycleAction("confirm")} /><Button label="Resolve no change" variant="secondary" isDisabled={!canReview || actionPending} onClick={() => onLifecycleAction("resolve_no_change")} /><Button label="Reject" variant="secondary" isDisabled={!canReview || actionPending} onClick={() => onLifecycleAction("reject")} /></div></div>
    <ActionHistory actions={detail.actions} onExplainAtom={onExplainAtom} />
  </article>
}

function ActionHistory({ actions, onExplainAtom }: { readonly actions: readonly LabelOntologyActionRecord[]; readonly onExplainAtom: (atomRef: string | null | undefined) => void }) {
  if (actions.length === 0) return <Text as="p" type="supporting">No actions recorded for this signal.</Text>
  return <section className={styles.history}><Heading level={4}>Actions</Heading>{actions.map((action) => <article key={action.id} className={styles.historyRow}><div className={styles.rowBadges}><Badge variant="neutral" label={action.action_type} /><Badge variant="neutral" label={`requires ${action.validation_requirement}`} /><Badge variant={validationVariant(action.validation_effective_outcome)} label={action.validation_effective_outcome} /><Text as="span" type="supporting">{shortId(action.id)}</Text></div><Text as="p" type="supporting">{action.reason}</Text>{action.result_atom_id || action.result_atom_content_hash ? <div className={styles.groupSignals}>{action.result_atom_id ? <Button label={`atom ${shortId(action.result_atom_id)}`} variant="ghost" size="sm" onClick={() => onExplainAtom(action.result_atom_id)} /> : null}{action.result_atom_content_hash ? <Button label={`hash ${shortId(action.result_atom_content_hash)}`} variant="ghost" size="sm" onClick={() => onExplainAtom(action.result_atom_content_hash)} /> : null}</div> : null}</article>)}</section>
}

export function AtomExplainView({ phase, explain }: { readonly phase: ReadPhase; readonly explain: LabelAtomExplainRecord | null }) {
  if (phase === "loading" && explain === null) return <div className={styles.loading} role="status"><span /><span /></div>
  if (explain === null) return <div className={styles.empty}>Enter an atom id or content hash.</div>
  return <article className={styles.atomExplain}><div className={styles.detailBadges}><Badge variant={explain.legacy_untracked ? "warning" : "success"} label={explain.legacy_untracked ? "legacy untracked" : "has provenance records"} />{explain.atom ? <Badge variant="neutral" label={explain.atom.label_name} /> : null}</div>{explain.atom ? <div className={styles.atomCard}><strong>{explain.atom.kind}</strong><Text as="p" type="supporting">{explain.atom.text}</Text><Text as="p" type="supporting">{explain.atom.id} / {explain.atom.content_hash}</Text></div> : <Text as="p" type="supporting">No current atom resolved for {explain.query}.</Text>}{explain.legacy_reason ? <Text as="p" type="supporting">{explain.legacy_reason}</Text> : null}<div className={styles.metrics}><span><b>{explain.provenance_actions.length}</b> actions</span><span><b>{explain.supporting_signals.length}</b> signal rows</span><span><b>{explain.validation_history.length}</b> validations</span></div>{explain.provenance_actions.slice(0, 4).map((entry) => <div key={entry.action.id} className={styles.historyRow}><div className={styles.rowBadges}><Badge variant="neutral" label={entry.action.action_type} /><Text as="span" type="supporting">{entry.matched_by}</Text></div><Text as="p" type="supporting">{entry.action.reason}</Text></div>)}{explain.validation_history.slice(0, 4).map((entry) => <div key={entry.action.id} className={styles.historyRow}><div className={styles.rowBadges}><Badge variant={validationVariant(entry.validation_status)} label={entry.validation_status} /><Text as="span" type="supporting">parent {shortId(entry.parent_action_id)}</Text></div>{entry.warnings.length ? <Text as="p" type="supporting">{entry.warnings.join("; ")}</Text> : null}</div>)}</article>
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return <div><dt>{label}</dt><dd translate="no">{value}</dd></div>
}
