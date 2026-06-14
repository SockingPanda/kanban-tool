-- Allow decision comments as first-class comment kind.

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE task_comments_new (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'text' CHECK(kind IN ('text', 'system', 'worker', 'decision')),
  created_at INTEGER NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'human' CHECK(author_type IN ('human', 'agent', 'system')),
  agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL)
);

INSERT INTO task_comments_new (
  id, board_id, task_id, author, body, kind, created_at, author_type, agent_type
)
SELECT
  id, board_id, task_id, author, body, kind, created_at, author_type, agent_type
FROM task_comments;

DROP TABLE task_comments;
ALTER TABLE task_comments_new RENAME TO task_comments;

CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (5, '005_decision_comment_kind', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
