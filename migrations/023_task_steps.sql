PRAGMA foreign_keys = ON;

BEGIN;

CREATE TABLE IF NOT EXISTS task_steps (
  id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  body TEXT,
  linked_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo', 'done', 'skipped')),
  resolution_note TEXT,
  resolved_by TEXT,
  resolved_at INTEGER,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  CHECK(linked_task_id IS NULL OR parent_task_id != linked_task_id)
);

CREATE INDEX IF NOT EXISTS idx_steps_parent_position
  ON task_steps(parent_task_id, position);

CREATE INDEX IF NOT EXISTS idx_steps_linked_task
  ON task_steps(linked_task_id);

CREATE INDEX IF NOT EXISTS idx_steps_board_status
  ON task_steps(board_id, status);

INSERT INTO task_steps(
  id, board_id, parent_task_id, position, title, body, linked_task_id, required,
  status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at
)
SELECT
  'step_' || lower(hex(randomblob(16))),
  s.board_id,
  s.parent_task_id,
  s.position,
  COALESCE(NULLIF(trim(t.title), ''), 'Linked task ' || s.child_task_id),
  t.description,
  s.child_task_id,
  s.required,
  CASE
    WHEN t.status = 'done' THEN 'done'
    WHEN t.status = 'archived' OR t.archived_at IS NOT NULL THEN 'skipped'
    ELSE 'todo'
  END,
  CASE
    WHEN t.status = 'done' THEN 'Migrated from completed subtask ' || s.child_task_id
    WHEN t.status = 'archived' OR t.archived_at IS NOT NULL THEN 'Migrated from archived subtask ' || s.child_task_id
    ELSE NULL
  END,
  CASE
    WHEN t.status IN ('done', 'archived') OR t.archived_at IS NOT NULL THEN s.created_by
    ELSE NULL
  END,
  CASE
    WHEN t.status = 'done' THEN t.completed_at
    WHEN t.status = 'archived' OR t.archived_at IS NOT NULL THEN t.archived_at
    ELSE NULL
  END,
  s.created_by,
  s.created_at,
  s.created_by,
  s.created_at
FROM task_subtasks s
JOIN tasks t ON t.id = s.child_task_id
WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_subtasks')
  AND NOT EXISTS (
    SELECT 1 FROM task_steps existing
    WHERE existing.parent_task_id = s.parent_task_id
      AND existing.linked_task_id = s.child_task_id
  );

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (23, '023_task_steps', '', strftime('%s','now') * 1000);
PRAGMA user_version = 23;

COMMIT;
