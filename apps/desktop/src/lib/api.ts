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

type Envelope<T> = { data: T; meta?: Record<string, unknown> }
type ErrorEnvelope = { error: { code: string; message: string } }

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

  async health() {
    return this.request<{ ok: boolean; db: string; version: string }>("/health")
  }

  async listBoardColumns() {
    return this.request<BoardColumn[]>(`/api/v1/boards/${this.board}/columns`)
  }

  async listTasks(options: { search?: string; includeArchived?: boolean; statuses?: TaskStatus[] } = {}) {
    const params = new URLSearchParams()
    params.set("include_archived", String(options.includeArchived ?? false))
    params.set("sort", "-updated_at")
    for (const status of options.statuses ?? []) params.append("status", status)
    if (options.search?.trim()) params.set("q", options.search.trim())
    return this.request<Task[]>(`/api/v1/boards/${this.board}/tasks?${params.toString()}`)
  }

  async createTask(input: { title: string; description?: string; status?: TaskStatus }) {
    return this.request<Task>(`/api/v1/boards/${this.board}/tasks`, {
      method: "POST",
      body: {
        title: input.title,
        description: input.description ?? null,
        status: input.status ?? undefined,
        actor: this.actor,
      },
    })
  }

  async updateTask(taskId: string, patch: Partial<Pick<Task, "title" | "description" | "assignee" | "priority" | "due_at" | "scheduled_at">>) {
    return this.request<Task>(`/api/v1/tasks/${taskId}`, {
      method: "PATCH",
      body: { ...patch, actor: this.actor },
    })
  }

  async listDependencies(taskId: string) {
    return this.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies`)
  }

  async addDependency(taskId: string, parentTaskId: string) {
    return this.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies`, {
      method: "POST",
      body: { parent_task_id: parentTaskId, actor: this.actor },
    })
  }

  async removeDependency(taskId: string, parentTaskId: string) {
    return this.request<Dependencies>(`/api/v1/tasks/${taskId}/dependencies/${parentTaskId}`, {
      method: "DELETE",
    })
  }

  async listRuns(taskId: string) {
    return this.request<Run[]>(`/api/v1/tasks/${taskId}/runs`)
  }

  async getRunLog(runId: string) {
    return this.request<RunLog>(`/api/v1/runs/${runId}/log`)
  }

  async listComments(taskId: string) {
    return this.request<CommentRecord[]>(`/api/v1/tasks/${taskId}/comments`)
  }

  async createComment(taskId: string, body: string) {
    return this.request<CommentRecord>(`/api/v1/tasks/${taskId}/comments`, {
      method: "POST",
      body: { author: this.actor, body },
    })
  }

  async listEvents(taskId: string) {
    const params = new URLSearchParams({ board: this.board, task_id: taskId, limit: "50" })
    return this.request<EventRecord[]>(`/api/v1/events?${params.toString()}`)
  }

  async listEventsAfter(after: number) {
    const params = new URLSearchParams({ board: this.board, after: String(after), limit: "100" })
    return this.request<EventRecord[]>(`/api/v1/events?${params.toString()}`)
  }

  async transition(task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "complete" | "submit-review" | "block" | "unblock" | "archive", body: Record<string, unknown> = {}) {
    const payload = { actor: this.actor, ...body }
    return this.request<Task | ClaimResponse>(`/api/v1/tasks/${task.id}/transitions/${action}`, {
      method: "POST",
      body: payload,
    })
  }

  private async request<T>(path: string, init: { method?: string; body?: unknown } = {}) {
    const response = await fetch(`${this.config.apiBaseUrl}${path}`, {
      method: init.method ?? "GET",
      headers: {
        "Content-Type": "application/json",
        "X-KB-Actor": this.actor,
      },
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
    })
    const text = await response.text()
    const json = parseJsonEnvelope<T>(text)
    if (!response.ok || !json || "error" in json) {
      const error = json && "error" in json
        ? json.error
        : { code: "http_error", message: `${response.status} ${response.statusText}`.trim() }
      throw new ApiError(error.code, error.message)
    }
    return json.data
  }
}

function parseJsonEnvelope<T>(text: string): Envelope<T> | ErrorEnvelope | null {
  if (!text) return null
  try {
    return JSON.parse(text) as Envelope<T> | ErrorEnvelope
  } catch {
    return null
  }
}
