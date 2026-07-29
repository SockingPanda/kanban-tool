-- Database-scoped derived projection protocol v2.
--
-- SQLite remains canonical. An outbox item fans out into one board-scoped
-- delivery per physical store. A v2 acknowledgement is bound to a database
-- identity, generation, lease token, and monotonically increasing fence epoch.

BEGIN;

CREATE TABLE projection_database (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  database_instance_id TEXT NOT NULL UNIQUE CHECK(database_instance_id LIKE 'db_%'),
  protocol_version INTEGER NOT NULL CHECK(protocol_version = 2),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE projection_store_state (
  store_name TEXT PRIMARY KEY
    REFERENCES derived_store_state(store_name) ON DELETE RESTRICT,
  database_instance_id TEXT NOT NULL
    REFERENCES projection_database(database_instance_id) ON DELETE RESTRICT,
  protocol_version INTEGER NOT NULL CHECK(protocol_version = 2),
  schema_version INTEGER NOT NULL CHECK(schema_version > 0),
  control_plane TEXT NOT NULL DEFAULT 'legacy'
    CHECK(control_plane IN ('legacy', 'v2')),
  active_generation TEXT,
  active_fingerprint TEXT,
  active_fence_epoch INTEGER,
  active_snapshot_cursor INTEGER CHECK(active_snapshot_cursor >= 0),
  active_provider TEXT,
  active_provider_fingerprint TEXT,
  active_canonical_count INTEGER CHECK(active_canonical_count >= 0),
  active_canonical_digest TEXT,
  active_delivery_count INTEGER CHECK(active_delivery_count >= 0),
  active_delivery_digest TEXT,
  previous_generation TEXT,
  previous_fingerprint TEXT,
  previous_fence_epoch INTEGER,
  previous_snapshot_cursor INTEGER CHECK(previous_snapshot_cursor >= 0),
  previous_provider TEXT,
  previous_provider_fingerprint TEXT,
  previous_canonical_count INTEGER CHECK(previous_canonical_count >= 0),
  previous_canonical_digest TEXT,
  previous_delivery_count INTEGER CHECK(previous_delivery_count >= 0),
  previous_delivery_digest TEXT,
  building_generation TEXT,
  building_fingerprint TEXT,
  building_fence_epoch INTEGER,
  building_provider TEXT,
  building_provider_fingerprint TEXT,
  building_canonical_count INTEGER CHECK(building_canonical_count >= 0),
  building_canonical_digest TEXT,
  building_delivery_count INTEGER CHECK(building_delivery_count >= 0),
  building_delivery_digest TEXT,
  building_phase TEXT
    CHECK(building_phase IN ('snapshotting', 'prepared', 'store_published')),
  snapshot_cursor INTEGER NOT NULL DEFAULT 0 CHECK(snapshot_cursor >= 0),
  checkpoint_cursor INTEGER NOT NULL DEFAULT 0 CHECK(checkpoint_cursor >= 0),
  legacy_checkpoint_cursor INTEGER NOT NULL DEFAULT 0
    CHECK(legacy_checkpoint_cursor >= 0),
  lifecycle_status TEXT NOT NULL DEFAULT 'bootstrap_required'
    CHECK(lifecycle_status IN (
      'bootstrap_required', 'idle', 'rebuilding', 'ready', 'error'
    )),
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at INTEGER,
  last_success_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL,
  CHECK(
    (lease_owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL)
    OR
    (length(trim(lease_owner)) > 0 AND length(trim(lease_token)) > 0
      AND lease_expires_at IS NOT NULL)
  ),
  CHECK(
    (active_generation IS NULL AND active_fingerprint IS NULL
      AND active_fence_epoch IS NULL AND active_snapshot_cursor IS NULL
      AND active_provider IS NULL
      AND active_provider_fingerprint IS NULL AND active_canonical_count IS NULL
      AND active_canonical_digest IS NULL AND active_delivery_count IS NULL
      AND active_delivery_digest IS NULL)
    OR
    (active_generation IS NOT NULL AND length(trim(active_fingerprint)) > 0
      AND active_fence_epoch IS NOT NULL AND active_snapshot_cursor IS NOT NULL
      AND length(trim(active_provider)) > 0
      AND length(trim(active_provider_fingerprint)) > 0
      AND active_canonical_count IS NOT NULL
      AND length(trim(active_canonical_digest)) > 0
      AND active_delivery_count IS NOT NULL
      AND length(trim(active_delivery_digest)) > 0)
  ),
  CHECK(
    (previous_generation IS NULL AND previous_fingerprint IS NULL
      AND previous_fence_epoch IS NULL AND previous_snapshot_cursor IS NULL
      AND previous_provider IS NULL
      AND previous_provider_fingerprint IS NULL AND previous_canonical_count IS NULL
      AND previous_canonical_digest IS NULL AND previous_delivery_count IS NULL
      AND previous_delivery_digest IS NULL)
    OR
    (previous_generation IS NOT NULL AND length(trim(previous_fingerprint)) > 0
      AND previous_fence_epoch IS NOT NULL AND previous_snapshot_cursor IS NOT NULL
      AND length(trim(previous_provider)) > 0
      AND length(trim(previous_provider_fingerprint)) > 0
      AND previous_canonical_count IS NOT NULL
      AND length(trim(previous_canonical_digest)) > 0
      AND previous_delivery_count IS NOT NULL
      AND length(trim(previous_delivery_digest)) > 0)
  ),
  CHECK(
    (building_generation IS NULL AND building_fingerprint IS NULL
      AND building_fence_epoch IS NULL AND building_provider IS NULL
      AND building_provider_fingerprint IS NULL AND building_canonical_count IS NULL
      AND building_canonical_digest IS NULL AND building_delivery_count IS NULL
      AND building_delivery_digest IS NULL AND building_phase IS NULL)
    OR
    (building_generation IS NOT NULL AND building_fence_epoch IS NOT NULL
      AND length(trim(building_provider)) > 0
      AND length(trim(building_provider_fingerprint)) > 0
      AND building_canonical_count IS NOT NULL
      AND length(trim(building_canonical_digest)) > 0
      AND building_delivery_count IS NOT NULL
      AND length(trim(building_delivery_digest)) > 0
      AND building_phase IS NOT NULL)
  ),
  CHECK(
    building_phase = 'snapshotting'
    OR building_phase IS NULL
    OR length(trim(building_fingerprint)) > 0
  ),
  CHECK(previous_generation IS NULL OR previous_generation != active_generation),
  CHECK(building_generation IS NULL OR building_generation != active_generation)
);

CREATE INDEX idx_projection_store_lease
  ON projection_store_state(lease_expires_at, store_name);

CREATE TABLE projection_deliveries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  outbox_id INTEGER NOT NULL REFERENCES index_outbox(id) ON DELETE RESTRICT,
  store_name TEXT NOT NULL
    REFERENCES derived_store_state(store_name) ON DELETE RESTRICT,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE RESTRICT,
  source_event_id INTEGER REFERENCES task_events(id) ON DELETE SET NULL,
  cursor INTEGER NOT NULL CHECK(cursor > 0),
  action TEXT NOT NULL CHECK(action IN ('upsert', 'delete', 'rebuild')),
  entity_uri TEXT NOT NULL CHECK(entity_uri LIKE 'kb://%'),
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK(status IN ('pending', 'running', 'done', 'failed', 'legacy_done')),
  attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
  next_attempt_at INTEGER NOT NULL DEFAULT 0,
  claim_owner TEXT,
  claim_token TEXT,
  claim_lease_token TEXT,
  claim_fence_epoch INTEGER,
  claim_generation TEXT,
  claim_expires_at INTEGER,
  published_generation TEXT,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(outbox_id, store_name),
  UNIQUE(store_name, cursor),
  CHECK(
    status != 'running'
    OR
    (length(trim(claim_owner)) > 0 AND length(trim(claim_token)) > 0
      AND length(trim(claim_lease_token)) > 0 AND claim_fence_epoch IS NOT NULL
      AND length(trim(claim_generation)) > 0 AND claim_expires_at IS NOT NULL)
  ),
  CHECK(
    status = 'running'
    OR
    (claim_owner IS NULL AND claim_token IS NULL AND claim_lease_token IS NULL
      AND claim_fence_epoch IS NULL AND claim_generation IS NULL
      AND claim_expires_at IS NULL)
  ),
  CHECK(status != 'done' OR length(trim(published_generation)) > 0)
);

CREATE INDEX idx_projection_deliveries_ready
  ON projection_deliveries(store_name, status, next_attempt_at, cursor);

CREATE INDEX idx_projection_deliveries_claim
  ON projection_deliveries(store_name, claim_token, status);

CREATE INDEX idx_projection_deliveries_board
  ON projection_deliveries(store_name, board_id, cursor);

-- Fail closed when an outbox record cannot be resolved to exactly one board.
CREATE TRIGGER projection_delivery_board_guard_insert
BEFORE INSERT ON projection_deliveries
BEGIN
  SELECT CASE
    WHEN NEW.source_event_id IS NOT NULL
      AND (SELECT board_id FROM task_events WHERE id=NEW.source_event_id) != NEW.board_id
    THEN RAISE(ABORT, 'projection delivery event board mismatch')
  END;
  SELECT CASE
    WHEN (SELECT board_id FROM entities WHERE uri=NEW.entity_uri) IS NOT NULL
      AND (SELECT board_id FROM entities WHERE uri=NEW.entity_uri) != NEW.board_id
    THEN RAISE(ABORT, 'projection delivery entity board mismatch')
  END;
END;

CREATE TRIGGER projection_delivery_board_guard_update
BEFORE UPDATE OF board_id,source_event_id,entity_uri ON projection_deliveries
BEGIN
  SELECT CASE
    WHEN NEW.source_event_id IS NOT NULL
      AND (SELECT board_id FROM task_events WHERE id=NEW.source_event_id) != NEW.board_id
    THEN RAISE(ABORT, 'projection delivery event board mismatch')
  END;
  SELECT CASE
    WHEN (SELECT board_id FROM entities WHERE uri=NEW.entity_uri) IS NOT NULL
      AND (SELECT board_id FROM entities WHERE uri=NEW.entity_uri) != NEW.board_id
    THEN RAISE(ABORT, 'projection delivery entity board mismatch')
  END;
END;

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
    SELECT 'tantivy_tasks' AS store_name WHERE NEW.target IN ('tantivy', 'all')
    UNION ALL
    SELECT 'oxigraph_relations' WHERE NEW.target IN ('oxigraph', 'all')
    UNION ALL
    SELECT 'lancedb_chunks' WHERE NEW.target IN ('lancedb', 'all')
  ) stores;
END;

-- A legacy consumer may finish while that store is still legacy-owned. Record
-- that fact without treating it as v2 generation coverage or advancing a v2
-- checkpoint.
CREATE TRIGGER projection_deliveries_after_legacy_outbox_done
AFTER UPDATE OF status ON index_outbox
WHEN NEW.status='done' AND OLD.status!='done'
BEGIN
  UPDATE projection_deliveries
  SET status='legacy_done',
      claim_owner=NULL,
      claim_token=NULL,
      claim_lease_token=NULL,
      claim_fence_epoch=NULL,
      claim_generation=NULL,
      claim_expires_at=NULL,
      updated_at=NEW.updated_at
  WHERE outbox_id=NEW.id
    AND status!='done'
    AND EXISTS (
      SELECT 1 FROM projection_store_state s
      WHERE s.store_name=projection_deliveries.store_name
        AND s.control_plane='legacy'
    );
END;

INSERT INTO projection_deliveries(
  outbox_id, store_name, board_id, source_event_id, cursor, action,
  entity_uri, payload_json, status, attempts, next_attempt_at,
  created_at, updated_at
)
SELECT
  o.id,
  stores.store_name,
  COALESCE(e.board_id, entity.board_id),
  o.source_event_id,
  o.id,
  o.action,
  o.entity_uri,
  o.payload_json,
  CASE WHEN o.status='done' THEN 'legacy_done' ELSE 'pending' END,
  o.attempts,
  0,
  o.created_at,
  o.updated_at
FROM index_outbox o
LEFT JOIN task_events e ON e.id=o.source_event_id
LEFT JOIN entities entity ON entity.uri=o.entity_uri
JOIN (
  SELECT 'tantivy' AS target, 'tantivy_tasks' AS store_name
  UNION ALL SELECT 'oxigraph', 'oxigraph_relations'
  UNION ALL SELECT 'lancedb', 'lancedb_chunks'
  UNION ALL SELECT 'all', 'tantivy_tasks'
  UNION ALL SELECT 'all', 'oxigraph_relations'
  UNION ALL SELECT 'all', 'lancedb_chunks'
) stores ON stores.target=o.target;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (
  26,
  '026_projection_v2',
  '',
  CAST(strftime('%s','now') AS INTEGER) * 1000
);

COMMIT;
