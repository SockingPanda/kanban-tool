export type TaskStatus =
  | "triage"
  | "todo"
  | "scheduled"
  | "ready"
  | "running"
  | "blocked"
  | "review"
  | "done"
  | "archived"

export type RuntimeConfig = {
  apiBaseUrl: string
  actor: string
  board: string
}

export type Board = {
  id: string
  slug: string
  name: string
  description: string | null
  created_at: number
  updated_at: number
  archived_at: number | null
}

export type CreateBoardInput = {
  slug: string
  name: string
  description?: string | null
}

export type CreateTaskInput = {
  title: string
  description?: string
  status?: TaskStatus
  taskId?: string
  idempotencyKey?: string
}

export type Task = {
  id: string
  board_id: string
  board_slug: string
  ref: string
  seq: number
  title: string
  description: string | null
  status: TaskStatus
  status_reason: string | null
  assignee: string | null
  priority: number
  position: number
  scheduled_at: number | null
  due_at: number | null
  created_by: string
  created_at: number
  updated_at: number
  started_at: number | null
  completed_at: number | null
  archived_at: number | null
  claim_owner: string | null
  claim_expires_at: number | null
  last_heartbeat_at: number | null
  current_run_id: string | null
  retry_count: number
  max_retries: number | null
  result_summary: string | null
  result: unknown | null
  metadata: unknown
  lock_version: number
  dependency_blocked: boolean
  unfinished_parent_count: number
  execution_plan_state: StepPlanState
  required_step_count: number
  completed_required_step_count: number
  optional_step_count: number
  labels: LabelRecord[]
}

export type LabelRecord = {
  id: string
  board_id: string
  name: string
  color: string | null
  created_at: number
  updated_at: number
}

/** Canonical attachment metadata; bytes are fetched only through the download operation. */
export type Attachment = {
  id: string
  board_id: string
  task_id: string
  filename: string
  rel_path: string
  content_type: string | null
  size_bytes: number
  sha256: string | null
  created_by: string
  created_at: number
}

export type CreateAttachmentInput = {
  id?: string
  filename: string
  content: number[]
  content_type?: string | null
  rel_path?: string | null
  sha256?: string | null
  actor?: string | null
}

export type DownloadedAttachment = {
  content_type: string | null
  attachment_id: string | null
  sha256: string | null
  content: Uint8Array
}

export type LabelSuggestionEvidenceAtom = {
  atom_id: string
  label_id: string
  label_name: string
  polarity: string
  kind: string
  text: string
  score: number
}

export type SelectedLabelSuggestion = {
  label_id: string
  label_name: string
  score: number
  weight: number
  already_applied: boolean
  evidence_atoms: LabelSuggestionEvidenceAtom[]
  negative_evidence_atoms: LabelSuggestionEvidenceAtom[]
}

export type LabelSuggestionCandidate = SelectedLabelSuggestion

export type LabelSuggestionResult = {
  task_id: string
  board_id: string
  selected_labels: SelectedLabelSuggestion[]
  candidates: LabelSuggestionCandidate[]
  coverage: number
  coverage_cosine: number
  residual_norm: number
  needs_new_label: boolean
  reason_codes: string[]
  degraded: boolean
  diagnostics: string[]
}

export type LabelOntologySignalKind = string

export type LabelOntologySignalStatus = string

export type SignalStatus = string

export type SignalObservationRecord = {
  id: string
  board_id: string
  task_id: string | null
  task_ref_snapshot: string | null
  run_id: string | null
  comment_id: string | null
  actor: string
  agent_type: string | null
  source: string | null
  evidence: Record<string, unknown>
  created_at: number
}

export type SignalRecord = {
  id: string
  board_id: string
  observation_id: string
  kind: string
  title: string
  summary: string
  severity: string
  status: SignalStatus
  dedupe_key: string | null
  superseded_by_signal_id: string | null
  reviewed_by: string | null
  reviewed_at: number | null
  review_reason: string | null
  created_at: number
  updated_at: number
  observation: SignalObservationRecord
}

export type LabelOntologyProposedAction = string

export type LabelOntologyActionType =
  | "confirm"
  | "reject"
  | "supersede"
  | "resolve_no_change"
  | "add_positive_atom"
  | "add_negative_atom"
  | "adopt_existing_atom"
  | "update_semantics"
  | "create_label_proposal"
  | "bootstrap_label"
  | "rename_label"
  | "split_label"
  | "merge_labels"
  | "revert_ontology_mutation"
  | "validate"

export type LabelOntologyValidationStatus = "not_required" | "pending" | "passed" | "failed" | "partial"
export type LabelOntologyValidationRequirement = "none" | "required" | "unsupported"
export type LabelOntologyValidationEffectiveOutcome =
  | "not_required"
  | "unsupported"
  | "pending"
  | "passed"
  | "failed"
  | "partial"

export type LabelOntologySuggestState = string

export type LabelOntologyReviewGroupBy = "label" | "candidate_atom" | "proposed_label" | "cluster"

export type LabelAtomRecord = {
  id: string
  label_id: string
  board_id: string
  label_name: string
  polarity: string
  kind: string
  text: string
  ordinal: number
  content_hash: string
  created_at: number
  updated_at: number
}

export type LabelOntologyObservationRecord = {
  id: string
  board_id: string
  task_id: string
  task_ref_snapshot: string
  task_snapshot: Record<string, unknown>
  suggest_input_hash: string | null
  agent_candidates: unknown[]
  suggestion_snapshot: Record<string, unknown>
  final_decision: Record<string, unknown>
  suggest_coverage: number | null
  suggest_coverage_cosine: number | null
  suggest_residual_norm: number | null
  suggest_needs_new_label: boolean
  suggest_degraded: boolean
  diagnostics: unknown[]
  capture_fingerprint: string
  created_by: string
  created_by_type: string
  agent_type: string | null
  created_at: number
  signals: LabelOntologySignalRecord[]
}

export type LabelOntologySignalRecord = {
  id: string
  observation_id: string
  board_id: string
  kind: LabelOntologySignalKind
  status: LabelOntologySignalStatus
  target_label_id: string | null
  target_label_name_snapshot: string | null
  related_labels: unknown[]
  proposed_action: LabelOntologyProposedAction
  candidate_atom_polarity: string | null
  candidate_atom_kind: string | null
  candidate_text: string | null
  candidate_content_hash: string | null
  proposed_label_name: string | null
  proposed_label_name_normalized: string | null
  proposal: Record<string, unknown>
  agent_selected: boolean
  suggest_state: LabelOntologySuggestState | null
  suggest_score: number | null
  suggest_rank: number | null
  final_selected: boolean
  rationale: string
  confidence: number | null
  signal_key: string
  superseded_by_signal_id: string | null
  status_reason: string | null
  created_at: number
  updated_at: number
  reviewed_at: number | null
  closed_at: number | null
}

export type LabelOntologyActionRecord = {
  id: string
  board_id: string
  parent_action_id: string | null
  action_type: LabelOntologyActionType
  reason: string
  target_label_id: string | null
  result_label_id: string | null
  result_atom_id: string | null
  result_atom_content_hash: string | null
  result_proposal_id: string | null
  canonical_before_hash: string | null
  canonical_after_hash: string | null
  change: Record<string, unknown>
  validation_requirement: LabelOntologyValidationRequirement
  validation_status: LabelOntologyValidationStatus
  validation_effective_outcome: LabelOntologyValidationEffectiveOutcome
  validation_latest_attempt_id: string | null
  validation: Record<string, unknown>
  created_by: string
  created_by_type: string
  agent_type: string | null
  created_at: number
  signal_ids: string[]
}

export type LabelOntologySignalDetail = {
  signal: LabelOntologySignalRecord
  observation: LabelOntologyObservationRecord
  actions: LabelOntologyActionRecord[]
}

export type LabelOntologyReviewLabelRef = {
  id: string
  name: string | null
}

export type LabelOntologyReviewAtomVariant = {
  content_hash: string
  polarity: string | null
  kind: string | null
  text: string | null
  signal_count: number
}

export type LabelOntologyReviewGroup = {
  group_by: LabelOntologyReviewGroupBy
  key: string
  label_id: string | null
  label_name: string | null
  candidate_atom_polarity: string | null
  candidate_atom_kind: string | null
  candidate_text: string | null
  candidate_content_hash: string | null
  proposed_label_name: string | null
  proposed_label_name_normalized: string | null
  cluster_key: string | null
  cluster_reason: string | null
  task_count: number
  signal_count: number
  open_count: number
  confirmed_count: number
  resolved_count: number
  rejected_count: number
  superseded_count: number
  degraded_count: number
  average_score: number | null
  median_score: number | null
  oldest_signal_at: number
  latest_signal_at: number
  sample_task_refs: string[]
  signal_ids: string[]
  action_count: number
  action_ids: string[]
  proposal_ids: string[]
  labels: LabelOntologyReviewLabelRef[]
  candidate_atom_variants: LabelOntologyReviewAtomVariant[]
}

export type LabelAtomExplainAction = {
  action: LabelOntologyActionRecord
  matched_by: string
}

export type LabelAtomExplainSignal = {
  signal: LabelOntologySignalRecord
  observation: LabelOntologyObservationRecord
  source_task: Task
  task_ref_snapshot: string
  suggest_input_stale: boolean
  suggest_degraded: boolean
  warnings: string[]
}

export type LabelAtomExplainValidation = {
  action: LabelOntologyActionRecord
  parent_action_id: string
  validation_status: LabelOntologyValidationStatus
  manual: unknown
  summary: unknown
  cases: unknown
  warnings: string[]
}

export type LabelAtomExplainRecord = {
  query: string
  atom: LabelAtomRecord | null
  current_semantics: unknown | null
  provenance_actions: LabelAtomExplainAction[]
  supporting_signals: LabelAtomExplainSignal[]
  validation_history: LabelAtomExplainValidation[]
  legacy_untracked: boolean
  legacy_reason: string | null
}

export type Run = {
  id: string
  task_id: string
  status: "running" | "succeeded" | "failed" | "canceled" | "expired"
  worker_profile: string | null
  worker_pid: number | null
  claim_owner: string
  started_at: number
  finished_at: number | null
  exit_code: number | null
  summary: string | null
  error: string | null
  has_log: boolean
  metadata: unknown
}

export type EventRecord = {
  id: number
  event_id: string
  board_id: string
  task_id: string | null
  run_id: string | null
  kind: string
  actor: string | null
  payload: unknown
  created_at: number
}

export type BoardColumn = {
  id: string
  board_id: string
  status: TaskStatus
  title: string
  position: number
  hidden: boolean
  wip_limit: number | null
  created_at: number
  updated_at: number
}

export type CommentRecord = {
  id: string
  board_id: string
  task_id: string
  author: string
  author_type: string
  agent_type: string | null
  body: string
  kind: "note" | "decision" | "signal"
  metadata: Record<string, unknown>
  created_at: number
}

export type RunLog = {
  run_id: string
  content: string
  truncated: boolean
}

export type HealthStatus = {
  ok: boolean
  db: string
  version: string
  db_path?: string
  db_fingerprint?: string
}

export type BoardStats = {
  board_id: string
  generated_at: number
  status_counts: StatusCount[]
  stale_claims: StaleClaim[]
  blocked_reasons: BlockedReason[]
}

export type StatusCount = {
  status: string
  count: number
}

export type StaleClaim = {
  task_id: string
  seq: number
  title: string
  claim_owner: string | null
  claim_expires_at: number | null
  last_heartbeat_at: number | null
  current_run_id: string | null
  retry_count: number
  max_retries: number | null
}

export type BlockedReason = {
  reason: string
  count: number
}

export type BackupReport = {
  out_path: string
  checksum_sha256: string
  bytes: number
  source_fingerprint: string
}

export type ExportReport = {
  out_path: string
  checksum_sha256: string
  bytes: number
  record_count: number
  source_fingerprint: string
}

export type ImportReport = {
  in_path: string
  source_fingerprint: string
  imported_records: number
  skipped_records: number
  rebuild_jobs_enqueued: number
  journal_id: string
}

export type VacuumReport = {
  ok: boolean
  before_bytes: number
  after_bytes: number
  source_fingerprint: string
}

export type MaintenanceOwnerStatus = {
  owner: string | null
  mode: string | null
  lease_expires_at: number | null
  fence_epoch: number
  build_identity: string | null
  last_heartbeat_at: number | null
  active: boolean
}

export type ProjectionStoreStatus = {
  store_name: string
  active_generation: string | null
  active_fingerprint: string | null
  previous_generation: string | null
  building_generation: string | null
  lifecycle_status: string
  fence_epoch: number
  last_event_id: number
  dirty: boolean
  pending: number
  running: number
  failed: number
  last_error: string | null
  updated_at: number
}

export type MaintenanceStatusReport = {
  database_instance_id: string
  protocol_version: number
  owner: MaintenanceOwnerStatus
  stores: ProjectionStoreStatus[]
}

export type MaintenanceRunReport = {
  database_instance_id: string
  protocol_version: number
  owner: string
  mode: string
  action: string
  processed: number
  stores: ProjectionStoreStatus[]
}

export type LegacyImportTableCount = {
  table: string
  source_rows: number
  target_rows: number
}

export type LegacyImportReport = {
  journal_id: string
  phase: string
  source_path: string
  source_fingerprint: string
  schema_fingerprint: string
  resumed: boolean
  attachment_count: number
  table_counts: LegacyImportTableCount[]
}

export type DoctorDerivedStore = {
  store_name: string
  schema_version: number
  last_event_id: number
  dirty: boolean
  last_error: string | null
  pending_outbox: number
  running_outbox: number
  failed_outbox: number
}

export type DoctorIssue = {
  severity: string
  code: string
  message: string
  record_ids: string[]
}

export type DoctorReport = {
  ok: boolean
  integrity_check: string
  migration_version: number | null
  user_version: number
  expired_running_tasks: number
  running_tasks_without_active_run: number
  orphan_running_runs: number
  dependency_cycles: number
  archived_dependency_edges: number
  missing_run_logs: number
  suspicious_run_log_paths: number
  executable_dependency_violations: number
  executable_spec_violations: number
  executable_schedule_violations: number
  outbox_pending: number
  outbox_running: number
  unplanned_active_tasks: number
  active_parents_with_incomplete_required_steps: number
  outbox_failed: number
  derived_dirty_stores: number
  derived_error_stores: number
  derived_stores: DoctorDerivedStore[]
  consistency_errors: number
  consistency_warnings: number
  consistency_issues: DoctorIssue[]
  ontology_ledger_errors: number
  ontology_ledger_warnings: number
  ontology_ledger_issues: DoctorIssue[]
}

export type CheckpointReport = {
  busy: number
  log_frames: number
  checkpointed_frames: number
}
export type DependencyTask = {
  id: string
  board_id: string
  board_slug: string
  ref: string
  title: string
  status: TaskStatus
}

export type DependencyEdge = {
  parent: DependencyTask
  child: DependencyTask
}

export type Dependencies = {
  task?: DependencyTask
  parents: Task[]
  children: Task[]
  edges?: DependencyEdge[]
}

export type StepPlanState = "unplanned" | "planned" | "not_required"

export type StepStatus = "todo" | "done" | "skipped"

export type TaskStep = {
  id: string
  parent_task_id: string
  title: string
  body: string | null
  linked_task: Task | null
  position: number
  required: boolean
  status: StepStatus
  resolution_note: string | null
  resolved_by: string | null
  resolved_at: number | null
  created_by: string
  created_at: number
  updated_by: string
  updated_at: number
}

export type TaskExecutionPlan = {
  board_id: string
  task_id: string
  state: StepPlanState
  reason: string | null
  updated_by: string
  updated_at: number
}

export type TaskSteps = {
  task_id: string
  steps: TaskStep[]
  execution_plan: TaskExecutionPlan
}

export type TaskGraphNodeRole =
  | "center"
  | "dependency_parent"
  | "dependency_child"
  | "step_parent"
  | "step_child"
  | "active"
  | "context"

export type TaskGraphEdgeKind = "dependency" | "step"

export type TaskGraphNode = {
  task: Task
  role: TaskGraphNodeRole
  context_only: boolean
}

export type TaskGraphEdge = {
  id: string
  source_task_id: string
  target_task_id: string
  kind: TaskGraphEdgeKind
  required: boolean
  blocking: boolean
}

export type TaskGraphMeta = {
  depth?: number
  context_depth?: number
  generated_at: number
  truncated: boolean
  node_count: number
  edge_count: number
  active_statuses?: TaskStatus[]
  include_done_context?: boolean
  include_archived_context?: boolean
  active_only?: boolean
}

export type TaskNeighborhood = {
  center_task_id: string
  nodes: TaskGraphNode[]
  edges: TaskGraphEdge[]
  meta: TaskGraphMeta
}

export type BoardTaskMap = {
  nodes: TaskGraphNode[]
  edges: TaskGraphEdge[]
  meta: TaskGraphMeta
}

export type CreateStepInput = {
  idempotency_key?: string
  title: string
  body?: string | null
  linked_task_ref?: string | null
  position?: number
  required?: boolean
}

export type UpdateStepInput = {
  title?: string
  body?: string | null
  linked_task_ref?: string | null
  unlink_task?: boolean
  position?: number
  required?: boolean
}

export type ClaimResponse = {
  task: Task
  run: Run
  claim_token: string
  claim_expires_at: number | null
}

export type SearchTasksMeta = {
  backend: string
  stale: boolean
  index_version: string | null
  last_event_id: number | null
  index_lag_events: number | null
}

export type SearchIndexStatus = SearchTasksMeta & {
  derived_index: boolean
  message: string
}

export type ApiEnvelope<T, M = Record<string, unknown>> = { data: T; meta?: M }

export type PageMeta = {
  limit: number
  offset: number
  total: number | null
}

export type TaskPageResult = {
  tasks: Task[]
  page: PageMeta
}

export type TaskStatusWindow = {
  status: TaskStatus
  tasks: Task[]
  page: PageMeta
}

export type TaskStatusWindowsResult = {
  statuses: TaskStatusWindow[]
}

export type TaskPlanFilter = "plan_needed" | "has_steps" | "incomplete_required_steps"

export type TaskListSort =
  | "seq"
  | "-seq"
  | "title"
  | "-title"
  | "status"
  | "-status"
  | "priority"
  | "-priority"
  | "assignee"
  | "-assignee"
  | "scheduled_at"
  | "-scheduled_at"
  | "due_at"
  | "-due_at"
  | "created_at"
  | "-created_at"
  | "updated_at"
  | "-updated_at"

export type SearchTasksResult = {
  tasks: Task[]
  searchMeta: SearchTasksMeta
  page: PageMeta
}

export type SearchTaskStatusWindow = TaskStatusWindow & {
  searchMeta: SearchTasksMeta
}

export type SearchTaskStatusWindowsResult = {
  statuses: SearchTaskStatusWindow[]
}

export type EventMeta = {
  next_after?: number
}

export type EventPage = {
  events: EventRecord[]
  meta: EventMeta
}

export type ErrorBody = { code: string; message: string; details?: unknown }
export type ErrorEnvelope = { error: ErrorBody }

export type SearchTaskHit = {
  task_id: string
  seq: number
  score: number
  snippet: string | null
  task: Task
}

export type SearchTasksResponse = {
  hits: SearchTaskHit[]
  meta: SearchTasksMeta
}

export type RequiredOffsetPageMeta = { limit: number; offset: number }
export type RequiredTotalPageMeta = RequiredOffsetPageMeta & { total: number }

export type TaskStatusWindowResponse = {
  status: TaskStatus
  tasks: Task[]
  page: RequiredTotalPageMeta
 }

export type TaskStatusWindowsResponse = {
  statuses: TaskStatusWindowResponse[]
 }

export type SearchTaskStatusWindowResponse = {
  status: TaskStatus
  tasks: Task[]
  search_meta: SearchTasksMeta
  page?: PageEnvelopeMeta
}

export type SearchTaskStatusWindowsResponse = {
  statuses: SearchTaskStatusWindowResponse[]
}

export type RequestOptions = {
  method?: string
  body?: unknown
  idempotencyKey?: string
  actorHeader?: boolean
  signal?: AbortSignal
}

export type TaskListOptions = {
  includeArchived?: boolean
  statuses?: TaskStatus[]
  priorities?: number[]
  labels?: string[]
  planFilters?: TaskPlanFilter[]
  query?: string
  sort?: TaskListSort
  limit?: number
  offset?: number
  signal?: AbortSignal
}

export type BoardListOptions = {
  includeArchived?: boolean
  signal?: AbortSignal
}

export type SearchTaskOptions = TaskListOptions & {
  query: string
}

export type SignalListOptions = {
  statuses?: SignalStatus[]
  kinds?: string[]
  task?: string
  includeAll?: boolean
  limit?: number
  signal?: AbortSignal
}

export type LabelOntologySignalListOptions = {
  statuses?: LabelOntologySignalStatus[]
  kinds?: LabelOntologySignalKind[]
  task?: string
  label?: string
  proposedLabel?: string
  includeAll?: boolean
  limit?: number
  signal?: AbortSignal
}

export type LabelOntologyReviewOptions = {
  groupBy?: LabelOntologyReviewGroupBy
  includeAll?: boolean
  limit?: number
  signal?: AbortSignal
}

export type LabelOntologyActionCreateInput = {
  actionType: Extract<LabelOntologyActionType, "confirm" | "reject" | "supersede" | "resolve_no_change">
  signalIds: string[]
  reason: string
  supersededBySignalId?: string | null
}

export type PageEnvelopeMeta = Partial<PageMeta>
