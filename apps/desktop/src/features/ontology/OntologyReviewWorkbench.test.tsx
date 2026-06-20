import { isValidElement, type ReactElement, type ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
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

import { AtomExplain, ReviewGroups, SignalDetail, SignalList } from "./OntologyReviewWorkbench"

describe("OntologyReviewWorkbench presentation", () => {
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
  })

  it("lets review groups select source signals without introducing mutation commands", () => {
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

    const confirmButton = findButtonByText(tree, "Confirm")
    expect(confirmButton?.props.disabled).toBe(false)
    confirmButton?.props.onClick?.()
    expect(onLifecycleAction).toHaveBeenCalledWith("confirm")

    const labels = findButtonControls(tree).map((button) => textContent(button))
    expect(labels).toEqual(expect.arrayContaining(["Confirm", "Resolve no change", "Reject"]))
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

    expect(findButtonByText(confirmedTree, "Confirm")?.props.disabled).toBe(true)
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

    expect(findButtonByText(resolvedTree, "Confirm")?.props.disabled).toBe(true)
  })

  it("renders degraded evidence and non-passing validation without showing it as passed", () => {
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
    expect(html).toContain("failed")
    expect(html).toContain("dirty index")
    expect(html).not.toContain("passed")
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

function signalFixture(overrides: Partial<LabelOntologySignalRecord> = {}): LabelOntologySignalRecord {
  return {
    id: "los_1",
    observation_id: "loo_1",
    board_id: "b_1",
    kind: "false_negative",
    status: "open",
    target_label_id: "lab_cli",
    target_label_name_snapshot: "cli",
    related_labels_json: "[]",
    proposed_action: "add_positive_atom",
    candidate_atom_polarity: "positive",
    candidate_atom_kind: "applies_when",
    candidate_text: "touches CLI behavior",
    candidate_content_hash: "hash_1",
    proposed_label_name: null,
    proposed_label_name_normalized: null,
    proposal_json: "{}",
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
    task_snapshot_json: "{}",
    suggest_input_hash: "input-hash",
    agent_candidates_json: "[]",
    suggestion_snapshot_json: "{}",
    final_decision_json: "{}",
    suggest_coverage: 0.6,
    suggest_coverage_cosine: 0.7,
    suggest_residual_norm: 0.4,
    suggest_needs_new_label: false,
    suggest_degraded: false,
    diagnostics_json: "[]",
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
    change_json: "{}",
    validation_status: "not_required",
    validation_json: "{}",
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
    result_json: null,
    metadata_json: "{}",
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    labels: [],
    ...overrides,
  }
}
