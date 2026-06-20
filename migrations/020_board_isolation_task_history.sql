-- Add schema-level board isolation for task history tables.
--
-- Comments and attachments are strict task-owned rows, so they use
-- board-scoped composite foreign keys. Events retain nullable task/run refs
-- with ON DELETE SET NULL; triggers enforce board scope when refs are present.

PRAGMA foreign_keys=OFF;

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_runs_id_board
  ON task_runs(id, board_id);

DROP INDEX IF EXISTS idx_comments_task_created;

CREATE TABLE task_comments_new (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
  agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL),
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision')),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

INSERT INTO task_comments_new(
  id,
  board_id,
  task_id,
  author,
  author_type,
  agent_type,
  body,
  kind,
  metadata_json,
  created_at
)
SELECT
  id,
  board_id,
  task_id,
  author,
  author_type,
  agent_type,
  body,
  kind,
  metadata_json,
  created_at
FROM task_comments;

DROP TABLE task_comments;
ALTER TABLE task_comments_new RENAME TO task_comments;

CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

CREATE TABLE task_attachments_new (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

INSERT INTO task_attachments_new(
  id,
  board_id,
  task_id,
  filename,
  rel_path,
  content_type,
  size_bytes,
  sha256,
  created_by,
  created_at
)
SELECT
  id,
  board_id,
  task_id,
  filename,
  rel_path,
  content_type,
  size_bytes,
  sha256,
  created_by,
  created_at
FROM task_attachments;

DROP TABLE task_attachments;
ALTER TABLE task_attachments_new RENAME TO task_attachments;

DROP TRIGGER IF EXISTS trg_task_events_board_insert;
DROP TRIGGER IF EXISTS trg_task_events_board_update;

CREATE TRIGGER trg_task_events_board_insert
BEFORE INSERT ON task_events
BEGIN
  SELECT RAISE(ABORT, 'task_events.board_id must match task_id board_id')
    WHERE NEW.task_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM tasks
        WHERE id = NEW.task_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'task_events.board_id must match run_id board_id')
    WHERE NEW.run_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM task_runs
        WHERE id = NEW.run_id
          AND board_id = NEW.board_id
      );
END;

CREATE TRIGGER trg_task_events_board_update
BEFORE UPDATE OF board_id, task_id, run_id ON task_events
BEGIN
  SELECT RAISE(ABORT, 'task_events.board_id must match task_id board_id')
    WHERE NEW.task_id IS NOT NULL
      AND (NEW.board_id IS NOT OLD.board_id OR NEW.task_id IS NOT OLD.task_id)
      AND NOT EXISTS (
        SELECT 1 FROM tasks
        WHERE id = NEW.task_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'task_events.board_id must match run_id board_id')
    WHERE NEW.run_id IS NOT NULL
      AND (NEW.board_id IS NOT OLD.board_id OR NEW.run_id IS NOT OLD.run_id)
      AND NOT EXISTS (
        SELECT 1 FROM task_runs
        WHERE id = NEW.run_id
          AND board_id = NEW.board_id
      );
END;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (20, '020_board_isolation_task_history', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;

PRAGMA foreign_keys=ON;
