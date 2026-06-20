-- Separate whether a mutation requires validation from individual validation outcomes.

BEGIN;

ALTER TABLE label_ontology_actions
  ADD COLUMN validation_requirement TEXT NOT NULL DEFAULT 'none' CHECK(validation_requirement IN (
    'none',
    'required',
    'unsupported'
  ));

UPDATE label_ontology_actions
SET validation_requirement = CASE
  WHEN validation_status = 'pending'
    AND action_type IN ('add_positive_atom', 'add_negative_atom', 'bootstrap_label')
    THEN 'required'
  WHEN validation_status = 'pending'
    AND action_type IN (
      'update_semantics',
      'revert_ontology_mutation',
      'rename_label',
      'split_label',
      'merge_labels'
    )
    THEN 'unsupported'
  ELSE 'none'
END;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (19, '019_label_ontology_validation_requirement', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
