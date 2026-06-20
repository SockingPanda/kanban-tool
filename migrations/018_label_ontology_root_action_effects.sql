-- Add atom effect links for one-root-action ontology mutations.
--
-- Effects are historical snapshots owned by an action. The atom fields are not
-- live foreign keys because removed atoms may no longer exist in label_atoms.

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_actions_id_board
  ON label_ontology_actions(id, board_id);

CREATE TABLE IF NOT EXISTS label_ontology_action_atom_effects (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL,
  label_id_snapshot TEXT NOT NULL CHECK(length(trim(label_id_snapshot)) > 0),
  atom_id_snapshot TEXT NOT NULL CHECK(atom_id_snapshot LIKE 'la_%'),
  atom_content_hash TEXT NOT NULL CHECK(length(trim(atom_content_hash)) > 0),
  polarity TEXT NOT NULL CHECK(polarity IN ('positive', 'negative')),
  kind TEXT NOT NULL CHECK(kind IN (
    'name',
    'description',
    'applies_when',
    'positive_example',
    'excludes_when',
    'negative_example'
  )),
  text TEXT NOT NULL CHECK(length(trim(text)) > 0),
  effect TEXT NOT NULL CHECK(effect IN ('added', 'removed')),
  created_at INTEGER NOT NULL,
  FOREIGN KEY(action_id, board_id) REFERENCES label_ontology_actions(id, board_id) ON DELETE CASCADE,
  UNIQUE(action_id, atom_content_hash, effect)
);

CREATE INDEX IF NOT EXISTS idx_label_ontology_action_atom_effects_hash
  ON label_ontology_action_atom_effects(board_id, atom_content_hash, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_label_ontology_action_atom_effects_label
  ON label_ontology_action_atom_effects(board_id, label_id_snapshot, created_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (18, '018_label_ontology_root_action_effects', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
