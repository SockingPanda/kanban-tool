-- Kanban Knowledge Substrate foundation.
-- SQLite remains the operational source of truth; these tables provide stable
-- identity, typed relation metadata, outbox jobs, and derived-store health.

BEGIN;

CREATE TABLE IF NOT EXISTS entities (
  uri TEXT PRIMARY KEY CHECK(uri LIKE 'kb://%'),
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  source_table TEXT NOT NULL CHECK(length(trim(source_table)) > 0),
  source_id TEXT NOT NULL CHECK(length(trim(source_id)) > 0),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
  title TEXT,
  summary TEXT,
  content_hash TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER,
  UNIQUE(source_table, source_id)
);

CREATE INDEX IF NOT EXISTS idx_entities_kind_updated
  ON entities(kind, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_entities_board_kind
  ON entities(board_id, kind, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_entities_task
  ON entities(task_id, kind);

CREATE TABLE IF NOT EXISTS relation_predicates (
  name TEXT PRIMARY KEY CHECK(length(trim(name)) > 0),
  domain_kind TEXT,
  range_kind TEXT,
  cardinality TEXT,
  authoritative_store TEXT NOT NULL CHECK(length(trim(authoritative_store)) > 0),
  description TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entity_relations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  subject_uri TEXT NOT NULL REFERENCES entities(uri) ON DELETE CASCADE,
  predicate TEXT NOT NULL REFERENCES relation_predicates(name) ON DELETE RESTRICT,
  object_uri TEXT NOT NULL REFERENCES entities(uri) ON DELETE CASCADE,
  graph_uri TEXT NOT NULL DEFAULT 'kb://graph/indexed' CHECK(graph_uri LIKE 'kb://%'),
  authoritative_store TEXT NOT NULL CHECK(length(trim(authoritative_store)) > 0),
  source_table TEXT,
  source_id TEXT,
  source_event_id INTEGER REFERENCES task_events(id) ON DELETE SET NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(subject_uri, predicate, object_uri, graph_uri)
);

CREATE INDEX IF NOT EXISTS idx_entity_relations_subject
  ON entity_relations(subject_uri, predicate);

CREATE INDEX IF NOT EXISTS idx_entity_relations_object
  ON entity_relations(object_uri, predicate);

CREATE INDEX IF NOT EXISTS idx_entity_relations_predicate
  ON entity_relations(predicate, updated_at DESC);

CREATE TABLE IF NOT EXISTS index_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_event_id INTEGER REFERENCES task_events(id) ON DELETE SET NULL,
  target TEXT NOT NULL CHECK(target IN ('tantivy', 'oxigraph', 'lancedb', 'all')),
  entity_uri TEXT NOT NULL CHECK(entity_uri LIKE 'kb://%'),
  action TEXT NOT NULL CHECK(action IN ('upsert', 'delete', 'rebuild')),
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'done', 'failed')),
  attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_index_outbox_pending
  ON index_outbox(status, id);

CREATE INDEX IF NOT EXISTS idx_index_outbox_entity
  ON index_outbox(entity_uri, id DESC);

CREATE TABLE IF NOT EXISTS derived_store_state (
  store_name TEXT PRIMARY KEY CHECK(length(trim(store_name)) > 0),
  schema_version INTEGER NOT NULL,
  last_event_id INTEGER NOT NULL DEFAULT 0 CHECK(last_event_id >= 0),
  dirty INTEGER NOT NULL DEFAULT 0 CHECK(dirty IN (0, 1)),
  last_rebuild_at INTEGER,
  last_sync_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (2, '002_knowledge_substrate', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;
