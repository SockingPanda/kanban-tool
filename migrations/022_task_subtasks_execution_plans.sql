PRAGMA foreign_keys = ON;

BEGIN;

CREATE TABLE IF NOT EXISTS task_subtasks (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  child_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subtasks_parent_position
  ON task_subtasks(parent_task_id, position);

CREATE INDEX IF NOT EXISTS idx_subtasks_child
  ON task_subtasks(child_task_id);

CREATE TABLE IF NOT EXISTS task_execution_plans (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK(state IN ('unplanned', 'planned', 'not_required')),
  reason TEXT,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_execution_plans_board_state
  ON task_execution_plans(board_id, state);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (22, '022_task_subtasks_execution_plans', '', strftime('%s','now') * 1000);
PRAGMA user_version = 22;

COMMIT;
