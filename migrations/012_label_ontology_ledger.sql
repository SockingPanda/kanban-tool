-- Label ontology observation/signal/action ledger.
-- This records evidence and review history; labels/semantics/proposals remain canonical truth.

BEGIN;

CREATE TABLE IF NOT EXISTS label_ontology_observations (
  id TEXT PRIMARY KEY CHECK(id LIKE 'lor_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  task_ref_snapshot TEXT NOT NULL CHECK(length(trim(task_ref_snapshot)) > 0),
  task_snapshot_json TEXT NOT NULL CHECK(json_valid(task_snapshot_json)),
  agent_candidates_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(agent_candidates_json)),
  suggestion_snapshot_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(suggestion_snapshot_json)),
  final_decision_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(final_decision_json)),
  suggest_coverage REAL,
  suggest_coverage_cosine REAL,
  suggest_residual_norm REAL,
  suggest_needs_new_label INTEGER NOT NULL DEFAULT 0 CHECK(suggest_needs_new_label IN (0, 1)),
  suggest_degraded INTEGER NOT NULL DEFAULT 0 CHECK(suggest_degraded IN (0, 1)),
  diagnostics_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(diagnostics_json)),
  capture_fingerprint TEXT NOT NULL CHECK(length(trim(capture_fingerprint)) > 0),
  created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
  created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
  agent_type TEXT,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(board_id, task_id) REFERENCES tasks(board_id, id) ON DELETE CASCADE,
  UNIQUE(board_id, capture_fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_label_ontology_observations_task_created
  ON label_ontology_observations(board_id, task_id, created_at DESC);

CREATE TABLE IF NOT EXISTS label_ontology_signals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'los_%'),
  observation_id TEXT NOT NULL REFERENCES label_ontology_observations(id) ON DELETE CASCADE,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN (
    'false_negative',
    'false_positive',
    'vocabulary_gap',
    'name_issue',
    'boundary_issue',
    'structure_issue'
  )),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN (
    'open',
    'confirmed',
    'resolved',
    'rejected',
    'superseded'
  )),
  target_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
  target_label_name_snapshot TEXT,
  related_labels_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(related_labels_json)),
  proposed_action TEXT NOT NULL CHECK(proposed_action IN (
    'observe',
    'add_positive_atom',
    'add_negative_atom',
    'update_semantics',
    'bootstrap_label',
    'rename_label',
    'split_label',
    'merge_labels'
  )),
  candidate_atom_polarity TEXT CHECK(candidate_atom_polarity IS NULL OR candidate_atom_polarity IN ('positive', 'negative')),
  candidate_atom_kind TEXT CHECK(candidate_atom_kind IS NULL OR candidate_atom_kind IN (
    'applies_when',
    'positive_example',
    'excludes_when',
    'negative_example'
  )),
  candidate_text TEXT,
  candidate_content_hash TEXT,
  proposed_label_name TEXT,
  proposed_label_name_normalized TEXT,
  proposal_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(proposal_json)),
  agent_selected INTEGER NOT NULL DEFAULT 0 CHECK(agent_selected IN (0, 1)),
  suggest_state TEXT CHECK(suggest_state IS NULL OR suggest_state IN (
    'selected',
    'candidate',
    'absent',
    'unavailable'
  )),
  suggest_score REAL,
  suggest_rank INTEGER,
  final_selected INTEGER NOT NULL DEFAULT 0 CHECK(final_selected IN (0, 1)),
  rationale TEXT NOT NULL CHECK(length(trim(rationale)) > 0),
  confidence REAL CHECK(confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  signal_key TEXT NOT NULL CHECK(length(trim(signal_key)) > 0),
  superseded_by_signal_id TEXT REFERENCES label_ontology_signals(id) ON DELETE SET NULL,
  status_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  reviewed_at INTEGER,
  closed_at INTEGER,
  UNIQUE(observation_id, signal_key)
);

CREATE INDEX IF NOT EXISTS idx_label_ontology_signals_unresolved
  ON label_ontology_signals(board_id, status, created_at ASC)
  WHERE status IN ('open', 'confirmed');

CREATE INDEX IF NOT EXISTS idx_label_ontology_signals_label_kind
  ON label_ontology_signals(board_id, target_label_id, kind, status);

CREATE INDEX IF NOT EXISTS idx_label_ontology_signals_candidate_atom
  ON label_ontology_signals(board_id, candidate_content_hash, status)
  WHERE candidate_content_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_label_ontology_signals_proposed_label
  ON label_ontology_signals(board_id, proposed_label_name_normalized, status)
  WHERE proposed_label_name_normalized IS NOT NULL;

CREATE TABLE IF NOT EXISTS label_ontology_actions (
  id TEXT PRIMARY KEY CHECK(id LIKE 'loa_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_action_id TEXT REFERENCES label_ontology_actions(id) ON DELETE SET NULL,
  action_type TEXT NOT NULL CHECK(action_type IN (
    'confirm',
    'reject',
    'supersede',
    'resolve_no_change',
    'add_positive_atom',
    'add_negative_atom',
    'update_semantics',
    'create_label_proposal',
    'bootstrap_label',
    'rename_label',
    'split_label',
    'merge_labels',
    'validate'
  )),
  reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
  target_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
  result_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
  result_atom_id TEXT,
  result_atom_content_hash TEXT,
  result_proposal_id TEXT REFERENCES label_semantic_proposals(id) ON DELETE SET NULL,
  canonical_before_hash TEXT,
  canonical_after_hash TEXT,
  change_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(change_json)),
  validation_status TEXT NOT NULL DEFAULT 'not_required' CHECK(validation_status IN (
    'not_required',
    'pending',
    'passed',
    'failed',
    'partial'
  )),
  validation_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(validation_json)),
  created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
  created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
  agent_type TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_label_ontology_actions_board_type_created
  ON label_ontology_actions(board_id, action_type, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_label_ontology_actions_label_created
  ON label_ontology_actions(board_id, target_label_id, created_at DESC);

CREATE TABLE IF NOT EXISTS label_ontology_action_signals (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL REFERENCES label_ontology_actions(id) ON DELETE CASCADE,
  signal_id TEXT NOT NULL REFERENCES label_ontology_signals(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(action_id, signal_id)
);

CREATE INDEX IF NOT EXISTS idx_label_ontology_action_signals_signal
  ON label_ontology_action_signals(signal_id, action_id);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (12, '012_label_ontology_ledger', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
