-- Add explicit comment author identity while preserving existing kind values.

BEGIN;

ALTER TABLE task_comments
  ADD COLUMN author_type TEXT NOT NULL DEFAULT 'human'
  CHECK(author_type IN ('human', 'agent', 'system'));

UPDATE task_comments
SET author_type = CASE kind
  WHEN 'worker' THEN 'agent'
  WHEN 'system' THEN 'system'
  ELSE 'human'
END
WHERE author_type = 'human';

ALTER TABLE task_comments
  ADD COLUMN agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (3, '003_comment_author_identity', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
