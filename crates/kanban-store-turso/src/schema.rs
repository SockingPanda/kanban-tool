pub(crate) const CANONICAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL DEFAULT '',
  applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS boards (
  id TEXT PRIMARY KEY CHECK(id LIKE 'b_%'),
  slug TEXT NOT NULL UNIQUE CHECK(length(trim(slug)) > 0),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER
);

CREATE TABLE IF NOT EXISTS board_columns (
  id TEXT PRIMARY KEY CHECK(id LIKE 'col_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  position INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0, 1)),
  wip_limit INTEGER CHECK(wip_limit IS NULL OR wip_limit >= 0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, status),
  UNIQUE(board_id, position)
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY CHECK(id LIKE 't_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,
  idempotency_key TEXT,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,
  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 3 CHECK(priority BETWEEN 0 AND 3),
  position INTEGER NOT NULL DEFAULT 0,
  scheduled_at INTEGER,
  due_at INTEGER,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  started_at INTEGER,
  completed_at INTEGER,
  archived_at INTEGER,
  claim_token TEXT,
  claim_owner TEXT,
  claim_expires_at INTEGER,
  last_heartbeat_at INTEGER,
  current_run_id TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0),
  max_retries INTEGER CHECK(max_retries IS NULL OR max_retries >= 0),
  result_summary TEXT,
  result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  lock_version INTEGER NOT NULL DEFAULT 0 CHECK(lock_version >= 0),
  UNIQUE(board_id, id),
  UNIQUE(id, board_id),
  UNIQUE(board_id, seq),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_idempotency
  ON tasks(board_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_execution_plans (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK(state IN ('unplanned', 'planned', 'not_required')),
  reason TEXT,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(task_id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_steps (
  id TEXT PRIMARY KEY CHECK(id LIKE 'step_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  idempotency_key TEXT,
  position INTEGER NOT NULL,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  body TEXT,
  linked_task_id TEXT,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo', 'done', 'skipped')),
  resolution_note TEXT,
  resolved_by TEXT,
  resolved_at INTEGER,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(parent_task_id, idempotency_key),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(linked_task_id, board_id) REFERENCES tasks(id, board_id),
  CHECK(linked_task_id IS NULL OR parent_task_id != linked_task_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_steps_idempotency
  ON task_steps(parent_task_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL,
  child_task_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('running', 'succeeded', 'failed', 'canceled', 'expired')),
  worker_profile TEXT,
  worker_pid INTEGER,
  claim_token TEXT NOT NULL,
  claim_owner TEXT NOT NULL,
  claim_expires_at INTEGER NOT NULL,
  started_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER,
  finished_at INTEGER,
  exit_code INTEGER,
  summary TEXT,
  error TEXT,
  log_path TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_task_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_runs_one_active
  ON task_runs(task_id)
  WHERE status = 'running';

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  idempotency_key TEXT,
  author TEXT NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
  agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL),
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision')),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  UNIQUE(task_id, idempotency_key)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_comments_idempotency
  ON task_comments(task_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  run_id TEXT,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id),
  FOREIGN KEY(run_id, board_id) REFERENCES task_runs(id, board_id)
);

CREATE INDEX IF NOT EXISTS idx_task_events_board_created
  ON task_events(board_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_task_events_task_created
  ON task_events(task_id, id DESC);
"#;

pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const SCHEMA_NAME: &str = "001_canonical_baseline";

pub(crate) const DEFAULT_COLUMNS: [(&str, &str, i64, bool); 9] = [
    ("triage", "Triage", 10, false),
    ("todo", "Todo", 20, false),
    ("scheduled", "Scheduled", 30, false),
    ("ready", "Ready", 40, false),
    ("running", "Running", 50, false),
    ("blocked", "Blocked", 60, false),
    ("review", "Review", 70, false),
    ("done", "Done", 80, false),
    ("archived", "Archived", 90, true),
];
