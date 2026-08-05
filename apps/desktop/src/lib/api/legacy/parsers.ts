import { ApiError, expectArray, expectRecord, expectFiniteNumber, expectExactKeys, expectString, expectBoolean, expectSafeInteger, expectNullableString, expectNullableInteger } from "../parsers"
import type { LabelOntologyActionRecord, LabelOntologyActionType, LabelOntologyObservationRecord, LabelOntologyReviewGroup, LabelOntologyReviewGroupBy, LabelOntologySignalDetail, LabelOntologySignalRecord, LabelRecord, LabelSuggestionEvidenceAtom, LabelSuggestionResult, SelectedLabelSuggestion, SignalObservationRecord, SignalRecord, Task } from "../types"
import { parseApiLabel, parseApiTask } from "../operations/task/parsers"

const SIGNAL_OBSERVATION_KEYS = ["id", "board_id", "task_id", "task_ref_snapshot", "run_id", "comment_id", "actor", "agent_type", "source", "evidence", "created_at"] as const
const SIGNAL_KEYS = ["id", "board_id", "observation_id", "kind", "title", "summary", "severity", "status", "dedupe_key", "superseded_by_signal_id", "reviewed_by", "reviewed_at", "review_reason", "created_at", "updated_at", "observation"] as const
export function parseSignalObservation(value: unknown, label: string): SignalObservationRecord {
 const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, SIGNAL_OBSERVATION_KEYS, label)
 for (const key of ["id", "board_id", "actor"] as const) expectString(record[key], `.`)
 record.evidence = expectRecord<Record<string, unknown>>(record.evidence, `${label}.evidence`)
 for (const key of ["task_id", "task_ref_snapshot", "run_id", "comment_id", "agent_type", "source"] as const) expectNullableString(record[key], `.`)
 expectSafeInteger(record.created_at, `.created_at`, true); return record as SignalObservationRecord
}
export function parseSignalRecord(value: unknown, label: string): SignalRecord {
 const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, SIGNAL_KEYS, label)
 for (const key of ["id", "board_id", "observation_id", "kind", "title", "summary", "severity"] as const) expectString(record[key], `.`)
 expectString(record.status, `${label}.status`)
 for (const key of ["dedupe_key", "superseded_by_signal_id", "reviewed_by", "review_reason"] as const) expectNullableString(record[key], `.`)
 expectNullableInteger(record.reviewed_at, `.reviewed_at`); expectSafeInteger(record.created_at, `.created_at`, true); expectSafeInteger(record.updated_at, `.updated_at`, true)
 record.observation = parseSignalObservation(record.observation, `.observation`); return record as SignalRecord
}
export function parseSignalListEnvelope(value: unknown): { data: SignalRecord[]; meta: { include_all: boolean; limit: number } } {
 const envelope = expectRecord<Record<string, unknown>>(value, "signals response"); expectExactKeys(envelope, ["data", "meta"], "signals response")
 const meta = expectRecord<Record<string, unknown>>(envelope.meta, "signals response meta"); expectExactKeys(meta, ["include_all", "limit"], "signals response meta")
 return { data: expectArray<unknown>(envelope.data, "signals response data").map((entry) => parseSignalRecord(entry, `signals response data[]`)), meta: { include_all: expectBoolean(meta.include_all, "signals response meta.include_all"), limit: expectSafeInteger(meta.limit, "signals response meta.limit", true) } }
}
export function parseSignalEnvelope(value: unknown): { data: SignalRecord } { const envelope = expectRecord<Record<string, unknown>>(value, "signal response"); expectExactKeys(envelope, ["data"], "signal response"); return { data: parseSignalRecord(envelope.data, "signal response data") } }

const ONTOLOGY_SIGNAL_KEYS = ["id", "observation_id", "board_id", "kind", "status", "target_label_id", "target_label_name_snapshot", "proposed_action", "candidate_atom_polarity", "candidate_atom_kind", "candidate_text", "candidate_content_hash", "proposed_label_name", "proposed_label_name_normalized", "agent_selected", "suggest_state", "suggest_score", "suggest_rank", "final_selected", "rationale", "confidence", "signal_key", "superseded_by_signal_id", "status_reason", "created_at", "updated_at", "reviewed_at", "closed_at", "related_labels", "proposal"] as const
const ONTOLOGY_OBSERVATION_KEYS = ["id", "board_id", "task_id", "task_ref_snapshot", "suggest_input_hash", "suggest_coverage", "suggest_coverage_cosine", "suggest_residual_norm", "suggest_needs_new_label", "suggest_degraded", "capture_fingerprint", "created_by", "created_by_type", "agent_type", "created_at", "signals", "task_snapshot", "agent_candidates", "suggestion_snapshot", "final_decision", "diagnostics"] as const
const ONTOLOGY_ACTION_KEYS = ["id", "board_id", "parent_action_id", "action_type", "reason", "target_label_id", "result_label_id", "result_atom_id", "result_atom_content_hash", "result_proposal_id", "canonical_before_hash", "canonical_after_hash", "validation_requirement", "validation_status", "validation_effective_outcome", "validation_latest_attempt_id", "created_by", "created_by_type", "agent_type", "created_at", "signal_ids", "change", "validation"] as const
const ONTOLOGY_REVIEW_GROUP_KEYS = ["group_by", "key", "label_id", "label_name", "candidate_atom_polarity", "candidate_atom_kind", "candidate_text", "candidate_content_hash", "proposed_label_name", "proposed_label_name_normalized", "cluster_key", "cluster_reason", "task_count", "signal_count", "open_count", "confirmed_count", "resolved_count", "rejected_count", "superseded_count", "degraded_count", "average_score", "median_score", "oldest_signal_at", "latest_signal_at", "sample_task_refs", "signal_ids", "action_count", "action_ids", "proposal_ids", "labels", "candidate_atom_variants"] as const
const ONTOLOGY_REVIEW_GROUP_BY = new Set<LabelOntologyReviewGroupBy>(["label", "candidate_atom", "proposed_label", "cluster"])
const ONTOLOGY_ACTION_TYPES = new Set<LabelOntologyActionType>(["confirm", "reject", "supersede", "resolve_no_change", "add_positive_atom", "add_negative_atom", "adopt_existing_atom", "update_semantics", "create_label_proposal", "bootstrap_label", "rename_label", "split_label", "merge_labels", "revert_ontology_mutation", "validate"])

export function parseLabelOntologySignal(value: unknown, label: string): LabelOntologySignalRecord {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ONTOLOGY_SIGNAL_KEYS, label)
  for (const key of ["id", "observation_id", "board_id", "rationale", "signal_key"] as const) expectString(record[key], `${label}.${key}`)
  for (const key of ["target_label_id", "target_label_name_snapshot", "candidate_atom_polarity", "candidate_atom_kind", "candidate_text", "candidate_content_hash", "proposed_label_name", "proposed_label_name_normalized", "suggest_state", "superseded_by_signal_id", "status_reason"] as const) expectNullableString(record[key], `${label}.${key}`)
  for (const key of ["kind", "status", "proposed_action"] as const) expectString(record[key], `${label}.${key}`)
  for (const key of ["agent_selected", "final_selected"] as const) expectBoolean(record[key], `${label}.${key}`)
  for (const key of ["suggest_score", "confidence"] as const) if (record[key] !== null) expectFiniteNumber(record[key], `${label}.${key}`)
  for (const key of ["suggest_rank", "reviewed_at", "closed_at"] as const) expectNullableInteger(record[key], `${label}.${key}`)
  for (const key of ["created_at", "updated_at"] as const) expectSafeInteger(record[key], `${label}.${key}`, true)
  record.related_labels = expectArray<unknown>(record.related_labels, `${label}.related_labels`)
  record.proposal = expectRecord<Record<string, unknown>>(record.proposal, `${label}.proposal`)
  return record as LabelOntologySignalRecord
}

export function parseLabelOntologyObservation(value: unknown, label: string): LabelOntologyObservationRecord {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ONTOLOGY_OBSERVATION_KEYS, label)
  for (const key of ["id", "board_id", "task_id", "task_ref_snapshot", "capture_fingerprint", "created_by", "created_by_type"] as const) expectString(record[key], `${label}.${key}`)
  for (const key of ["suggest_input_hash", "agent_type"] as const) expectNullableString(record[key], `${label}.${key}`)
  for (const key of ["suggest_coverage", "suggest_coverage_cosine", "suggest_residual_norm"] as const) if (record[key] !== null) expectFiniteNumber(record[key], `${label}.${key}`)
  for (const key of ["suggest_needs_new_label", "suggest_degraded"] as const) expectBoolean(record[key], `${label}.${key}`)
  expectSafeInteger(record.created_at, `${label}.created_at`, true)
  record.task_snapshot = expectRecord<Record<string, unknown>>(record.task_snapshot, `${label}.task_snapshot`)
  record.agent_candidates = expectArray<unknown>(record.agent_candidates, `${label}.agent_candidates`)
  record.suggestion_snapshot = expectRecord<Record<string, unknown>>(record.suggestion_snapshot, `${label}.suggestion_snapshot`)
  record.final_decision = expectRecord<Record<string, unknown>>(record.final_decision, `${label}.final_decision`)
  record.diagnostics = expectArray<unknown>(record.diagnostics, `${label}.diagnostics`)
  record.signals = expectArray<unknown>(record.signals, `${label}.signals`).map((entry, index) => parseLabelOntologySignal(entry, `${label}.signals[${index}]`))
  return record as LabelOntologyObservationRecord
}

export function parseLabelOntologyAction(value: unknown, label: string): LabelOntologyActionRecord {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ONTOLOGY_ACTION_KEYS, label)
  for (const key of ["id", "board_id", "reason", "created_by", "created_by_type"] as const) expectString(record[key], `${label}.${key}`)
  for (const key of ["parent_action_id", "target_label_id", "result_label_id", "result_atom_id", "result_atom_content_hash", "result_proposal_id", "canonical_before_hash", "canonical_after_hash", "validation_latest_attempt_id", "agent_type"] as const) expectNullableString(record[key], `${label}.${key}`)
  if (!ONTOLOGY_ACTION_TYPES.has(record.action_type as LabelOntologyActionType)) throw new ApiError("invalid_response", `${label}.action_type is unknown`)
  if (!["none", "required", "unsupported"].includes(record.validation_requirement as string)) throw new ApiError("invalid_response", `${label}.validation_requirement is unknown`)
  if (!["not_required", "pending", "passed", "failed", "partial"].includes(record.validation_status as string)) throw new ApiError("invalid_response", `${label}.validation_status is unknown`)
  if (!["not_required", "unsupported", "pending", "passed", "failed", "partial"].includes(record.validation_effective_outcome as string)) throw new ApiError("invalid_response", `${label}.validation_effective_outcome is unknown`)
  expectSafeInteger(record.created_at, `${label}.created_at`, true)
  record.change = expectRecord<Record<string, unknown>>(record.change, `${label}.change`)
  record.validation = expectRecord<Record<string, unknown>>(record.validation, `${label}.validation`)
  record.signal_ids = expectArray<unknown>(record.signal_ids, `${label}.signal_ids`).map((entry, index) => expectString(entry, `${label}.signal_ids[${index}]`))
  return record as LabelOntologyActionRecord
}

export function parseLabelOntologyReviewGroup(value: unknown, label: string): LabelOntologyReviewGroup {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ONTOLOGY_REVIEW_GROUP_KEYS, label)
  if (!ONTOLOGY_REVIEW_GROUP_BY.has(record.group_by as LabelOntologyReviewGroupBy)) throw new ApiError("invalid_response", `${label}.group_by is unknown`)
  expectString(record.key, `${label}.key`)
  for (const key of ["label_id", "label_name", "candidate_atom_polarity", "candidate_atom_kind", "candidate_text", "candidate_content_hash", "proposed_label_name", "proposed_label_name_normalized", "cluster_key", "cluster_reason"] as const) expectNullableString(record[key], `${label}.${key}`)
  for (const key of ["task_count", "signal_count", "open_count", "confirmed_count", "resolved_count", "rejected_count", "superseded_count", "degraded_count", "oldest_signal_at", "latest_signal_at", "action_count"] as const) expectSafeInteger(record[key], `${label}.${key}`, true)
  for (const key of ["average_score", "median_score"] as const) if (record[key] !== null) expectFiniteNumber(record[key], `${label}.${key}`)
  for (const key of ["sample_task_refs", "signal_ids", "action_ids", "proposal_ids"] as const) record[key] = expectArray<unknown>(record[key], `${label}.${key}`).map((entry, index) => expectString(entry, `${label}.${key}[${index}]`))
  record.labels = expectArray<unknown>(record.labels, `${label}.labels`).map((entry, index) => { const item = expectRecord<Record<string, unknown>>(entry, `${label}.labels[${index}]`); expectExactKeys(item, ["id", "name"], `${label}.labels[${index}]`); return { id: expectString(item.id, `${label}.labels[${index}].id`), name: expectNullableString(item.name, `${label}.labels[${index}].name`) } })
  record.candidate_atom_variants = expectArray<unknown>(record.candidate_atom_variants, `${label}.candidate_atom_variants`).map((entry, index) => { const item = expectRecord<Record<string, unknown>>(entry, `${label}.candidate_atom_variants[${index}]`); expectExactKeys(item, ["content_hash", "polarity", "kind", "text", "signal_count"], `${label}.candidate_atom_variants[${index}]`); return { content_hash: expectString(item.content_hash, `${label}.candidate_atom_variants[${index}].content_hash`), polarity: expectNullableString(item.polarity, `${label}.candidate_atom_variants[${index}].polarity`), kind: expectNullableString(item.kind, `${label}.candidate_atom_variants[${index}].kind`), text: expectNullableString(item.text, `${label}.candidate_atom_variants[${index}].text`), signal_count: expectSafeInteger(item.signal_count, `${label}.candidate_atom_variants[${index}].signal_count`, true) } })
  return record as LabelOntologyReviewGroup
}

export function parseLabelOntologyDetailEnvelope(value: unknown): { data: LabelOntologySignalDetail } {
  const envelope = expectRecord<Record<string, unknown>>(value, "label ontology signal response"); expectExactKeys(envelope, ["data"], "label ontology signal response")
  const data = expectRecord<Record<string, unknown>>(envelope.data, "label ontology signal response data"); expectExactKeys(data, ["signal", "observation", "actions"], "label ontology signal response data")
  return { data: { signal: parseLabelOntologySignal(data.signal, "label ontology signal response data.signal"), observation: parseLabelOntologyObservation(data.observation, "label ontology signal response data.observation"), actions: expectArray<unknown>(data.actions, "label ontology signal response data.actions").map((entry, index) => parseLabelOntologyAction(entry, `label ontology signal response data.actions[${index}]`)) } }
}

export function parseLabelOntologyActionEnvelope(value: unknown): { data: LabelOntologyActionRecord } {
  const envelope = expectRecord<Record<string, unknown>>(value, "label ontology action response"); expectExactKeys(envelope, ["data"], "label ontology action response")
  return { data: parseLabelOntologyAction(envelope.data, "label ontology action response data") }
}

const LABEL_EVIDENCE_KEYS = ["atom_id", "label_id", "label_name", "polarity", "kind", "text", "score"] as const
const LABEL_SUGGESTION_KEYS = ["label_id", "label_name", "score", "weight", "already_applied", "evidence_atoms", "negative_evidence_atoms"] as const
const LABEL_SUGGESTION_RESULT_KEYS = ["task_id", "board_id", "selected_labels", "candidates", "coverage", "coverage_cosine", "residual_norm", "needs_new_label", "reason_codes", "degraded", "diagnostics"] as const

export function parseLabelEvidence(value: unknown, label: string): LabelSuggestionEvidenceAtom {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, LABEL_EVIDENCE_KEYS, label)
  for (const key of ["atom_id", "label_id", "label_name", "polarity", "kind", "text"] as const) expectString(record[key], `.`)
  expectFiniteNumber(record.score, `.score`)
  return record as LabelSuggestionEvidenceAtom
}
export function parseSelectedLabel(value: unknown, label: string): SelectedLabelSuggestion {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, LABEL_SUGGESTION_KEYS, label)
  expectString(record.label_id, `.label_id`); expectString(record.label_name, `.label_name`)
  expectFiniteNumber(record.score, `.score`); expectFiniteNumber(record.weight, `.weight`); expectBoolean(record.already_applied, `.already_applied`)
  record.evidence_atoms = expectArray<unknown>(record.evidence_atoms, `.evidence_atoms`).map((entry, index) => parseLabelEvidence(entry, `.evidence_atoms[]`))
  record.negative_evidence_atoms = expectArray<unknown>(record.negative_evidence_atoms, `.negative_evidence_atoms`).map((entry, index) => parseLabelEvidence(entry, `.negative_evidence_atoms[]`))
  return record as SelectedLabelSuggestion
}
export function parseLabelSuggestionEnvelope(value: unknown): { data: LabelSuggestionResult } {
  const envelope = expectRecord<Record<string, unknown>>(value, "label suggestions response"); expectExactKeys(envelope, ["data"], "label suggestions response")
  const record = expectRecord<Record<string, unknown>>(envelope.data, "label suggestions response data"); expectExactKeys(record, LABEL_SUGGESTION_RESULT_KEYS, "label suggestions response data")
  expectString(record.task_id, "label suggestions response data.task_id"); expectString(record.board_id, "label suggestions response data.board_id")
  record.selected_labels = expectArray<unknown>(record.selected_labels, "label suggestions response data.selected_labels").map((entry, index) => parseSelectedLabel(entry, `label suggestions response data.selected_labels[]`))
  record.candidates = expectArray<unknown>(record.candidates, "label suggestions response data.candidates").map((entry, index) => parseSelectedLabel(entry, `label suggestions response data.candidates[]`))
  for (const key of ["coverage", "coverage_cosine", "residual_norm"] as const) expectFiniteNumber(record[key], `label suggestions response data.`)
  expectBoolean(record.needs_new_label, "label suggestions response data.needs_new_label"); expectBoolean(record.degraded, "label suggestions response data.degraded")
  for (const key of ["reason_codes", "diagnostics"] as const) expectArray<unknown>(record[key], `label suggestions response data.`).forEach((entry, index) => expectString(entry, `label suggestions response data.[]`))
  return { data: record as LabelSuggestionResult }
}

export function parseAddTaskLabelEnvelope(value: unknown): { data: Task; meta?: { created_labels: LabelRecord[] } } {
  const envelope = expectRecord<Record<string, unknown>>(value, "add task label response")
  const hasMeta = Object.prototype.hasOwnProperty.call(envelope, "meta")
  expectExactKeys(envelope, hasMeta ? ["data", "meta"] : ["data"], "add task label response")
  const result: { data: Task; meta?: { created_labels: LabelRecord[] } } = { data: parseApiTask(envelope.data, "add task label response data") }
  if (hasMeta) {
    const meta = expectRecord<Record<string, unknown>>(envelope.meta, "add task label response meta"); expectExactKeys(meta, ["created_labels"], "add task label response meta")
    result.meta = { created_labels: expectArray<unknown>(meta.created_labels, "add task label response meta.created_labels").map((entry, index) => parseApiLabel(entry, `add task label response meta.created_labels[${index}]`)) }
  }
  return result
}
export function parseRemoveTaskLabelEnvelope(value: unknown): { data: Task } {
  const envelope = expectRecord<Record<string, unknown>>(value, "remove task label response"); expectExactKeys(envelope, ["data"], "remove task label response")
  return { data: parseApiTask(envelope.data, "remove task label response data") }
}
