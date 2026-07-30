-- Persist the corpus/model/dimension identity carried by LanceDB Projection v2
-- artifacts. Existing v29 generations intentionally remain unbound: migration
-- must not fabricate evidence that was never durably recorded.

BEGIN;

ALTER TABLE projection_store_state
  ADD COLUMN active_corpus_schema TEXT
  CHECK(active_corpus_schema IS NULL OR length(trim(active_corpus_schema)) > 0);
ALTER TABLE projection_store_state
  ADD COLUMN active_corpus_fingerprint TEXT
  CHECK(
    active_corpus_fingerprint IS NULL
    OR length(trim(active_corpus_fingerprint)) > 0
  );
ALTER TABLE projection_store_state
  ADD COLUMN active_embedding_model TEXT
  CHECK(active_embedding_model IS NULL OR length(trim(active_embedding_model)) > 0);
ALTER TABLE projection_store_state
  ADD COLUMN active_embedding_dimensions INTEGER
  CHECK(
    (
      active_corpus_schema IS NULL
      AND active_corpus_fingerprint IS NULL
      AND active_embedding_model IS NULL
      AND active_embedding_dimensions IS NULL
    )
    OR
    (
      (
        (store_name='lancedb_chunks' AND active_corpus_schema='task-chunks-v2')
        OR
        (
          store_name='lancedb_label_atoms'
          AND active_corpus_schema='label-atoms-v2'
        )
      )
      AND active_corpus_schema IS NOT NULL
      AND length(trim(active_corpus_schema)) > 0
      AND active_corpus_fingerprint IS NOT NULL
      AND length(trim(active_corpus_fingerprint)) > 0
      AND active_embedding_model IS NOT NULL
      AND length(trim(active_embedding_model)) > 0
      AND active_embedding_dimensions IS NOT NULL
      AND active_embedding_dimensions > 0
    )
  );

ALTER TABLE projection_store_state
  ADD COLUMN previous_corpus_schema TEXT
  CHECK(previous_corpus_schema IS NULL OR length(trim(previous_corpus_schema)) > 0);
ALTER TABLE projection_store_state
  ADD COLUMN previous_corpus_fingerprint TEXT
  CHECK(
    previous_corpus_fingerprint IS NULL
    OR length(trim(previous_corpus_fingerprint)) > 0
  );
ALTER TABLE projection_store_state
  ADD COLUMN previous_embedding_model TEXT
  CHECK(
    previous_embedding_model IS NULL
    OR length(trim(previous_embedding_model)) > 0
  );
ALTER TABLE projection_store_state
  ADD COLUMN previous_embedding_dimensions INTEGER
  CHECK(
    (
      previous_corpus_schema IS NULL
      AND previous_corpus_fingerprint IS NULL
      AND previous_embedding_model IS NULL
      AND previous_embedding_dimensions IS NULL
    )
    OR
    (
      (
        (
          store_name='lancedb_chunks'
          AND previous_corpus_schema='task-chunks-v2'
        )
        OR
        (
          store_name='lancedb_label_atoms'
          AND previous_corpus_schema='label-atoms-v2'
        )
      )
      AND previous_corpus_schema IS NOT NULL
      AND length(trim(previous_corpus_schema)) > 0
      AND previous_corpus_fingerprint IS NOT NULL
      AND length(trim(previous_corpus_fingerprint)) > 0
      AND previous_embedding_model IS NOT NULL
      AND length(trim(previous_embedding_model)) > 0
      AND previous_embedding_dimensions IS NOT NULL
      AND previous_embedding_dimensions > 0
    )
  );

ALTER TABLE projection_store_state
  ADD COLUMN building_corpus_schema TEXT
  CHECK(building_corpus_schema IS NULL OR length(trim(building_corpus_schema)) > 0);
ALTER TABLE projection_store_state
  ADD COLUMN building_corpus_fingerprint TEXT
  CHECK(
    building_corpus_fingerprint IS NULL
    OR length(trim(building_corpus_fingerprint)) > 0
  );
ALTER TABLE projection_store_state
  ADD COLUMN building_embedding_model TEXT
  CHECK(
    building_embedding_model IS NULL
    OR length(trim(building_embedding_model)) > 0
  );
ALTER TABLE projection_store_state
  ADD COLUMN building_embedding_dimensions INTEGER
  CHECK(
    (
      building_corpus_schema IS NULL
      AND building_corpus_fingerprint IS NULL
      AND building_embedding_model IS NULL
      AND building_embedding_dimensions IS NULL
    )
    OR
    (
      (
        (
          store_name='lancedb_chunks'
          AND building_corpus_schema='task-chunks-v2'
        )
        OR
        (
          store_name='lancedb_label_atoms'
          AND building_corpus_schema='label-atoms-v2'
        )
      )
      AND building_corpus_schema IS NOT NULL
      AND length(trim(building_corpus_schema)) > 0
      AND building_corpus_fingerprint IS NOT NULL
      AND length(trim(building_corpus_fingerprint)) > 0
      AND building_embedding_model IS NOT NULL
      AND length(trim(building_embedding_model)) > 0
      AND building_embedding_dimensions IS NOT NULL
      AND building_embedding_dimensions > 0
    )
  );

-- The all-NULL alternative above is required only to carry forward v29 rows
-- without inventing evidence. Such a legacy generation can only be carried
-- unchanged or cleared; it cannot be retroactively bound or replaced in place.
-- A v30 transition cannot create another unbound generation, nor can a complete
-- v30 binding be erased in place.
CREATE TRIGGER projection_corpus_generation_insert_guard
BEFORE INSERT ON projection_store_state
WHEN NEW.store_name IN ('lancedb_chunks','lancedb_label_atoms')
  AND (
    (
      (NEW.active_generation IS NULL)
      !=
      (
        NEW.active_corpus_schema IS NULL
        AND NEW.active_corpus_fingerprint IS NULL
        AND NEW.active_embedding_model IS NULL
        AND NEW.active_embedding_dimensions IS NULL
      )
    )
    OR
    (
      (NEW.previous_generation IS NULL)
      !=
      (
        NEW.previous_corpus_schema IS NULL
        AND NEW.previous_corpus_fingerprint IS NULL
        AND NEW.previous_embedding_model IS NULL
        AND NEW.previous_embedding_dimensions IS NULL
      )
    )
    OR
    (
      (NEW.building_generation IS NULL)
      !=
      (
        NEW.building_corpus_schema IS NULL
        AND NEW.building_corpus_fingerprint IS NULL
        AND NEW.building_embedding_model IS NULL
        AND NEW.building_embedding_dimensions IS NULL
      )
    )
  )
BEGIN
  SELECT RAISE(
    ABORT,
    'inserted LanceDB generation and corpus binding must match'
  );
END;

CREATE TRIGGER projection_active_corpus_generation_guard
BEFORE UPDATE OF active_generation,active_corpus_schema,
  active_corpus_fingerprint,active_embedding_model,active_embedding_dimensions
ON projection_store_state
WHEN NEW.store_name IN ('lancedb_chunks','lancedb_label_atoms')
  AND (
    (
      OLD.active_generation IS NOT NULL
      AND OLD.active_corpus_schema IS NULL
      AND OLD.active_corpus_fingerprint IS NULL
      AND OLD.active_embedding_model IS NULL
      AND OLD.active_embedding_dimensions IS NULL
      AND NOT (
        NEW.active_corpus_schema IS NULL
        AND NEW.active_corpus_fingerprint IS NULL
        AND NEW.active_embedding_model IS NULL
        AND NEW.active_embedding_dimensions IS NULL
        AND (
          NEW.active_generation IS OLD.active_generation
          OR NEW.active_generation IS NULL
        )
      )
    )
    OR
    (
      NEW.active_generation IS NOT NULL
      AND NEW.active_corpus_schema IS NULL
      AND NEW.active_corpus_fingerprint IS NULL
      AND NEW.active_embedding_model IS NULL
      AND NEW.active_embedding_dimensions IS NULL
      AND NOT (
        OLD.active_generation IS NEW.active_generation
        AND OLD.active_corpus_schema IS NULL
        AND OLD.active_corpus_fingerprint IS NULL
        AND OLD.active_embedding_model IS NULL
        AND OLD.active_embedding_dimensions IS NULL
      )
    )
    OR
    (
      NEW.active_generation IS NULL
      AND (
        NEW.active_corpus_schema IS NOT NULL
        OR NEW.active_corpus_fingerprint IS NOT NULL
        OR NEW.active_embedding_model IS NOT NULL
        OR NEW.active_embedding_dimensions IS NOT NULL
      )
      AND NOT (
        OLD.active_generation IS NOT NULL
        AND NEW.active_corpus_schema IS OLD.active_corpus_schema
        AND NEW.active_corpus_fingerprint IS OLD.active_corpus_fingerprint
        AND NEW.active_embedding_model IS OLD.active_embedding_model
        AND NEW.active_embedding_dimensions IS OLD.active_embedding_dimensions
      )
    )
  )
BEGIN
  SELECT RAISE(
    ABORT,
    'LanceDB active generation and corpus binding must match'
  );
END;

CREATE TRIGGER projection_previous_corpus_generation_guard
BEFORE UPDATE OF previous_generation,previous_corpus_schema,
  previous_corpus_fingerprint,previous_embedding_model,previous_embedding_dimensions
ON projection_store_state
WHEN NEW.store_name IN ('lancedb_chunks','lancedb_label_atoms')
  AND (
    (
      OLD.previous_generation IS NOT NULL
      AND OLD.previous_corpus_schema IS NULL
      AND OLD.previous_corpus_fingerprint IS NULL
      AND OLD.previous_embedding_model IS NULL
      AND OLD.previous_embedding_dimensions IS NULL
      AND NOT (
        NEW.previous_corpus_schema IS NULL
        AND NEW.previous_corpus_fingerprint IS NULL
        AND NEW.previous_embedding_model IS NULL
        AND NEW.previous_embedding_dimensions IS NULL
        AND (
          NEW.previous_generation IS OLD.previous_generation
          OR NEW.previous_generation IS NULL
        )
      )
    )
    OR
    (
      NEW.previous_generation IS NOT NULL
      AND NEW.previous_corpus_schema IS NULL
      AND NEW.previous_corpus_fingerprint IS NULL
      AND NEW.previous_embedding_model IS NULL
      AND NEW.previous_embedding_dimensions IS NULL
      AND NOT (
        OLD.previous_generation IS NEW.previous_generation
        AND OLD.previous_corpus_schema IS NULL
        AND OLD.previous_corpus_fingerprint IS NULL
        AND OLD.previous_embedding_model IS NULL
        AND OLD.previous_embedding_dimensions IS NULL
      )
    )
    OR
    (
      NEW.previous_generation IS NULL
      AND (
        NEW.previous_corpus_schema IS NOT NULL
        OR NEW.previous_corpus_fingerprint IS NOT NULL
        OR NEW.previous_embedding_model IS NOT NULL
        OR NEW.previous_embedding_dimensions IS NOT NULL
      )
      AND NOT (
        OLD.previous_generation IS NOT NULL
        AND NEW.previous_corpus_schema IS OLD.previous_corpus_schema
        AND NEW.previous_corpus_fingerprint IS OLD.previous_corpus_fingerprint
        AND NEW.previous_embedding_model IS OLD.previous_embedding_model
        AND NEW.previous_embedding_dimensions IS OLD.previous_embedding_dimensions
      )
    )
  )
BEGIN
  SELECT RAISE(
    ABORT,
    'LanceDB previous generation and corpus binding must match'
  );
END;

CREATE TRIGGER projection_building_corpus_generation_guard
BEFORE UPDATE OF building_generation,building_corpus_schema,
  building_corpus_fingerprint,building_embedding_model,building_embedding_dimensions
ON projection_store_state
WHEN NEW.store_name IN ('lancedb_chunks','lancedb_label_atoms')
  AND (
    (
      OLD.building_generation IS NOT NULL
      AND OLD.building_corpus_schema IS NULL
      AND OLD.building_corpus_fingerprint IS NULL
      AND OLD.building_embedding_model IS NULL
      AND OLD.building_embedding_dimensions IS NULL
      AND NOT (
        NEW.building_corpus_schema IS NULL
        AND NEW.building_corpus_fingerprint IS NULL
        AND NEW.building_embedding_model IS NULL
        AND NEW.building_embedding_dimensions IS NULL
        AND (
          NEW.building_generation IS OLD.building_generation
          OR NEW.building_generation IS NULL
        )
      )
    )
    OR
    (
      NEW.building_generation IS NOT NULL
      AND NEW.building_corpus_schema IS NULL
      AND NEW.building_corpus_fingerprint IS NULL
      AND NEW.building_embedding_model IS NULL
      AND NEW.building_embedding_dimensions IS NULL
      AND NOT (
        OLD.building_generation IS NEW.building_generation
        AND OLD.building_corpus_schema IS NULL
        AND OLD.building_corpus_fingerprint IS NULL
        AND OLD.building_embedding_model IS NULL
        AND OLD.building_embedding_dimensions IS NULL
      )
    )
    OR
    (
      NEW.building_generation IS NULL
      AND (
        NEW.building_corpus_schema IS NOT NULL
        OR NEW.building_corpus_fingerprint IS NOT NULL
        OR NEW.building_embedding_model IS NOT NULL
        OR NEW.building_embedding_dimensions IS NOT NULL
      )
      AND NOT (
        OLD.building_generation IS NOT NULL
        AND NEW.building_corpus_schema IS OLD.building_corpus_schema
        AND NEW.building_corpus_fingerprint IS OLD.building_corpus_fingerprint
        AND NEW.building_embedding_model IS OLD.building_embedding_model
        AND NEW.building_embedding_dimensions IS OLD.building_embedding_dimensions
      )
    )
  )
BEGIN
  SELECT RAISE(
    ABORT,
    'LanceDB building generation and corpus binding must match'
  );
END;

-- Import/restore and incompatible-generation recovery predate these columns.
-- When either path clears a generation, clear its new binding in a separate
-- non-recursive update without touching canonical outbox/delivery state.
CREATE TRIGGER projection_active_corpus_after_generation_reset
AFTER UPDATE OF active_generation ON projection_store_state
WHEN NEW.active_generation IS NULL
  AND (
    NEW.active_corpus_schema IS NOT NULL
    OR NEW.active_corpus_fingerprint IS NOT NULL
    OR NEW.active_embedding_model IS NOT NULL
    OR NEW.active_embedding_dimensions IS NOT NULL
  )
BEGIN
  UPDATE projection_store_state
  SET active_corpus_schema=NULL,
      active_corpus_fingerprint=NULL,
      active_embedding_model=NULL,
      active_embedding_dimensions=NULL
  WHERE store_name=NEW.store_name;
END;

CREATE TRIGGER projection_previous_corpus_after_generation_reset
AFTER UPDATE OF previous_generation ON projection_store_state
WHEN NEW.previous_generation IS NULL
  AND (
    NEW.previous_corpus_schema IS NOT NULL
    OR NEW.previous_corpus_fingerprint IS NOT NULL
    OR NEW.previous_embedding_model IS NOT NULL
    OR NEW.previous_embedding_dimensions IS NOT NULL
  )
BEGIN
  UPDATE projection_store_state
  SET previous_corpus_schema=NULL,
      previous_corpus_fingerprint=NULL,
      previous_embedding_model=NULL,
      previous_embedding_dimensions=NULL
  WHERE store_name=NEW.store_name;
END;

CREATE TRIGGER projection_building_corpus_after_generation_reset
AFTER UPDATE OF building_generation ON projection_store_state
WHEN NEW.building_generation IS NULL
  AND (
    NEW.building_corpus_schema IS NOT NULL
    OR NEW.building_corpus_fingerprint IS NOT NULL
    OR NEW.building_embedding_model IS NOT NULL
    OR NEW.building_embedding_dimensions IS NOT NULL
  )
BEGIN
  UPDATE projection_store_state
  SET building_corpus_schema=NULL,
      building_corpus_fingerprint=NULL,
      building_embedding_model=NULL,
      building_embedding_dimensions=NULL
  WHERE store_name=NEW.store_name;
END;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (
  30,
  '030_projection_corpus_bindings',
  '',
  CAST(strftime('%s','now') AS INTEGER) * 1000
);

COMMIT;
