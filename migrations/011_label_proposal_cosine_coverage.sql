-- Persist cosine coverage captured when label semantic proposals are created.

BEGIN;

ALTER TABLE label_semantic_proposals
  ADD COLUMN heuristic_coverage_cosine REAL NOT NULL DEFAULT 0.0
  CHECK(heuristic_coverage_cosine >= 0.0 AND heuristic_coverage_cosine <= 1.0);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (11, '011_label_proposal_cosine_coverage', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
