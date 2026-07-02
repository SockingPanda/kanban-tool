-- Strengthen generic signal ledger board isolation and query indexes.

PRAGMA foreign_keys = ON;

BEGIN;
PRAGMA defer_foreign_keys = ON;

DROP INDEX IF EXISTS idx_signals_observation;
DROP INDEX IF EXISTS idx_signals_board_status_created;
DROP INDEX IF EXISTS idx_signals_dedupe_key;

DROP TABLE IF EXISTS signals_new;
CREATE TABLE signals_new (
  id TEXT PRIMARY KEY CHECK(id LIKE 'sig_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  observation_id TEXT NOT NULL REFERENCES signal_observations(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  summary TEXT NOT NULL CHECK(length(trim(summary)) > 0),
  severity TEXT NOT NULL DEFAULT 'info' CHECK(length(trim(severity)) > 0),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'confirmed', 'rejected', 'superseded', 'resolved')),
  dedupe_key TEXT,
  superseded_by_signal_id TEXT REFERENCES signals_new(id),
  reviewed_by TEXT,
  reviewed_at INTEGER,
  review_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  UNIQUE(observation_id, board_id),
  CHECK(id != superseded_by_signal_id),
  FOREIGN KEY(observation_id, board_id) REFERENCES signal_observations(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(superseded_by_signal_id, board_id) REFERENCES signals_new(id, board_id)
);

INSERT INTO signals_new(
  id,
  board_id,
  observation_id,
  kind,
  title,
  summary,
  severity,
  status,
  dedupe_key,
  superseded_by_signal_id,
  reviewed_by,
  reviewed_at,
  review_reason,
  created_at,
  updated_at
)
SELECT
  id,
  board_id,
  observation_id,
  kind,
  title,
  summary,
  severity,
  status,
  dedupe_key,
  superseded_by_signal_id,
  reviewed_by,
  reviewed_at,
  review_reason,
  created_at,
  updated_at
FROM signals;

DROP TABLE signals;
ALTER TABLE signals_new RENAME TO signals;

DROP INDEX IF EXISTS idx_signal_observations_task_created;
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

INSERT INTO schema_migrations(version, name, checksum, applied_at)
VALUES (25, '025_generic_signal_ledger', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
