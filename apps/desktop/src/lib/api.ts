import { invoke } from "@tauri-apps/api/core"
import { getCurrentDesktopLocale, type DesktopLocale } from "@/i18n"

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

const WEB_DEV_API_BASE_URL = "/__kb_api__"
const WEB_DEV_DEFAULT_ACTOR = "desktop-dev"
const WEB_DEV_DEFAULT_BOARD = "kanban-tool"

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
  payload: Record<string, unknown>
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

export type Dependencies = {
  parents: Task[]
  children: Task[]
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

type ErrorBody = { code: string; message: string; details?: unknown }
type ErrorEnvelope = { error: ErrorBody }

type SearchTaskHit = {
  task_id: string
  seq: number
  score: number
  snippet: string | null
  task: Task
}

type SearchTasksResponse = {
  hits: SearchTaskHit[]
  meta: SearchTasksMeta
}

type RequiredOffsetPageMeta = { limit: number; offset: number }
type RequiredTotalPageMeta = RequiredOffsetPageMeta & { total: number }

type TaskStatusWindowResponse = {
  status: TaskStatus
  tasks: Task[]
  page: RequiredTotalPageMeta
 }

type TaskStatusWindowsResponse = {
  statuses: TaskStatusWindowResponse[]
 }

type SearchTaskStatusWindowResponse = {
  status: TaskStatus
  tasks: Task[]
  search_meta: SearchTasksMeta
  page?: PageEnvelopeMeta
}

type SearchTaskStatusWindowsResponse = {
  statuses: SearchTaskStatusWindowResponse[]
}

export class ApiError extends Error {
  constructor(
    public code: string,
    message: string,
    public details?: unknown,
  ) {
    super(message)
  }
}

export async function loadRuntimeConfig(): Promise<RuntimeConfig> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return invoke<RuntimeConfig>("runtime_config")
  }
  const configuredApiBaseUrl = normalizeApiBaseUrl(import.meta.env.VITE_KB_API_BASE_URL)
  const usingWebDevDefault = !configuredApiBaseUrl && import.meta.env.DEV
  const apiBaseUrl = configuredApiBaseUrl || (usingWebDevDefault ? WEB_DEV_API_BASE_URL : "")
  if (!apiBaseUrl) {
    throw new Error(
      "VITE_KB_API_BASE_URL is required outside Tauri; set it to an explicit API origin or an explicit Vite proxy base such as /__kb_api__.",
    )
  }
  return {
    apiBaseUrl,
    actor: import.meta.env.VITE_KB_ACTOR ?? WEB_DEV_DEFAULT_ACTOR,
    board: import.meta.env.VITE_KB_BOARD ?? (usingWebDevDefault ? WEB_DEV_DEFAULT_BOARD : "default"),
  }
}

function normalizeApiBaseUrl(value: string | undefined) {
  const trimmed = value?.trim()
  if (!trimmed) return ""
  return trimmed.length > 1 ? trimmed.replace(/\/+$/, "") : trimmed
}

export class KanbanApi {
  constructor(
    private config: RuntimeConfig,
    private options: { locale?: DesktopLocale } = {},
  ) {}

  get actor() {
    return this.config.actor
  }

  get board() {
    return this.config.board
  }

  async health(options: RequestOptions = {}) {
    return this.request<HealthStatus>("/health", options)
  }

  async listBoards(options: BoardListOptions = {}) {
    const params = new URLSearchParams({
      include_archived: String(options.includeArchived ?? false),
    })
    const envelope = parseListBoardsEnvelope(await this.requestRaw(`/api/v1/boards?${params.toString()}`, {
      signal: options.signal,
    }))
    return envelope.data
  }

  async createBoard(input: CreateBoardInput, options: RequestOptions = {}) {
    return parseCreateBoardEnvelope(await this.requestRaw("/api/v1/boards", {
      method: "POST",
      body: {
        slug: input.slug,
        name: input.name,
        description: input.description ?? null,
        actor: this.actor,
      },
      actorHeader: true,
      signal: options.signal,
    })).data
  }

  async getBoard(board: string, options: RequestOptions = {}) {
    return parseGetBoardEnvelope(await this.requestRaw(
      `/api/v1/boards/${encodeURIComponent(board)}`,
      { signal: options.signal },
    )).data
  }

  async archiveBoard(board: string, options: RequestOptions = {}) {
    return parseArchiveBoardEnvelope(await this.requestRaw(
      `/api/v1/boards/${encodeURIComponent(board)}/archive`,
      {
        method: "POST",
        body: { actor: this.actor },
        actorHeader: true,
        signal: options.signal,
      },
    )).data
  }

  async stats(options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: this.board })
    return this.request<BoardStats>(`/api/v1/stats?${params.toString()}`, options)
  }

  async searchStatus(options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: this.board })
    return this.request<SearchIndexStatus>(`/api/v1/search/status?${params.toString()}`, options)
  }

  async doctor(options: RequestOptions = {}) {
    const envelope = await this.requestEnvelope<unknown>("/api/v1/maintenance/doctor", {
      method: "POST",
      signal: options.signal,
    })
    expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "doctor response")
    return parseDoctorReport(envelope.data)
  }

  async checkpoint(options: RequestOptions = {}) {
    const envelope = await this.requestEnvelope<unknown>("/api/v1/maintenance/checkpoint", {
      method: "POST",
      signal: options.signal,
    })
    expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "checkpoint response")
    return parseCheckpointReport(envelope.data)
  }

  async listBoardColumns(options: RequestOptions = {}) {
    return this.request<BoardColumn[]>(`/api/v1/boards/${this.board}/columns`, options)
  }

  async listTasks(options: TaskListOptions = {}) {
    const params = new URLSearchParams()
    const limit = options.limit ?? 100
    const offset = options.offset ?? 0
    params.set("include_archived", String(options.includeArchived ?? false))
    params.set("limit", String(limit))
    params.set("offset", String(offset))
    params.set("sort", options.sort ?? "-updated_at")
    if (options.query?.trim()) params.set("q", options.query.trim())
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const priority of options.priorities ?? []) params.append("priority", String(priority))
    for (const label of options.labels ?? []) {
      if (label.trim()) params.append("label", label.trim())
    }
    for (const filter of options.planFilters ?? []) params.append("plan_filter", filter)
    const envelope = parseTaskListEnvelope(await this.requestRaw(`/api/v1/boards/${this.board}/tasks?${params.toString()}`, { signal: options.signal }))
    return { tasks: envelope.data, page: envelope.meta } satisfies TaskPageResult
  }

  async listTasksByStatus(options: TaskListOptions & { statuses: TaskStatus[] }) {
    const params = this.taskListParams(options)
    const envelope = parseTaskStatusEnvelope(await this.requestRaw(`/api/v1/boards/${this.board}/tasks/by-status?${params.toString()}`, { signal: options.signal }))
    const data = envelope.data
    return {
      statuses: expectArray<TaskStatusWindowResponse>(data.statuses, "task status windows").map((entry) => ({
        status: entry.status,
        tasks: expectArray<Task>(entry.tasks, "task status window tasks"),
        page: expectRequiredTotalPageMeta(entry.page, "task status window page"),
      })),
    } satisfies TaskStatusWindowsResult
  }

  async searchTasks(options: SearchTaskOptions) {
    const params = new URLSearchParams()
    const limit = options.limit ?? 100
    const offset = options.offset ?? 0
    params.set("board", this.board)
    params.set("q", options.query.trim())
    params.set("include_archived", String(options.includeArchived ?? false))
    params.set("limit", String(limit))
    params.set("offset", String(offset))
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const label of options.labels ?? []) {
      if (label.trim()) params.append("label", label.trim())
    }
    const envelope = await this.requestEnvelope<SearchTasksResponse, PageEnvelopeMeta>(
      `/api/v1/search/tasks?${params.toString()}`,
      { signal: options.signal },
    )
    const search = expectRecord<SearchTasksResponse>(envelope.data, "search response data")
    const hits = expectArray<SearchTaskHit>(search.hits, "search hits")
    return {
      tasks: hits.map((hit) => hit.task),
      searchMeta: search.meta,
      page: normalizePageMeta(envelope.meta, { limit, offset }),
    } satisfies SearchTasksResult
  }

  async searchTasksByStatus(options: SearchTaskOptions & { statuses: TaskStatus[] }) {
    const params = this.searchTaskParams(options)
    const envelope = await this.requestEnvelope<SearchTaskStatusWindowsResponse, PageEnvelopeMeta>(
      `/api/v1/search/tasks/by-status?${params.toString()}`,
      { signal: options.signal },
    )
    const data = expectRecord<SearchTaskStatusWindowsResponse>(envelope.data, "search status windows response data")
    return {
      statuses: expectArray<SearchTaskStatusWindowResponse>(data.statuses, "search status windows").map((entry) => ({
        status: entry.status,
        tasks: expectArray<Task>(entry.tasks, "search status window tasks"),
        searchMeta: entry.search_meta,
        page: normalizePageMeta(entry.page, { limit: options.limit ?? 100, offset: options.offset ?? 0 }),
      })),
    } satisfies SearchTaskStatusWindowsResult
  }

  async createTask(input: CreateTaskInput, options: RequestOptions = {}) {
    const taskId = input.taskId ?? newClientTaskId()
    const envelope = await this.requestEnvelope<unknown>(`/api/v1/boards/${this.board}/tasks`, {
      method: "POST",
      body: {
        task_id: taskId,
        idempotency_key: input.idempotencyKey ?? `task.create:${taskId}`,
        title: input.title,
        description: input.description ?? null,
        status: input.status ?? undefined,
        actor: this.actor,
      },
      signal: options.signal,
    })
    const record = expectRecord<Record<string, unknown>>(envelope, "create task response")
    expectExactKeys(record, ["data"], "create task response")
    return parseApiTask(record.data, "create task response data")
  }

  async updateTask(taskId: string, patch: Partial<Pick<Task, "title" | "description" | "assignee" | "priority" | "due_at" | "scheduled_at">>, options: RequestOptions = {}) {
    return this.request<Task>(`/api/v1/tasks/${taskId}`, {
      method: "PATCH",
      body: { ...patch, actor: this.actor },
      signal: options.signal,
    })
  }

  async getTask(taskId: string, options: RequestOptions = {}) {
    return this.request<Task>(`/api/v1/tasks/${taskId}`, options)
  }

  async listDependencies(taskId: string, options: RequestOptions = {}) {
    return this.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies`, options)
  }

  async addDependency(taskId: string, parentTaskId: string, options: RequestOptions = {}) {
    return this.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies`, {
      method: "POST",
      body: { parent_task_id: parentTaskId, actor: this.actor },
      signal: options.signal,
    })
  }

  async removeDependency(taskId: string, parentTaskId: string, options: RequestOptions = {}) {
    return this.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies/${parentTaskId}`, {
      method: "DELETE",
      signal: options.signal,
    })
  }

  async getTaskNeighborhood(
    taskId: string,
    options: RequestOptions & { depth?: number; limitNodes?: number } = {},
  ) {
    const params = new URLSearchParams({ depth: String(options.depth ?? 1) })
    if (typeof options.limitNodes === "number") params.set("limit_nodes", String(options.limitNodes))
    return this.request<TaskNeighborhood>("/api/v1/tasks/" + taskId + "/neighborhood?" + params.toString(), {
      signal: options.signal,
    })
  }

  async getBoardTaskMap(
    board = this.board,
    options: RequestOptions & {
      activeOnly?: boolean
      contextDepth?: number
      includeDoneContext?: boolean
      includeArchivedContext?: boolean
      hideIsolated?: boolean
      limitNodes?: number
    } = {},
  ) {
    const params = new URLSearchParams({
      active_only: String(options.activeOnly ?? true),
      context_depth: String(options.contextDepth ?? 1),
    })
    if (typeof options.includeDoneContext === "boolean") {
      params.set("include_done_context", String(options.includeDoneContext))
    }
    if (typeof options.includeArchivedContext === "boolean") {
      params.set("include_archived_context", String(options.includeArchivedContext))
    }
    if (typeof options.hideIsolated === "boolean") {
      params.set("hide_isolated", String(options.hideIsolated))
    }
    if (typeof options.limitNodes === "number") params.set("limit_nodes", String(options.limitNodes))
    return this.request<BoardTaskMap>("/api/v1/boards/" + board + "/task-map?" + params.toString(), {
      signal: options.signal,
    })
  }

  async listSteps(taskId: string, options: RequestOptions = {}) {
    return parseListStepsEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps", options)).data
  }

  async createStep(taskId: string, input: CreateStepInput, options: RequestOptions = {}) {
    return parseCreateStepEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps", {
      method: "POST",
      body: { ...input, actor: this.actor },
      signal: options.signal,
    })).data
  }

  async updateStep(
    taskId: string,
    stepId: string,
    input: UpdateStepInput,
    options: RequestOptions = {},
  ) {
    return parseUpdateStepEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId, {
      method: "PATCH",
      body: { ...input, actor: this.actor },
      signal: options.signal,
    })).data
  }

  async removeStep(taskId: string, stepId: string, options: RequestOptions = {}) {
    return parseRemoveStepEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId, {
      method: "DELETE",
      signal: options.signal,
    })).data
  }

  async completeStep(taskId: string, stepId: string, note: string, options: RequestOptions = {}) {
    return parseCompleteStepEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId + "/done", {
      method: "POST",
      body: { note, actor: this.actor },
      signal: options.signal,
    })).data
  }

  async skipStep(taskId: string, stepId: string, reason: string, options: RequestOptions = {}) {
    return parseSkipStepEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId + "/skip", {
      method: "POST",
      body: { reason, actor: this.actor },
      signal: options.signal,
    })).data
  }

  async reopenStep(taskId: string, stepId: string, reason: string, options: RequestOptions = {}) {
    return parseReopenStepEnvelope(await this.requestRaw("/api/v1/tasks/" + taskId + "/steps/" + stepId + "/reopen", {
      method: "POST",
      body: { reason, actor: this.actor },
      signal: options.signal,
    })).data
  }

  async markExecutionPlanNotRequired(taskId: string, reason: string, options: RequestOptions = {}) {
    return this.request<TaskExecutionPlan>("/api/v1/tasks/" + taskId + "/execution-plan/not-required", {
      method: "POST",
      body: { reason, actor: this.actor },
      signal: options.signal,
    })
  }

  async listRuns(taskId: string, options: RequestOptions = {}) {
    return parseListRunsEnvelope(await this.requestRaw(`/api/v1/tasks/${taskId}/runs`, options)).data
  }

  async getRun(runId: string, options: RequestOptions = {}) {
    return parseGetRunEnvelope(await this.requestRaw(`/api/v1/runs/${runId}`, options)).data
  }

  async getRunLog(runId: string, options: RequestOptions = {}) {
    return this.request<RunLog>(`/api/v1/runs/${runId}/log`, options)
  }

  async listComments(taskId: string, options: RequestOptions = {}) {
    return parseListCommentsEnvelope(await this.requestRaw(`/api/v1/tasks/${taskId}/comments`, options)).data
  }

  async createComment(taskId: string, body: string, options: RequestOptions = {}) {
    return parseCreateCommentEnvelope(await this.requestRaw(`/api/v1/tasks/${taskId}/comments`, {
      method: "POST", body: { author: this.actor, body }, signal: options.signal,
    })).data
  }

  async addTaskLabel(taskId: string, name: string, options: RequestOptions = {}) {
    return parseAddTaskLabelEnvelope(await this.requestRaw(`/api/v1/tasks/${taskId}/labels`, {
      method: "POST", body: { name, actor: this.actor }, signal: options.signal,
    })).data
  }

  async suggestTaskLabels(
    taskId: string,
    options: RequestOptions & {
      limit?: number
      candidateLimit?: number
      atomLimit?: number
      maxSelectedLabels?: number
      minScore?: number
    } = {},
  ) {
    const params = new URLSearchParams({ limit: String(options.limit ?? 5) })
    if (typeof options.candidateLimit === "number") {
      params.set("candidate_limit", String(options.candidateLimit))
    }
    if (typeof options.atomLimit === "number") {
      params.set("atom_limit", String(options.atomLimit))
    }
    if (typeof options.maxSelectedLabels === "number") {
      params.set("max_selected_labels", String(options.maxSelectedLabels))
    }
    if (typeof options.minScore === "number") {
      params.set("min_score", String(options.minScore))
    }
    return parseLabelSuggestionEnvelope(await this.requestRaw(
      `/api/v1/tasks/${taskId}/labels/suggestions?${params.toString()}`,
      { signal: options.signal },
    )).data
  }

  async removeTaskLabel(taskId: string, labelId: string, options: RequestOptions = {}) {
    return parseRemoveTaskLabelEnvelope(await this.requestRaw(`/api/v1/tasks/${taskId}/labels/${labelId}`, {
      method: "DELETE", signal: options.signal,
    })).data
  }

  async listSignals(options: SignalListOptions = {}) {
    const params = signalSearchParams(options)
    const signals = parseSignalListEnvelope(await this.requestRaw(
      `/api/v1/boards/${this.board}/signals?${params.toString()}`,
      { signal: options.signal },
    )).data
    return signals
  }

  async reviewSignals(options: SignalListOptions = {}) {
    const params = signalSearchParams(options)
    const signals = parseSignalListEnvelope(await this.requestRaw(
      `/api/v1/boards/${this.board}/signals/review?${params.toString()}`,
      { signal: options.signal },
    )).data
    return signals
  }

  async getSignal(signalId: string, options: RequestOptions = {}) {
    return parseSignalEnvelope(await this.requestRaw(`/api/v1/signals/${encodeURIComponent(signalId)}`, options)).data
  }

  async listLabelOntologySignals(options: LabelOntologySignalListOptions = {}) {
    const params = new URLSearchParams({
      include_all: String(options.includeAll ?? false),
      limit: String(options.limit ?? 100),
    })
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const kind of options.kinds ?? []) params.append("kind", kind)
    if (options.task?.trim()) params.set("task", options.task.trim())
    if (options.label?.trim()) params.set("label", options.label.trim())
    if (options.proposedLabel?.trim()) params.set("proposed_label", options.proposedLabel.trim())
    const response = expectRecord(
      await this.requestRaw(`/api/v1/boards/${this.board}/label-ontology/signals?${params.toString()}`, {
        signal: options.signal,
      }),
      "label ontology signals response",
    )
    expectExactKeys(response, ["data", "meta"], "label ontology signals response")
    const meta = expectRecord(response.meta, "label ontology signals response meta")
    expectExactKeys(meta, ["limit"], "label ontology signals response meta")
    expectSafeInteger(meta.limit, "label ontology signals response meta.limit", true)
    return expectArray<unknown>(response.data, "label ontology signals response data").map((entry, index) =>
      parseLabelOntologySignal(entry, `label ontology signals response data[${index}]`),
    )
  }

  async reviewLabelOntology(options: LabelOntologyReviewOptions = {}) {
    const params = new URLSearchParams({
      group_by: options.groupBy ?? "label",
      include_all: String(options.includeAll ?? false),
      limit: String(options.limit ?? 100),
    })
    const response = expectRecord<Record<string, unknown>>(await this.requestRaw(
      `/api/v1/boards/${this.board}/label-ontology/review?${params.toString()}`,
      { signal: options.signal },
    ), "label ontology review response")
    expectExactKeys(response, ["data", "meta"], "label ontology review response")
    const meta = expectRecord<Record<string, unknown>>(response.meta, "label ontology review response meta")
    expectExactKeys(meta, ["group_by", "include_all", "limit"], "label ontology review response meta")
    expectString(meta.group_by, "label ontology review response meta.group_by")
    expectBoolean(meta.include_all, "label ontology review response meta.include_all")
    expectSafeInteger(meta.limit, "label ontology review response meta.limit", true)
    return expectArray<unknown>(response.data, "label ontology review response data").map((entry, index) =>
      parseLabelOntologyReviewGroup(entry, `label ontology review response data[${index}]`),
    )
  }

  async getLabelOntologySignal(signalId: string, options: RequestOptions = {}) {
    return parseLabelOntologyDetailEnvelope(await this.requestRaw(
      `/api/v1/label-ontology/signals/${encodeURIComponent(signalId)}`, options,
    )).data
  }

  async createLabelOntologyAction(input: LabelOntologyActionCreateInput, options: RequestOptions = {}) {
    return parseLabelOntologyActionEnvelope(await this.requestRaw(`/api/v1/boards/${this.board}/label-ontology/actions`, {
      method: "POST",
      body: {
        actor: { name: this.actor, type: "user", agent_type: null },
        action_type: input.actionType,
        signal_ids: input.signalIds,
        reason: input.reason,
        superseded_by_signal_id: input.supersededBySignalId ?? null,
      },
      signal: options.signal,
    })).data
  }

  async explainLabelAtom(atomRef: string, options: RequestOptions = {}) {
    return this.request<LabelAtomExplainRecord>(
      `/api/v1/boards/${this.board}/labels/atoms/${encodeURIComponent(atomRef)}/explain`,
      options,
    )
  }

  async listEvents(taskId: string, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: this.board, task_id: taskId, limit: "50" })
    const envelope = await this.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      options,
    )
    return { events: envelope.data, meta: envelope.meta ?? {} } satisfies EventPage
  }

  async listBoardEvents(options: { after?: number; limit?: number; signal?: AbortSignal } = {}) {
    const params = new URLSearchParams({ board: this.board, limit: String(options.limit ?? 100) })
    if (typeof options.after === "number") params.set("after", String(options.after))
    const envelope = await this.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      { signal: options.signal },
    )
    return { events: envelope.data, meta: envelope.meta ?? {} } satisfies EventPage
  }

  async listEventsAfter(after: number, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: this.board, after: String(after), limit: "100" })
    const envelope = await this.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      options,
    )
    return { events: envelope.data, meta: envelope.meta ?? {} } satisfies EventPage
  }

  async transition(task: Task, action: "specify" | "promote" | "reopen" | "unblock" | "archive", body?: Record<string, unknown>, options?: RequestOptions): Promise<Task>
  async transition(task: Task, action: "claim" | "heartbeat" | "complete" | "submit-review" | "block", body?: Record<string, unknown>, options?: RequestOptions): Promise<Task | ClaimResponse>
  async transition(task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "complete" | "reopen" | "submit-review" | "block" | "unblock" | "archive", body?: Record<string, unknown>, options?: RequestOptions): Promise<Task | ClaimResponse>
  async transition(task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "complete" | "reopen" | "submit-review" | "block" | "unblock" | "archive", body: Record<string, unknown> = {}, options: RequestOptions = {}): Promise<Task | ClaimResponse> {
    const payload = { actor: this.actor, ...body }
    const path = `/api/v1/tasks/${task.id}/transitions/${action}`
    if (action === "specify" || action === "promote" || action === "reopen" || action === "unblock" || action === "archive") {
      return parseTransitionTaskEnvelope(await this.requestRaw(path, {
        method: "POST",
        body: payload,
        signal: options.signal,
      }))
    }
    return this.request<Task | ClaimResponse>(path, {
      method: "POST",
      body: payload,
      signal: options.signal,
    })
  }

  private async requestRaw(path: string, init: RequestOptions = {}): Promise<unknown> {
    const headers: Record<string, string> = { "Accept-Language": this.options.locale ?? getCurrentDesktopLocale() }
    if (init.body !== undefined) headers["Content-Type"] = "application/json"
    if (init.actorHeader) headers["X-KB-Actor"] = this.actor
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, { method: init.method ?? "GET", headers, body: init.body === undefined ? undefined : JSON.stringify(init.body), signal: init.signal })
    const text = await response.text()
    let json: unknown = null
    try { json = text ? JSON.parse(text) : null } catch { throw new ApiError("invalid_response", "response must be valid JSON") }
    const record = json && typeof json === "object" && !Array.isArray(json) ? json as Record<string, unknown> : null
    if (record && "error" in record) {
      const error = parseTaskReadErrorEnvelope(record)
      throw new ApiError(error.code, error.message, error.details)
    }
    if (!response.ok) throw new ApiError("http_error", `${response.status} ${response.statusText}`.trim())
    return json
  }

  async requestEnvelope<T, M = Record<string, unknown>>(path: string, init: RequestOptions = {}) {
    const method = init.method ?? "GET"
    const headers: Record<string, string> = {
      "Accept-Language": this.options.locale ?? getCurrentDesktopLocale(),
    }
    if (init.body !== undefined) headers["Content-Type"] = "application/json"
    if (method.toUpperCase() !== "GET") headers["X-KB-Actor"] = this.actor
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, {
      method,
      headers,
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      signal: init.signal,
    })
    const text = await response.text()
    const json = parseJsonEnvelope<T, M>(text)
    if (!response.ok || !json || "error" in json) {
      const error = json && "error" in json
        ? json.error
        : { code: "http_error", message: `${response.status} ${response.statusText}`.trim() }
      throw new ApiError(error.code, error.message)
    }
    return json
  }

  private async request<T>(path: string, init: RequestOptions = {}) {
    const envelope = await this.requestEnvelope<T>(path, init)
    return envelope.data
  }

  private taskListParams(options: TaskListOptions = {}) {
    const params = new URLSearchParams()
    const limit = options.limit ?? 100
    const offset = options.offset ?? 0
    params.set("include_archived", String(options.includeArchived ?? false))
    params.set("limit", String(limit))
    params.set("offset", String(offset))
    params.set("sort", options.sort ?? "-updated_at")
    if (options.query?.trim()) params.set("q", options.query.trim())
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const priority of options.priorities ?? []) params.append("priority", String(priority))
    for (const label of options.labels ?? []) {
      if (label.trim()) params.append("label", label.trim())
    }
    for (const filter of options.planFilters ?? []) params.append("plan_filter", filter)
    return params
  }

  private searchTaskParams(options: SearchTaskOptions) {
    const params = new URLSearchParams()
    const limit = options.limit ?? 100
    const offset = options.offset ?? 0
    params.set("board", this.board)
    params.set("q", options.query.trim())
    params.set("include_archived", String(options.includeArchived ?? false))
    params.set("limit", String(limit))
    params.set("offset", String(offset))
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const label of options.labels ?? []) {
      if (label.trim()) params.append("label", label.trim())
    }
    return params
  }
}

function newClientTaskId() {
  return `t_${crypto.randomUUID().replace(/-/g, "").toUpperCase()}`
}

type RequestOptions = {
  method?: string
  body?: unknown
  actorHeader?: boolean
  signal?: AbortSignal
}

type TaskListOptions = {
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

type BoardListOptions = {
  includeArchived?: boolean
  signal?: AbortSignal
}

type SearchTaskOptions = TaskListOptions & {
  query: string
}

type SignalListOptions = {
  statuses?: SignalStatus[]
  kinds?: string[]
  task?: string
  includeAll?: boolean
  limit?: number
  signal?: AbortSignal
}

type LabelOntologySignalListOptions = {
  statuses?: LabelOntologySignalStatus[]
  kinds?: LabelOntologySignalKind[]
  task?: string
  label?: string
  proposedLabel?: string
  includeAll?: boolean
  limit?: number
  signal?: AbortSignal
}

type LabelOntologyReviewOptions = {
  groupBy?: LabelOntologyReviewGroupBy
  includeAll?: boolean
  limit?: number
  signal?: AbortSignal
}

type LabelOntologyActionCreateInput = {
  actionType: Extract<LabelOntologyActionType, "confirm" | "reject" | "supersede" | "resolve_no_change">
  signalIds: string[]
  reason: string
  supersededBySignalId?: string | null
}

type PageEnvelopeMeta = Partial<PageMeta>

function signalSearchParams(options: SignalListOptions) {
  const params = new URLSearchParams({
    include_all: String(options.includeAll ?? false),
    limit: String(options.limit ?? 100),
  })
  for (const status of options.statuses ?? []) params.append("status", status)
  for (const kind of options.kinds ?? []) {
    if (kind.trim()) params.append("kind", kind.trim())
  }
  if (options.task?.trim()) params.set("task", options.task.trim())
  return params
}

function parseJsonEnvelope<T, M>(text: string): ApiEnvelope<T, M> | ErrorEnvelope | null {
  if (!text) return null
  try {
    return JSON.parse(text) as ApiEnvelope<T, M> | ErrorEnvelope
  } catch {
    return null
  }
}

function expectArray<T>(value: unknown, label: string): T[] {
  if (!Array.isArray(value)) {
    throw new ApiError("invalid_response", `${label} must be an array`)
  }
  return value as T[]
}

function expectRecord<T extends Record<string, unknown>>(value: unknown, label: string): T {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new ApiError("invalid_response", `${label} must be an object`)
  }
  return value as T
}

function normalizePageMeta(meta: PageEnvelopeMeta | undefined, fallback: { limit: number; offset: number }): PageMeta {
  const limit = numericMeta(meta?.limit, fallback.limit)
  const offset = numericMeta(meta?.offset, fallback.offset)
  const total = typeof meta?.total === "number" && Number.isFinite(meta.total) ? meta.total : null
  return { limit, offset, total }
}

function numericMeta(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

function expectRequiredOffsetPageMeta(value: unknown, label: string): RequiredOffsetPageMeta {
  const meta = expectRecord<Record<string, unknown>>(value, label)
  return {
    limit: expectFiniteNumber(meta.limit, label + ".limit"),
    offset: expectFiniteNumber(meta.offset, label + ".offset"),
  }
}

function expectRequiredTotalPageMeta(value: unknown, label: string): RequiredTotalPageMeta {
  const meta = expectRequiredOffsetPageMeta(value, label)
  const record = expectRecord<Record<string, unknown>>(value, label)
  return { ...meta, total: expectFiniteNumber(record.total, label + ".total") }
}

function expectFiniteNumber(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new ApiError("invalid_response", label + " must be a finite number")
  }
  return value
}


const DOCTOR_STORE_KEYS = ["store_name", "schema_version", "last_event_id", "dirty", "last_error", "pending_outbox", "running_outbox", "failed_outbox"] as const
const DOCTOR_ISSUE_KEYS = ["severity", "code", "message", "record_ids"] as const
const DOCTOR_KEYS = ["ok", "integrity_check", "migration_version", "user_version", "expired_running_tasks", "running_tasks_without_active_run", "orphan_running_runs", "dependency_cycles", "archived_dependency_edges", "missing_run_logs", "suspicious_run_log_paths", "executable_dependency_violations", "executable_spec_violations", "executable_schedule_violations", "unplanned_active_tasks", "active_parents_with_incomplete_required_steps", "outbox_pending", "outbox_running", "outbox_failed", "derived_dirty_stores", "derived_error_stores", "derived_stores", "consistency_errors", "consistency_warnings", "consistency_issues", "ontology_ledger_errors", "ontology_ledger_warnings", "ontology_ledger_issues"] as const
const CHECKPOINT_KEYS = ["busy", "log_frames", "checkpointed_frames"] as const

function parseDoctorIssue(value: unknown, label: string): DoctorIssue {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, DOCTOR_ISSUE_KEYS, label)
  expectString(record.severity, label + ".severity")
  expectString(record.code, label + ".code")
  expectString(record.message, label + ".message")
  expectArray<unknown>(record.record_ids, label + ".record_ids").forEach((entry, index) => expectString(entry, label + ".record_ids[" + index + "]"))
  return record as DoctorIssue
}

function parseDoctorStore(value: unknown, label: string): DoctorDerivedStore {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, DOCTOR_STORE_KEYS, label)
  expectString(record.store_name, label + ".store_name")
  for (const key of ["schema_version", "last_event_id", "pending_outbox", "running_outbox", "failed_outbox"] as const) expectSafeInteger(record[key], label + "." + key)
  expectBoolean(record.dirty, label + ".dirty")
  expectNullableString(record.last_error, label + ".last_error")
  return record as DoctorDerivedStore
}

function parseDoctorReport(value: unknown): DoctorReport {
  const record = expectRecord<Record<string, unknown>>(value, "doctor response data")
  expectExactKeys(record, DOCTOR_KEYS, "doctor response data")
  expectBoolean(record.ok, "doctor response data.ok")
  expectString(record.integrity_check, "doctor response data.integrity_check")
  expectNullableInteger(record.migration_version, "doctor response data.migration_version")
  for (const key of DOCTOR_KEYS) {
    if (!["ok", "integrity_check", "migration_version", "derived_stores", "consistency_issues", "ontology_ledger_issues"].includes(key)) {
      expectSafeInteger(record[key], "doctor response data." + key)
    }
  }
  expectArray<unknown>(record.derived_stores, "doctor response data.derived_stores").forEach((entry, index) => parseDoctorStore(entry, "doctor response data.derived_stores[" + index + "]"))
  for (const key of ["consistency_issues", "ontology_ledger_issues"] as const) {
    expectArray<unknown>(record[key], "doctor response data." + key).forEach((entry, index) => parseDoctorIssue(entry, "doctor response data." + key + "[" + index + "]"))
  }
  return record as DoctorReport
}

function parseCheckpointReport(value: unknown): CheckpointReport {
  const record = expectRecord<Record<string, unknown>>(value, "checkpoint response data")
  expectExactKeys(record, CHECKPOINT_KEYS, "checkpoint response data")
  for (const key of CHECKPOINT_KEYS) expectSafeInteger(record[key], "checkpoint response data." + key)
  return record as CheckpointReport
}
const TASK_STATUSES = new Set<TaskStatus>(["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"])
const PLAN_STATES = new Set<StepPlanState>(["unplanned", "planned", "not_required"])
const TASK_KEYS = ["id", "board_id", "board_slug", "ref", "seq", "title", "description", "status", "status_reason", "assignee", "priority", "position", "scheduled_at", "due_at", "created_by", "created_at", "updated_at", "started_at", "completed_at", "archived_at", "claim_owner", "claim_expires_at", "last_heartbeat_at", "current_run_id", "retry_count", "max_retries", "result_summary", "result", "metadata", "lock_version", "dependency_blocked", "unfinished_parent_count", "execution_plan_state", "required_step_count", "completed_required_step_count", "optional_step_count", "labels"] as const
const LABEL_KEYS = ["id", "board_id", "name", "color", "created_at", "updated_at"] as const

function expectExactKeys(record: Record<string, unknown>, expected: readonly string[], label: string) {
  const actual = Object.keys(record)
  if (actual.length !== expected.length || actual.some((key) => !expected.includes(key))) {
    throw new ApiError("invalid_response", `${label} must contain exactly: ${expected.join(", ")}`)
  }
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") throw new ApiError("invalid_response", `${label} must be a string`)
  return value
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") throw new ApiError("invalid_response", `${label} must be a boolean`)
  return value
}

function expectSafeInteger(value: unknown, label: string, nonNegative = false): number {
  if (!Number.isSafeInteger(value) || (nonNegative && (value as number) < 0)) {
    throw new ApiError("invalid_response", `${label} must be ${nonNegative ? "a non-negative " : "a "}safe integer`)
  }
  return value as number
}

function expectNullableString(value: unknown, label: string): string | null {
  return value === null ? null : expectString(value, label)
}

function expectNullableInteger(value: unknown, label: string): number | null {
  return value === null ? null : expectSafeInteger(value, label)
}

function parseApiLabel(value: unknown, label: string): LabelRecord {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, LABEL_KEYS, label)
  expectString(record.id, `${label}.id`); expectString(record.board_id, `${label}.board_id`); expectString(record.name, `${label}.name`)
  expectNullableString(record.color, `${label}.color`); expectSafeInteger(record.created_at, `${label}.created_at`); expectSafeInteger(record.updated_at, `${label}.updated_at`)
  return record as LabelRecord
}

function parseApiTask(value: unknown, label: string): Task {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, TASK_KEYS, label)
  for (const key of ["id", "board_id", "board_slug", "ref", "title", "created_by"] as const) expectString(record[key], `${label}.${key}`)
  for (const key of ["description", "status_reason", "assignee", "claim_owner", "current_run_id", "result_summary"] as const) expectNullableString(record[key], `${label}.${key}`)
  for (const key of ["seq", "position", "created_at", "updated_at", "retry_count", "lock_version", "unfinished_parent_count", "required_step_count", "completed_required_step_count", "optional_step_count"] as const) expectSafeInteger(record[key], `${label}.${key}`)
  for (const key of ["scheduled_at", "due_at", "started_at", "completed_at", "archived_at", "claim_expires_at", "last_heartbeat_at", "max_retries"] as const) expectNullableInteger(record[key], `${label}.${key}`)
  if (!TASK_STATUSES.has(record.status as TaskStatus)) throw new ApiError("invalid_response", `${label}.status is unknown`)
  if (!Number.isSafeInteger(record.priority) || (record.priority as number) < 0 || (record.priority as number) > 3) throw new ApiError("invalid_response", `${label}.priority must be an integer in 0..=3`)
  expectBoolean(record.dependency_blocked, `${label}.dependency_blocked`)
  if (!PLAN_STATES.has(record.execution_plan_state as StepPlanState)) throw new ApiError("invalid_response", `${label}.execution_plan_state is unknown`)
  record.labels = expectArray<unknown>(record.labels, `${label}.labels`).map((entry, index) => parseApiLabel(entry, `${label}.labels[${index}]`))
  return record as Task
}

function parseTransitionTaskEnvelope(value: unknown): Task {
  const envelope = expectRecord<Record<string, unknown>>(value, "task transition response")
  expectExactKeys(envelope, ["data"], "task transition response")
  return parseApiTask(envelope.data, "task transition response.data")
}

function parseTotalMeta(value: unknown, label: string): RequiredTotalPageMeta {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ["limit", "offset", "total"], label)
  return { limit: expectSafeInteger(record.limit, `${label}.limit`, true), offset: expectSafeInteger(record.offset, `${label}.offset`, true), total: expectSafeInteger(record.total, `${label}.total`, true) }
}

function parseOffsetMeta(value: unknown, label: string): RequiredOffsetPageMeta {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, ["limit", "offset"], label)
  return { limit: expectSafeInteger(record.limit, `${label}.limit`, true), offset: expectSafeInteger(record.offset, `${label}.offset`, true) }
}

function parseTaskReadErrorEnvelope(value: unknown): ErrorBody {
  const envelope = expectRecord<Record<string, unknown>>(value, "task-read error response")
  expectExactKeys(envelope, ["error"], "task-read error response")
  const error = expectRecord<Record<string, unknown>>(envelope.error, "task-read error response.error")
  const keys = Object.keys(error)
  const hasDetails = keys.includes("details")
  expectExactKeys(error, hasDetails ? ["code", "message", "details"] : ["code", "message"], "task-read error response.error")
  return {
    code: expectString(error.code, "task-read error response.error.code"),
    message: expectString(error.message, "task-read error response.error.message"),
    ...(hasDetails ? { details: error.details } : {}),
  }
}


const RUN_STATUSES = new Set(["running", "succeeded", "failed", "canceled", "expired"])
function parseApiRun(value: unknown, label: string): Run {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ["id", "task_id", "status", "worker_profile", "worker_pid", "claim_owner", "started_at", "finished_at", "exit_code", "summary", "error", "has_log", "metadata"], label)
  if (!RUN_STATUSES.has(record.status as string)) throw new ApiError("invalid_response", `${label}.status is unknown`)
  return { id: expectString(record.id, `${label}.id`), task_id: expectString(record.task_id, `${label}.task_id`), status: record.status as Run["status"], worker_profile: expectNullableString(record.worker_profile, `${label}.worker_profile`), worker_pid: expectNullableInteger(record.worker_pid, `${label}.worker_pid`), claim_owner: expectString(record.claim_owner, `${label}.claim_owner`), started_at: expectSafeInteger(record.started_at, `${label}.started_at`, true), finished_at: expectNullableInteger(record.finished_at, `${label}.finished_at`), exit_code: expectNullableInteger(record.exit_code, `${label}.exit_code`), summary: expectNullableString(record.summary, `${label}.summary`), error: expectNullableString(record.error, `${label}.error`), has_log: expectBoolean(record.has_log, `${label}.has_log`), metadata: record.metadata }
}
function parseListRunsEnvelope(value: unknown): { data: Run[] } {
  const envelope = expectRecord<Record<string, unknown>>(value, "list runs response"); expectExactKeys(envelope, ["data"], "list runs response")
  return { data: expectArray<unknown>(envelope.data, "list runs response data").map((entry, index) => parseApiRun(entry, `list runs response data[${index}]`)) }
}
function parseGetRunEnvelope(value: unknown): { data: Run } {
  const envelope = expectRecord<Record<string, unknown>>(value, "get run response"); expectExactKeys(envelope, ["data"], "get run response")
  return { data: parseApiRun(envelope.data, "get run response data") }
}

const SIGNAL_OBSERVATION_KEYS = ["id", "board_id", "task_id", "task_ref_snapshot", "run_id", "comment_id", "actor", "agent_type", "source", "evidence", "created_at"] as const
const SIGNAL_KEYS = ["id", "board_id", "observation_id", "kind", "title", "summary", "severity", "status", "dedupe_key", "superseded_by_signal_id", "reviewed_by", "reviewed_at", "review_reason", "created_at", "updated_at", "observation"] as const
function parseSignalObservation(value: unknown, label: string): SignalObservationRecord {
 const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, SIGNAL_OBSERVATION_KEYS, label)
 for (const key of ["id", "board_id", "actor"] as const) expectString(record[key], `.`)
 record.evidence = expectRecord<Record<string, unknown>>(record.evidence, `${label}.evidence`)
 for (const key of ["task_id", "task_ref_snapshot", "run_id", "comment_id", "agent_type", "source"] as const) expectNullableString(record[key], `.`)
 expectSafeInteger(record.created_at, `.created_at`, true); return record as SignalObservationRecord
}
function parseSignalRecord(value: unknown, label: string): SignalRecord {
 const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, SIGNAL_KEYS, label)
 for (const key of ["id", "board_id", "observation_id", "kind", "title", "summary", "severity"] as const) expectString(record[key], `.`)
 expectString(record.status, `${label}.status`)
 for (const key of ["dedupe_key", "superseded_by_signal_id", "reviewed_by", "review_reason"] as const) expectNullableString(record[key], `.`)
 expectNullableInteger(record.reviewed_at, `.reviewed_at`); expectSafeInteger(record.created_at, `.created_at`, true); expectSafeInteger(record.updated_at, `.updated_at`, true)
 record.observation = parseSignalObservation(record.observation, `.observation`); return record as SignalRecord
}
function parseSignalListEnvelope(value: unknown): { data: SignalRecord[]; meta: { include_all: boolean; limit: number } } {
 const envelope = expectRecord<Record<string, unknown>>(value, "signals response"); expectExactKeys(envelope, ["data", "meta"], "signals response")
 const meta = expectRecord<Record<string, unknown>>(envelope.meta, "signals response meta"); expectExactKeys(meta, ["include_all", "limit"], "signals response meta")
 return { data: expectArray<unknown>(envelope.data, "signals response data").map((entry) => parseSignalRecord(entry, `signals response data[]`)), meta: { include_all: expectBoolean(meta.include_all, "signals response meta.include_all"), limit: expectSafeInteger(meta.limit, "signals response meta.limit", true) } }
}
function parseSignalEnvelope(value: unknown): { data: SignalRecord } { const envelope = expectRecord<Record<string, unknown>>(value, "signal response"); expectExactKeys(envelope, ["data"], "signal response"); return { data: parseSignalRecord(envelope.data, "signal response data") } }

const ONTOLOGY_SIGNAL_KEYS = ["id", "observation_id", "board_id", "kind", "status", "target_label_id", "target_label_name_snapshot", "proposed_action", "candidate_atom_polarity", "candidate_atom_kind", "candidate_text", "candidate_content_hash", "proposed_label_name", "proposed_label_name_normalized", "agent_selected", "suggest_state", "suggest_score", "suggest_rank", "final_selected", "rationale", "confidence", "signal_key", "superseded_by_signal_id", "status_reason", "created_at", "updated_at", "reviewed_at", "closed_at", "related_labels", "proposal"] as const
const ONTOLOGY_OBSERVATION_KEYS = ["id", "board_id", "task_id", "task_ref_snapshot", "suggest_input_hash", "suggest_coverage", "suggest_coverage_cosine", "suggest_residual_norm", "suggest_needs_new_label", "suggest_degraded", "capture_fingerprint", "created_by", "created_by_type", "agent_type", "created_at", "signals", "task_snapshot", "agent_candidates", "suggestion_snapshot", "final_decision", "diagnostics"] as const
const ONTOLOGY_ACTION_KEYS = ["id", "board_id", "parent_action_id", "action_type", "reason", "target_label_id", "result_label_id", "result_atom_id", "result_atom_content_hash", "result_proposal_id", "canonical_before_hash", "canonical_after_hash", "validation_requirement", "validation_status", "validation_effective_outcome", "validation_latest_attempt_id", "created_by", "created_by_type", "agent_type", "created_at", "signal_ids", "change", "validation"] as const
const ONTOLOGY_REVIEW_GROUP_KEYS = ["group_by", "key", "label_id", "label_name", "candidate_atom_polarity", "candidate_atom_kind", "candidate_text", "candidate_content_hash", "proposed_label_name", "proposed_label_name_normalized", "cluster_key", "cluster_reason", "task_count", "signal_count", "open_count", "confirmed_count", "resolved_count", "rejected_count", "superseded_count", "degraded_count", "average_score", "median_score", "oldest_signal_at", "latest_signal_at", "sample_task_refs", "signal_ids", "action_count", "action_ids", "proposal_ids", "labels", "candidate_atom_variants"] as const
const ONTOLOGY_REVIEW_GROUP_BY = new Set<LabelOntologyReviewGroupBy>(["label", "candidate_atom", "proposed_label", "cluster"])
const ONTOLOGY_ACTION_TYPES = new Set<LabelOntologyActionType>(["confirm", "reject", "supersede", "resolve_no_change", "add_positive_atom", "add_negative_atom", "adopt_existing_atom", "update_semantics", "create_label_proposal", "bootstrap_label", "rename_label", "split_label", "merge_labels", "revert_ontology_mutation", "validate"])

function parseLabelOntologySignal(value: unknown, label: string): LabelOntologySignalRecord {
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

function parseLabelOntologyObservation(value: unknown, label: string): LabelOntologyObservationRecord {
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

function parseLabelOntologyAction(value: unknown, label: string): LabelOntologyActionRecord {
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

function parseLabelOntologyReviewGroup(value: unknown, label: string): LabelOntologyReviewGroup {
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

function parseLabelOntologyDetailEnvelope(value: unknown): { data: LabelOntologySignalDetail } {
  const envelope = expectRecord<Record<string, unknown>>(value, "label ontology signal response"); expectExactKeys(envelope, ["data"], "label ontology signal response")
  const data = expectRecord<Record<string, unknown>>(envelope.data, "label ontology signal response data"); expectExactKeys(data, ["signal", "observation", "actions"], "label ontology signal response data")
  return { data: { signal: parseLabelOntologySignal(data.signal, "label ontology signal response data.signal"), observation: parseLabelOntologyObservation(data.observation, "label ontology signal response data.observation"), actions: expectArray<unknown>(data.actions, "label ontology signal response data.actions").map((entry, index) => parseLabelOntologyAction(entry, `label ontology signal response data.actions[${index}]`)) } }
}

function parseLabelOntologyActionEnvelope(value: unknown): { data: LabelOntologyActionRecord } {
  const envelope = expectRecord<Record<string, unknown>>(value, "label ontology action response"); expectExactKeys(envelope, ["data"], "label ontology action response")
  return { data: parseLabelOntologyAction(envelope.data, "label ontology action response data") }
}

const LABEL_EVIDENCE_KEYS = ["atom_id", "label_id", "label_name", "polarity", "kind", "text", "score"] as const
const LABEL_SUGGESTION_KEYS = ["label_id", "label_name", "score", "weight", "already_applied", "evidence_atoms", "negative_evidence_atoms"] as const
const LABEL_SUGGESTION_RESULT_KEYS = ["task_id", "board_id", "selected_labels", "candidates", "coverage", "coverage_cosine", "residual_norm", "needs_new_label", "reason_codes", "degraded", "diagnostics"] as const

function parseLabelEvidence(value: unknown, label: string): LabelSuggestionEvidenceAtom {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, LABEL_EVIDENCE_KEYS, label)
  for (const key of ["atom_id", "label_id", "label_name", "polarity", "kind", "text"] as const) expectString(record[key], `.`)
  expectFiniteNumber(record.score, `.score`)
  return record as LabelSuggestionEvidenceAtom
}
function parseSelectedLabel(value: unknown, label: string): SelectedLabelSuggestion {
  const record = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(record, LABEL_SUGGESTION_KEYS, label)
  expectString(record.label_id, `.label_id`); expectString(record.label_name, `.label_name`)
  expectFiniteNumber(record.score, `.score`); expectFiniteNumber(record.weight, `.weight`); expectBoolean(record.already_applied, `.already_applied`)
  record.evidence_atoms = expectArray<unknown>(record.evidence_atoms, `.evidence_atoms`).map((entry, index) => parseLabelEvidence(entry, `.evidence_atoms[]`))
  record.negative_evidence_atoms = expectArray<unknown>(record.negative_evidence_atoms, `.negative_evidence_atoms`).map((entry, index) => parseLabelEvidence(entry, `.negative_evidence_atoms[]`))
  return record as SelectedLabelSuggestion
}
function parseLabelSuggestionEnvelope(value: unknown): { data: LabelSuggestionResult } {
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

function parseAddTaskLabelEnvelope(value: unknown): { data: Task; meta?: { created_labels: LabelRecord[] } } {
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
function parseRemoveTaskLabelEnvelope(value: unknown): { data: Task } {
  const envelope = expectRecord<Record<string, unknown>>(value, "remove task label response"); expectExactKeys(envelope, ["data"], "remove task label response")
  return { data: parseApiTask(envelope.data, "remove task label response data") }
}

function parseApiComment(value: unknown, label: string): CommentRecord {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ["id", "board_id", "task_id", "author", "author_type", "agent_type", "body", "kind", "metadata", "created_at"], label)
  if (record.author_type !== "user" && record.author_type !== "agent") throw new ApiError("invalid_response", `${label}.author_type is unknown`)
  if (record.kind !== "note" && record.kind !== "decision" && record.kind !== "signal") throw new ApiError("invalid_response", `${label}.kind is unknown`)
  return { id: expectString(record.id, `${label}.id`), board_id: expectString(record.board_id, `${label}.board_id`), task_id: expectString(record.task_id, `${label}.task_id`), author: expectString(record.author, `${label}.author`), author_type: record.author_type, agent_type: expectNullableString(record.agent_type, `${label}.agent_type`), body: expectString(record.body, `${label}.body`), kind: record.kind, metadata: expectRecord<Record<string, unknown>>(record.metadata, `${label}.metadata`), created_at: expectSafeInteger(record.created_at, `${label}.created_at`, true) }
}

function parseApiBoard(value: unknown, label: string): Board {
  const record = expectRecord<Record<string, unknown>>(value, label)
  expectExactKeys(record, ["id", "slug", "name", "description", "created_at", "updated_at", "archived_at"], label)
  return {
    id: expectString(record.id, `${label}.id`),
    slug: expectString(record.slug, `${label}.slug`),
    name: expectString(record.name, `${label}.name`),
    description: expectNullableString(record.description, `${label}.description`),
    created_at: expectSafeInteger(record.created_at, `${label}.created_at`, true),
    updated_at: expectSafeInteger(record.updated_at, `${label}.updated_at`, true),
    archived_at: record.archived_at === null ? null : expectSafeInteger(record.archived_at, `${label}.archived_at`, true),
  }
}

function parseListBoardsEnvelope(value: unknown): { data: Board[] } {
  const envelope = expectRecord<Record<string, unknown>>(value, "boards response")
  expectExactKeys(envelope, ["data"], "boards response")
  return {
    data: expectArray<unknown>(envelope.data, "boards response data")
      .map((entry, index) => parseApiBoard(entry, `boards response data[${index}]`)),
  }
}
function parseCreateBoardEnvelope(value: unknown): { data: Board } {
  const envelope = expectRecord<Record<string, unknown>>(value, "create board response")
  expectExactKeys(envelope, ["data"], "create board response")
  return { data: parseApiBoard(envelope.data, "create board response data") }
}
function parseGetBoardEnvelope(value: unknown): { data: Board } {
  const envelope = expectRecord<Record<string, unknown>>(value, "get board response")
  expectExactKeys(envelope, ["data"], "get board response")
  return { data: parseApiBoard(envelope.data, "get board response data") }
}
function parseArchiveBoardEnvelope(value: unknown): { data: Board } {
  const envelope = expectRecord<Record<string, unknown>>(value, "archive board response")
  expectExactKeys(envelope, ["data"], "archive board response")
  return { data: parseApiBoard(envelope.data, "archive board response data") }
}
function parseListCommentsEnvelope(value: unknown): { data: CommentRecord[] } {
  const envelope = expectRecord<Record<string, unknown>>(value, "list comments response"); expectExactKeys(envelope, ["data"], "list comments response")
  return { data: expectArray<unknown>(envelope.data, "list comments response data").map((entry, index) => parseApiComment(entry, `list comments response data[${index}]`)) }
}
function parseCreateCommentEnvelope(value: unknown): { data: CommentRecord } {
  const envelope = expectRecord<Record<string, unknown>>(value, "create comment response"); expectExactKeys(envelope, ["data"], "create comment response")
  return { data: parseApiComment(envelope.data, "create comment response data") }
}

const STEP_KEYS = ["id", "parent_task_id", "title", "body", "linked_task", "position", "required", "status", "resolution_note", "resolved_by", "resolved_at", "created_by", "created_at", "updated_by", "updated_at"] as const
const EXECUTION_PLAN_KEYS = ["board_id", "task_id", "state", "reason", "updated_by", "updated_at"] as const
const STEP_STATUSES = new Set<StepStatus>(["todo", "done", "skipped"])

function parseTaskStep(value: unknown, label: string): TaskStep {
  const step = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(step, STEP_KEYS, label)
  for (const key of ["id", "parent_task_id", "title", "created_by", "updated_by"] as const) expectString(step[key], `${label}.${key}`)
  for (const key of ["body", "resolution_note", "resolved_by"] as const) expectNullableString(step[key], `${label}.${key}`)
  for (const key of ["position", "created_at", "updated_at"] as const) expectSafeInteger(step[key], `${label}.${key}`)
  expectNullableInteger(step.resolved_at, `${label}.resolved_at`); expectBoolean(step.required, `${label}.required`)
  if (!STEP_STATUSES.has(step.status as StepStatus)) throw new ApiError("invalid_response", `${label}.status is unknown`)
  if (step.linked_task !== null) step.linked_task = parseApiTask(step.linked_task, `${label}.linked_task`)
  return step as TaskStep
}

function parseExecutionPlan(value: unknown, label: string): TaskExecutionPlan {
  const plan = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(plan, EXECUTION_PLAN_KEYS, label)
  for (const key of ["board_id", "task_id", "updated_by"] as const) expectString(plan[key], `${label}.${key}`)
  expectNullableString(plan.reason, `${label}.reason`); expectSafeInteger(plan.updated_at, `${label}.updated_at`)
  if (!PLAN_STATES.has(plan.state as StepPlanState)) throw new ApiError("invalid_response", `${label}.state is unknown`)
  return plan as TaskExecutionPlan
}

function parseStepsEnvelope(value: unknown, label: string): { data: TaskSteps } {
  const envelope = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(envelope, ["data"], label)
  const data = expectRecord<Record<string, unknown>>(envelope.data, `${label}.data`); expectExactKeys(data, ["task_id", "steps", "execution_plan"], `${label}.data`)
  return { data: { task_id: expectString(data.task_id, `${label}.data.task_id`), steps: expectArray<unknown>(data.steps, `${label}.data.steps`).map((step, index) => parseTaskStep(step, `${label}.data.steps[${index}]`)), execution_plan: parseExecutionPlan(data.execution_plan, `${label}.data.execution_plan`) } }
}
function parseListStepsEnvelope(value: unknown) { return parseStepsEnvelope(value, "list steps response") }
function parseCreateStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "create step response") }
function parseUpdateStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "update step response") }
function parseRemoveStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "remove step response") }
function parseCompleteStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "complete step response") }
function parseSkipStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "skip step response") }
function parseReopenStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "reopen step response") }

function parseTaskListEnvelope(value: unknown): { data: Task[]; meta: RequiredTotalPageMeta } {
  const envelope = expectRecord<Record<string, unknown>>(value, "tasks response"); expectExactKeys(envelope, ["data", "meta"], "tasks response")
  return { data: expectArray<unknown>(envelope.data, "tasks response data").map((entry, index) => parseApiTask(entry, `tasks response data[${index}]`)), meta: parseTotalMeta(envelope.meta, "tasks response meta") }
}

function parseTaskStatusEnvelope(value: unknown): { data: TaskStatusWindowsResponse; meta: RequiredOffsetPageMeta } {
  const envelope = expectRecord<Record<string, unknown>>(value, "task status windows response"); expectExactKeys(envelope, ["data", "meta"], "task status windows response")
  const data = expectRecord<Record<string, unknown>>(envelope.data, "task status windows response data"); expectExactKeys(data, ["statuses"], "task status windows response data")
  const statuses = expectArray<unknown>(data.statuses, "task status windows").map((entry, index) => {
    const label = `task status windows[${index}]`; const window = expectRecord<Record<string, unknown>>(entry, label); expectExactKeys(window, ["status", "tasks", "page"], label)
    if (!TASK_STATUSES.has(window.status as TaskStatus)) throw new ApiError("invalid_response", `${label}.status is unknown`)
    return { status: window.status as TaskStatus, tasks: expectArray<unknown>(window.tasks, `${label}.tasks`).map((task, taskIndex) => parseApiTask(task, `${label}.tasks[${taskIndex}]`)), page: parseTotalMeta(window.page, `${label}.page`) }
  })
  return { data: { statuses }, meta: parseOffsetMeta(envelope.meta, "task status windows response meta") }
}
