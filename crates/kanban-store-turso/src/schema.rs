pub(crate) const CANONICAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL DEFAULT '',
  applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS boards (
  id TEXT PRIMARY KEY CHECK(id LIKE 'b_%'),
  slug TEXT NOT NULL UNIQUE CHECK(length(trim(slug)) > 0),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER
);

CREATE TABLE IF NOT EXISTS board_columns (
  id TEXT PRIMARY KEY CHECK(id LIKE 'col_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  position INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0, 1)),
  wip_limit INTEGER CHECK(wip_limit IS NULL OR wip_limit >= 0),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, status),
  UNIQUE(board_id, position)
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY CHECK(id LIKE 't_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,
  idempotency_key TEXT,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK(status IN (
    'triage', 'todo', 'scheduled', 'ready', 'running', 'blocked', 'review', 'done', 'archived'
  )),
  status_reason TEXT,
  assignee TEXT,
  priority INTEGER NOT NULL DEFAULT 3 CHECK(priority BETWEEN 0 AND 3),
  position INTEGER NOT NULL DEFAULT 0,
  scheduled_at INTEGER,
  due_at INTEGER,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  started_at INTEGER,
  completed_at INTEGER,
  archived_at INTEGER,
  claim_token TEXT,
  claim_owner TEXT,
  claim_expires_at INTEGER,
  last_heartbeat_at INTEGER,
  current_run_id TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0),
  max_retries INTEGER CHECK(max_retries IS NULL OR max_retries >= 0),
  result_summary TEXT,
  result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  lock_version INTEGER NOT NULL DEFAULT 0 CHECK(lock_version >= 0),
  UNIQUE(board_id, id),
  UNIQUE(id, board_id),
  UNIQUE(board_id, seq),
  CHECK(
    (status != 'running') OR
    (claim_token IS NOT NULL AND claim_owner IS NOT NULL AND claim_expires_at IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_idempotency
  ON tasks(board_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_execution_plans (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  state TEXT NOT NULL CHECK(state IN ('unplanned', 'planned', 'not_required')),
  reason TEXT,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(task_id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_steps (
  id TEXT PRIMARY KEY CHECK(id LIKE 'step_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  idempotency_key TEXT,
  position INTEGER NOT NULL,
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  body TEXT,
  linked_task_id TEXT,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo', 'done', 'skipped')),
  resolution_note TEXT,
  resolved_by TEXT,
  resolved_at INTEGER,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(parent_task_id, idempotency_key),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(linked_task_id, board_id) REFERENCES tasks(id, board_id),
  CHECK(linked_task_id IS NULL OR parent_task_id != linked_task_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_steps_idempotency
  ON task_steps(parent_task_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_dependencies (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL,
  child_task_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS task_runs (
  id TEXT PRIMARY KEY CHECK(id LIKE 'r_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('running', 'succeeded', 'failed', 'canceled', 'expired')),
  worker_profile TEXT,
  worker_pid INTEGER,
  claim_token TEXT NOT NULL,
  claim_owner TEXT NOT NULL,
  claim_expires_at INTEGER NOT NULL,
  started_at INTEGER NOT NULL,
  last_heartbeat_at INTEGER,
  finished_at INTEGER,
  exit_code INTEGER,
  summary TEXT,
  error TEXT,
  log_path TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_task_runs_task_started
  ON task_runs(task_id, started_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_runs_one_active
  ON task_runs(task_id)
  WHERE status = 'running';

CREATE TABLE IF NOT EXISTS task_comments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  idempotency_key TEXT,
  author TEXT NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
  agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL),
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision')),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  UNIQUE(task_id, idempotency_key)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_comments_idempotency
  ON task_comments(task_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS task_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE CHECK(event_id LIKE 'e_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  run_id TEXT,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  actor TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id),
  FOREIGN KEY(run_id, board_id) REFERENCES task_runs(id, board_id)
);

CREATE INDEX IF NOT EXISTS idx_task_events_board_created
  ON task_events(board_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_task_events_task_created
  ON task_events(task_id, id DESC);
"#;

pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const SCHEMA_NAME: &str = "001_canonical_baseline";

/// schema family 与数字 migration version 有意分离。`version = 1` 但 family 不匹配时，
/// 即使表名为 `schema_migrations` 也不是 Turso 数据库，不能被自动采用。
pub(crate) const SCHEMA_FAMILY: &str = "kanban.turso";
pub(crate) const SCHEMA_LINEAGE: &str = "v1";

/// 对 v1 的精确 table/column 清单计算 SHA-256。`migration::validate_v1_shape` 会逐表拒绝
/// 缺列和多余列，因此该字面量既是 lineage 标识，也是升级前备份的 shape witness。
pub(crate) const CURRENT_V1_SCHEMA_FINGERPRINT: &str =
    "columns-sha256:c235e96f250e780f62241b55a9721b14b5ebe9244172e01a5655e16af6d18d00";

pub(crate) const FULL_SCHEMA_VERSION: i64 = 2;
pub(crate) const FULL_SCHEMA_NAME: &str = "002_turso_full_feature_baseline";

/// 完整 feature migration 新增的表。这里使用 Turso-native additive migration；本 crate
/// 不执行旧 SQLite v30 的 table-rebuild 脚本。
pub(crate) const FULL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_identity (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  family TEXT NOT NULL,
  lineage TEXT NOT NULL,
  version INTEGER NOT NULL CHECK(version >= 1),
  fingerprint TEXT NOT NULL,
  migration_checksum TEXT NOT NULL,
  upgraded_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_capabilities (
  capability TEXT PRIMARY KEY,
  available INTEGER NOT NULL CHECK(available IN (0, 1)),
  detail TEXT NOT NULL,
  checked_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_attachments (
  id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
  rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
  content_type TEXT,
  size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
  sha256 TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS labels (
  id TEXT PRIMARY KEY CHECK(id LIKE 'l_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  color TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, name),
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS task_labels (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  label_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(task_id, label_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL CHECK(json_valid(value_json)),
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_subtasks (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_task_id TEXT NOT NULL,
  child_task_id TEXT NOT NULL,
  position INTEGER NOT NULL,
  required INTEGER NOT NULL DEFAULT 1 CHECK(required IN (0, 1)),
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(parent_task_id, child_task_id),
  CHECK(parent_task_id != child_task_id),
  FOREIGN KEY(parent_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(child_task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS entities (
  uri TEXT PRIMARY KEY CHECK(uri LIKE 'kb://%'),
  kind TEXT NOT NULL,
  source_table TEXT NOT NULL,
  source_id TEXT NOT NULL,
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  title TEXT,
  summary TEXT,
  content_hash TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  archived_at INTEGER,
  UNIQUE(source_table, source_id),
  UNIQUE(uri, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS relation_predicates (
  name TEXT PRIMARY KEY,
  domain_kind TEXT,
  range_kind TEXT,
  cardinality TEXT NOT NULL DEFAULT 'many',
  authoritative_store TEXT NOT NULL DEFAULT 'turso',
  description TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entity_relations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  subject_uri TEXT NOT NULL REFERENCES entities(uri) ON DELETE CASCADE,
  predicate TEXT NOT NULL REFERENCES relation_predicates(name) ON DELETE RESTRICT,
  object_uri TEXT NOT NULL REFERENCES entities(uri) ON DELETE CASCADE,
  graph_uri TEXT NOT NULL CHECK(graph_uri LIKE 'kb://%'),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  authoritative_store TEXT NOT NULL DEFAULT 'turso',
  source_table TEXT,
  source_id TEXT,
  source_event_id INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(subject_uri, predicate, object_uri, graph_uri),
  FOREIGN KEY(subject_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE,
  FOREIGN KEY(object_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS projection_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  source_event_id INTEGER REFERENCES task_events(id) ON DELETE SET NULL,
  target TEXT NOT NULL CHECK(target IN ('fts', 'vector_tasks', 'vector_label_atoms', 'relations', 'all')),
  entity_uri TEXT CHECK(entity_uri IS NULL OR entity_uri LIKE 'kb://%'),
  dedupe_key TEXT,
  operation TEXT NOT NULL CHECK(operation IN ('upsert', 'delete', 'rebuild')),
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(payload_json)),
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'running', 'done', 'failed')),
  attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0),
  max_attempts INTEGER NOT NULL DEFAULT 10 CHECK(max_attempts > 0),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at INTEGER,
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  generation TEXT,
  next_attempt_at INTEGER,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK(operation = 'rebuild' OR entity_uri IS NOT NULL),
  CHECK((status = 'running') = (lease_owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS projection_state (
  projection TEXT PRIMARY KEY CHECK(projection IN ('fts', 'vector_tasks', 'vector_label_atoms', 'relations')),
  lifecycle_status TEXT NOT NULL DEFAULT 'bootstrap_required' CHECK(lifecycle_status IN ('bootstrap_required', 'idle', 'rebuilding', 'ready', 'degraded', 'error')),
  active_generation TEXT,
  active_fingerprint TEXT,
  previous_generation TEXT,
  previous_fingerprint TEXT,
  building_generation TEXT,
  building_fingerprint TEXT,
  provider TEXT,
  provider_fingerprint TEXT,
  corpus_schema TEXT,
  corpus_fingerprint TEXT,
  embedding_model TEXT,
  embedding_dimensions INTEGER,
  last_event_id INTEGER NOT NULL DEFAULT 0 CHECK(last_event_id >= 0),
  dirty INTEGER NOT NULL DEFAULT 1 CHECK(dirty IN (0, 1)),
  lease_owner TEXT,
  lease_token TEXT,
  lease_expires_at INTEGER,
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  last_success_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL,
  CHECK(embedding_dimensions IS NULL OR embedding_dimensions > 0),
  CHECK((lease_owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL) OR (lease_owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)),
  CHECK(previous_generation IS NULL OR previous_generation != active_generation),
  CHECK(building_generation IS NULL OR building_generation != active_generation)
);

CREATE TABLE IF NOT EXISTS label_semantics (
  label_id TEXT PRIMARY KEY,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  description TEXT,
  applies_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(applies_when) AND json_type(applies_when) = 'array'),
  excludes_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(excludes_when) AND json_type(excludes_when) = 'array'),
  positive_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(positive_examples) AND json_type(positive_examples) = 'array'),
  negative_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(negative_examples) AND json_type(negative_examples) = 'array'),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_atoms (
  id TEXT PRIMARY KEY CHECK(id LIKE 'la_%'),
  label_id TEXT NOT NULL,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  polarity TEXT NOT NULL CHECK(polarity IN ('positive', 'negative')),
  kind TEXT NOT NULL CHECK(kind IN ('name', 'description', 'applies_when', 'positive_example', 'excludes_when', 'negative_example')),
  text TEXT NOT NULL CHECK(length(trim(text)) > 0),
  ordinal INTEGER NOT NULL DEFAULT 0 CHECK(ordinal >= 0),
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(label_id, polarity, kind, ordinal),
  UNIQUE(label_id, content_hash),
  FOREIGN KEY(label_id, board_id) REFERENCES labels(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_atom_index_boards (
  store_name TEXT NOT NULL,
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  dirty INTEGER NOT NULL DEFAULT 1 CHECK(dirty IN (0, 1)),
  last_rebuild_at INTEGER,
  last_error TEXT,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(store_name, board_id)
);

CREATE TABLE IF NOT EXISTS label_semantic_proposals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'lp_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'proposed' CHECK(status IN ('proposed', 'accepted', 'rejected')),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  description TEXT,
  applies_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(applies_when)),
  excludes_when TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(excludes_when)),
  positive_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(positive_examples)),
  negative_examples TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(negative_examples)),
  heuristic_coverage REAL NOT NULL DEFAULT 0.0 CHECK(heuristic_coverage BETWEEN 0 AND 1),
  heuristic_residual_norm REAL NOT NULL DEFAULT 1.0 CHECK(heuristic_residual_norm BETWEEN 0 AND 1),
  heuristic_coverage_cosine REAL CHECK(heuristic_coverage_cosine IS NULL OR heuristic_coverage_cosine BETWEEN 0 AND 1),
  top1_existing_label_id TEXT,
  top1_existing_label_name TEXT,
  diagnostics_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(diagnostics_json)),
  created_by TEXT NOT NULL,
  decision_reason TEXT,
  resolved_label_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  decided_at INTEGER,
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(top1_existing_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(resolved_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS label_ontology_observations (
  id TEXT PRIMARY KEY CHECK(id LIKE 'lor_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  task_ref_snapshot TEXT NOT NULL CHECK(length(trim(task_ref_snapshot)) > 0),
  task_snapshot_json TEXT NOT NULL CHECK(json_valid(task_snapshot_json)),
  agent_candidates_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(agent_candidates_json)),
  suggestion_snapshot_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(suggestion_snapshot_json)),
  final_decision_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(final_decision_json)),
  suggest_coverage REAL,
  suggest_coverage_cosine REAL,
  suggest_residual_norm REAL,
  suggest_needs_new_label INTEGER NOT NULL DEFAULT 0 CHECK(suggest_needs_new_label IN (0, 1)),
  suggest_degraded INTEGER NOT NULL DEFAULT 0 CHECK(suggest_degraded IN (0, 1)),
  diagnostics_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(diagnostics_json)),
  capture_fingerprint TEXT NOT NULL CHECK(length(trim(capture_fingerprint)) > 0),
  suggest_input_hash TEXT,
  created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
  created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
  agent_type TEXT,
  created_at INTEGER NOT NULL,
  UNIQUE(board_id, capture_fingerprint),
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_ontology_signals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'los_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  observation_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('false_negative', 'false_positive', 'vocabulary_gap', 'name_issue', 'boundary_issue', 'structure_issue')),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'confirmed', 'resolved', 'rejected', 'superseded')),
  target_label_id TEXT,
  target_label_name_snapshot TEXT,
  related_labels_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(related_labels_json)),
  proposed_action TEXT NOT NULL CHECK(proposed_action IN ('observe', 'add_positive_atom', 'add_negative_atom', 'update_semantics', 'bootstrap_label', 'rename_label', 'split_label', 'merge_labels')),
  candidate_atom_polarity TEXT,
  candidate_atom_kind TEXT,
  candidate_text TEXT,
  candidate_content_hash TEXT,
  proposed_label_name TEXT,
  proposed_label_name_normalized TEXT,
  proposal_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(proposal_json)),
  agent_selected INTEGER NOT NULL DEFAULT 0 CHECK(agent_selected IN (0, 1)),
  suggest_state TEXT CHECK(suggest_state IS NULL OR suggest_state IN ('selected', 'candidate', 'absent', 'unavailable')),
  suggest_score REAL,
  suggest_rank INTEGER,
  final_selected INTEGER NOT NULL DEFAULT 0 CHECK(final_selected IN (0, 1)),
  rationale TEXT NOT NULL CHECK(length(trim(rationale)) > 0),
  confidence REAL CHECK(confidence IS NULL OR confidence BETWEEN 0 AND 1),
  signal_key TEXT NOT NULL CHECK(length(trim(signal_key)) > 0),
  superseded_by_signal_id TEXT,
  status_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  reviewed_at INTEGER,
  closed_at INTEGER,
  UNIQUE(observation_id, signal_key),
  UNIQUE(id, board_id),
  FOREIGN KEY(observation_id, board_id) REFERENCES label_ontology_observations(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(target_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(superseded_by_signal_id) REFERENCES label_ontology_signals(id) ON DELETE SET NULL,
  CHECK(id != superseded_by_signal_id)
);

CREATE TABLE IF NOT EXISTS label_ontology_actions (
  id TEXT PRIMARY KEY CHECK(id LIKE 'loa_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  parent_action_id TEXT,
  action_type TEXT NOT NULL CHECK(action_type IN ('confirm', 'reject', 'supersede', 'resolve_no_change', 'add_positive_atom', 'add_negative_atom', 'adopt_existing_atom', 'update_semantics', 'create_label_proposal', 'bootstrap_label', 'rename_label', 'split_label', 'merge_labels', 'validate', 'revert_ontology_mutation')),
  reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
  target_label_id TEXT,
  result_label_id TEXT,
  result_atom_id TEXT,
  result_atom_content_hash TEXT,
  result_proposal_id TEXT,
  canonical_before_hash TEXT,
  canonical_after_hash TEXT,
  change_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(change_json)),
  validation_status TEXT NOT NULL DEFAULT 'not_required' CHECK(validation_status IN ('not_required', 'pending', 'passed', 'failed', 'partial')),
  validation_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(validation_json)),
  validation_requirement TEXT NOT NULL DEFAULT 'none' CHECK(validation_requirement IN ('none', 'required', 'unsupported')),
  created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
  created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
  agent_type TEXT,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(parent_action_id) REFERENCES label_ontology_actions(id) ON DELETE SET NULL,
  FOREIGN KEY(target_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(result_label_id) REFERENCES labels(id) ON DELETE SET NULL,
  FOREIGN KEY(result_proposal_id) REFERENCES label_semantic_proposals(id) ON DELETE SET NULL,
  UNIQUE(id, board_id)
);

CREATE TABLE IF NOT EXISTS label_ontology_action_signals (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL,
  signal_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(action_id, signal_id),
  FOREIGN KEY(action_id, board_id) REFERENCES label_ontology_actions(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(signal_id, board_id) REFERENCES label_ontology_signals(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS label_ontology_action_atom_effects (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL,
  label_id_snapshot TEXT NOT NULL CHECK(length(trim(label_id_snapshot)) > 0),
  atom_id_snapshot TEXT NOT NULL CHECK(atom_id_snapshot LIKE 'la_%'),
  atom_content_hash TEXT NOT NULL CHECK(length(trim(atom_content_hash)) > 0),
  polarity TEXT NOT NULL CHECK(polarity IN ('positive', 'negative')),
  kind TEXT NOT NULL CHECK(kind IN ('name', 'description', 'applies_when', 'positive_example', 'excludes_when', 'negative_example')),
  text TEXT NOT NULL CHECK(length(trim(text)) > 0),
  effect TEXT NOT NULL CHECK(effect IN ('added', 'removed')),
  created_at INTEGER NOT NULL,
  UNIQUE(action_id, atom_content_hash, effect),
  FOREIGN KEY(action_id, board_id) REFERENCES label_ontology_actions(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS signal_observations (
  id TEXT PRIMARY KEY CHECK(id LIKE 'obs_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT,
  run_id TEXT,
  comment_id TEXT,
  task_ref_snapshot TEXT,
  actor TEXT NOT NULL CHECK(length(trim(actor)) > 0),
  agent_type TEXT,
  source TEXT,
  evidence_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(evidence_json) AND json_type(evidence_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE SET NULL,
  FOREIGN KEY(run_id) REFERENCES task_runs(id) ON DELETE SET NULL,
  FOREIGN KEY(comment_id) REFERENCES task_comments(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS signals (
  id TEXT PRIMARY KEY CHECK(id LIKE 'sig_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  observation_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  title TEXT NOT NULL CHECK(length(trim(title)) > 0),
  summary TEXT NOT NULL CHECK(length(trim(summary)) > 0),
  severity TEXT NOT NULL DEFAULT 'info',
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'confirmed', 'rejected', 'superseded', 'resolved')),
  dedupe_key TEXT,
  superseded_by_signal_id TEXT,
  reviewed_by TEXT,
  reviewed_at INTEGER,
  review_reason TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  UNIQUE(observation_id, board_id),
  FOREIGN KEY(observation_id, board_id) REFERENCES signal_observations(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(superseded_by_signal_id) REFERENCES signals(id) ON DELETE SET NULL,
  CHECK(id != superseded_by_signal_id)
);

CREATE TABLE IF NOT EXISTS projection_maintenance_owner (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  owner TEXT,
  lease_token TEXT,
  mode TEXT CHECK(mode IS NULL OR mode IN ('rebuild', 'compact', 'import', 'backup')),
  lease_expires_at INTEGER,
  fence_epoch INTEGER NOT NULL DEFAULT 0 CHECK(fence_epoch >= 0),
  capabilities_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(capabilities_json)),
  build_identity TEXT,
  started_at INTEGER,
  last_heartbeat_at INTEGER,
  updated_at INTEGER NOT NULL,
  CHECK((owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL AND mode IS NULL) OR (owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL AND mode IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS retrieval_documents (
  id TEXT PRIMARY KEY CHECK(id LIKE 'doc_%'),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  entity_uri TEXT REFERENCES entities(uri) ON DELETE CASCADE,
  source_kind TEXT NOT NULL,
  content TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(board_id, entity_uri, source_kind),
  UNIQUE(id, board_id),
  FOREIGN KEY(entity_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS retrieval_vectors (
  id TEXT PRIMARY KEY CHECK(id LIKE 'vec_%'),
  board_id TEXT REFERENCES boards(id) ON DELETE CASCADE,
  entity_uri TEXT REFERENCES entities(uri) ON DELETE CASCADE,
  document_id TEXT REFERENCES retrieval_documents(id) ON DELETE CASCADE,
  embedding BLOB NOT NULL,
  dimensions INTEGER NOT NULL CHECK(dimensions > 0),
  embedding_model TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(document_id, embedding_model),
  FOREIGN KEY(entity_uri, board_id) REFERENCES entities(uri, board_id) ON DELETE CASCADE,
  FOREIGN KEY(document_id, board_id) REFERENCES retrieval_documents(id, board_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS import_journal (
  id TEXT PRIMARY KEY CHECK(id LIKE 'ij_%'),
  source_kind TEXT NOT NULL CHECK(source_kind IN ('jsonl', 'sqlite_v30')),
  source_path TEXT NOT NULL,
  snapshot_fingerprint TEXT NOT NULL,
  phase TEXT NOT NULL CHECK(phase IN ('prepared', 'staged', 'validated', 'published', 'completed', 'failed')),
  staged_database_path TEXT,
  staged_attachment_root TEXT,
  canonical_attachment_root TEXT,
  manifest_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(manifest_json)),
  previous_identity_json TEXT CHECK(previous_identity_json IS NULL OR json_valid(previous_identity_json)),
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS attachment_staging (
  id TEXT PRIMARY KEY CHECK(id LIKE 'as_%'),
  journal_id TEXT NOT NULL REFERENCES import_journal(id) ON DELETE CASCADE,
  attachment_id TEXT NOT NULL,
  source_rel_path TEXT NOT NULL,
  staged_rel_path TEXT NOT NULL,
  expected_sha256 TEXT,
  expected_size_bytes INTEGER NOT NULL CHECK(expected_size_bytes >= 0),
  observed_sha256 TEXT,
  observed_size_bytes INTEGER,
  phase TEXT NOT NULL DEFAULT 'planned' CHECK(phase IN ('planned', 'copied', 'verified', 'published', 'failed')),
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(journal_id, attachment_id)
);

CREATE INDEX IF NOT EXISTS idx_task_attachments_task_created ON task_attachments(task_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_task_labels_label ON task_labels(label_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_subtasks_parent_position ON task_subtasks(parent_task_id, position);
CREATE INDEX IF NOT EXISTS idx_entities_board_kind ON entities(board_id, kind, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_entities_task ON entities(task_id);
CREATE INDEX IF NOT EXISTS idx_entity_relations_subject ON entity_relations(subject_uri);
CREATE INDEX IF NOT EXISTS idx_entity_relations_object ON entity_relations(object_uri);
CREATE INDEX IF NOT EXISTS idx_projection_jobs_ready ON projection_jobs(status, next_attempt_at, updated_at ASC);
CREATE INDEX IF NOT EXISTS idx_projection_jobs_board ON projection_jobs(board_id, status, updated_at ASC);
CREATE INDEX IF NOT EXISTS idx_projection_jobs_lease ON projection_jobs(lease_owner, lease_expires_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_projection_jobs_dedupe ON projection_jobs(target, dedupe_key) WHERE dedupe_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_projection_state_dirty ON projection_state(dirty, updated_at ASC);
CREATE INDEX IF NOT EXISTS idx_label_semantics_board_updated ON label_semantics(board_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_label_atoms_board_kind ON label_atoms(board_id, polarity, kind, ordinal);
CREATE INDEX IF NOT EXISTS idx_label_atoms_label_ordinal ON label_atoms(label_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_label_proposals_board_status ON label_semantic_proposals(board_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_label_proposals_task_status ON label_semantic_proposals(task_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_observation_task ON label_ontology_observations(task_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_signal_status ON label_ontology_signals(board_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_signal_label_kind ON label_ontology_signals(board_id, target_label_id, kind, status);
CREATE INDEX IF NOT EXISTS idx_ontology_signal_candidate_atom ON label_ontology_signals(board_id, candidate_content_hash, status) WHERE candidate_content_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ontology_signal_proposed_label ON label_ontology_signals(board_id, proposed_label_name_normalized, status) WHERE proposed_label_name_normalized IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ontology_action_created ON label_ontology_actions(board_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_action_label ON label_ontology_actions(board_id, target_label_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ontology_action_create_proposal ON label_ontology_actions(board_id, result_proposal_id) WHERE action_type='create_label_proposal' AND result_proposal_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ontology_action_signals_signal ON label_ontology_action_signals(signal_id, action_id);
CREATE INDEX IF NOT EXISTS idx_ontology_action_atom_effects_hash ON label_ontology_action_atom_effects(board_id, atom_content_hash, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ontology_action_atom_effects_label ON label_ontology_action_atom_effects(board_id, label_id_snapshot, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signal_observation_created ON signal_observations(board_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signal_observation_task ON signal_observations(board_id, task_id, created_at DESC) WHERE task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_signals_status ON signals(board_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signals_observation ON signals(observation_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_signals_dedupe_key ON signals(board_id, dedupe_key) WHERE dedupe_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_retrieval_documents_board ON retrieval_documents(board_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_retrieval_vectors_board ON retrieval_vectors(board_id, embedding_model);
CREATE INDEX IF NOT EXISTS idx_import_journal_phase ON import_journal(phase, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_import_journal_fingerprint ON import_journal(source_kind, snapshot_fingerprint);
CREATE INDEX IF NOT EXISTS idx_attachment_staging_phase ON attachment_staging(journal_id, phase);
CREATE TRIGGER IF NOT EXISTS task_events_board_guard_insert
BEFORE INSERT ON task_events
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id = NEW.task_id AND board_id = NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id = NEW.run_id AND board_id = NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'task_events reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS task_events_board_guard_update
BEFORE UPDATE OF board_id, task_id, run_id ON task_events
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id = NEW.task_id AND board_id = NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id = NEW.run_id AND board_id = NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'task_events reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_semantic_proposals_board_guard_insert
BEFORE INSERT ON label_semantic_proposals
WHEN (NEW.top1_existing_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.top1_existing_label_id AND board_id=NEW.board_id
)) OR (NEW.resolved_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.resolved_label_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_semantic_proposals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_semantic_proposals_board_guard_update
BEFORE UPDATE OF board_id, top1_existing_label_id, resolved_label_id ON label_semantic_proposals
WHEN (NEW.top1_existing_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.top1_existing_label_id AND board_id=NEW.board_id
)) OR (NEW.resolved_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.resolved_label_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_semantic_proposals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_signals_board_guard_insert
BEFORE INSERT ON label_ontology_signals
WHEN (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_signals
  WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_signals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_signals_board_guard_update
BEFORE UPDATE OF board_id, target_label_id, superseded_by_signal_id ON label_ontology_signals
WHEN (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_signals
  WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_signals reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_actions_board_guard_insert
BEFORE INSERT ON label_ontology_actions
WHEN (NEW.parent_action_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_actions WHERE id=NEW.parent_action_id AND board_id=NEW.board_id
)) OR (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.result_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.result_label_id AND board_id=NEW.board_id
)) OR (NEW.result_proposal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_semantic_proposals
  WHERE id=NEW.result_proposal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_actions reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS label_ontology_actions_board_guard_update
BEFORE UPDATE OF board_id, parent_action_id, target_label_id, result_label_id, result_proposal_id ON label_ontology_actions
WHEN (NEW.parent_action_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_ontology_actions WHERE id=NEW.parent_action_id AND board_id=NEW.board_id
)) OR (NEW.target_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.target_label_id AND board_id=NEW.board_id
)) OR (NEW.result_label_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM labels WHERE id=NEW.result_label_id AND board_id=NEW.board_id
)) OR (NEW.result_proposal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM label_semantic_proposals
  WHERE id=NEW.result_proposal_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_actions reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signal_observations_board_guard_insert
BEFORE INSERT ON signal_observations
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id=NEW.task_id AND board_id=NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id=NEW.run_id AND board_id=NEW.board_id
)) OR (NEW.comment_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_comments WHERE id=NEW.comment_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'signal_observations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signal_observations_board_guard_update
BEFORE UPDATE OF board_id, task_id, run_id, comment_id ON signal_observations
WHEN (NEW.task_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM tasks WHERE id=NEW.task_id AND board_id=NEW.board_id
)) OR (NEW.run_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_runs WHERE id=NEW.run_id AND board_id=NEW.board_id
)) OR (NEW.comment_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_comments WHERE id=NEW.comment_id AND board_id=NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'signal_observations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signals_board_guard_insert
BEFORE INSERT ON signals
WHEN NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM signals WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'signals superseded reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS signals_board_guard_update
BEFORE UPDATE OF board_id, superseded_by_signal_id ON signals
WHEN NEW.superseded_by_signal_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM signals WHERE id=NEW.superseded_by_signal_id AND board_id=NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'signals superseded reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS entity_relations_board_guard_insert
BEFORE INSERT ON entity_relations
WHEN NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.subject_uri AND board_id IS NEW.board_id
) OR NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.object_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'entity_relations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS entity_relations_board_guard_update
BEFORE UPDATE OF board_id, subject_uri, object_uri ON entity_relations
WHEN NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.subject_uri AND board_id IS NEW.board_id
) OR NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.object_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'entity_relations reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS projection_jobs_board_guard_insert
BEFORE INSERT ON projection_jobs
WHEN (NEW.source_event_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_events WHERE id = NEW.source_event_id AND board_id IS NEW.board_id
)) OR (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS projection_jobs_board_guard_update
BEFORE UPDATE OF board_id, source_event_id, entity_uri ON projection_jobs
WHEN (NEW.source_event_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM task_events WHERE id = NEW.source_event_id AND board_id IS NEW.board_id
)) OR (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'projection_jobs reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_documents_board_guard_insert
BEFORE INSERT ON retrieval_documents
WHEN NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'retrieval_documents entity board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_documents_board_guard_update
BEFORE UPDATE OF board_id, entity_uri ON retrieval_documents
WHEN NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)
BEGIN
  SELECT RAISE(ABORT, 'retrieval_documents entity board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_vectors_board_guard_insert
BEFORE INSERT ON retrieval_vectors
WHEN (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)) OR (NEW.document_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM retrieval_documents WHERE id = NEW.document_id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'retrieval_vectors reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS retrieval_vectors_board_guard_update
BEFORE UPDATE OF board_id, entity_uri, document_id ON retrieval_vectors
WHEN (NEW.entity_uri IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM entities WHERE uri = NEW.entity_uri AND board_id IS NEW.board_id
)) OR (NEW.document_id IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM retrieval_documents WHERE id = NEW.document_id AND board_id IS NEW.board_id
))
BEGIN
  SELECT RAISE(ABORT, 'retrieval_vectors reference board mismatch');
END;

CREATE TRIGGER IF NOT EXISTS task_attachments_path_guard_insert
BEFORE INSERT ON task_attachments
WHEN NEW.rel_path LIKE '/%'
  OR NEW.rel_path LIKE '\%'
  OR instr('/' || replace(NEW.rel_path, '\', '/') || '/', '/../') > 0
BEGIN
  SELECT RAISE(ABORT, 'attachment rel_path escapes database directory');
END;

CREATE TRIGGER IF NOT EXISTS task_attachments_path_guard_update
BEFORE UPDATE OF rel_path ON task_attachments
WHEN NEW.rel_path LIKE '/%'
  OR NEW.rel_path LIKE '\%'
  OR instr('/' || replace(NEW.rel_path, '\', '/') || '/', '/../') > 0
BEGIN
  SELECT RAISE(ABORT, 'attachment rel_path escapes database directory');
END;
"#;

/// FTS index 独立于 canonical schema checksum，但 `kanban-store-turso` 必须启用
/// Turso 的 `fts` feature；初始化若不能创建该索引，会把 capability 记录为不可用。
pub(crate) const FTS_SCHEMA: &str = "CREATE INDEX IF NOT EXISTS idx_retrieval_documents_fts ON retrieval_documents USING fts (content);";

pub(crate) const DEFAULT_COLUMNS: [(&str, &str, i64, bool); 9] = [
    ("triage", "Triage", 10, false),
    ("todo", "Todo", 20, false),
    ("scheduled", "Scheduled", 30, false),
    ("ready", "Ready", 40, false),
    ("running", "Running", 50, false),
    ("blocked", "Blocked", 60, false),
    ("review", "Review", 70, false),
    ("done", "Done", 80, false),
    ("archived", "Archived", 90, true),
];
