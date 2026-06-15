-- Label semantics and atom truth.
-- labels remains the canonical label identity; task_labels remains final task binding.

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_labels_id_board
  ON labels(id, board_id);

CREATE TABLE IF NOT EXISTS label_semantics (
  label_id TEXT PRIMARY KEY REFERENCES labels(id) ON DELETE CASCADE,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  description TEXT,
  applies_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(applies_when)),
  excludes_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(excludes_when)),
  positive_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(positive_examples)),
  negative_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(negative_examples)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_label_semantics_board_updated
  ON label_semantics(board_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS label_atoms (
  id TEXT PRIMARY KEY CHECK(id LIKE 'la_%'),
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
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
  ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
  content_hash TEXT NOT NULL CHECK(length(trim(content_hash)) > 0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE,
  UNIQUE(label_id, polarity, kind, ordinal),
  UNIQUE(label_id, content_hash)
);

CREATE INDEX IF NOT EXISTS idx_label_atoms_board_polarity_kind
  ON label_atoms(board_id, polarity, kind, ordinal);

CREATE INDEX IF NOT EXISTS idx_label_atoms_label_ordinal
  ON label_atoms(label_id, ordinal);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (7, '007_label_semantics_atoms', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
