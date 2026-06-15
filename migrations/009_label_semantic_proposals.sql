-- Persistent new-label proposal lifecycle.
-- labels/task_labels/label_semantics/label_atoms remain canonical truth.

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_board_id_id
  ON tasks(board_id, id);

CREATE TABLE IF NOT EXISTS label_semantic_proposals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'lp_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN ('proposed', 'accepted', 'rejected')),

  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  applies_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(applies_when)),
  excludes_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(excludes_when)),
  positive_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(positive_examples)),
  negative_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(negative_examples)),

  heuristic_coverage REAL NOT NULL DEFAULT 0.0 CHECK(heuristic_coverage >= 0.0 AND heuristic_coverage <= 1.0),
  heuristic_residual_norm REAL NOT NULL DEFAULT 1.0 CHECK(heuristic_residual_norm >= 0.0 AND heuristic_residual_norm <= 1.0),
  top1_existing_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
  top1_existing_label_name TEXT,
  diagnostics_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(diagnostics_json)),

  created_by TEXT NOT NULL,
  decision_reason TEXT,
  resolved_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  decided_at INTEGER,

  FOREIGN KEY(board_id, task_id) REFERENCES tasks(board_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_label_semantic_proposals_board_status_created
  ON label_semantic_proposals(board_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_label_semantic_proposals_task_status_created
  ON label_semantic_proposals(task_id, status, created_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (9, '009_label_semantic_proposals', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
