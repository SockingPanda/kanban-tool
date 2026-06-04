-- Kanban Tool initial SQLite schema
-- Time convention: INTEGER unix epoch milliseconds UTC.
-- JSON convention: TEXT with CHECK(json_valid(...)).

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;

BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
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

  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,

  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 0,
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

  UNIQUE(board_id, seq),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  child_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id)
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

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
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json))
);

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'text' CHECK(kind IN ('text', 'system', 'worker')),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_attachments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
  id TEXT PRIMARY KEY CHECK(id LIKE 'l_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, name)
);

CREATE TABLE IF NOT EXISTS task_labels (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id)
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK(json_valid(value_json)),
  updated_at INTEGER NOT NULL
);

-- Indexes: tasks
CREATE INDEX IF NOT EXISTS idx_tasks_board_status_position
  ON tasks(board_id, status, position);

CREATE INDEX IF NOT EXISTS idx_tasks_board_priority_created
  ON tasks(board_id, priority DESC, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_tasks_assignee_status
  ON tasks(board_id, assignee, status);

CREATE INDEX IF NOT EXISTS idx_tasks_scheduled
  ON tasks(board_id, status, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_tasks_claim_expiry
  ON tasks(board_id, status, claim_expires_at);

CREATE INDEX IF NOT EXISTS idx_tasks_updated
  ON tasks(board_id, updated_at DESC);

-- Indexes: dependencies
CREATE INDEX IF NOT EXISTS idx_deps_child
  ON task_dependencies(child_task_id);

CREATE INDEX IF NOT EXISTS idx_deps_parent
  ON task_dependencies(parent_task_id);

-- Indexes: runs
CREATE INDEX IF NOT EXISTS idx_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_runs_status
  ON task_runs(board_id, status, started_at DESC);

-- Indexes: comments
CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

-- Indexes: events
CREATE INDEX IF NOT EXISTS idx_events_board_id
  ON task_events(board_id, id ASC);

CREATE INDEX IF NOT EXISTS idx_events_task_created
  ON task_events(task_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_events_kind_created
  ON task_events(kind, created_at DESC);

-- Indexes: labels
CREATE INDEX IF NOT EXISTS idx_task_labels_label
  ON task_labels(label_id, task_id);

INSERT OR IGNORE INTO schema_migrations(version, name, applied_at)
VALUES (1, '001_initial', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
