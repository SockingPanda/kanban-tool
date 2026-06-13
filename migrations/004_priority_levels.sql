-- Constrain task priority to P0..P3. P0 is highest; P3 is lowest/default.

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE tasks_new (
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

  UNIQUE(board_id, seq),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

INSERT INTO tasks_new (
  id, board_id, seq, title, description, status, status_reason, assignee,
  priority, position, scheduled_at, due_at, created_by, created_at, updated_at,
  started_at, completed_at, archived_at, claim_token, claim_owner,
  claim_expires_at, last_heartbeat_at, current_run_id, retry_count,
  max_retries, result_summary, result_json, metadata_json, lock_version
)
SELECT
  id, board_id, seq, title, description, status, status_reason, assignee,
  CASE
    WHEN priority <= 0 THEN 0
    WHEN priority IN (1, 2, 3) THEN priority
    ELSE 3
  END,
  position, scheduled_at, due_at, created_by, created_at, updated_at,
  started_at, completed_at, archived_at, claim_token, claim_owner,
  claim_expires_at, last_heartbeat_at, current_run_id, retry_count,
  max_retries, result_summary, result_json, metadata_json, lock_version
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX IF NOT EXISTS idx_tasks_board_status_position
  ON tasks(board_id, status, position);

CREATE INDEX IF NOT EXISTS idx_tasks_board_priority_created
  ON tasks(board_id, priority ASC, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_tasks_assignee_status
  ON tasks(board_id, assignee, status);

CREATE INDEX IF NOT EXISTS idx_tasks_scheduled
  ON tasks(board_id, status, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_tasks_claim_expiry
  ON tasks(board_id, status, claim_expires_at);

CREATE INDEX IF NOT EXISTS idx_tasks_updated
  ON tasks(board_id, updated_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (4, '004_priority_levels', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
