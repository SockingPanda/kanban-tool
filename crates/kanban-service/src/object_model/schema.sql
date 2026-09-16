-- 迁移按具名 DDL 块校验定义，trigger 必须作为完整语句处理。
-- @object table object_model_schema
CREATE TABLE object_model_schema (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  version INTEGER NOT NULL CHECK(version = 2),
  checksum TEXT NOT NULL,
  catalog_version INTEGER NOT NULL CHECK(catalog_version >= 1),
  installed_at INTEGER NOT NULL
);
-- @object table object_types
CREATE TABLE object_types (
  key TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
  layout_json TEXT NOT NULL CHECK(json_valid(layout_json) AND json_type(layout_json) = 'object'),
  capability TEXT NOT NULL CHECK(capability IN ('plain','task')),
  system INTEGER NOT NULL CHECK(system IN (0,1)),
  retired INTEGER NOT NULL DEFAULT 0 CHECK(retired IN (0,1)),
  version INTEGER NOT NULL CHECK(version >= 1),
  CHECK(system = 1 OR capability = 'plain')
);
-- @object table object_properties
CREATE TABLE object_properties (
  key TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
  kind TEXT NOT NULL CHECK(kind IN ('text','number','boolean','date','object','select')),
  cardinality TEXT NOT NULL CHECK(cardinality IN ('one','many')),
  reference_type TEXT REFERENCES object_types(key),
  system INTEGER NOT NULL CHECK(system IN (0,1)),
  version INTEGER NOT NULL CHECK(version >= 1),
  UNIQUE(key, kind, cardinality),
  CHECK(reference_type IS NULL OR kind = 'object')
);
-- @object table object_options
CREATE TABLE object_options (
  property_key TEXT NOT NULL REFERENCES object_properties(key) ON DELETE CASCADE,
  key TEXT NOT NULL,
  name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
  PRIMARY KEY(property_key, key)
);
-- @object table object_type_properties
CREATE TABLE object_type_properties (
  type_key TEXT NOT NULL REFERENCES object_types(key),
  property_key TEXT NOT NULL REFERENCES object_properties(key),
  required INTEGER NOT NULL CHECK(required IN (0,1)),
  position INTEGER NOT NULL,
  defaults_json TEXT NOT NULL CHECK(json_valid(defaults_json) AND json_type(defaults_json) = 'array'),
  storage TEXT NOT NULL CHECK(storage IN ('value','task.status','task.priority','task.assignee','task.due_at')),
  writable INTEGER NOT NULL CHECK(writable IN (0,1)),
  PRIMARY KEY(type_key, property_key),
  UNIQUE(type_key, property_key, storage),
  CHECK(storage = 'value' OR (type_key = 'task' AND writable = 0))
);
-- @object table objects
CREATE TABLE objects (
  id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  type_key TEXT NOT NULL REFERENCES object_types(key),
  task_id TEXT UNIQUE,
  title TEXT,
  body TEXT,
  version INTEGER NOT NULL CHECK(version >= 1),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER,
  UNIQUE(id, board_id),
  UNIQUE(id, board_id, type_key),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  CHECK((type_key = 'task' AND task_id = id AND task_id IS NOT NULL AND title IS NULL AND body IS NULL AND archived_at IS NULL)
     OR (type_key != 'task' AND task_id IS NULL AND title IS NOT NULL AND length(trim(title)) BETWEEN 1 AND 200))
);
-- @object table object_property_slots
CREATE TABLE object_property_slots (
  object_id TEXT NOT NULL,
  board_id TEXT NOT NULL,
  type_key TEXT NOT NULL,
  property_key TEXT NOT NULL,
  kind TEXT NOT NULL,
  cardinality TEXT NOT NULL,
  storage TEXT NOT NULL DEFAULT 'value' CHECK(storage = 'value'),
  CHECK(kind != 'object'),
  PRIMARY KEY(object_id, property_key),
  UNIQUE(object_id, board_id, property_key, kind, cardinality),
  FOREIGN KEY(object_id, board_id, type_key) REFERENCES objects(id, board_id, type_key) ON DELETE CASCADE,
  FOREIGN KEY(type_key, property_key, storage) REFERENCES object_type_properties(type_key, property_key, storage),
  FOREIGN KEY(property_key, kind, cardinality) REFERENCES object_properties(key, kind, cardinality)
);
-- @object table object_property_values
CREATE TABLE object_property_values (
  object_id TEXT NOT NULL,
  board_id TEXT NOT NULL,
  property_key TEXT NOT NULL,
  kind TEXT NOT NULL,
  cardinality TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
  text_value TEXT,
  number_value REAL,
  integer_value INTEGER,
  option_key TEXT,
  PRIMARY KEY(object_id, property_key, ordinal),
  CHECK(kind != 'object'),
  FOREIGN KEY(object_id, board_id, property_key, kind, cardinality)
    REFERENCES object_property_slots(object_id, board_id, property_key, kind, cardinality) ON DELETE CASCADE,
  FOREIGN KEY(property_key, option_key) REFERENCES object_options(property_key, key),
  CHECK(cardinality = 'many' OR ordinal = 0),
  CHECK((kind = 'text' AND text_value IS NOT NULL AND number_value IS NULL AND integer_value IS NULL AND option_key IS NULL)
     OR (kind = 'number' AND text_value IS NULL AND number_value IS NOT NULL AND integer_value IS NULL AND option_key IS NULL)
     OR (kind = 'boolean' AND text_value IS NULL AND number_value IS NULL AND integer_value IN (0,1) AND integer_value IS NOT NULL AND option_key IS NULL)
     OR (kind = 'date' AND text_value IS NULL AND number_value IS NULL AND integer_value IS NOT NULL AND option_key IS NULL)
     OR (kind = 'select' AND text_value IS NULL AND number_value IS NULL AND integer_value IS NULL AND option_key IS NOT NULL))
);
-- @object table object_requests
CREATE TABLE object_requests (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  request_id TEXT NOT NULL CHECK(length(request_id) BETWEEN 1 AND 128),
  request_hash TEXT NOT NULL,
  receipt_json TEXT NOT NULL CHECK(json_valid(receipt_json)),
  event_sequence INTEGER NOT NULL REFERENCES task_events(id),
  created_at INTEGER NOT NULL,
  PRIMARY KEY(board_id, request_id)
);
-- @object table object_snapshots
CREATE TABLE object_snapshots (
  id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  object_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  body_json TEXT NOT NULL CHECK(json_valid(body_json)),
  created_at INTEGER NOT NULL,
  FOREIGN KEY(object_id, board_id) REFERENCES objects(id, board_id)
);
-- @object index idx_objects_board_type
CREATE INDEX idx_objects_board_type ON objects(board_id, type_key, id);
-- @object index idx_object_values_text
CREATE INDEX idx_object_values_text ON object_property_values(board_id, property_key, text_value, object_id);
-- @object index idx_object_values_option
CREATE INDEX idx_object_values_option ON object_property_values(board_id, property_key, option_key, object_id);
-- @object index idx_object_values_number
CREATE INDEX idx_object_values_number ON object_property_values(board_id, property_key, number_value, object_id);
-- @object index idx_object_values_integer
CREATE INDEX idx_object_values_integer ON object_property_values(board_id, property_key, integer_value, object_id);
-- @object index idx_object_snapshots_owner
CREATE INDEX idx_object_snapshots_owner ON object_snapshots(board_id, object_id, created_at, id);
-- @object trigger object_task_identity_insert
CREATE TRIGGER object_task_identity_insert AFTER INSERT ON tasks BEGIN
  INSERT INTO objects(id, board_id, type_key, task_id, title, body, version, created_at, updated_at, archived_at)
  VALUES (NEW.id, NEW.board_id, 'task', NEW.id, NULL, NULL, 1, NEW.created_at, NEW.created_at, NULL);
END;
-- @object table object_event_links
CREATE TABLE object_event_links (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  object_id TEXT NOT NULL,
  event_sequence INTEGER NOT NULL REFERENCES task_events(id),
  PRIMARY KEY(object_id, event_sequence),
  FOREIGN KEY(object_id, board_id) REFERENCES objects(id, board_id)
);

-- @object table object_relation_types
CREATE TABLE object_relation_types (
  key TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200),
  source_type TEXT NOT NULL REFERENCES object_types(key),
  target_type TEXT REFERENCES object_types(key),
  source_property TEXT NOT NULL UNIQUE REFERENCES object_properties(key),
  target_property TEXT UNIQUE REFERENCES object_properties(key),
  source_cardinality TEXT NOT NULL CHECK(source_cardinality IN ('one','many')),
  target_cardinality TEXT NOT NULL CHECK(target_cardinality IN ('one','many')),
  acyclic INTEGER NOT NULL CHECK(acyclic IN (0,1)),
  system INTEGER NOT NULL CHECK(system IN (0,1)),
  version INTEGER NOT NULL CHECK(version >= 1),
  UNIQUE(key, source_type, source_cardinality, target_cardinality),
  FOREIGN KEY(source_type,source_property) REFERENCES object_type_properties(type_key,property_key),
  FOREIGN KEY(target_type,target_property) REFERENCES object_type_properties(type_key,property_key),
  CHECK(target_property IS NULL OR target_type IS NOT NULL),
  CHECK(target_property IS NULL OR source_property != target_property),
  CHECK(acyclic = 0 OR (target_type IS NOT NULL AND source_type = target_type))
);
-- @object table object_relation_edges
CREATE TABLE object_relation_edges (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  relation_key TEXT NOT NULL,
  source_id TEXT NOT NULL,
  target_id TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_cardinality TEXT NOT NULL CHECK(source_cardinality IN ('one','many')),
  target_cardinality TEXT NOT NULL CHECK(target_cardinality IN ('one','many')),
  created_at INTEGER NOT NULL,
  PRIMARY KEY(board_id,relation_key,source_id,target_id),
  FOREIGN KEY(relation_key,source_type,source_cardinality,target_cardinality)
    REFERENCES object_relation_types(key,source_type,source_cardinality,target_cardinality),
  FOREIGN KEY(source_id,board_id,source_type) REFERENCES objects(id,board_id,type_key),
  FOREIGN KEY(target_id,board_id) REFERENCES objects(id,board_id)
);
-- @object index idx_object_relation_source_one
CREATE UNIQUE INDEX idx_object_relation_source_one
  ON object_relation_edges(board_id,relation_key,source_id) WHERE source_cardinality='one';
-- @object index idx_object_relation_target_one
CREATE UNIQUE INDEX idx_object_relation_target_one
  ON object_relation_edges(board_id,relation_key,target_id) WHERE target_cardinality='one';
-- @object index idx_object_relation_target
CREATE INDEX idx_object_relation_target ON object_relation_edges(board_id,relation_key,target_id,source_id);
-- @object trigger object_relation_target_insert
CREATE TRIGGER object_relation_target_insert BEFORE INSERT ON object_relation_edges
WHEN EXISTS (
  SELECT 1 FROM object_relation_types r JOIN objects o ON o.id=NEW.target_id
  WHERE r.key=NEW.relation_key AND r.target_type IS NOT NULL AND r.target_type != o.type_key
) BEGIN
  SELECT RAISE(ABORT, 'relation target type mismatch');
END;
-- @object trigger object_relation_immutable_update
CREATE TRIGGER object_relation_immutable_update BEFORE UPDATE ON object_relation_edges BEGIN
  SELECT RAISE(ABORT, 'relation edges are immutable; use transactional delete and insert');
END;
-- @object trigger object_relation_acyclic_insert
CREATE TRIGGER object_relation_acyclic_insert BEFORE INSERT ON object_relation_edges
WHEN (SELECT acyclic FROM object_relation_types WHERE key=NEW.relation_key)=1 BEGIN
  SELECT RAISE(ABORT, 'acyclic relation would contain a cycle') WHERE NEW.source_id=NEW.target_id;
END;
-- @object table object_workflows
CREATE TABLE object_workflows (
  key TEXT PRIMARY KEY,
  type_key TEXT NOT NULL UNIQUE REFERENCES object_types(key),
  definition_json TEXT NOT NULL CHECK(json_valid(definition_json) AND json_type(definition_json)='object'),
  system INTEGER NOT NULL CHECK(system IN (0,1)),
  version INTEGER NOT NULL CHECK(version >= 1)
);
-- @object table object_rollups
CREATE TABLE object_rollups (
  key TEXT PRIMARY KEY,
  type_key TEXT NOT NULL UNIQUE REFERENCES object_types(key),
  definition_json TEXT NOT NULL CHECK(json_valid(definition_json) AND json_type(definition_json)='object'),
  system INTEGER NOT NULL CHECK(system IN (0,1)),
  version INTEGER NOT NULL CHECK(version >= 1)
);
-- @object index idx_object_workflow_closure
CREATE UNIQUE INDEX idx_object_workflow_closure ON object_snapshots(object_id,kind) WHERE kind='workflow.closed';
