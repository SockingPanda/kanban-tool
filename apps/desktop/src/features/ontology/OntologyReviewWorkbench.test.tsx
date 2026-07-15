import { readFileSync } from "node:fs"
import { isValidElement, type ReactElement, type ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it, vi } from "vitest"

import type {
  LabelAtomExplainRecord,
  LabelOntologyActionRecord,
  LabelOntologyObservationRecord,
  LabelOntologyReviewGroup,
  LabelOntologySignalDetail,
  LabelOntologySignalRecord,
  Task,
} from "@/lib/api"

import { AtomExplain, OntologyReviewWorkbench, ReviewGroups, SignalDetail, SignalList } from "./OntologyReviewWorkbench"

const workbenchSource = readFileSync(new URL("./OntologyReviewWorkbench.tsx", import.meta.url), "utf8")

describe("OntologyReviewWorkbench presentation", () => {
  it("states the canonical semantics boundary and loaded limit in the shell", () => {
    const html = renderWorkbenchShell()

    expect(html).toContain("Review aid; does not modify canonical semantics.")
    expect(html).toContain("Lifecycle actions do not modify canonical label semantics.")
    expect(html).toContain("Signal rows")
    expect(html).toContain("0 of up to 100 loaded")
    expect(html).toContain("0 of up to 100 groups loaded")
  })

  it("renders open, confirmed, and resolved signal states distinctly", () => {
    const html = renderToStaticMarkup(
      <SignalList
        loading={false}
        signals={[
          signalFixture({ id: "los_open", status: "open", target_label_name_snapshot: "cli" }),
          signalFixture({ id: "los_confirmed", status: "confirmed", target_label_name_snapshot: "api" }),
          signalFixture({ id: "los_resolved", status: "resolved", target_label_name_snapshot: "docs" }),
        ]}
        selectedSignalId="los_confirmed"
        onSelectSignal={() => undefined}
      />,
    )

    expect(html).toContain("cli")
    expect(html).toContain("api")
    expect(html).toContain("docs")
    expect(html).toContain("open")
    expect(html).toContain("confirmed")
    expect(html).toContain("resolved")
    expect(html).toContain("recorded score")
  })

  it("lets review groups select source signal rows without quality/error-rate language", () => {
    const onSelectSignal = vi.fn()
    const tree = ReviewGroups({
      loading: false,
      groups: [reviewGroupFixture({ signal_ids: ["los_1", "los_2"] })],
      onSelectSignal,
    })

    const signalButton = findButtonByText(tree, "los_1")
    expect(signalButton?.props.type).toBe("button")
    signalButton?.props.onClick?.()
    expect(onSelectSignal).toHaveBeenCalledWith("los_1")

    const html = renderToStaticMarkup(tree)
    expect(html).toContain("1 source tasks")
    expect(html).toContain("signal rows")
    expect(html).not.toMatch(/\b(precision|recall|correct|error rate)\b/i)

    const labels = findButtonControls(tree).map((button) => textContent(button))
    expect(labels.join(" ")).not.toMatch(/\b(apply|validate|revert)\b/i)
  })

  it("maps lifecycle controls to lifecycle callbacks and disables them for resolved signals", () => {
    const onLifecycleAction = vi.fn()
    const detail = signalDetailFixture({
      signal: signalFixture({ id: "los_open", status: "open" }),
      actions: [
        actionFixture({ id: "loa_apply", action_type: "add_positive_atom", validation_status: "pending" }),
        actionFixture({ id: "loa_validate", action_type: "validate", validation_status: "failed" }),
      ],
    })

    const tree = SignalDetail({
      loading: false,
      detail,
      actionReason: "Reviewed in desktop",
      actionPending: false,
      onActionReasonChange: () => undefined,
      onLifecycleAction,
      onExplainAtom: () => undefined,
    })

    const confirmButton = findButtonByText(tree, "Confirm signal")
    expect(confirmButton?.props.disabled).toBe(false)
    confirmButton?.props.onClick?.()
    expect(onLifecycleAction).toHaveBeenCalledWith("confirm")

    const labels = findButtonControls(tree).map((button) => textContent(button))
    expect(labels).toEqual(expect.arrayContaining(["Confirm signal", "Resolve no change", "Reject"]))
    expect(labels.join(" ")).not.toMatch(/\b(Apply|Validate|Revert)\b/)

    const confirmedTree = SignalDetail({
      loading: false,
      detail: signalDetailFixture({ signal: signalFixture({ id: "los_reviewed", status: "confirmed" }) }),
      actionReason: "Reviewed in desktop",
      actionPending: false,
      onActionReasonChange: () => undefined,
      onLifecycleAction,
      onExplainAtom: () => undefined,
    })

    expect(findButtonByText(confirmedTree, "Confirm signal")?.props.disabled).toBe(true)
    expect(findButtonByText(confirmedTree, "Resolve no change")?.props.disabled).toBe(false)
    expect(findButtonByText(confirmedTree, "Reject")?.props.disabled).toBe(false)

    const resolvedTree = SignalDetail({
      loading: false,
      detail: signalDetailFixture({ signal: signalFixture({ id: "los_done", status: "resolved" }) }),
      actionReason: "Reviewed in desktop",
      actionPending: false,
      onActionReasonChange: () => undefined,
      onLifecycleAction,
      onExplainAtom: () => undefined,
    })

    expect(findButtonByText(resolvedTree, "Confirm signal")?.props.disabled).toBe(true)
  })

  it("renders degraded signal detail and effective validation state", () => {
    const html = renderToStaticMarkup(
      <SignalDetail
        loading={false}
        detail={signalDetailFixture({
          observation: observationFixture({ suggest_degraded: true }),
          actions: [
            actionFixture({
              id: "loa_required",
              action_type: "add_positive_atom",
              validation_requirement: "required",
              validation_status: "pending",
              validation_effective_outcome: "failed",
            }),
          ],
        })}
        actionReason="Reviewed in desktop"
        actionPending={false}
        onActionReasonChange={() => undefined}
        onLifecycleAction={() => undefined}
        onExplainAtom={() => undefined}
      />,
    )

    expect(html).toContain("observation degraded")
    expect(html).toContain("source task")
    expect(html).toContain("recorded score")
    expect(html).toContain("recorded confidence")
    expect(html).toContain("requires required")
    expect(html).toContain("failed")
    expect(html).not.toContain("pending")
  })

  it("renders degraded atom evidence and non-passing validation without showing it as passed", () => {
    const html = renderToStaticMarkup(
      <AtomExplain
        loading={false}
        explain={atomExplainFixture({
          supporting_signals: [
            {
              signal: signalFixture({ id: "los_degraded", status: "confirmed" }),
              observation: observationFixture({ id: "loo_degraded" }),
              source_task: taskFixture({ id: "t_degraded", ref: "default#9" }),
              task_ref_snapshot: "default#9",
              suggest_input_stale: false,
              suggest_degraded: true,
              warnings: ["index degraded"],
            },
          ],
          validation_history: [
            {
              action: actionFixture({ id: "loa_failed", action_type: "validate", validation_status: "failed" }),
              parent_action_id: "loa_parent",
              validation_status: "failed",
              manual: {},
              summary: {},
              cases: {},
              warnings: ["dirty index"],
            },
          ],
        })}
      />,
    )

    expect(html).toContain("degraded evidence")
    expect(html).toContain("has provenance records")
    expect(html).toContain("signal rows")
    expect(html).toContain("failed")
    expect(html).toContain("dirty index")
    expect(html).not.toContain("passed")
  })

  it("keeps canonical mutation controls out of the workbench source", () => {
    expect(workbenchSource.match(/createLabelOntologyAction/g) ?? []).toHaveLength(1)
    expect(workbenchSource).toContain(
      'type LifecycleAction = Extract<LabelOntologyActionType, "confirm" | "reject" | "resolve_no_change">',
    )
    expect(workbenchSource).not.toMatch(
      /"(?:add_positive_atom|add_negative_atom|adopt_existing_atom|update_semantics|bootstrap_label|revert_ontology_mutation|validate)"/,
    )
    expect(workbenchSource).not.toMatch(
      /\.(?:applyLabelOntologyAtom|upsertLabelSemantics|deleteLabelSemantics|revertLabelOntologyMutation|validateLabelOntologyAction)\b/,
    )
  })
})

type ButtonProps = {
  type?: "button" | "submit" | "reset"
  disabled?: boolean
  onClick?: () => void
  children?: ReactNode
}

function findButtonByText(node: ReactNode, text: string) {
  return findButtonControls(node).find((button) => textContent(button).trim() === text) ?? null
}

function findButtonControls(node: ReactNode): ReactElement<ButtonProps>[] {
  if (Array.isArray(node)) return node.flatMap((child) => findButtonControls(child))
  if (!isValidElement(node)) return []

  const element = node as ReactElement<ButtonProps>
  const matches = element.props.type === "button" ? [element] : []
  return matches.concat(findButtonControls(element.props.children))
}

function textContent(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node)
  if (Array.isArray(node)) return node.map(textContent).join("")
  if (!isValidElement(node)) return ""
  return textContent((node as ReactElement<{ children?: ReactNode }>).props.children)
}

function renderWorkbenchShell(): string {
  const queryClient = new QueryClient()
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <OntologyReviewWorkbench api={null} />
    </QueryClientProvider>,
  )
}

function signalFixture(overrides: Partial<LabelOntologySignalRecord> = {}): LabelOntologySignalRecord {
  return {
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
  }
}

function observationFixture(overrides: Partial<LabelOntologyObservationRecord> = {}): LabelOntologyObservationRecord {
  return {
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
    created_by: "desktop-test",
    created_by_type: "user",
    agent_type: null,
    created_at: 1,
    signals: [],
    ...overrides,
  }
}

function actionFixture(overrides: Partial<LabelOntologyActionRecord> = {}): LabelOntologyActionRecord {
  return {
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
    created_by: "desktop-test",
    created_by_type: "user",
    agent_type: null,
    created_at: 1,
    signal_ids: ["los_1"],
    ...overrides,
  }
}

function signalDetailFixture(overrides: Partial<LabelOntologySignalDetail> = {}): LabelOntologySignalDetail {
  const signal = overrides.signal ?? signalFixture()
  return {
    signal,
    observation: observationFixture({ signals: [signal] }),
    actions: [],
    ...overrides,
  }
}

function reviewGroupFixture(overrides: Partial<LabelOntologyReviewGroup> = {}): LabelOntologyReviewGroup {
  return {
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
  }
}

function atomExplainFixture(overrides: Partial<LabelAtomExplainRecord> = {}): LabelAtomExplainRecord {
  return {
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
  }
}

function taskFixture(overrides: Partial<Task> = {}): Task {
  return {
    id: "t_1",
    board_id: "b_1",
    board_slug: "default",
    ref: "default#1",
    seq: 1,
    title: "Task",
    description: null,
    status: "ready",
    status_reason: null,
    assignee: null,
    priority: 0,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "seed",
    created_at: 1,
    updated_at: 1,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result: null,
    metadata: {},
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "unplanned",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
    ...overrides,
  }
}
