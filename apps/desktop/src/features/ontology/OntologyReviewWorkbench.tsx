import { FormEvent, type ReactNode, useEffect, useMemo, useState } from "react"
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Braces, CheckCircle2, CircleDashed, RefreshCcw, Search, XCircle } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge, type BadgeProps } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { MetricStrip, PageToolbar, SectionCard } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { translate, useI18n } from "@/i18n"
import type {
  KanbanApi,
  LabelAtomExplainRecord,
  LabelOntologyActionRecord,
  LabelOntologyActionType,
  LabelOntologyReviewGroup,
  LabelOntologyReviewGroupBy,
  LabelOntologySignalDetail,
  LabelOntologySignalRecord,
  LabelOntologySignalStatus,
} from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"

const SIGNAL_STATUSES: LabelOntologySignalStatus[] = ["open", "confirmed"]
const REVIEW_LIMIT = 100

type LifecycleAction = Extract<LabelOntologyActionType, "confirm" | "reject" | "resolve_no_change">
type Translate = (key: string, values?: Record<string, string | number>) => string

const englishT: Translate = (key, values) => translate("en", key, values)

export function OntologyReviewWorkbench({ api }: { api: KanbanApi | null }) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [selectedSignalId, setSelectedSignalId] = useState<string | null>(null)
  const [groupBy, setGroupBy] = useState<LabelOntologyReviewGroupBy>("label")
  const [includeAll, setIncludeAll] = useState(false)
  const [actionReason, setActionReason] = useState("")
  const [atomDraft, setAtomDraft] = useState("")
  const [atomRef, setAtomRef] = useState("")
  const [localError, setLocalError] = useState<string | null>(null)

  const signalsQueryShape = useMemo(
    () => ({
      board: api?.board ?? "pending",
      statuses: includeAll ? [] : SIGNAL_STATUSES,
      kinds: [],
      includeAll,
      limit: REVIEW_LIMIT,
    }),
    [api?.board, includeAll],
  )
  const reviewQueryShape = useMemo(
    () => ({
      board: api?.board ?? "pending",
      groupBy,
      includeAll,
      limit: REVIEW_LIMIT,
    }),
    [api?.board, groupBy, includeAll],
  )

  const signalsQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.ontologySignals(signalsQueryShape),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.listLabelOntologySignals({
        statuses: signalsQueryShape.statuses,
        kinds: signalsQueryShape.kinds,
        includeAll: signalsQueryShape.includeAll,
        limit: signalsQueryShape.limit,
        signal,
      })
    },
    placeholderData: keepPreviousData,
  })

  const reviewQuery = useQuery({
    enabled: Boolean(api),
    queryKey: queryKeys.ontologyReview(reviewQueryShape),
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.reviewLabelOntology({
        groupBy: reviewQueryShape.groupBy,
        includeAll: reviewQueryShape.includeAll,
        limit: reviewQueryShape.limit,
        signal,
      })
    },
    placeholderData: keepPreviousData,
  })

  const signalDetailQuery = useQuery({
    enabled: Boolean(api && selectedSignalId),
    queryKey: selectedSignalId ? queryKeys.ontologySignal(selectedSignalId) : ["label-ontology-signal", "none"],
    queryFn: ({ signal }) => {
      if (!api || !selectedSignalId) throw new Error(t("Signal detail query is not ready."))
      return api.getLabelOntologySignal(selectedSignalId, { signal })
    },
  })

  const atomExplainQuery = useQuery({
    enabled: Boolean(api && atomRef),
    queryKey: atomRef ? queryKeys.ontologyAtomExplain(api?.board ?? "pending", atomRef) : ["label-ontology-atom", "none"],
    queryFn: ({ signal }) => {
      if (!api || !atomRef) throw new Error(t("Atom explain query is not ready."))
      return api.explainLabelAtom(atomRef, { signal })
    },
  })

  const lifecycleMutation = useMutation({
    mutationFn: ({ actionType, signalId, reason }: { actionType: LifecycleAction; signalId: string; reason: string }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.createLabelOntologyAction({
        actionType,
        signalIds: [signalId],
        reason,
      })
    },
    onSuccess: async (_action, variables) => {
      setActionReason("")
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.ontologyRoot(api?.board ?? "pending") }),
        queryClient.invalidateQueries({ queryKey: queryKeys.ontologySignal(variables.signalId) }),
      ])
    },
  })

  const signals = signalsQuery.data ?? []
  const groups = reviewQuery.data ?? []
  const detail = signalDetailQuery.data ?? null

  useEffect(() => {
    if (selectedSignalId && signals.some((signal) => signal.id === selectedSignalId)) return
    setSelectedSignalId(signals[0]?.id ?? null)
  }, [selectedSignalId, signals])

  useEffect(() => {
    if (signalsQuery.error) setLocalError(errorText(signalsQuery.error))
  }, [signalsQuery.error])

  useEffect(() => {
    if (reviewQuery.error) setLocalError(errorText(reviewQuery.error))
  }, [reviewQuery.error])

  useEffect(() => {
    if (signalDetailQuery.error) setLocalError(errorText(signalDetailQuery.error))
  }, [signalDetailQuery.error])

  useEffect(() => {
    if (atomExplainQuery.error) setLocalError(errorText(atomExplainQuery.error))
  }, [atomExplainQuery.error])

  async function refreshAll() {
    setLocalError(null)
    const refetches: Array<Promise<unknown>> = [signalsQuery.refetch(), reviewQuery.refetch()]
    if (selectedSignalId) refetches.push(signalDetailQuery.refetch())
    if (atomRef) refetches.push(atomExplainQuery.refetch())
    await Promise.all(refetches)
  }

  function submitAtomSearch(event: FormEvent) {
    event.preventDefault()
    const value = atomDraft.trim()
    if (!value) return
    setAtomRef(value)
  }

  function selectAtomRef(value: string | null | undefined) {
    const trimmed = value?.trim()
    if (!trimmed) return
    setAtomDraft(trimmed)
    setAtomRef(trimmed)
  }

  async function runLifecycleAction(actionType: LifecycleAction) {
    if (!selectedSignalId || !actionReason.trim()) return
    setLocalError(null)
    try {
      await lifecycleMutation.mutateAsync({
        actionType,
        signalId: selectedSignalId,
        reason: actionReason.trim(),
      })
    } catch (err) {
      setLocalError(errorText(err))
    }
  }

  return (
    <ScrollArea className="flex-1 bg-card">
      <div className="flex min-h-full min-w-0 flex-col gap-3 p-4">
        <PageToolbar
          className="rounded-md border border-border bg-background"
          title={t("Ontology review")}
          description={t("Review aid; does not modify canonical semantics. Lifecycle actions do not modify canonical label semantics.")}
          meta={
            <>
            <Button
              type="button"
              variant={includeAll ? "secondary" : "outline"}
              size="sm"
              aria-pressed={includeAll}
              onClick={() => setIncludeAll((current) => !current)}
            >
              {includeAll ? t("All history") : t("Open + confirmed")}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={!api || signalsQuery.isFetching || reviewQuery.isFetching}
              onClick={() => void refreshAll()}
            >
              <RefreshCcw className={cn("h-4 w-4", (signalsQuery.isFetching || reviewQuery.isFetching) && "animate-spin")} />
              {t("Refresh")}
            </Button>
            </>
          }
        />

        {localError ? (
          <Alert className="border-destructive/50">
            <AlertDescription className="text-destructive">{localError}</AlertDescription>
          </Alert>
        ) : null}

        <div className="grid min-h-[680px] min-w-0 gap-3 xl:grid-cols-[minmax(260px,0.85fr)_minmax(340px,1fr)_minmax(380px,1.2fr)]">
          <Panel title={t("Signal rows")} meta={t("{count} of up to {limit} loaded", { count: signals.length, limit: REVIEW_LIMIT })}>
            <SignalList
              t={t}
              loading={signalsQuery.isLoading}
              signals={signals}
              selectedSignalId={selectedSignalId}
              onSelectSignal={setSelectedSignalId}
            />
          </Panel>

          <Panel
            title={t("Grouped review")}
            meta={reviewQuery.isFetching ? t("refreshing") : t("{count} of up to {limit} groups loaded", { count: groups.length, limit: REVIEW_LIMIT })}
            controls={
              <Tabs value={groupBy} onValueChange={(value) => setGroupBy(value as LabelOntologyReviewGroupBy)}>
                <TabsList>
                  <TabsTrigger value="label">{t("Label")}</TabsTrigger>
                  <TabsTrigger value="candidate_atom">{t("Atom")}</TabsTrigger>
                  <TabsTrigger value="proposed_label">{t("Proposal")}</TabsTrigger>
                </TabsList>
              </Tabs>
            }
          >
            <ReviewGroups t={t} loading={reviewQuery.isLoading} groups={groups} onSelectSignal={setSelectedSignalId} />
          </Panel>

          <div className="grid min-h-0 gap-3 lg:grid-rows-[1fr_minmax(260px,0.65fr)]">
            <Panel title={t("Signal detail")} meta={detail ? detail.signal.status : t("none")}>
              <SignalDetail
                t={t}
                loading={signalDetailQuery.isLoading}
                detail={detail}
                actionReason={actionReason}
                actionPending={lifecycleMutation.isPending}
                onActionReasonChange={setActionReason}
                onLifecycleAction={(actionType) => void runLifecycleAction(actionType)}
                onExplainAtom={selectAtomRef}
              />
            </Panel>

            <Panel title={t("Atom explain")} meta={atomExplainQuery.isFetching ? t("refreshing") : atomRef || t("idle")}>
              <form onSubmit={submitAtomSearch} className="mb-3 flex gap-2">
                <Input
                  aria-label={t("Atom id or content hash")}
                  name="atom-ref"
                  autoComplete="off"
                  value={atomDraft}
                  onChange={(event) => setAtomDraft(event.target.value)}
                  placeholder={t("Atom id or content hash")}
                />
                <Button type="submit" variant="secondary" disabled={!api || !atomDraft.trim()}>
                  <Search className="h-4 w-4" />
                  {t("Explain")}
                </Button>
              </form>
              <AtomExplain t={t} loading={atomExplainQuery.isLoading} explain={atomExplainQuery.data ?? null} />
            </Panel>
          </div>
        </div>
      </div>
    </ScrollArea>
  )
}

function Panel({
  title,
  meta,
  controls,
  children,
}: {
  title: string
  meta?: string
  controls?: ReactNode
  children: ReactNode
}) {
  return (
    <SectionCard
      title={title}
      className="flex min-h-0 min-w-0 flex-col"
      actions={
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          {meta ? <Badge variant="secondary">{meta}</Badge> : null}
          {controls}
        </div>
      }
    >
      <div className="min-h-0 flex-1 overflow-auto p-3">{children}</div>
    </SectionCard>
  )
}

export function SignalList({
  t = englishT,
  loading,
  signals,
  selectedSignalId,
  onSelectSignal,
}: {
  t?: Translate
  loading: boolean
  signals: LabelOntologySignalRecord[]
  selectedSignalId: string | null
  onSelectSignal: (signalId: string) => void
}) {
  if (loading) return <Skeleton className="h-28" />
  if (signals.length === 0) {
    return (
      <Empty>
        <EmptyDescription>{t("No ontology signal rows returned.")}</EmptyDescription>
      </Empty>
    )
  }
  return (
    <div className="space-y-2">
      {signals.map((signal) => (
        <Button
          key={signal.id}
          type="button"
          variant="ghost"
          className={cn(
            "h-auto w-full justify-start rounded-md border border-border bg-card px-3 py-2 text-left text-sm transition-colors hover:bg-muted/60",
            selectedSignalId === signal.id && "border-primary/50 bg-muted",
          )}
          onClick={() => onSelectSignal(signal.id)}
        >
          <div className="flex min-w-0 items-start justify-between gap-2">
            <div className="min-w-0">
              <div className="truncate font-medium">{signalTitle(signal)}</div>
              <div className="mt-1 truncate text-xs text-muted-foreground">{signal.id}</div>
            </div>
            <Badge variant={signalStatusTone(signal.status)}>{signal.status}</Badge>
          </div>
          <div className="mt-2 flex flex-wrap gap-1">
            <Badge variant="secondary">{signal.kind}</Badge>
            <Badge variant="secondary">{signal.proposed_action}</Badge>
            {signal.suggest_score !== null ? (
              <Badge variant="secondary">{t("recorded score {score}", { score: formatScore(signal.suggest_score) })}</Badge>
            ) : null}
          </div>
        </Button>
      ))}
    </div>
  )
}

export function ReviewGroups({
  t = englishT,
  loading,
  groups,
  onSelectSignal,
}: {
  t?: Translate
  loading: boolean
  groups: LabelOntologyReviewGroup[]
  onSelectSignal: (signalId: string) => void
}) {
  if (loading) return <Skeleton className="h-28" />
  if (groups.length === 0) {
    return (
      <Empty>
        <EmptyDescription>{t("No review groups returned.")}</EmptyDescription>
      </Empty>
    )
  }
  return (
    <div className="space-y-2">
      {groups.map((group) => (
        <div key={`${group.group_by}:${group.key}`} className="rounded-md border border-border bg-card p-3 text-sm">
          <div className="flex min-w-0 items-start justify-between gap-2">
            <div className="min-w-0">
              <div className="truncate font-medium">{groupTitle(group)}</div>
              <div className="mt-1 truncate text-xs text-muted-foreground">
                {group.sample_task_refs.length ? group.sample_task_refs.join(", ") : group.key}
              </div>
            </div>
            <Badge variant="review">{t("{count} source tasks", { count: group.task_count })}</Badge>
          </div>
          {group.candidate_text ? <p className="mt-2 text-xs text-muted-foreground">{group.candidate_text}</p> : null}
          <MetricStrip
            className="mt-3 grid-cols-4 text-xs"
            items={[
              { id: "signal-rows", label: t("signal rows"), value: group.signal_count },
              { id: "open", label: t("open"), value: group.open_count },
              { id: "confirmed", label: t("confirmed"), value: group.confirmed_count },
              { id: "actions", label: t("actions"), value: group.action_count },
            ]}
          />
          <div className="mt-3 flex flex-wrap gap-1">
            {group.signal_ids.slice(0, 4).map((signalId) => (
              <Button key={signalId} type="button" variant="outline" size="sm" onClick={() => onSelectSignal(signalId)}>
                {shortId(signalId)}
              </Button>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}

export function SignalDetail({
  t = englishT,
  loading,
  detail,
  actionReason,
  actionPending,
  onActionReasonChange,
  onLifecycleAction,
  onExplainAtom,
}: {
  t?: Translate
  loading: boolean
  detail: LabelOntologySignalDetail | null
  actionReason: string
  actionPending: boolean
  onActionReasonChange: (value: string) => void
  onLifecycleAction: (actionType: LifecycleAction) => void
  onExplainAtom: (atomRef: string | null | undefined) => void
}) {
  if (loading) return <Skeleton className="h-32" />
  if (!detail) {
    return (
      <Empty>
        <EmptyDescription>{t("Select a signal to inspect its observation and actions.")}</EmptyDescription>
      </Empty>
    )
  }
  const signal = detail.signal
  const actionReady = !actionPending && Boolean(actionReason.trim())
  const confirmDisabled = !actionReady || signal.status !== "open"
  const reviewActionDisabled = !actionReady || !["open", "confirmed"].includes(signal.status)
  return (
    <div className="space-y-4 text-sm">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={signalStatusTone(signal.status)}>{signal.status}</Badge>
        <Badge variant="secondary">{signal.kind}</Badge>
        <Badge variant="secondary">{signal.proposed_action}</Badge>
        {detail.observation.suggest_degraded ? <Badge variant="review">{t("observation degraded")}</Badge> : null}
      </div>
      <div>
        <h3 className="text-sm font-semibold">{signalTitle(signal)}</h3>
        <p className="mt-1 text-muted-foreground">{signal.rationale || t("No rationale recorded.")}</p>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Info label={t("source task")} value={detail.observation.task_ref_snapshot} />
        <Info label={t("target")} value={signal.target_label_name_snapshot ?? signal.proposed_label_name ?? "-"} />
        <Info label={t("suggest")} value={signal.suggest_state ?? "-"} />
        <Info label={t("recorded score")} value={signal.suggest_score === null ? "-" : formatScore(signal.suggest_score)} />
        <Info label={t("rank")} value={signal.suggest_rank === null ? "-" : String(signal.suggest_rank)} />
        <Info label={t("recorded confidence")} value={signal.confidence === null ? "-" : formatScore(signal.confidence)} />
      </div>
      {signal.candidate_text ? (
        <div className="rounded-md border border-border bg-card p-3">
          <div className="mb-2 flex items-center justify-between gap-2">
            <span className="text-xs font-medium uppercase text-muted-foreground">{t("Candidate atom")}</span>
            <Button type="button" variant="outline" size="sm" onClick={() => onExplainAtom(signal.candidate_content_hash)}>
              <Braces className="h-4 w-4" />
              {t("Explain hash")}
            </Button>
          </div>
          <p className="text-sm">{signal.candidate_text}</p>
          <div className="mt-2 text-xs text-muted-foreground">
            {signal.candidate_atom_polarity ?? "-"} / {signal.candidate_atom_kind ?? "-"} / {signal.candidate_content_hash ?? "-"}
          </div>
        </div>
      ) : null}
      <Separator />
      <div className="space-y-2">
        <Textarea
          aria-label={t("Review action reason")}
          name="ontology-action-reason"
          autoComplete="off"
          value={actionReason}
          onChange={(event) => onActionReasonChange(event.target.value)}
          placeholder={t("Reason for lifecycle action")}
        />
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="secondary" disabled={confirmDisabled} onClick={() => onLifecycleAction("confirm")}>
            <CheckCircle2 className="h-4 w-4" />
            {t("Confirm signal")}
          </Button>
          <Button type="button" variant="outline" disabled={reviewActionDisabled} onClick={() => onLifecycleAction("resolve_no_change")}>
            <CircleDashed className="h-4 w-4" />
            {t("Resolve no change")}
          </Button>
          <Button type="button" variant="outline" disabled={reviewActionDisabled} onClick={() => onLifecycleAction("reject")}>
            <XCircle className="h-4 w-4" />
            {t("Reject")}
          </Button>
        </div>
      </div>
      <ActionHistory t={t} actions={detail.actions} onExplainAtom={onExplainAtom} />
    </div>
  )
}

function ActionHistory({
  t,
  actions,
  onExplainAtom,
}: {
  t: Translate
  actions: LabelOntologyActionRecord[]
  onExplainAtom: (atomRef: string | null | undefined) => void
}) {
  if (actions.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("No actions recorded for this signal.")}</p>
  }
  return (
    <div className="space-y-2">
      <h3 className="text-sm font-semibold">{t("Actions")}</h3>
      {actions.map((action) => (
        <div key={action.id} className="rounded-md border border-border bg-card p-3">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">{action.action_type}</Badge>
            <Badge variant="secondary">{t("requires {requirement}", { requirement: action.validation_requirement })}</Badge>
            <Badge variant={validationTone(action.validation_effective_outcome)}>{action.validation_effective_outcome}</Badge>
            <span className="text-xs text-muted-foreground">{shortId(action.id)}</span>
          </div>
          <p className="mt-2 text-sm text-muted-foreground">{action.reason}</p>
          {action.result_atom_id || action.result_atom_content_hash ? (
            <div className="mt-2 flex flex-wrap gap-2">
              <Button type="button" variant="outline" size="sm" onClick={() => onExplainAtom(action.result_atom_id)}>
                {t("atom {id}", { id: shortId(action.result_atom_id) })}
              </Button>
              <Button type="button" variant="outline" size="sm" onClick={() => onExplainAtom(action.result_atom_content_hash)}>
                {t("hash {id}", { id: shortId(action.result_atom_content_hash) })}
              </Button>
            </div>
          ) : null}
        </div>
      ))}
    </div>
  )
}

export function AtomExplain({ t = englishT, loading, explain }: { t?: Translate; loading: boolean; explain: LabelAtomExplainRecord | null }) {
  if (loading) return <Skeleton className="h-24" />
  if (!explain) {
    return (
      <Empty className="p-3">
        <EmptyDescription>{t("Enter an atom id or content hash.")}</EmptyDescription>
      </Empty>
    )
  }
  return (
    <div className="space-y-3 text-sm">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={explain.legacy_untracked ? "review" : "ready"}>
          {explain.legacy_untracked ? t("legacy untracked") : t("has provenance records")}
        </Badge>
        {explain.atom ? <Badge variant="secondary">{explain.atom.label_name}</Badge> : null}
        {explain.supporting_signals.some((entry) => entry.suggest_degraded) ? (
          <Badge variant="review">{t("degraded evidence")}</Badge>
        ) : null}
      </div>
      {explain.atom ? (
        <div className="rounded-md border border-border bg-card p-3">
          <div className="font-medium">{explain.atom.kind}</div>
          <p className="mt-1 text-muted-foreground">{explain.atom.text}</p>
          <div className="mt-2 text-xs text-muted-foreground">{explain.atom.id} / {explain.atom.content_hash}</div>
        </div>
      ) : (
        <p className="text-muted-foreground">{t("No current atom resolved for {query}.", { query: explain.query })}</p>
      )}
      {explain.legacy_reason ? <p className="text-xs text-muted-foreground">{explain.legacy_reason}</p> : null}
      <MetricStrip
        className="grid-cols-3 text-xs"
        items={[
          { id: "actions", label: t("actions"), value: explain.provenance_actions.length },
          { id: "signal-rows", label: t("signal rows"), value: explain.supporting_signals.length },
          { id: "validations", label: t("validations"), value: explain.validation_history.length },
        ]}
      />
      {explain.provenance_actions.slice(0, 4).map((entry) => (
        <div key={entry.action.id} className="rounded-md border border-border bg-card p-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">{entry.action.action_type}</Badge>
            <span className="text-xs text-muted-foreground">{entry.matched_by}</span>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{entry.action.reason}</p>
        </div>
      ))}
      {explain.validation_history.slice(0, 4).map((entry) => (
        <div key={entry.action.id} className="rounded-md border border-border bg-card p-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={validationTone(entry.validation_status)}>{entry.validation_status}</Badge>
            <span className="text-xs text-muted-foreground">{t("parent {id}", { id: shortId(entry.parent_action_id) })}</span>
          </div>
          {entry.warnings.length ? <p className="mt-1 text-xs text-muted-foreground">{entry.warnings.join("; ")}</p> : null}
        </div>
      ))}
    </div>
  )
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border border-border bg-card px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="truncate text-sm font-medium">{value}</div>
    </div>
  )
}

function signalTitle(signal: LabelOntologySignalRecord) {
  return signal.target_label_name_snapshot ?? signal.proposed_label_name ?? signal.candidate_text ?? signal.kind
}

function groupTitle(group: LabelOntologyReviewGroup) {
  return group.label_name ?? group.proposed_label_name ?? group.candidate_text ?? group.key
}

function signalStatusTone(status: LabelOntologySignalStatus): BadgeProps["variant"] {
  if (status === "confirmed") return "review"
  if (status === "resolved") return "ready"
  if (status === "rejected" || status === "superseded") return "blocked"
  return "secondary"
}

function validationTone(status: string): BadgeProps["variant"] {
  if (status === "passed" || status === "not_required") return "ready"
  if (status === "pending" || status === "partial") return "review"
  if (status === "failed") return "blocked"
  return "secondary"
}

function formatScore(value: number) {
  return value.toFixed(2)
}

function shortId(value: string | null | undefined) {
  if (!value) return "-"
  return value.length > 12 ? `${value.slice(0, 10)}...` : value
}

function errorText(err: unknown) {
  if (err instanceof Error) return err.message
  return String(err)
}
