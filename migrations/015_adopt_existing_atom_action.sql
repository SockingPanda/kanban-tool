-- Add a provenance-only action type for signals that adopt an already-present atom.

PRAGMA foreign_keys=OFF;

BEGIN;

CREATE TABLE label_ontology_actions_new (
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
    'adopt_existing_atom',
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

INSERT INTO label_ontology_actions_new(
  id,
  board_id,
  parent_action_id,
  action_type,
  reason,
  target_label_id,
  result_label_id,
  result_atom_id,
  result_atom_content_hash,
  result_proposal_id,
  canonical_before_hash,
  canonical_after_hash,
  change_json,
  validation_status,
  validation_json,
  created_by,
  created_by_type,
  agent_type,
  created_at
)
SELECT
  id,
  board_id,
  parent_action_id,
  action_type,
  reason,
  target_label_id,
  result_label_id,
  result_atom_id,
  result_atom_content_hash,
  result_proposal_id,
  canonical_before_hash,
  canonical_after_hash,
  change_json,
  validation_status,
  validation_json,
  created_by,
  created_by_type,
  agent_type,
  created_at
FROM label_ontology_actions;

DROP TABLE label_ontology_actions;
ALTER TABLE label_ontology_actions_new RENAME TO label_ontology_actions;

CREATE INDEX IF NOT EXISTS idx_label_ontology_actions_board_type_created
  ON label_ontology_actions(board_id, action_type, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_label_ontology_actions_label_created
  ON label_ontology_actions(board_id, target_label_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_actions_unique_create_proposal
  ON label_ontology_actions(board_id, result_proposal_id)
  WHERE action_type = 'create_label_proposal'
    AND result_proposal_id IS NOT NULL;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (15, '015_adopt_existing_atom_action', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;

PRAGMA foreign_keys=ON;
