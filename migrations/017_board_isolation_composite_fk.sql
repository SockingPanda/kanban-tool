-- Add schema-level board isolation for key relationship tables.
--
-- Pre-existing cross-board rows are rejected by init.rs before this migration
-- runs so that the failure can name the table and row key. The migration then
-- rebuilds the relationship tables with composite foreign keys that include
-- board_id.

PRAGMA foreign_keys=OFF;

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_id_board
  ON tasks(id, board_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_labels_id_board
  ON labels(id, board_id);

CREATE TABLE task_labels_new (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

INSERT INTO task_labels_new(board_id, task_id, label_id, created_at)
SELECT board_id, task_id, label_id, created_at
FROM task_labels;

DROP TABLE task_labels;
ALTER TABLE task_labels_new RENAME TO task_labels;

CREATE INDEX IF NOT EXISTS idx_task_labels_label
  ON task_labels(label_id, task_id);

CREATE TABLE task_dependencies_new (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  child_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

INSERT INTO task_dependencies_new(board_id, parent_task_id, child_task_id, created_at)
SELECT board_id, parent_task_id, child_task_id, created_at
FROM task_dependencies;

DROP TABLE task_dependencies;
ALTER TABLE task_dependencies_new RENAME TO task_dependencies;

CREATE INDEX IF NOT EXISTS idx_deps_child
  ON task_dependencies(child_task_id);

CREATE INDEX IF NOT EXISTS idx_deps_parent
  ON task_dependencies(parent_task_id);

CREATE TABLE task_runs_new (
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
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),

  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

INSERT INTO task_runs_new(
  id,
  board_id,
  task_id,
  status,
  worker_profile,
  worker_pid,
  claim_token,
  claim_owner,
  claim_expires_at,
  started_at,
  last_heartbeat_at,
  finished_at,
  exit_code,
  summary,
  error,
  log_path,
  metadata_json
)
SELECT
  id,
  board_id,
  task_id,
  status,
  worker_profile,
  worker_pid,
  claim_token,
  claim_owner,
  claim_expires_at,
  started_at,
  last_heartbeat_at,
  finished_at,
  exit_code,
  summary,
  error,
  log_path,
  metadata_json
FROM task_runs;

DROP TABLE task_runs;
ALTER TABLE task_runs_new RENAME TO task_runs;

CREATE INDEX IF NOT EXISTS idx_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_runs_status
  ON task_runs(board_id, status, started_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (17, '017_board_isolation_composite_fk', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;

PRAGMA foreign_keys=ON;
