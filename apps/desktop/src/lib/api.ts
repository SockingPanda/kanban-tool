import { invoke } from "@tauri-apps/api/core"

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
  dbPath: string
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
  result_json: string | null
  metadata_json: string
  lock_version: number
  dependency_blocked: boolean
  unfinished_parent_count: number
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

export type Run = {
  id: string
  task_id: string
  status: string
  worker_profile: string | null
  worker_pid: number | null
  claim_owner: string
  started_at: number
  finished_at: number | null
  exit_code: number | null
  summary: string | null
  error: string | null
  log_path: string | null
  metadata_json: string
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
  kind: "note" | "decision"
  metadata_json: string
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

export type EventMeta = {
  next_after?: number
}

export type EventPage = {
  events: EventRecord[]
  meta: EventMeta
}

type ErrorEnvelope = { error: { code: string; message: string } }

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

export class ApiError extends Error {
  constructor(
    public code: string,
    message: string,
  ) {
    super(message)
  }
}

export async function loadRuntimeConfig(): Promise<RuntimeConfig> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return invoke<RuntimeConfig>("runtime_config")
  }
  const apiBaseUrl = normalizeApiBaseUrl(import.meta.env.VITE_KB_API_BASE_URL)
  if (!apiBaseUrl) {
    throw new Error(
      "VITE_KB_API_BASE_URL is required outside Tauri; set it to an explicit API origin or an explicit Vite proxy base such as /__kb_api__.",
    )
  }
  return {
    apiBaseUrl,
    dbPath: import.meta.env.VITE_KB_DB_PATH?.trim() || "external API",
    actor: import.meta.env.VITE_KB_ACTOR ?? "desktop-dev",
    board: import.meta.env.VITE_KB_BOARD ?? "default",
  }
}

function normalizeApiBaseUrl(value: string | undefined) {
  const trimmed = value?.trim()
  if (!trimmed) return ""
  return trimmed.length > 1 ? trimmed.replace(/\/+$/, "") : trimmed
}

export class KanbanApi {
  constructor(private config: RuntimeConfig) {}

  get actor() {
    return this.config.actor
  }

  get board() {
    return this.config.board
  }

  get dbPath() {
    return this.config.dbPath
  }

  async health(options: RequestOptions = {}) {
    return this.request<HealthStatus>("/health", options)
  }

  async listBoards(options: BoardListOptions = {}) {
    const params = new URLSearchParams({
      include_archived: String(options.includeArchived ?? false),
    })
    const boards = await this.request<Board[]>(`/api/v1/boards?${params.toString()}`, {
      signal: options.signal,
    })
    return expectArray<Board>(boards, "boards response data")
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
    return this.request<DoctorReport>("/api/v1/maintenance/doctor", {
      method: "POST",
      body: { board: this.board, actor: this.actor },
      signal: options.signal,
    })
  }

  async checkpoint(options: RequestOptions = {}) {
    return this.request<CheckpointReport>("/api/v1/maintenance/checkpoint", {
      method: "POST",
      body: { board: this.board, actor: this.actor },
      signal: options.signal,
    })
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
    const envelope = await this.requestEnvelope<Task[], PageEnvelopeMeta>(
      `/api/v1/boards/${this.board}/tasks?${params.toString()}`,
      { signal: options.signal },
    )
    const tasks = expectArray<Task>(envelope.data, "tasks response data")
    return {
      tasks,
      page: normalizePageMeta(envelope.meta, { limit, offset }),
    } satisfies TaskPageResult
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

  async createTask(input: { title: string; description?: string; status?: TaskStatus }, options: RequestOptions = {}) {
    return this.request<Task>(`/api/v1/boards/${this.board}/tasks`, {
      method: "POST",
      body: {
        title: input.title,
        description: input.description ?? null,
        status: input.status ?? undefined,
        actor: this.actor,
      },
      signal: options.signal,
    })
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

  async listRuns(taskId: string, options: RequestOptions = {}) {
    return this.request<Run[]>(`/api/v1/tasks/${taskId}/runs`, options)
  }

  async getRunLog(runId: string, options: RequestOptions = {}) {
    return this.request<RunLog>(`/api/v1/runs/${runId}/log`, options)
  }

  async listComments(taskId: string, options: RequestOptions = {}) {
    return this.request<CommentRecord[]>(`/api/v1/tasks/${taskId}/comments`, options)
  }

  async createComment(taskId: string, body: string, options: RequestOptions = {}) {
    return this.request<CommentRecord>(`/api/v1/tasks/${taskId}/comments`, {
      method: "POST",
      body: { author: this.actor, body },
      signal: options.signal,
    })
  }

  async addTaskLabel(taskId: string, name: string, options: RequestOptions = {}) {
    return this.request<Task>(`/api/v1/tasks/${taskId}/labels`, {
      method: "POST",
      body: { name, actor: this.actor },
      signal: options.signal,
    })
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
    return this.request<LabelSuggestionResult>(
      `/api/v1/tasks/${taskId}/labels/suggestions?${params.toString()}`,
      {
        signal: options.signal,
      },
    )
  }

  async removeTaskLabel(taskId: string, labelId: string, options: RequestOptions = {}) {
    return this.request<Task>(`/api/v1/tasks/${taskId}/labels/${labelId}`, {
      method: "DELETE",
      signal: options.signal,
    })
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

  async transition(task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "complete" | "submit-review" | "block" | "unblock" | "archive", body: Record<string, unknown> = {}, options: RequestOptions = {}) {
    const payload = { actor: this.actor, ...body }
    return this.request<Task | ClaimResponse>(`/api/v1/tasks/${task.id}/transitions/${action}`, {
      method: "POST",
      body: payload,
      signal: options.signal,
    })
  }

  async requestEnvelope<T, M = Record<string, unknown>>(path: string, init: RequestOptions = {}) {
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, {
      method: init.method ?? "GET",
      headers: {
        "Content-Type": "application/json",
        "X-KB-Actor": this.actor,
      },
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
}

type RequestOptions = {
  method?: string
  body?: unknown
  signal?: AbortSignal
}

type TaskListOptions = {
  includeArchived?: boolean
  statuses?: TaskStatus[]
  priorities?: number[]
  labels?: string[]
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

type PageEnvelopeMeta = Partial<PageMeta>

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
