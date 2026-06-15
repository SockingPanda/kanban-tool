-- Collapse comment author/kind semantics and add structured metadata payloads.

PRAGMA foreign_keys = OFF;

BEGIN;

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
  created_at INTEGER NOT NULL
);

INSERT INTO task_comments_new (
  id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at
)
SELECT
  id,
  board_id,
  task_id,
  author,
  CASE
    WHEN author_type = 'agent' OR author_type = 'system' OR kind IN ('worker', 'system') THEN 'agent'
    ELSE 'user'
  END AS author_type,
  CASE
    WHEN author_type = 'agent' OR author_type = 'system' OR kind IN ('worker', 'system') THEN agent_type
    ELSE NULL
  END AS agent_type,
  body,
  'note' AS kind,
  '{}' AS metadata_json,
  created_at
FROM task_comments;

DROP TABLE task_comments;
ALTER TABLE task_comments_new RENAME TO task_comments;

CREATE INDEX IF NOT EXISTS idx_comments_task_created
  ON task_comments(task_id, created_at ASC);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (6, '006_comment_metadata_contract', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
