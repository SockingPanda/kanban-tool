import { isValidElement, type ReactElement, type ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import type {
  LabelAtomExplainRecord,
  LabelOntologyActionRecord,
  LabelOntologyReviewGroup,
  LabelOntologySignalDetail,
  LabelOntologySignalRecord,
} from "../../lib/api/signals-ontology-read-model"
import { featureCopyForLocale } from "../../lib/i18n"
import type { OntologyRouteFilters } from "../../lib/router"

import {
  AtomExplainView,
  OntologyScreenView,
  ReviewGroupsView,
  OntologySignalDetailView,
  type LifecycleAction,
} from "./OntologyScreen"
import { createOntologyLifecycleAttempt, executeOntologyLifecycleAttempt, sameOntologyLifecycleIdentity } from "./ontology-lifecycle"
import { scopeReviewGroupsToKnownSignals } from "./ontology-review-scope"

const signalFixture = (overrides: Partial<LabelOntologySignalRecord> = {}): LabelOntologySignalRecord => ({
  id: "los_1",
  observation_id: "loo_1",
  board_id: "b_1",
  kind: "false_negative",
  status: "open",
  target_label_id: "lab_cli",
  target_label_name_snapshot: "cli",
  related_labels: [],
  proposed_action: "add_positive_atom",
  candidate_atom_polarity: "positive",
  candidate_atom_kind: "applies_when",
  candidate_text: "touches CLI behavior",
  candidate_content_hash: "hash_1",
  proposed_label_name: null,
  proposed_label_name_normalized: null,
  proposal: {},
  agent_selected: true,
  suggest_state: "absent",
  suggest_score: 0.12,
  suggest_rank: 4,
  final_selected: true,
  rationale: "Review rationale",
  confidence: 0.9,
  signal_key: "signal-key",
  superseded_by_signal_id: null,
  status_reason: null,
  created_at: 1,
  updated_at: 1,
  reviewed_at: null,
  closed_at: null,
  ...overrides,
})

const actionFixture = (overrides: Partial<LabelOntologyActionRecord> = {}): LabelOntologyActionRecord => ({
  id: "loa_1",
  board_id: "b_1",
  parent_action_id: null,
  action_type: "confirm",
  reason: "Reviewed",
  target_label_id: null,
  result_label_id: null,
  result_atom_id: "lat_1",
  result_atom_content_hash: "hash_1",
  result_proposal_id: null,
  canonical_before_hash: null,
  canonical_after_hash: null,
  change: {},
  validation_requirement: "none",
  validation_status: "not_required",
  validation_effective_outcome: "not_required",
  validation_latest_attempt_id: null,
  validation: {},
  created_by: "codex",
  created_by_type: "user",
  agent_type: null,
  created_at: 1,
  signal_ids: ["los_1"],
  ...overrides,
})

const detailFixture = (overrides: Partial<LabelOntologySignalDetail> = {}): LabelOntologySignalDetail => ({
  signal: signalFixture(),
  observation: {
    id: "loo_1",
    board_id: "b_1",
    task_id: "t_1",
    task_ref_snapshot: "default#1",
    task_snapshot: {},
    suggest_input_hash: "input-hash",
    agent_candidates: [],
    suggestion_snapshot: {},
    final_decision: {},
    suggest_coverage: 0.6,
    suggest_coverage_cosine: 0.7,
    suggest_residual_norm: 0.4,
    suggest_needs_new_label: false,
    suggest_degraded: false,
    diagnostics: [],
    capture_fingerprint: "fingerprint",
    created_by: "codex",
    created_by_type: "user",
    agent_type: null,
    created_at: 1,
    signals: [],
  },
  actions: [],
  ...overrides,
})

const reviewGroupFixture = (overrides: Partial<LabelOntologyReviewGroup> = {}): LabelOntologyReviewGroup => ({
  group_by: "label",
  key: "lab_cli",
  label_id: "lab_cli",
  label_name: "cli",
  candidate_atom_polarity: "positive",
  candidate_atom_kind: "applies_when",
  candidate_text: "touches CLI behavior",
  candidate_content_hash: "hash_1",
  proposed_label_name: null,
  proposed_label_name_normalized: null,
  cluster_key: null,
  cluster_reason: null,
  task_count: 1,
  signal_count: 1,
  open_count: 1,
  confirmed_count: 0,
  resolved_count: 0,
  rejected_count: 0,
  superseded_count: 0,
  degraded_count: 0,
  average_score: 0.12,
  median_score: 0.12,
  oldest_signal_at: 1,
  latest_signal_at: 1,
  sample_task_refs: ["default#1"],
  signal_ids: ["los_1"],
  action_count: 0,
  action_ids: [],
  proposal_ids: [],
  labels: [{ id: "lab_cli", name: "cli" }],
  candidate_atom_variants: [],
  ...overrides,
})

const atomExplainFixture = (overrides: Partial<LabelAtomExplainRecord> = {}): LabelAtomExplainRecord => ({
  query: "hash_1",
  atom: {
    id: "lat_1",
    label_id: "lab_cli",
    board_id: "b_1",
    label_name: "cli",
    polarity: "positive",
    kind: "applies_when",
    text: "touches CLI behavior",
    ordinal: 0,
    content_hash: "hash_1",
    created_at: 1,
    updated_at: 1,
  },
  current_semantics: null,
  provenance_actions: [{ action: actionFixture({ action_type: "add_positive_atom" }), matched_by: "content_hash" }],
  supporting_signals: [],
  validation_history: [],
  legacy_untracked: false,
  legacy_reason: null,
  ...overrides,
})

const filters: OntologyRouteFilters = { includeAll: false, groupBy: "label" }

describe("Ontology screen presentation", () => {
  test("states the canonical semantics boundary and renders read projections", () => {
    const html = renderToStaticMarkup(
      <OntologyScreenView
        boardName="Default"
        filters={filters}
        signals={{ phase: "success", data: [signalFixture()], error: null }}
        groups={{ phase: "success", data: [reviewGroupFixture()], error: null }}
        detail={{ phase: "success", data: detailFixture(), error: null }}
        atom={{ phase: "idle", data: null, error: null }}
        selectedSignalId="los_1"
        atomRef=""
        online
        actionReason=""
        actionPending={false}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onActionReasonChange={() => undefined}
        onLifecycleAction={() => undefined}
        onExplainAtom={() => undefined}
        onAtomSearch={() => undefined}
      />,
    )

    expect(html).toContain("Ontology review")
    expect(html).toContain("does not modify canonical semantics")
    expect(html).toContain("Signal rows")
    expect(html).toContain("Grouped review")
    expect(html).toContain("Candidate atom")
  })

  test.each([
    ["zh", "本体审阅", "关闭详情"],
    ["en", "Ontology review", "Close detail"],
  ] as const)("renders feature copy in %s while keeping machine values literal", (locale, heading, closeDetail) => {
    const html = renderToStaticMarkup(
      <OntologyScreenView
        boardName="Default"
        filters={filters}
        signals={{ phase: "success", data: [signalFixture()], error: null }}
        groups={{ phase: "success", data: [reviewGroupFixture()], error: null }}
        detail={{ phase: "success", data: detailFixture(), error: null }}
        atom={{ phase: "success", data: atomExplainFixture(), error: null }}
        selectedSignalId="los_1"
        atomRef="hash_1"
        atomDraft="hash_1"
        online
        actionReason=""
        actionPending={false}
        copy={featureCopyForLocale(locale).ontology}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onActionReasonChange={() => undefined}
        onLifecycleAction={() => undefined}
        onExplainAtom={() => undefined}
        onAtomSearch={() => undefined}
        onCloseDetail={() => undefined}
      />,
    )

    expect(html).toContain(heading)
    expect(html).toContain(closeDetail)
    expect(html).toContain('translate="no">los_1')
    expect(html).toContain('translate="no">positive / applies_when / hash_1')
  })

  test("localizes the ontology detail status subtitle while preserving the machine badge", () => {
    const html = renderToStaticMarkup(
      <OntologyScreenView
        boardName="Default"
        filters={filters}
        signals={{ phase: "success", data: [signalFixture()], error: null }}
        groups={{ phase: "success", data: [reviewGroupFixture()], error: null }}
        detail={{ phase: "success", data: detailFixture(), error: null }}
        atom={{ phase: "idle", data: null, error: null }}
        selectedSignalId="los_1"
        atomRef=""
        online
        actionReason=""
        actionPending={false}
        copy={featureCopyForLocale("zh").ontology}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onActionReasonChange={() => undefined}
        onLifecycleAction={() => undefined}
        onExplainAtom={() => undefined}
        onAtomSearch={() => undefined}
      />,
    )

    expect(html).toContain("开放")
    expect(html).toMatch(/<span translate="no"><span[^>]*>open<\/span><\/span>/)
  })

  test("selects review group source rows and avoids quality-rate claims", () => {
    const onSelectSignal = vi.fn()
    const tree = ReviewGroupsView({ phase: "success", groups: [reviewGroupFixture({ signal_ids: ["los_1"] })], onSelectSignal })
    const button = findButtonByText(tree, "los_1")
    button?.props.onClick?.()
    expect(onSelectSignal).toHaveBeenCalledWith("los_1")
    const html = renderToStaticMarkup(tree)
    expect(html).toContain("1 source tasks")
    expect(html).toContain("signal rows")
    expect(html).not.toMatch(/precision|recall|error rate/i)
  })

  test("bounds review-group scope checks to a complete current-board signal page", () => {
    const groups = [reviewGroupFixture(), reviewGroupFixture({ key: "foreign", signal_ids: ["los_other"] }), reviewGroupFixture({ key: "empty", signal_ids: [] })]
    expect(scopeReviewGroupsToKnownSignals(groups, [signalFixture()], true).map((group) => group.key)).toEqual(["lab_cli"])
    expect(scopeReviewGroupsToKnownSignals(groups, [signalFixture()], false).map((group) => group.key)).toEqual(["lab_cli", "foreign"])
  })

  test("does not expose an unsupported cluster grouping tab", () => {
    const html = renderToStaticMarkup(
      <OntologyScreenView
        boardName="Default"
        filters={{ includeAll: false, groupBy: "label" }}
        signals={{ phase: "success", data: [], error: null }}
        groups={{ phase: "success", data: [reviewGroupFixture()], error: null }}
        detail={{ phase: "idle", data: null, error: null }}
        atom={{ phase: "idle", data: null, error: null }}
        selectedSignalId={null}
        atomRef=""
        online
        actionReason=""
        actionPending={false}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onActionReasonChange={() => undefined}
        onLifecycleAction={() => undefined}
        onExplainAtom={() => undefined}
        onAtomSearch={() => undefined}
      />,
    )
    expect(html).not.toContain(">Cluster<")
  })

  test("maps lifecycle controls to callbacks and keeps them disabled for resolved signals", () => {
    const action = vi.fn<(action: LifecycleAction) => void>()
    const tree = OntologySignalDetailView({
        phase: "success",
        detail: detailFixture(),
        actionReason: "Reviewed",
        actionPending: false,
        lifecycleEnabled: true,
        onActionReasonChange: () => undefined,
        onLifecycleAction: action,
        onExplainAtom: () => undefined,
      })
    const confirm = findButtonByText(tree, "Confirm signal")
    expect(confirm?.props.isDisabled).toBe(false)
    confirm?.props.onClick?.()
    expect(action).toHaveBeenCalledWith("confirm")
    const resolved = OntologySignalDetailView({
        phase: "success",
        detail: detailFixture({ signal: signalFixture({ status: "resolved" }) }),
        actionReason: "Reviewed",
        actionPending: false,
        onActionReasonChange: () => undefined,
        onLifecycleAction: action,
        onExplainAtom: () => undefined,
      })
    expect(findButtonByText(resolved, "Confirm signal")?.props.isDisabled).toBe(true)
  })

  test("keeps the read workbench action-free until a lifecycle writer is supplied", () => {
    const tree = OntologySignalDetailView({
      phase: "success",
      detail: detailFixture(),
      actionReason: "Reviewed",
      actionPending: false,
      lifecycleEnabled: false,
      onActionReasonChange: () => undefined,
      onLifecycleAction: () => undefined,
      onExplainAtom: () => undefined,
    })
    expect(findButtonByText(tree, "Confirm signal")?.props.isDisabled).toBe(true)
  })

  test("freezes A retry after switching to B and fences the stale async response", async () => {
    const identityA = { api: null, signalId: "los_a" } as const
    const identityB = { api: null, signalId: "los_b" } as const
    const attemptA = createOntologyLifecycleAttempt("confirm", "los_a", "keep A", identityA)
    const attemptB = createOntologyLifecycleAttempt("reject", "los_b", "reject B", identityB)
    const calls: Array<{ action: LifecycleAction; signalId: string; reason: string }> = []
    let rejectA!: (error: unknown) => void
    const pendingA = new Promise<void>((_resolve, reject) => {
      rejectA = reject
    })
    let currentIdentity: typeof identityA | typeof identityB = identityA
    const invoke = vi.fn((action: LifecycleAction, signalId: string, reason: string) => {
      calls.push({ action, signalId, reason })
      return calls.length === 1 ? pendingA : Promise.resolve()
    })

    const first = executeOntologyLifecycleAttempt(attemptA, invoke, () => sameOntologyLifecycleIdentity(currentIdentity, attemptA.identity))
    currentIdentity = identityB // A fails after the user has selected B.
    rejectA(new Error("A failed"))
    await expect(first).resolves.toMatchObject({ kind: "stale" })
    await expect(executeOntologyLifecycleAttempt(attemptA, invoke, () => sameOntologyLifecycleIdentity(currentIdentity, attemptA.identity))).resolves.toMatchObject({ kind: "stale" })
    expect(calls).toEqual([
      { action: "confirm", signalId: "los_a", reason: "keep A" },
      { action: "confirm", signalId: "los_a", reason: "keep A" },
    ])
    expect(attemptA.signalId).toBe("los_a")
    expect(attemptA.reason).toBe("keep A")
    expect(sameOntologyLifecycleIdentity(identityB, attemptA.identity)).toBe(false)
    expect(sameOntologyLifecycleIdentity(identityB, attemptB.identity)).toBe(true)
  })

  test("UI retry keeps A snapshot after selecting B and hides the stale response", async () => {
    const identityA = { api: null, signalId: "los_a" } as const
    const identityB = { api: null, signalId: "los_b" } as const
    const attemptA = createOntologyLifecycleAttempt("confirm", "los_a", "keep A", identityA)
    const calls: Array<{ action: LifecycleAction; signalId: string; reason: string }> = []
    const results: string[] = []
    let rejectA!: (error: unknown) => void
    const pendingA = new Promise<void>((_resolve, reject) => {
      rejectA = reject
    })
    let currentIdentity: typeof identityA | typeof identityB = identityA
    const invoke = vi.fn((action: LifecycleAction, signalId: string, reason: string) => {
      calls.push({ action, signalId, reason })
      return calls.length === 1 ? pendingA : Promise.resolve()
    })
    const retryA = () => {
      void executeOntologyLifecycleAttempt(attemptA, invoke, () => sameOntologyLifecycleIdentity(currentIdentity, attemptA.identity))
        .then((result) => results.push(result.kind))
    }
    const treeA = OntologySignalDetailView({
      phase: "success",
      detail: detailFixture({ signal: signalFixture({ id: "los_a" }) }),
      actionError: new Error("A failed"),
      actionReason: "keep A",
      actionPending: false,
      lifecycleEnabled: true,
      onRetryAction: retryA,
      onActionReasonChange: () => undefined,
      onLifecycleAction: () => undefined,
      onExplainAtom: () => undefined,
    })
    const retryButton = findButtonByLabel(treeA, "Retry action")
    expect(retryButton).not.toBeNull()
    retryButton?.props.onClick?.()

    currentIdentity = identityB
    rejectA(new Error("A failed"))
    await vi.waitFor(() => expect(results).toEqual(["stale"]))
    const treeB = OntologySignalDetailView({
      phase: "success",
      detail: detailFixture({ signal: signalFixture({ id: "los_b" }) }),
      actionError: null,
      actionReason: "new B reason",
      actionPending: false,
      lifecycleEnabled: true,
      onRetryAction: retryA,
      onActionReasonChange: () => undefined,
      onLifecycleAction: () => undefined,
      onExplainAtom: () => undefined,
    })
    expect(findButtonByLabel(treeB, "Retry action")).toBeNull()

    retryButton?.props.onClick?.()
    await vi.waitFor(() => expect(results).toEqual(["stale", "stale"]))
    expect(calls).toEqual([
      { action: "confirm", signalId: "los_a", reason: "keep A" },
      { action: "confirm", signalId: "los_a", reason: "keep A" },
    ])
    expect(renderToStaticMarkup(treeB)).not.toContain("A failed")
  })

  test("separates a missing signal detail from an empty selection", () => {
    const tree = OntologySignalDetailView({
      phase: "error",
      detail: null,
      error: { status: 404 },
      actionReason: "",
      actionPending: false,
      onActionReasonChange: () => undefined,
      onLifecycleAction: () => undefined,
      onExplainAtom: () => undefined,
    })
    expect(renderToStaticMarkup(tree)).toContain("Ontology signal is no longer available")
  })

  test("removes a not-found detail row from the list projection", () => {
    const html = renderToStaticMarkup(
      <OntologyScreenView
        boardName="Default"
        filters={filters}
        signals={{ phase: "success", data: [signalFixture()], error: null }}
        groups={{ phase: "success", data: [], error: null }}
        detail={{ phase: "error", data: null, error: { status: 404 } }}
        atom={{ phase: "idle", data: null, error: null }}
        selectedSignalId="los_1"
        atomRef=""
        online
        actionReason=""
        actionPending={false}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onActionReasonChange={() => undefined}
        onLifecycleAction={() => undefined}
        onExplainAtom={() => undefined}
        onAtomSearch={() => undefined}
      />,
    )
    expect(html).not.toContain('translate="no">los_1 · false_negative')
    expect(html).toContain("Ontology signal is no longer available")
  })

  test("renders degraded atom evidence and validation status without claiming success", () => {
    const html = renderToStaticMarkup(
      <AtomExplainView
        phase="success"
        explain={atomExplainFixture()}
      />,
    )
    expect(html).toContain("has provenance records")
    expect(html).toContain("touches CLI behavior")
    expect(html).not.toContain("passed")
  })
})

type ButtonProps = { type?: "button"; onClick?: () => void; isDisabled?: boolean; children?: ReactNode; description?: ReactNode; endContent?: ReactNode; label?: string }

function findButtonByText(node: ReactNode, text: string): ReactElement<ButtonProps> | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findButtonByText(child, text)
      if (match) return match
    }
    return null
  }
  if (!isValidElement(node)) return null
  const element = node as ReactElement<ButtonProps>
  if (element.props.label === text) return element
  return findButtonByText([element.props.children, element.props.description, element.props.endContent], text)
}

function findButtonByLabel(node: unknown, text: string): ReactElement<ButtonProps> | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findButtonByLabel(child, text)
      if (match) return match
    }
    return null
  }
  if (!isValidElement(node)) return null
  const element = node as ReactElement<ButtonProps>
  if (element.props.label === text) return element
  for (const value of Object.values(element.props as Record<string, unknown>)) {
    const match = findButtonByLabel(value, text)
    if (match) return match
  }
  return null
}
