-- Stable label atom content hashes.
-- The Rust init backfill rebuilds label_atoms from label_semantics using
-- label_id + polarity + kind + normalized_text, then marks lancedb_label_atoms
-- dirty. This SQL migration is the durable schema/version marker.

BEGIN;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (10, '010_stable_label_atom_hashes', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
