-- Add a narrow task-input hash for label suggestion comparability.
-- Existing observation rows remain nullable and are treated as legacy-incomparable.

BEGIN;

ALTER TABLE label_ontology_observations
  ADD COLUMN suggest_input_hash TEXT;

COMMIT;
