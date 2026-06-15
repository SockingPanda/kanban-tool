-- Board-scoped label atom derived-store state.
-- SQLite label_semantics / label_atoms remain the truth; this table only records
-- which boards still need the rebuildable LanceDB label atom store refreshed.

BEGIN;

CREATE TABLE IF NOT EXISTS label_atom_index_boards (
  store_name TEXT NOT NULL,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  dirty INTEGER NOT NULL DEFAULT 0 CHECK(dirty IN (0, 1)),
  last_rebuild_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(store_name, board_id),
  FOREIGN KEY(store_name) REFERENCES derived_store_state(store_name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_label_atom_index_boards_dirty
  ON label_atom_index_boards(store_name, dirty, board_id);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (8, '008_label_atom_index_boards', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
