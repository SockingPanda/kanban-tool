-- Generic agent/product signal ledger.
--
-- This ledger is intentionally separate from label_ontology_signals. Label
-- ontology signals remain domain-specific provenance for ontology review, while
-- these tables capture general operator-visible signals such as CLI friction.

PRAGMA foreign_keys = ON;

BEGIN;

CREATE TABLE IF NOT EXISTS signal_observations (
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

CREATE TABLE IF NOT EXISTS signals (
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
  CHECK(id != superseded_by_signal_id),
  FOREIGN KEY(observation_id, board_id) REFERENCES signal_observations(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(superseded_by_signal_id, board_id) REFERENCES signals(id, board_id)
);

CREATE INDEX IF NOT EXISTS idx_signal_observations_board_created
  ON signal_observations(board_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signal_observations_task
  ON signal_observations(board_id, task_id, created_at DESC)
  WHERE task_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_signals_board_status_created
  ON signals(board_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signals_observation
  ON signals(observation_id);

CREATE INDEX IF NOT EXISTS idx_signals_dedupe_key
  ON signals(board_id, dedupe_key)
  WHERE dedupe_key IS NOT NULL;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (24, '024_generic_signal_ledger', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
