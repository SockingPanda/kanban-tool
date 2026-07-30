-- Route board-scoped label atom rebuilds through Projection v2 deliveries.
--
-- SQLite remains canonical. The additive projection_store selector separates
-- label atom rebuilds from legacy target='lancedb' task chunk work without
-- rebuilding index_outbox or projection_deliveries.

BEGIN;

ALTER TABLE index_outbox
  ADD COLUMN projection_store TEXT
  CHECK(
    projection_store IS NULL
    OR (
      target='lancedb'
      AND projection_store='lancedb_label_atoms'
      AND source_event_id IS NULL
      AND entity_uri LIKE 'kb://board/%'
      AND entity_uri != 'kb://board/'
      AND action='rebuild'
      AND payload_json='{"scope":"board","version":1}'
    )
  );

CREATE INDEX idx_index_outbox_projection_route
  ON index_outbox(projection_store, status, id);

-- Once an outbox row has fanned out, changing either half of its route would
-- make the immutable delivery set disagree with the parent row.
CREATE TRIGGER index_outbox_projection_route_immutable
BEFORE UPDATE OF source_event_id,target,projection_store,entity_uri,action,payload_json
ON index_outbox
WHEN NEW.target IS NOT OLD.target
  OR NEW.projection_store IS NOT OLD.projection_store
  OR (
    (
      OLD.projection_store='lancedb_label_atoms'
      OR NEW.projection_store='lancedb_label_atoms'
    )
    AND (
      NEW.source_event_id IS NOT OLD.source_event_id
      OR NEW.entity_uri IS NOT OLD.entity_uri
      OR NEW.action IS NOT OLD.action
      OR NEW.payload_json IS NOT OLD.payload_json
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'index_outbox projection route is immutable');
END;

DROP TRIGGER projection_deliveries_after_outbox_insert;

CREATE TRIGGER projection_deliveries_after_outbox_insert
AFTER INSERT ON index_outbox
BEGIN
  INSERT INTO projection_deliveries(
    outbox_id, store_name, board_id, source_event_id, cursor, action,
    entity_uri, payload_json, status, attempts, next_attempt_at,
    created_at, updated_at
  )
  SELECT
    NEW.id,
    stores.store_name,
    COALESCE(
      (SELECT board_id FROM task_events WHERE id=NEW.source_event_id),
      (SELECT board_id FROM entities WHERE uri=NEW.entity_uri)
    ),
    NEW.source_event_id,
    NEW.id,
    NEW.action,
    NEW.entity_uri,
    NEW.payload_json,
    CASE WHEN NEW.status='done' THEN 'legacy_done' ELSE 'pending' END,
    NEW.attempts,
    0,
    NEW.created_at,
    NEW.updated_at
  FROM (
    SELECT 'tantivy_tasks' AS store_name
    WHERE NEW.target IN ('tantivy', 'all')
    UNION ALL
    SELECT 'oxigraph_relations'
    WHERE NEW.target IN ('oxigraph', 'all')
    UNION ALL
    SELECT 'lancedb_chunks'
    WHERE NEW.target IN ('lancedb', 'all')
      AND NEW.projection_store IS NULL
    UNION ALL
    SELECT 'lancedb_label_atoms'
    WHERE NEW.target='lancedb'
      AND NEW.projection_store='lancedb_label_atoms'
  ) stores;
END;

-- Canonical label mutations and provider failures already mark this
-- board-scoped state in their transaction. Let the database enqueue the
-- rebuild in that same transaction so every service/helper path shares one
-- atomic, recoverable delivery seam.
CREATE TRIGGER label_atom_delivery_after_board_insert
AFTER INSERT ON label_atom_index_boards
WHEN NEW.store_name='lancedb_label_atoms'
  AND NEW.dirty=1
BEGIN
  INSERT INTO index_outbox(
    source_event_id,target,projection_store,entity_uri,action,payload_json,
    status,attempts,last_error,created_at,updated_at
  )
  SELECT
    NULL,
    'lancedb',
    'lancedb_label_atoms',
    'kb://board/' || NEW.board_id,
    'rebuild',
    '{"scope":"board","version":1}',
    'pending',
    0,
    NULL,
    NEW.updated_at,
    NEW.updated_at
  WHERE NOT EXISTS (
    SELECT 1
    FROM projection_deliveries delivery
    WHERE delivery.store_name='lancedb_label_atoms'
      AND delivery.board_id=NEW.board_id
      AND delivery.status IN ('pending','failed')
  );
END;

CREATE TRIGGER label_atom_delivery_after_board_update
AFTER UPDATE OF dirty,last_error ON label_atom_index_boards
WHEN NEW.store_name='lancedb_label_atoms'
  AND NEW.dirty=1
BEGIN
  INSERT INTO index_outbox(
    source_event_id,target,projection_store,entity_uri,action,payload_json,
    status,attempts,last_error,created_at,updated_at
  )
  SELECT
    NULL,
    'lancedb',
    'lancedb_label_atoms',
    'kb://board/' || NEW.board_id,
    'rebuild',
    '{"scope":"board","version":1}',
    'pending',
    0,
    NULL,
    NEW.updated_at,
    NEW.updated_at
  WHERE NOT EXISTS (
    SELECT 1
    FROM projection_deliveries delivery
    WHERE delivery.store_name='lancedb_label_atoms'
      AND delivery.board_id=NEW.board_id
      AND delivery.status IN ('pending','failed')
  );
END;

-- Existing dirty boards predate the delivery seam. Preserve their diagnostic
-- state and add one board-scoped rebuild, including boards with a prior
-- provider error so v2 recovery has explicit work to claim.
INSERT INTO index_outbox(
  source_event_id,target,projection_store,entity_uri,action,payload_json,
  status,attempts,last_error,created_at,updated_at
)
SELECT
  NULL,
  'lancedb',
  'lancedb_label_atoms',
  'kb://board/' || board_state.board_id,
  'rebuild',
  '{"scope":"board","version":1}',
  'pending',
  0,
  NULL,
  board_state.updated_at,
  board_state.updated_at
FROM label_atom_index_boards board_state
WHERE board_state.store_name='lancedb_label_atoms'
  AND board_state.dirty=1
  AND NOT EXISTS (
    SELECT 1
    FROM projection_deliveries delivery
    WHERE delivery.store_name='lancedb_label_atoms'
      AND delivery.board_id=board_state.board_id
      AND delivery.status IN ('pending','failed')
  )
ORDER BY board_state.board_id;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (
  29,
  '029_projection_label_atom_deliveries',
  '',
  CAST(strftime('%s','now') AS INTEGER) * 1000
);

COMMIT;
