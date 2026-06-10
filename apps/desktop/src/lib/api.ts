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
  body: string
  kind: "text" | "system" | "worker"
  created_at: number
}

export type RunLog = {
  run_id: string
  content: string
  truncated: boolean
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

export type SearchMeta = {
  backend: string
  stale: boolean
  index_version: string | null
  last_event_id: number | null
  index_lag_events: number | null
}

export type ApiEnvelope<T, M = Record<string, unknown>> = { data: T; meta?: M }

export type PageMeta = {
  limit: number
  offset: number
  total: number
}

export type TaskPageResult = {
  tasks: Task[]
  page: PageMeta
}

export type SearchTasksResult = {
  tasks: Task[]
  searchMeta: SearchMeta
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
  meta: SearchMeta
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
  return {
    apiBaseUrl: import.meta.env.VITE_KB_API_BASE_URL ?? "",
    dbPath: "external API",
    actor: "desktop-dev",
    board: "default",
  }
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
    return this.request<{ ok: boolean; db: string; version: string }>("/health", options)
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
    params.set("sort", "-updated_at")
    for (const status of options.statuses ?? []) params.append("status", status)
    const envelope = await this.requestEnvelope<Task[], PageEnvelopeMeta>(
      `/api/v1/boards/${this.board}/tasks?${params.toString()}`,
      { signal: options.signal },
    )
    return {
      tasks: envelope.data,
      page: normalizePageMeta(envelope.meta, envelope.data.length, { limit, offset }),
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
    const envelope = await this.requestEnvelope<SearchTasksResponse, PageEnvelopeMeta>(
      `/api/v1/search/tasks?${params.toString()}`,
      { signal: options.signal },
    )
    return {
      tasks: envelope.data.hits.map((hit) => hit.task),
      searchMeta: envelope.data.meta,
      page: normalizePageMeta(envelope.meta, envelope.data.hits.length, { limit, offset }),
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

  async listEvents(taskId: string, options: RequestOptions = {}) {
    const params = new URLSearchParams({ board: this.board, task_id: taskId, limit: "50" })
    const envelope = await this.requestEnvelope<EventRecord[], EventMeta>(
      `/api/v1/events?${params.toString()}`,
      options,
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
  limit?: number
  offset?: number
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

function normalizePageMeta(meta: PageEnvelopeMeta | undefined, count: number, fallback: { limit: number; offset: number }): PageMeta {
  const limit = numericMeta(meta?.limit, fallback.limit)
  const offset = numericMeta(meta?.offset, fallback.offset)
  const total = numericMeta(meta?.total, offset + count)
  return { limit, offset, total }
}

function numericMeta(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}
