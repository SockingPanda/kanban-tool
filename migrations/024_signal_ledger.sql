-- Add generic Agent/Product signal ledger and signal backlink comments.

BEGIN;

DROP TABLE IF EXISTS task_comments_new;
CREATE TABLE task_comments_new (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author TEXT NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
  agent_type TEXT,
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision', 'signal')),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  CHECK((author_type = 'agent') OR agent_type IS NULL)
);
INSERT INTO task_comments_new(id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at)
SELECT id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments;
DROP TABLE task_comments;
ALTER TABLE task_comments_new RENAME TO task_comments;
CREATE INDEX IF NOT EXISTS idx_comments_task_created ON task_comments(task_id, created_at ASC);

CREATE TABLE signal_observations (
  id TEXT PRIMARY KEY CHECK(id LIKE 'obs_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  task_ref_snapshot TEXT,
  run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
  comment_id TEXT REFERENCES task_comments(id) ON DELETE SET NULL,
  actor TEXT NOT NULL CHECK(length(trim(actor)) > 0),
  agent_type TEXT,
  source TEXT,
  evidence_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(evidence_json) AND json_type(evidence_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id)
);

CREATE TABLE signals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'sig_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  observation_id TEXT NOT NULL REFERENCES signal_observations(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  summary TEXT NOT NULL CHECK(length(trim(summary)) > 0),
  severity TEXT NOT NULL DEFAULT 'info' CHECK(length(trim(severity)) > 0),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'confirmed', 'rejected', 'superseded', 'resolved')),
  dedupe_key TEXT,
  superseded_by_signal_id TEXT REFERENCES signals(id),
  reviewed_by TEXT,
  reviewed_at INTEGER,
  review_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  UNIQUE(observation_id, board_id),
  CHECK(id != superseded_by_signal_id)
);

CREATE INDEX idx_signal_observations_board_created ON signal_observations(board_id, created_at DESC);
CREATE INDEX idx_signal_observations_task_created ON signal_observations(task_id, created_at DESC);
CREATE INDEX idx_signals_board_status_created ON signals(board_id, status, created_at DESC);
CREATE INDEX idx_signals_observation ON signals(observation_id);
CREATE INDEX idx_signals_dedupe_key ON signals(board_id, dedupe_key) WHERE dedupe_key IS NOT NULL;

INSERT INTO schema_migrations(version, name, checksum, applied_at)
VALUES (24, '024_signal_ledger', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
