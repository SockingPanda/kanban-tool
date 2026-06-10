-- Add explicit comment author identity while preserving existing kind values.

ALTER TABLE task_comments
  ADD COLUMN author_type TEXT NOT NULL DEFAULT 'human'
  CHECK(author_type IN ('human', 'agent', 'system'));

ALTER TABLE task_comments
  ADD COLUMN agent_type TEXT;

UPDATE task_comments
SET author_type = CASE kind
  WHEN 'worker' THEN 'agent'
  WHEN 'system' THEN 'system'
  ELSE 'human'
END
WHERE author_type = 'human';

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (3, '003_comment_author_identity', '', CAST(strftime('%s','now') AS INTEGER) * 1000);
