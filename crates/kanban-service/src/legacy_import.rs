//! 旧 SQLite v30 的只读逻辑导入器。
//!
//! 源数据库只读打开并在任何 Turso 写入前完成 schema、完整性、引用、board isolation
//! 和附件 checksum 预检。旧 projection/outbox 表仅用于识别来源，不作为事实迁移。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{
    Connection as SqliteConnection, OpenFlags, OptionalExtension, types::Value as SqlValue,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use turso::{Connection as TursoConnection, Value, transaction::TransactionBehavior};

use crate::{StoreError, TursoStore, schema, shared::now_ms};

const SOURCE_KIND: &str = "sqlite_v30";
const SOURCE_VERSION: i64 = 30;

/// SQLite v30 导入选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportOptions {
    pub source_path: PathBuf,
    pub canonical_attachment_root: Option<PathBuf>,
}

impl LegacyImportOptions {
    pub fn new(source_path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: source_path.into(),
            canonical_attachment_root: None,
        }
    }

    pub fn with_canonical_attachment_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.canonical_attachment_root = Some(root.into());
        self
    }
}

/// 单个 canonical 表的行数证明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportTableCount {
    pub table: String,
    pub source_rows: u64,
    pub target_rows: u64,
}

/// 导入结果。`source_fingerprint` 是 SQLite 文件、WAL 和附件快照的 SHA-256。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportResult {
    pub journal_id: String,
    pub phase: String,
    pub source_path: PathBuf,
    pub source_fingerprint: String,
    pub schema_fingerprint: String,
    pub resumed: bool,
    pub attachment_count: u64,
    pub table_counts: Vec<LegacyImportTableCount>,
}

pub type LegacySqliteImportOptions = LegacyImportOptions;
pub type LegacySqliteImportResult = LegacyImportResult;

/// v30 最终表集合。表名之外的 SQLite internal table 不计入识别。
const LEGACY_TABLES: &[&str] = &[
    "app_settings",
    "board_columns",
    "boards",
    "derived_store_state",
    "entities",
    "entity_relations",
    "index_outbox",
    "label_atom_index_boards",
    "label_atoms",
    "label_ontology_action_atom_effects",
    "label_ontology_action_signals",
    "label_ontology_actions",
    "label_ontology_observations",
    "label_ontology_signals",
    "label_semantic_proposals",
    "label_semantics",
    "labels",
    "projection_database",
    "projection_deliveries",
    "projection_maintenance_owner",
    "projection_store_state",
    "relation_predicates",
    "schema_migrations",
    "signal_observations",
    "signals",
    "task_attachments",
    "task_comments",
    "task_dependencies",
    "task_events",
    "task_execution_plans",
    "task_labels",
    "task_runs",
    "task_steps",
    "task_subtasks",
    "tasks",
];

/// 旧迁移留下的完整列 manifest。逗号分隔格式让表 fingerprint 保持在一个可审阅的常量内。
const LEGACY_COLUMNS: &[(&str, &str)] = &[
    ("app_settings", "key,value_json,updated_at"),
    (
        "board_columns",
        "id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at",
    ),
    (
        "boards",
        "id,slug,name,description,created_at,updated_at,archived_at",
    ),
    (
        "derived_store_state",
        "store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at",
    ),
    (
        "entities",
        "uri,kind,source_table,source_id,board_id,task_id,title,summary,content_hash,created_at,updated_at,archived_at",
    ),
    (
        "entity_relations",
        "id,subject_uri,predicate,object_uri,graph_uri,authoritative_store,source_table,source_id,source_event_id,metadata_json,created_at,updated_at",
    ),
    (
        "index_outbox",
        "id,source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,created_at,updated_at,projection_store",
    ),
    (
        "label_atom_index_boards",
        "store_name,board_id,dirty,last_rebuild_at,last_error,updated_at",
    ),
    (
        "label_atoms",
        "id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at",
    ),
    (
        "label_ontology_action_atom_effects",
        "board_id,action_id,label_id_snapshot,atom_id_snapshot,atom_content_hash,polarity,kind,text,effect,created_at",
    ),
    (
        "label_ontology_action_signals",
        "board_id,action_id,signal_id,created_at",
    ),
    (
        "label_ontology_actions",
        "id,board_id,parent_action_id,action_type,reason,target_label_id,result_label_id,result_atom_id,result_atom_content_hash,result_proposal_id,canonical_before_hash,canonical_after_hash,change_json,validation_status,validation_json,created_by,created_by_type,agent_type,created_at,validation_requirement",
    ),
    (
        "label_ontology_observations",
        "id,board_id,task_id,task_ref_snapshot,task_snapshot_json,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,created_by,created_by_type,agent_type,created_at,suggest_input_hash",
    ),
    (
        "label_ontology_signals",
        "id,observation_id,board_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,superseded_by_signal_id,status_reason,created_at,updated_at,reviewed_at,closed_at",
    ),
    (
        "label_semantic_proposals",
        "id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_residual_norm,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,decision_reason,resolved_label_id,created_at,updated_at,decided_at,heuristic_coverage_cosine",
    ),
    (
        "label_semantics",
        "label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at",
    ),
    ("labels", "id,board_id,name,color,created_at,updated_at"),
    (
        "projection_database",
        "singleton,database_instance_id,protocol_version,created_at,updated_at",
    ),
    (
        "projection_deliveries",
        "id,outbox_id,store_name,board_id,source_event_id,cursor,action,entity_uri,payload_json,status,attempts,next_attempt_at,claim_owner,claim_token,claim_lease_token,claim_fence_epoch,claim_generation,claim_expires_at,published_generation,last_error,created_at,updated_at",
    ),
    (
        "projection_maintenance_owner",
        "singleton,owner,lease_token,lease_expires_at,mode,started_at,last_heartbeat_at,updated_at,capabilities_json,build_identity",
    ),
    (
        "projection_store_state",
        "store_name,database_instance_id,protocol_version,schema_version,control_plane,active_generation,active_fingerprint,active_fence_epoch,active_snapshot_cursor,active_provider,active_provider_fingerprint,active_canonical_count,active_canonical_digest,active_delivery_count,active_delivery_digest,previous_generation,previous_fingerprint,previous_fence_epoch,previous_snapshot_cursor,previous_provider,previous_provider_fingerprint,previous_canonical_count,previous_canonical_digest,previous_delivery_count,previous_delivery_digest,building_generation,building_fingerprint,building_fence_epoch,building_provider,building_provider_fingerprint,building_canonical_count,building_canonical_digest,building_delivery_count,building_delivery_digest,building_phase,snapshot_cursor,checkpoint_cursor,legacy_checkpoint_cursor,lifecycle_status,fence_epoch,lease_owner,lease_token,lease_expires_at,last_success_at,last_error,updated_at,active_corpus_schema,active_corpus_fingerprint,active_embedding_model,active_embedding_dimensions,previous_corpus_schema,previous_corpus_fingerprint,previous_embedding_model,previous_embedding_dimensions,building_corpus_schema,building_corpus_fingerprint,building_embedding_model,building_embedding_dimensions",
    ),
    (
        "relation_predicates",
        "name,domain_kind,range_kind,cardinality,authoritative_store,description,created_at",
    ),
    ("schema_migrations", "version,name,checksum,applied_at"),
    (
        "signal_observations",
        "id,board_id,task_id,task_ref_snapshot,run_id,comment_id,actor,agent_type,source,evidence_json,created_at",
    ),
    (
        "signals",
        "id,board_id,observation_id,kind,title,summary,severity,status,dedupe_key,superseded_by_signal_id,reviewed_by,reviewed_at,review_reason,created_at,updated_at",
    ),
    (
        "task_attachments",
        "id,board_id,task_id,filename,rel_path,content_type,size_bytes,sha256,created_by,created_at",
    ),
    (
        "task_comments",
        "id,board_id,task_id,author,author_type,agent_type,body,kind,metadata_json,created_at",
    ),
    (
        "task_dependencies",
        "board_id,parent_task_id,child_task_id,created_at",
    ),
    (
        "task_events",
        "id,event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at",
    ),
    (
        "task_execution_plans",
        "board_id,task_id,state,reason,updated_by,updated_at",
    ),
    ("task_labels", "board_id,task_id,label_id,created_at"),
    (
        "task_runs",
        "id,board_id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,claim_expires_at,started_at,last_heartbeat_at,finished_at,exit_code,summary,error,log_path,metadata_json",
    ),
    (
        "task_steps",
        "id,board_id,parent_task_id,position,title,body,linked_task_id,required,status,resolution_note,resolved_by,resolved_at,created_by,created_at,updated_by,updated_at",
    ),
    (
        "task_subtasks",
        "board_id,parent_task_id,child_task_id,position,required,created_by,created_at",
    ),
    (
        "tasks",
        "id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version",
    ),
];

/// 不迁移的 projection/outbox 表，仅作为 source shape witness。
const CANONICAL_TABLES: &[&str] = &[
    "boards",
    "board_columns",
    "tasks",
    "task_execution_plans",
    "task_steps",
    "task_dependencies",
    "task_runs",
    "task_comments",
    "task_events",
    "task_attachments",
    "labels",
    "task_labels",
    "app_settings",
    "task_subtasks",
    "entities",
    "relation_predicates",
    "entity_relations",
    "label_semantics",
    "label_atoms",
    "label_semantic_proposals",
    "label_ontology_observations",
    "label_ontology_signals",
    "label_ontology_actions",
    "label_ontology_action_signals",
    "label_ontology_action_atom_effects",
    "signal_observations",
    "signals",
];

const MIGRATION_NAMES: &[&str] = &[
    "001_initial",
    "002_knowledge_substrate",
    "003_comment_author_identity",
    "004_priority_levels",
    "005_decision_comment_kind",
    "006_comment_metadata_contract",
    "007_label_semantics_atoms",
    "008_label_atom_index_boards",
    "009_label_semantic_proposals",
    "010_stable_label_atom_hashes",
    "011_label_proposal_cosine_coverage",
    "012_label_ontology_ledger",
    "013_label_ontology_suggest_input_hash",
    "014_unique_label_proposal_create_action",
    "015_adopt_existing_atom_action",
    "016_revert_ontology_mutation_action",
    "017_board_isolation_composite_fk",
    "018_label_ontology_root_action_effects",
    "019_label_ontology_validation_requirement",
    "020_board_isolation_task_history",
    "021_board_isolation_ontology_links",
    "022_task_subtasks_execution_plans",
    "023_task_steps",
    "024_signal_ledger",
    "025_generic_signal_ledger",
    "026_projection_v2",
    "027_projection_maintenance_owner",
    "028_projection_maintenance_runtime_identity",
    "029_projection_label_atom_deliveries",
    "030_projection_corpus_bindings",
];

/// v30 迁移脚本的 FNV-1a ledger checksum。v1/v4 的历史数据库允许旧初始化脚本曾使用过
/// 的兼容 checksum；其余版本必须与这组脚本 checksum 精确一致。
const MIGRATION_CHECKSUMS: &[&str] = &[
    "fnv64:c2b3cfdaf5fd0ac9",
    "fnv64:a549c98f375abb33",
    "fnv64:d753b64649f2d5e8",
    "fnv64:8e71302408c0f5ec",
    "fnv64:5f57751fa5ae355b",
    "fnv64:5722037d819848d2",
    "fnv64:49ab6f02badb38b9",
    "fnv64:d95bb151e044abc1",
    "fnv64:90d55fe98f14936d",
    "fnv64:a751d75a3e5f8baf",
    "fnv64:290fbadac17c29f1",
    "fnv64:6d8c91d4e8e19867",
    "fnv64:62c9ab9e70f13de6",
    "fnv64:695a4deb53af8a8f",
    "fnv64:48083714dafd3134",
    "fnv64:8f0929c89221a551",
    "fnv64:35e5380b866144cf",
    "fnv64:67124280774a3ab3",
    "fnv64:4e9fa46c02814766",
    "fnv64:ec251890669cc15c",
    "fnv64:03f363173d517df3",
    "fnv64:6fce00e46a30ddcf",
    "fnv64:0c7dd431257a6946",
    "fnv64:2401135db5f7d807",
    "fnv64:d8bf6ea31135dc83",
    "fnv64:c5eddec1f4511bae",
    "fnv64:5df731a27efdae55",
    "fnv64:7ea454008b72e2fc",
    "fnv64:f41cb49971216fe0",
    "fnv64:ad2e4075068e7794",
];

const LEGACY_CHECKSUM_ALIASES: &[(i64, &[&str])] = &[
    (
        1,
        &[
            "fnv64:0ca871be950fc8a6",
            "fnv64:3b08da4e2b6041f5",
            "fnv64:61b5ea6d6ed1eabe",
        ],
    ),
    (4, &["fnv64:127ec944f1b716ff"]),
];

const SOURCE_SCHEMA_SQL_FINGERPRINT: &str =
    "schema-sql-sha256:79baea5644d0ca2d1a786da97ee1a5794678a3c221f294c8e63705514669709f";

#[derive(Debug)]
struct Snapshot {
    source_path: PathBuf,
    schema_fingerprint: String,
    source_fingerprint: String,
    rows: BTreeMap<String, Vec<Vec<SqlValue>>>,
    columns: BTreeMap<String, Vec<String>>,
    counts: BTreeMap<String, u64>,
    attachments: Vec<Attachment>,
}

/// SQLite 在 WAL 模式下可能需要创建 `-shm`；将源文件及其现有 sidecar 复制到临时目录，
/// 让只读预检不会在用户提供的源目录创建或修改任何 SQLite sidecar。
struct ReadOnlySource {
    directory: PathBuf,
    path: PathBuf,
    wal_path: Option<PathBuf>,
}

impl Drop for ReadOnlySource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[derive(Debug, Clone)]
struct Attachment {
    id: String,
    rel_path: String,
    sha256: String,
    size: i64,
    bytes: Vec<u8>,
}

/// host service 使用的自由函数入口。
pub(crate) async fn import_legacy_sqlite_v30(
    store: &TursoStore,
    options: LegacyImportOptions,
) -> Result<LegacyImportResult, StoreError> {
    import_into_store(store, options).await
}

pub(crate) async fn import_into_store(
    store: &TursoStore,
    options: LegacyImportOptions,
) -> Result<LegacyImportResult, StoreError> {
    let snapshot = read_snapshot(&options.source_path)?;
    store
        .run_with_maintenance_lease("import", "host-admin", || async {
            import_snapshot_into_store(store, options, snapshot).await
        })
        .await
}

async fn import_snapshot_into_store(
    store: &TursoStore,
    options: LegacyImportOptions,
    snapshot: Snapshot,
) -> Result<LegacyImportResult, StoreError> {
    let target_root = options
        .canonical_attachment_root
        .clone()
        .unwrap_or_else(|| {
            store
                .database_path()
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("attachments")
        });
    let mut connection = store.connection().await?;
    let existing = find_journal(&connection, &snapshot.source_fingerprint).await?;
    let initial_counts = target_counts(&connection).await?;
    if let Some((journal_id, phase, staging_root)) = existing {
        if phase == "completed" {
            ensure_count_match(&snapshot.counts, &initial_counts)?;
            return Ok(result(&snapshot, journal_id, phase, false, initial_counts));
        }
        let all = counts_match(&snapshot.counts, &initial_counts);
        let none = logical_empty(&connection, &initial_counts).await?;
        if !all && !none {
            return Err(error(
                "同一 fingerprint 的 journal 处于部分提交状态，拒绝猜测恢复",
            ));
        }
        let staged = stage_attachments(&snapshot, &staging_root)?;
        if none {
            write_transaction(
                &mut connection,
                &snapshot,
                &journal_id,
                &target_root,
                &staging_root,
                &staged,
                true,
            )
            .await?;
        }
        publish(
            &mut connection,
            &journal_id,
            &staging_root,
            &target_root,
            &staged,
        )
        .await?;
        let counts = target_counts(&connection).await?;
        ensure_count_match(&snapshot.counts, &counts)?;
        return Ok(result(
            &snapshot,
            journal_id,
            "completed".to_owned(),
            true,
            counts,
        ));
    }
    if !logical_empty(&connection, &initial_counts).await? {
        return Err(error("目标 Turso canonical 数据库不是逻辑空库"));
    }
    let journal_id = format!(
        "ij_{}",
        &snapshot.source_fingerprint.trim_start_matches("sha256:")[..24]
    );
    let staging_root = staging_root(store.database_path(), &journal_id);
    let staged = stage_attachments(&snapshot, &staging_root)?;
    write_transaction(
        &mut connection,
        &snapshot,
        &journal_id,
        &target_root,
        &staging_root,
        &staged,
        true,
    )
    .await?;
    publish(
        &mut connection,
        &journal_id,
        &staging_root,
        &target_root,
        &staged,
    )
    .await?;
    let counts = target_counts(&connection).await?;
    ensure_count_match(&snapshot.counts, &counts)?;
    Ok(result(
        &snapshot,
        journal_id,
        "completed".to_owned(),
        false,
        counts,
    ))
}

fn error(message: impl Into<String>) -> StoreError {
    StoreError::LegacyImport(message.into())
}

fn result(
    snapshot: &Snapshot,
    journal_id: String,
    phase: String,
    resumed: bool,
    target: BTreeMap<String, u64>,
) -> LegacyImportResult {
    LegacyImportResult {
        journal_id,
        phase,
        source_path: snapshot.source_path.clone(),
        source_fingerprint: snapshot.source_fingerprint.clone(),
        schema_fingerprint: snapshot.schema_fingerprint.clone(),
        resumed,
        attachment_count: snapshot.attachments.len() as u64,
        table_counts: CANONICAL_TABLES
            .iter()
            .map(|table| LegacyImportTableCount {
                table: (*table).to_owned(),
                source_rows: snapshot.counts.get(*table).copied().unwrap_or_default(),
                target_rows: target.get(*table).copied().unwrap_or_default(),
            })
            .collect(),
    }
}

fn read_snapshot(path: &Path) -> Result<Snapshot, StoreError> {
    let path = fs::canonicalize(path).map_err(|e| error(format!("解析源 SQLite 路径失败: {e}")))?;
    let readonly = copy_source_for_read(&path)?;
    let connection =
        SqliteConnection::open_with_flags(&readonly.path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| error(format!("只读打开源 SQLite 失败: {e}")))?;
    verify_schema(&connection)?;
    let mut columns = BTreeMap::new();
    for (table, manifest) in LEGACY_COLUMNS {
        columns.insert(
            (*table).to_owned(),
            manifest
                .split(',')
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
        );
    }
    let mut rows = BTreeMap::new();
    let mut counts = BTreeMap::new();
    for table in CANONICAL_TABLES {
        let cols = columns
            .get(*table)
            .ok_or_else(|| error(format!("缺少源列 manifest: {table}")))?;
        let table_rows = read_rows(&connection, table, cols)?;
        counts.insert((*table).to_owned(), table_rows.len() as u64);
        rows.insert((*table).to_owned(), table_rows);
    }
    verify_integrity(&connection)?;
    verify_board_isolation(&connection)?;
    let source_root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("attachments");
    let attachments = read_attachments(
        rows.get("task_attachments")
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        &source_root,
        &columns,
    )?;
    let schema_fingerprint = schema_fingerprint();
    let snapshot_database =
        fs::read(&readonly.path).map_err(|e| error(format!("读取 SQLite 快照失败: {e}")))?;
    let snapshot_wal = readonly
        .wal_path
        .as_deref()
        .map(fs::read)
        .transpose()
        .map_err(|e| error(format!("读取 SQLite WAL 快照失败: {e}")))?;
    let source_fingerprint =
        source_fingerprint(&snapshot_database, snapshot_wal.as_deref(), &attachments);
    Ok(Snapshot {
        source_path: path,
        schema_fingerprint,
        source_fingerprint,
        rows,
        columns,
        counts,
        attachments,
    })
}

fn copy_source_for_read(path: &Path) -> Result<ReadOnlySource, StoreError> {
    let base = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| error(format!("生成只读快照临时目录名失败: {e}")))?
        .as_nanos();
    let mut directory = None;
    for attempt in 0..100_u32 {
        let candidate = base.join(format!(
            "kanban-legacy-import-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                directory = Some(candidate);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(io_error) => {
                return Err(error(format!("创建只读 SQLite 临时目录失败: {io_error}")));
            }
        }
    }
    let directory = directory.ok_or_else(|| error("无法分配只读 SQLite 临时目录"))?;
    let copied_path = directory.join("source.sqlite");
    if let Err(io_error) = fs::copy(path, &copied_path) {
        let _ = fs::remove_dir_all(&directory);
        return Err(error(format!(
            "复制源 SQLite 到只读临时目录失败: {io_error}"
        )));
    }
    let mut wal_path = None;
    for suffix in ["-wal", "-shm"] {
        let source_sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
        if source_sidecar.is_file() {
            let target_sidecar = PathBuf::from(format!("{}{suffix}", copied_path.display()));
            if let Err(io_error) = fs::copy(&source_sidecar, target_sidecar) {
                let _ = fs::remove_dir_all(&directory);
                return Err(error(format!(
                    "复制源 SQLite {suffix} sidecar 失败: {io_error}"
                )));
            }
            if suffix == "-wal" {
                wal_path = Some(PathBuf::from(format!("{}-wal", copied_path.display())));
            }
        }
    }
    Ok(ReadOnlySource {
        directory,
        path: copied_path,
        wal_path,
    })
}

fn verify_schema(connection: &SqliteConnection) -> Result<(), StoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| error(format!("读取 user_version 失败: {e}")))?;
    if version != SOURCE_VERSION {
        return Err(error(format!(
            "源 SQLite user_version={version}，需要精确 v30"
        )));
    }
    let actual = connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_internal_%' ORDER BY name")
        .and_then(|mut stmt| stmt.query_map([], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>())
        .map_err(|e| error(format!("读取源 table manifest 失败: {e}")))?.into_iter().collect::<BTreeSet<_>>();
    let expected = LEGACY_TABLES
        .iter()
        .map(|x| (*x).to_owned())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(error(format!(
            "源 SQLite table fingerprint 不匹配: missing={:?}, unexpected={:?}",
            expected.difference(&actual).collect::<Vec<_>>(),
            actual.difference(&expected).collect::<Vec<_>>()
        )));
    }
    for (table, manifest) in LEGACY_COLUMNS {
        let expected = manifest.split(',').collect::<Vec<_>>();
        let actual = connection
            .prepare(&format!("PRAGMA table_info({})", quote(table)))
            .map_err(|e| error(format!("读取 {table} columns 失败: {e}")))?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| error(format!("读取 {table} columns 失败: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| error(format!("读取 {table} columns 失败: {e}")))?;
        if actual != expected {
            return Err(error(format!(
                "源表 {table} column fingerprint 不匹配: expected={expected:?}, actual={actual:?}"
            )));
        }
    }
    let mut schema_digest = Sha256::new();
    let mut schema_stmt = connection
        .prepare("SELECT type,name,tbl_name,COALESCE(sql,'') FROM sqlite_master WHERE type IN ('table','index','trigger') AND name NOT LIKE 'sqlite_%' ORDER BY type,name")
        .map_err(|e| error(format!("读取源 schema SQL fingerprint 失败: {e}")))?;
    let schema_rows = schema_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| error(format!("读取源 schema SQL fingerprint 失败: {e}")))?;
    for row in schema_rows {
        let (kind, name, table, sql) =
            row.map_err(|e| error(format!("读取源 schema SQL fingerprint 失败: {e}")))?;
        for value in [kind, name, table, sql] {
            schema_digest.update(value.as_bytes());
            schema_digest.update([0]);
        }
    }
    let observed = format!("schema-sql-sha256:{:x}", schema_digest.finalize());
    if observed != SOURCE_SCHEMA_SQL_FINGERPRINT {
        return Err(error(format!(
            "源 SQLite constraint/index/trigger fingerprint 不匹配: {observed}"
        )));
    }
    let rows = connection
        .prepare("SELECT version,name,checksum FROM schema_migrations ORDER BY version")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|e| error(format!("读取 schema_migrations 失败: {e}")))?;
    if rows.len() != MIGRATION_NAMES.len()
        || rows.iter().enumerate().any(|(i, (v, n, checksum))| {
            *v != i as i64 + 1
                || n != MIGRATION_NAMES[i]
                || (!checksum.eq_ignore_ascii_case(MIGRATION_CHECKSUMS[i])
                    && !LEGACY_CHECKSUM_ALIASES
                        .iter()
                        .find(|(version, _)| *version == *v)
                        .is_some_and(|(_, aliases)| {
                            aliases
                                .iter()
                                .any(|alias| checksum.eq_ignore_ascii_case(alias))
                        }))
        })
    {
        return Err(error(
            "schema_migrations 不是连续且精确的 001..030 name/checksum ledger",
        ));
    }
    Ok(())
}

fn verify_integrity(connection: &SqliteConnection) -> Result<(), StoreError> {
    let integrity: String = connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .map_err(|e| error(format!("源 integrity_check 失败: {e}")))?;
    if !integrity.eq_ignore_ascii_case("ok") {
        return Err(error(format!("源 integrity_check 未通过: {integrity}")));
    }
    let mut stmt = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(|e| error(format!("读取 foreign_key_check 失败: {e}")))?;
    if stmt
        .query([])
        .map_err(|e| error(format!("读取 foreign_key_check 失败: {e}")))?
        .next()
        .map_err(|e| error(format!("读取 foreign_key_check 失败: {e}")))?
        .is_some()
    {
        return Err(error("源 SQLite foreign_key_check 失败"));
    }
    Ok(())
}

fn verify_board_isolation(connection: &SqliteConnection) -> Result<(), StoreError> {
    const CHECK_SQL: &str = r#"
SELECT 'task_execution_plans.task_id' WHERE EXISTS (
  SELECT 1 FROM task_execution_plans child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_steps.parent_task_id' WHERE EXISTS (
  SELECT 1 FROM task_steps child
  LEFT JOIN tasks parent ON parent.id=child.parent_task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_steps.linked_task_id' WHERE EXISTS (
  SELECT 1 FROM task_steps child
  LEFT JOIN tasks parent ON parent.id=child.linked_task_id AND parent.board_id=child.board_id
  WHERE child.linked_task_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'task_dependencies.parent_task_id' WHERE EXISTS (
  SELECT 1 FROM task_dependencies child
  LEFT JOIN tasks parent ON parent.id=child.parent_task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_dependencies.child_task_id' WHERE EXISTS (
  SELECT 1 FROM task_dependencies child
  LEFT JOIN tasks parent ON parent.id=child.child_task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_runs.task_id' WHERE EXISTS (
  SELECT 1 FROM task_runs child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_comments.task_id' WHERE EXISTS (
  SELECT 1 FROM task_comments child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_events.task_id' WHERE EXISTS (
  SELECT 1 FROM task_events child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE child.task_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'task_events.run_id' WHERE EXISTS (
  SELECT 1 FROM task_events child
  LEFT JOIN task_runs parent ON parent.id=child.run_id AND parent.board_id=child.board_id
  WHERE child.run_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'task_attachments.task_id' WHERE EXISTS (
  SELECT 1 FROM task_attachments child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_labels.task_id' WHERE EXISTS (
  SELECT 1 FROM task_labels child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_labels.label_id' WHERE EXISTS (
  SELECT 1 FROM task_labels child
  LEFT JOIN labels parent ON parent.id=child.label_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_subtasks.parent_task_id' WHERE EXISTS (
  SELECT 1 FROM task_subtasks child
  LEFT JOIN tasks parent ON parent.id=child.parent_task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'task_subtasks.child_task_id' WHERE EXISTS (
  SELECT 1 FROM task_subtasks child
  LEFT JOIN tasks parent ON parent.id=child.child_task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'entities.task_id' WHERE EXISTS (
  SELECT 1 FROM entities child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE child.task_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_semantics.label_id' WHERE EXISTS (
  SELECT 1 FROM label_semantics child
  LEFT JOIN labels parent ON parent.id=child.label_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_atoms.label_id' WHERE EXISTS (
  SELECT 1 FROM label_atoms child
  LEFT JOIN labels parent ON parent.id=child.label_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_semantic_proposals.task_id' WHERE EXISTS (
  SELECT 1 FROM label_semantic_proposals child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_semantic_proposals.top1_existing_label_id' WHERE EXISTS (
  SELECT 1 FROM label_semantic_proposals child
  LEFT JOIN labels parent ON parent.id=child.top1_existing_label_id AND parent.board_id=child.board_id
  WHERE child.top1_existing_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_semantic_proposals.resolved_label_id' WHERE EXISTS (
  SELECT 1 FROM label_semantic_proposals child
  LEFT JOIN labels parent ON parent.id=child.resolved_label_id AND parent.board_id=child.board_id
  WHERE child.resolved_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_observations.task_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_observations child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_signals.observation_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_signals child
  LEFT JOIN label_ontology_observations parent ON parent.id=child.observation_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_signals.target_label_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_signals child
  LEFT JOIN labels parent ON parent.id=child.target_label_id AND parent.board_id=child.board_id
  WHERE child.target_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_signals.superseded_by_signal_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_signals child
  LEFT JOIN label_ontology_signals parent ON parent.id=child.superseded_by_signal_id AND parent.board_id=child.board_id
  WHERE child.superseded_by_signal_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_actions.parent_action_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_actions child
  LEFT JOIN label_ontology_actions parent ON parent.id=child.parent_action_id AND parent.board_id=child.board_id
  WHERE child.parent_action_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_actions.target_label_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_actions child
  LEFT JOIN labels parent ON parent.id=child.target_label_id AND parent.board_id=child.board_id
  WHERE child.target_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_actions.result_label_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_actions child
  LEFT JOIN labels parent ON parent.id=child.result_label_id AND parent.board_id=child.board_id
  WHERE child.result_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_actions.result_proposal_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_actions child
  LEFT JOIN label_semantic_proposals parent ON parent.id=child.result_proposal_id AND parent.board_id=child.board_id
  WHERE child.result_proposal_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_action_signals.action_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_action_signals child
  LEFT JOIN label_ontology_actions parent ON parent.id=child.action_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_action_signals.signal_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_action_signals child
  LEFT JOIN label_ontology_signals parent ON parent.id=child.signal_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'signal_observations.task_id' WHERE EXISTS (
  SELECT 1 FROM signal_observations child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE child.task_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'signal_observations.run_id' WHERE EXISTS (
  SELECT 1 FROM signal_observations child
  LEFT JOIN task_runs parent ON parent.id=child.run_id AND parent.board_id=child.board_id
  WHERE child.run_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'signal_observations.comment_id' WHERE EXISTS (
  SELECT 1 FROM signal_observations child
  LEFT JOIN task_comments parent ON parent.id=child.comment_id AND parent.board_id=child.board_id
  WHERE child.comment_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'signals.observation_id' WHERE EXISTS (
  SELECT 1 FROM signals child
  LEFT JOIN signal_observations parent ON parent.id=child.observation_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'signals.superseded_by_signal_id' WHERE EXISTS (
  SELECT 1 FROM signals child
  LEFT JOIN signals parent ON parent.id=child.superseded_by_signal_id AND parent.board_id=child.board_id
  WHERE child.superseded_by_signal_id IS NOT NULL AND parent.id IS NULL
)
LIMIT 1
"#;
    if let Some(name) = connection
        .query_row(CHECK_SQL, [], |row| row.get::<_, String>(0))
        .optional()
        .map_err(|e| error(format!("board isolation 预检失败: {e}")))?
    {
        return Err(error(format!("源 SQLite 存在跨 board 引用: {name}")));
    }
    if connection
        .query_row(
            "SELECT 1 FROM entity_relations r JOIN entities s ON s.uri=r.subject_uri JOIN entities o ON o.uri=r.object_uri WHERE s.board_id IS NOT o.board_id LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| error(format!("entity_relations board 预检失败: {e}")))?
        .is_some()
    {
        return Err(error("源 entity_relations subject/object 跨 board"));
    }
    Ok(())
}

fn read_rows(
    connection: &SqliteConnection,
    table: &str,
    columns: &[String],
) -> Result<Vec<Vec<SqlValue>>, StoreError> {
    let sql = format!(
        "SELECT {} FROM {}",
        columns
            .iter()
            .map(|c| quote(c))
            .collect::<Vec<_>>()
            .join(","),
        quote(table)
    );
    let mut stmt = connection
        .prepare(&sql)
        .map_err(|e| error(format!("读取源表 {table} 失败: {e}")))?;
    stmt.query_map([], |row| {
        (0..columns.len())
            .map(|i| row.get::<_, SqlValue>(i))
            .collect::<rusqlite::Result<Vec<_>>>()
    })
    .map_err(|e| error(format!("读取源表 {table} 失败: {e}")))?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| error(format!("读取源表 {table} 失败: {e}")))
}

fn read_attachments(
    rows: &[Vec<SqlValue>],
    root: &Path,
    columns: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<Attachment>, StoreError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let root =
        fs::canonicalize(root).map_err(|e| error(format!("读取源 attachments 根目录失败: {e}")))?;
    let cols = columns
        .get("task_attachments")
        .ok_or_else(|| error("缺少 task_attachments columns"))?;
    let idx = |name: &str| {
        cols.iter()
            .position(|x| x == name)
            .ok_or_else(|| error(format!("task_attachments 缺少 {name}")))
    };
    let id = idx("id")?;
    let rel = idx("rel_path")?;
    let size = idx("size_bytes")?;
    let sha = idx("sha256")?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let idv = text(row.get(id), "task_attachments.id")?;
        let relv = safe_rel(&text(row.get(rel), "task_attachments.rel_path")?)?;
        let path = root.join(&relv);
        let meta =
            fs::symlink_metadata(&path).map_err(|e| error(format!("附件 {idv} 不可读: {e}")))?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(error(format!("附件 {idv} 不是普通文件")));
        }
        let canonical =
            fs::canonicalize(&path).map_err(|e| error(format!("附件 {idv} 路径解析失败: {e}")))?;
        if !canonical.starts_with(&root) {
            return Err(error(format!("附件 {idv} 路径穿越")));
        }
        let bytes = fs::read(&path).map_err(|e| error(format!("读取附件 {idv} 失败: {e}")))?;
        let expected_size = integer(row.get(size), "task_attachments.size_bytes")?;
        let expected_sha = optional_text(row.get(sha), "task_attachments.sha256")?;
        let observed_sha = sha256_bytes(&bytes);
        if expected_size < 0
            || meta.len() != expected_size as u64
            || bytes.len() != expected_size as usize
        {
            return Err(error(format!("附件 {idv} size 不匹配")));
        }
        if let Some(value) = expected_sha.as_deref()
            && !value.eq_ignore_ascii_case(&observed_sha)
        {
            return Err(error(format!("附件 {idv} SHA-256 不匹配")));
        }
        out.push(Attachment {
            id: idv,
            rel_path: path_to_string(&relv)?,
            sha256: expected_sha.unwrap_or(observed_sha),
            size: expected_size,
            bytes,
        });
    }
    Ok(out)
}

fn schema_fingerprint() -> String {
    let mut digest = Sha256::new();
    digest.update(b"kanban.sqlite.v30.schema\0");
    digest.update(SOURCE_VERSION.to_le_bytes());
    for (table, cols) in LEGACY_COLUMNS {
        digest.update(table.as_bytes());
        digest.update([0]);
        for col in cols.split(',') {
            digest.update(col.as_bytes());
            digest.update([0]);
        }
    }
    for (version, (name, checksum)) in MIGRATION_NAMES
        .iter()
        .zip(MIGRATION_CHECKSUMS.iter())
        .enumerate()
    {
        digest.update((version as i64 + 1).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(checksum.as_bytes());
        digest.update([0]);
    }
    format!("columns-sha256:{:x}", digest.finalize())
}

fn source_fingerprint(database: &[u8], wal: Option<&[u8]>, attachments: &[Attachment]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"kanban.sqlite_v30.snapshot\0");
    digest.update(SOURCE_VERSION.to_le_bytes());
    digest.update((database.len() as u64).to_le_bytes());
    digest.update(database);
    if let Some(wal) = wal {
        digest.update(b"\0wal\0");
        digest.update((wal.len() as u64).to_le_bytes());
        digest.update(wal);
    }
    let mut attachments = attachments.iter().collect::<Vec<_>>();
    attachments.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.rel_path.cmp(&right.rel_path))
    });
    for attachment in attachments {
        digest.update(attachment.id.as_bytes());
        digest.update([0]);
        digest.update(attachment.rel_path.as_bytes());
        digest.update([0]);
        digest.update((attachment.bytes.len() as u64).to_le_bytes());
        digest.update(&attachment.bytes);
    }
    format!("sha256:{:x}", digest.finalize())
}

fn ensure_no_follow_directory(path: &Path, label: &str) -> Result<(), StoreError> {
    if path.as_os_str().is_empty() {
        return Err(error(format!("{label} 路径为空")));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(error(format!(
                    "{label} 包含 symlink: {}",
                    current.display()
                )));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(error(format!("{label} 不是目录: {}", current.display())));
            }
            Ok(_) => {}
            Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|e| error(format!("创建 {label} 目录失败: {e}")))?;
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|e| error(format!("读取 {label} 目录失败: {e}")))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(error(format!(
                        "{label} 创建后不是普通目录: {}",
                        current.display()
                    )));
                }
            }
            Err(io_error) => {
                return Err(error(format!("读取 {label} 目录失败: {io_error}")));
            }
        }
    }
    Ok(())
}

fn ensure_no_follow_file(
    path: &Path,
    label: &str,
    allow_missing: bool,
) -> Result<bool, StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(error(format!("{label} 是 symlink: {}", path.display())))
        }
        Ok(metadata) if !metadata.is_file() => {
            Err(error(format!("{label} 不是普通文件: {}", path.display())))
        }
        Ok(_) => Ok(true),
        Err(io_error) if allow_missing && io_error.kind() == std::io::ErrorKind::NotFound => {
            Ok(false)
        }
        Err(io_error) => Err(error(format!("读取 {label} 失败: {io_error}"))),
    }
}

fn verify_file_containment(root: &Path, path: &Path, label: &str) -> Result<(), StoreError> {
    let canonical_root =
        fs::canonicalize(root).map_err(|e| error(format!("解析 {label} 根目录失败: {e}")))?;
    let canonical_path =
        fs::canonicalize(path).map_err(|e| error(format!("解析 {label} 路径失败: {e}")))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(error(format!("{label} 路径超出根目录: {}", path.display())));
    }
    Ok(())
}

fn stage_attachments(snapshot: &Snapshot, root: &Path) -> Result<Vec<Attachment>, StoreError> {
    if snapshot.attachments.is_empty() {
        return Ok(Vec::new());
    }
    ensure_no_follow_directory(root, "staging 根目录")?;
    let mut staged = Vec::with_capacity(snapshot.attachments.len());
    for attachment in &snapshot.attachments {
        let rel = safe_rel(&attachment.rel_path)?;
        let target = root.join(&rel);
        if let Some(parent) = target.parent() {
            ensure_no_follow_directory(parent, "staging 子目录")?;
        }
        let exists = ensure_no_follow_file(&target, "staging 附件", true)?;
        if exists {
            fs::remove_file(&target).map_err(|e| error(format!("替换 staging 附件失败: {e}")))?;
        }
        fs::write(&target, &attachment.bytes)
            .map_err(|e| error(format!("写入 staging 附件 {} 失败: {e}", attachment.id)))?;
        ensure_no_follow_file(&target, "staging 附件", false)?;
        verify_file_containment(root, &target, "staging 附件")?;
        let staged_bytes =
            fs::read(&target).map_err(|e| error(format!("读取 staging 附件失败: {e}")))?;
        if staged_bytes.len() as i64 != attachment.size
            || !sha256_bytes(&staged_bytes).eq_ignore_ascii_case(&attachment.sha256)
        {
            return Err(error(format!("附件 {} staging 校验失败", attachment.id)));
        }
        staged.push(Attachment {
            ..attachment.clone()
        });
    }
    Ok(staged)
}

async fn write_transaction(
    connection: &mut TursoConnection,
    snapshot: &Snapshot,
    journal: &str,
    target_root: &Path,
    staging: &Path,
    attachments: &[Attachment],
    clear_bootstrap: bool,
) -> Result<(), StoreError> {
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;
    if clear_bootstrap {
        tx.execute("DELETE FROM boards WHERE id='b_default' AND slug='default' AND name='Default' AND description IS NULL AND archived_at IS NULL", ()).await?;
        tx.execute("DELETE FROM relation_predicates", ()).await?;
    }
    let now = now_ms();
    let manifest = json!({"schema_fingerprint": snapshot.schema_fingerprint, "source_fingerprint": snapshot.source_fingerprint, "table_counts": snapshot.counts, "attachment_count": attachments.len()}).to_string();
    tx.execute("INSERT OR REPLACE INTO import_journal(id,source_kind,source_path,snapshot_fingerprint,phase,staged_database_path,staged_attachment_root,canonical_attachment_root,manifest_json,previous_identity_json,error,created_at,updated_at) VALUES(?1,?2,?3,?4,'validated',NULL,?5,?6,?7,NULL,NULL,?8,?8)", vec![Value::Text(journal.to_owned()),Value::Text(SOURCE_KIND.to_owned()),Value::Text(path_to_string(&snapshot.source_path)?),Value::Text(snapshot.source_fingerprint.clone()),Value::Text(path_to_string(staging)?),Value::Text(path_to_string(target_root)?),Value::Text(manifest),Value::Integer(now)]).await?;
    for attachment in attachments {
        tx.execute("INSERT INTO attachment_staging(id,journal_id,attachment_id,source_rel_path,staged_rel_path,expected_sha256,expected_size_bytes,observed_sha256,observed_size_bytes,phase,error,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?6,?7,'verified',NULL,?8,?8)", vec![Value::Text(staging_id(journal,&attachment.id)),Value::Text(journal.to_owned()),Value::Text(attachment.id.clone()),Value::Text(attachment.rel_path.clone()),Value::Text(attachment.rel_path.clone()),Value::Text(attachment.sha256.clone()),Value::Integer(attachment.size),Value::Integer(now)]).await?;
    }
    for table in CANONICAL_TABLES {
        let rows = snapshot.rows.get(*table).map(Vec::as_slice).unwrap_or(&[]);
        insert_table(
            &tx,
            table,
            snapshot
                .columns
                .get(*table)
                .ok_or_else(|| error(format!("缺少 {table} columns")))?,
            rows,
            snapshot,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn insert_table(
    tx: &turso::transaction::Transaction<'_>,
    table: &str,
    source_cols: &[String],
    rows: &[Vec<SqlValue>],
    snapshot: &Snapshot,
) -> Result<(), StoreError> {
    let target_cols = target_columns(table)?;
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote(table),
        target_cols
            .iter()
            .map(|x| quote(x))
            .collect::<Vec<_>>()
            .join(","),
        (1..=target_cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut deferred = Vec::new();
    for row in rows {
        let mut params = Vec::with_capacity(target_cols.len());
        for col in &target_cols {
            let value = match source_cols.iter().position(|x| x == col) {
                Some(index) if deferred_column(table, col) => {
                    let original = to_turso(row.get(index).ok_or_else(|| error("源行列数不足"))?)?;
                    let ididx = source_cols
                        .iter()
                        .position(|x| x == "id")
                        .ok_or_else(|| error(format!("{table} 缺少 id")))?;
                    let id = to_turso(row.get(ididx).ok_or_else(|| error("源主键缺失"))?)?;
                    deferred.push((col.clone(), id, original));
                    Value::Null
                }
                Some(index) => to_turso(row.get(index).ok_or_else(|| error("源行列数不足"))?)?,
                None if matches!(
                    (table, col.as_str()),
                    ("tasks", "idempotency_key")
                        | ("task_steps", "idempotency_key")
                        | ("task_comments", "idempotency_key")
                ) =>
                {
                    Value::Null
                }
                None if table == "entity_relations" && col == "board_id" => {
                    relation_board(row, source_cols, snapshot)?
                }
                None => return Err(error(format!("目标表 {table}.{col} 没有安全映射"))),
            };
            params.push(value);
        }
        tx.execute(&sql, params).await?;
    }
    for (col, id, value) in deferred {
        tx.execute(
            &format!("UPDATE {} SET {}=?1 WHERE id=?2", quote(table), quote(&col)),
            vec![value, id],
        )
        .await?;
    }
    Ok(())
}

fn target_columns(table: &str) -> Result<Vec<String>, StoreError> {
    let source = LEGACY_COLUMNS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, cols)| cols.split(',').map(ToOwned::to_owned).collect::<Vec<_>>())
        .ok_or_else(|| error(format!("缺少目标列 manifest {table}")))?;
    let mut cols = Vec::new();
    for col in source {
        if table == "entity_relations" && col == "authoritative_store" {
            cols.push("board_id".to_owned());
        }
        cols.push(col);
        if table == "tasks" && cols.last().is_some_and(|x| x == "seq") {
            cols.push("idempotency_key".to_owned());
        }
        if table == "task_steps" && cols.last().is_some_and(|x| x == "parent_task_id") {
            cols.push("idempotency_key".to_owned());
        }
        if table == "task_comments" && cols.last().is_some_and(|x| x == "task_id") {
            cols.push("idempotency_key".to_owned());
        }
    }
    Ok(cols)
}

fn relation_board(
    row: &[SqlValue],
    cols: &[String],
    snapshot: &Snapshot,
) -> Result<Value, StoreError> {
    let subject = text(
        row.get(
            cols.iter()
                .position(|x| x == "subject_uri")
                .ok_or_else(|| error("relation subject 缺失"))?,
        ),
        "subject_uri",
    )?;
    let object = text(
        row.get(
            cols.iter()
                .position(|x| x == "object_uri")
                .ok_or_else(|| error("relation object 缺失"))?,
        ),
        "object_uri",
    )?;
    let entities = snapshot
        .rows
        .get("entities")
        .ok_or_else(|| error("entities rows 缺失"))?;
    let ecols = snapshot
        .columns
        .get("entities")
        .ok_or_else(|| error("entities columns 缺失"))?;
    let ui = ecols.iter().position(|x| x == "uri").unwrap();
    let bi = ecols.iter().position(|x| x == "board_id").unwrap();
    let board = |uri: &str| -> Result<Option<String>, StoreError> {
        entities
            .iter()
            .find(|r| text(r.get(ui), "entities.uri").is_ok_and(|x| x == uri))
            .map(|r| optional_text(r.get(bi), "entities.board_id"))
            .transpose()
            .map(|x| x.flatten())
    };
    let a = board(&subject)?;
    let b = board(&object)?;
    if a != b {
        return Err(error("entity_relations subject/object board 不一致"));
    }
    Ok(a.map(Value::Text).unwrap_or(Value::Null))
}

fn deferred_column(table: &str, col: &str) -> bool {
    matches!(
        (table, col),
        ("label_ontology_actions", "parent_action_id")
            | ("label_ontology_signals", "superseded_by_signal_id")
            | ("signals", "superseded_by_signal_id")
    )
}

async fn publish(
    connection: &mut TursoConnection,
    journal: &str,
    staging: &Path,
    target: &Path,
    attachments: &[Attachment],
) -> Result<(), StoreError> {
    ensure_no_follow_directory(target, "canonical attachments 根目录")?;
    if !attachments.is_empty() {
        ensure_no_follow_directory(staging, "staging 根目录")?;
    }
    for attachment in attachments {
        let rel = safe_rel(&attachment.rel_path)?;
        let from = staging.join(&rel);
        let to = target.join(&rel);
        if let Some(parent) = to.parent() {
            ensure_no_follow_directory(parent, "canonical 附件目录")?;
        }
        let from_exists = ensure_no_follow_file(&from, "staging 附件", true)?;
        let to_exists = ensure_no_follow_file(&to, "canonical 附件", true)?;
        if to_exists {
            verify_file_containment(target, &to, "canonical 附件")?;
            let published = fs::read(&to).map_err(|e| error(format!("读取已发布附件失败: {e}")))?;
            if published.len() as i64 != attachment.size
                || !sha256_bytes(&published).eq_ignore_ascii_case(&attachment.sha256)
            {
                return Err(error(format!("目标附件校验不一致: {}", to.display())));
            }
            if from_exists {
                verify_file_containment(staging, &from, "staging 附件")?;
                fs::remove_file(from).map_err(|e| error(format!("清理 staging 附件失败: {e}")))?;
            }
        } else {
            if !from_exists {
                return Err(error(format!("staging 附件不存在: {}", from.display())));
            }
            verify_file_containment(staging, &from, "staging 附件")?;
            fs::rename(&from, &to)
                .map_err(|e| error(format!("原子发布附件 {} 失败: {e}", attachment.id)))?;
            ensure_no_follow_file(&to, "canonical 附件", false)?;
            verify_file_containment(target, &to, "canonical 附件")?;
        }
        let published = fs::read(&to).map_err(|e| error(format!("读取发布后附件失败: {e}")))?;
        if published.len() as i64 != attachment.size
            || !sha256_bytes(&published).eq_ignore_ascii_case(&attachment.sha256)
        {
            return Err(error(format!("发布后附件校验不一致: {}", to.display())));
        }
        connection.execute("UPDATE attachment_staging SET phase='published',observed_sha256=?1,observed_size_bytes=?2,updated_at=?3 WHERE journal_id=?4 AND attachment_id=?5", (attachment.sha256.as_str(),attachment.size,now_ms(),journal,attachment.id.as_str())).await?;
    }
    connection
        .execute(
            "UPDATE import_journal SET phase='completed',updated_at=?1 WHERE id=?2",
            (now_ms(), journal),
        )
        .await?;
    Ok(())
}

async fn find_journal(
    connection: &TursoConnection,
    fingerprint: &str,
) -> Result<Option<(String, String, PathBuf)>, StoreError> {
    let mut rows = connection.query("SELECT id,phase,staged_attachment_root FROM import_journal WHERE source_kind=?1 AND snapshot_fingerprint=?2 ORDER BY updated_at DESC LIMIT 1", (SOURCE_KIND,fingerprint)).await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    let id = match row.get_value(0)? {
        Value::Text(v) => v,
        _ => return Err(error("journal id 类型错误")),
    };
    let phase = match row.get_value(1)? {
        Value::Text(v) => v,
        _ => return Err(error("journal phase 类型错误")),
    };
    let root = match row.get_value(2)? {
        Value::Text(v) => PathBuf::from(v),
        _ => return Err(error("journal staging root 类型错误")),
    };
    Ok(Some((id, phase, root)))
}

async fn target_counts(connection: &TursoConnection) -> Result<BTreeMap<String, u64>, StoreError> {
    let mut out = BTreeMap::new();
    for table in CANONICAL_TABLES {
        let mut rows = connection
            .query(&format!("SELECT COUNT(*) FROM {}", quote(table)), ())
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| error(format!("目标表 {table} 计数为空")))?;
        let n = match row.get_value(0)? {
            Value::Integer(v) if v >= 0 => v as u64,
            _ => return Err(error(format!("目标表 {table} 计数类型错误"))),
        };
        out.insert((*table).to_owned(), n);
    }
    Ok(out)
}

async fn logical_empty(
    connection: &TursoConnection,
    counts: &BTreeMap<String, u64>,
) -> Result<bool, StoreError> {
    if counts.values().all(|x| *x == 0) {
        return Ok(true);
    }
    if counts.get("boards") != Some(&1)
        || counts.get("board_columns") != Some(&9)
        || CANONICAL_TABLES.iter().any(|t| {
            !matches!(*t, "boards" | "board_columns" | "relation_predicates")
                && counts.get(*t).copied().unwrap_or_default() != 0
        })
        || !matches!(counts.get("relation_predicates"), Some(0) | Some(3))
    {
        return Ok(false);
    }
    let mut rows = connection
        .query(
            "SELECT id,slug,name,description,archived_at FROM boards",
            (),
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(false);
    };
    let ok = matches!(row.get_value(0)?, Value::Text(v) if v=="b_default")
        && matches!(row.get_value(1)?, Value::Text(v) if v=="default")
        && matches!(row.get_value(2)?, Value::Text(v) if v=="Default")
        && matches!(row.get_value(3)?, Value::Null)
        && matches!(row.get_value(4)?, Value::Null)
        && rows.next().await?.is_none();
    if !ok {
        return Ok(false);
    }
    let mut columns = connection
        .query(
            "SELECT status,title,position,hidden,wip_limit FROM board_columns WHERE board_id='b_default' ORDER BY position",
            (),
        )
        .await?;
    for (status, title, position, hidden) in schema::DEFAULT_COLUMNS {
        let Some(row) = columns.next().await? else {
            return Ok(false);
        };
        if !matches!(row.get_value(0)?, Value::Text(v) if v == status)
            || !matches!(row.get_value(1)?, Value::Text(v) if v == title)
            || !matches!(row.get_value(2)?, Value::Integer(v) if v == position)
            || !matches!(row.get_value(3)?, Value::Integer(v) if v == i64::from(hidden))
            || !matches!(row.get_value(4)?, Value::Null)
        {
            return Ok(false);
        }
    }
    if columns.next().await?.is_some() {
        return Ok(false);
    }
    if counts.get("relation_predicates") == Some(&3) {
        let mut predicates = connection
            .query(
                "SELECT name,domain_kind,range_kind,cardinality,authoritative_store,description FROM relation_predicates ORDER BY name",
                (),
            )
            .await?;
        for (name, domain, range) in [
            ("belongs_to_board", "task", "board"),
            ("depends_on", "task", "task"),
            ("mentions", "task", "task"),
        ] {
            let Some(row) = predicates.next().await? else {
                return Ok(false);
            };
            if !matches!(row.get_value(0)?, Value::Text(v) if v == name)
                || !matches!(row.get_value(1)?, Value::Text(v) if v == domain)
                || !matches!(row.get_value(2)?, Value::Text(v) if v == range)
                || !matches!(row.get_value(3)?, Value::Text(v) if v == "many")
                || !matches!(row.get_value(4)?, Value::Text(v) if v == "turso")
                || !matches!(row.get_value(5)?, Value::Null)
            {
                return Ok(false);
            }
        }
        if predicates.next().await?.is_some() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn counts_match(expected: &BTreeMap<String, u64>, actual: &BTreeMap<String, u64>) -> bool {
    CANONICAL_TABLES
        .iter()
        .all(|t| expected.get(*t) == actual.get(*t))
}
fn ensure_count_match(
    expected: &BTreeMap<String, u64>,
    actual: &BTreeMap<String, u64>,
) -> Result<(), StoreError> {
    if counts_match(expected, actual) {
        Ok(())
    } else {
        Err(error("导入行数证明失败"))
    }
}
fn staging_root(path: &Path, journal: &str) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".kanban-import")
        .join(journal)
        .join("attachments")
}
fn staging_id(journal: &str, attachment: &str) -> String {
    let mut d = Sha256::new();
    d.update(journal.as_bytes());
    d.update([0]);
    d.update(attachment.as_bytes());
    format!("as_{:x}", d.finalize())[..35].to_owned()
}
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn path_to_string(path: &Path) -> Result<String, StoreError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| error(format!("路径不是有效 UTF-8: {}", path.display())))
}
fn safe_rel(raw: &str) -> Result<PathBuf, StoreError> {
    if raw.is_empty() || raw.contains('\0') || raw.contains('\\') {
        return Err(error("附件 rel_path 为空或包含不安全字符"));
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(error("附件 rel_path 不能是绝对路径"));
    }
    let mut clean = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(v) => clean.push(v),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(error("附件 rel_path 包含路径穿越"));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        Err(error("附件 rel_path 为空"))
    } else {
        Ok(clean)
    }
}
fn text(value: Option<&SqlValue>, field: &str) -> Result<String, StoreError> {
    match value {
        Some(SqlValue::Text(v)) => Ok(v.clone()),
        _ => Err(error(format!("{field} 不是文本"))),
    }
}
fn optional_text(value: Option<&SqlValue>, field: &str) -> Result<Option<String>, StoreError> {
    match value {
        Some(SqlValue::Null) => Ok(None),
        Some(SqlValue::Text(v)) => Ok(Some(v.clone())),
        _ => Err(error(format!("{field} 不是可空文本"))),
    }
}
fn integer(value: Option<&SqlValue>, field: &str) -> Result<i64, StoreError> {
    match value {
        Some(SqlValue::Integer(v)) => Ok(*v),
        _ => Err(error(format!("{field} 不是整数"))),
    }
}
fn to_turso(value: &SqlValue) -> Result<Value, StoreError> {
    Ok(match value {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(v) => Value::Integer(*v),
        SqlValue::Real(v) => Value::Real(*v),
        SqlValue::Text(v) => Value::Text(v.clone()),
        SqlValue::Blob(v) => Value::Blob(v.clone()),
    })
}
fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_rel_path_rejects_escape_and_accepts_nested_file() {
        assert!(safe_rel("../outside.bin").is_err());
        assert!(safe_rel("/absolute.bin").is_err());
        assert!(safe_rel("nested\\outside.bin").is_err());
        assert_eq!(
            safe_rel("nested/file.bin").unwrap(),
            PathBuf::from("nested/file.bin")
        );
    }

    #[test]
    fn v30_manifest_has_exact_table_and_migration_shape() {
        assert_eq!(LEGACY_TABLES.len(), 35);
        assert_eq!(LEGACY_COLUMNS.len(), LEGACY_TABLES.len());
        assert_eq!(MIGRATION_NAMES.len(), SOURCE_VERSION as usize);
        assert_eq!(MIGRATION_CHECKSUMS.len(), MIGRATION_NAMES.len());
    }

    #[test]
    fn source_preflight_rejects_non_v30_without_touching_source() {
        let directory = tempfile::tempdir().expect("temporary source directory");
        let source = directory.path().join("legacy.sqlite");
        let connection = SqliteConnection::open(&source).expect("open source");
        connection
            .execute_batch("PRAGMA user_version=29; CREATE TABLE marker(value TEXT);")
            .expect("write invalid fixture");
        drop(connection);
        let wal = PathBuf::from(format!("{}-wal", source.display()));
        let shm = PathBuf::from(format!("{}-shm", source.display()));
        let before = fs::read(&source).expect("read source before preflight");
        let result = read_snapshot(&source);
        assert!(result.is_err());
        assert_eq!(
            fs::read(&source).expect("read source after preflight"),
            before
        );
        assert!(!wal.exists());
        assert!(!shm.exists());
    }

    #[tokio::test]
    async fn initialized_default_target_is_logically_empty() {
        let directory = tempfile::tempdir().expect("temporary target directory");
        let path = directory.path().join("target.turso");
        let store = TursoStore::open(&path).await.expect("open target");
        store.initialize().await.expect("initialize target");
        let connection = store.connection().await.expect("target connection");
        let counts = target_counts(&connection).await.expect("target counts");
        assert!(
            logical_empty(&connection, &counts)
                .await
                .expect("empty proof")
        );
    }

    #[test]
    fn staging_rechecks_attachment_size_and_checksum() {
        let directory = tempfile::tempdir().expect("temporary attachment directory");
        let source = directory.path().join("source.bin");
        fs::write(&source, b"immutable attachment").expect("write attachment");
        let checksum = sha256_bytes(&fs::read(&source).expect("read attachment"));
        let snapshot = Snapshot {
            source_path: directory.path().join("legacy.sqlite"),
            schema_fingerprint: "schema".to_owned(),
            source_fingerprint: "sha256:test".to_owned(),
            rows: BTreeMap::new(),
            columns: BTreeMap::new(),
            counts: BTreeMap::new(),
            attachments: vec![Attachment {
                id: "a_test".to_owned(),
                rel_path: "nested/file.bin".to_owned(),
                sha256: checksum.clone(),
                size: b"immutable attachment".len() as i64,
                bytes: b"immutable attachment".to_vec(),
            }],
        };
        let staging_root = directory.path().join("staging");
        let staged = stage_attachments(&snapshot, &staging_root).expect("stage attachment");
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].sha256, checksum);
        assert_eq!(
            fs::read(staging_root.join("nested/file.bin")).expect("read staged"),
            b"immutable attachment"
        );
    }

    #[cfg(unix)]
    #[test]
    fn staging_rejects_symlink_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary staging directory");
        let root = directory.path().join("staging");
        fs::create_dir_all(&root).expect("staging root");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&outside).expect("outside root");
        symlink(&outside, root.join("nested")).expect("symlink parent");
        let snapshot = Snapshot {
            source_path: directory.path().join("legacy.sqlite"),
            schema_fingerprint: "schema".to_owned(),
            source_fingerprint: "sha256:test".to_owned(),
            rows: BTreeMap::new(),
            columns: BTreeMap::new(),
            counts: BTreeMap::new(),
            attachments: vec![Attachment {
                id: "a_test".to_owned(),
                rel_path: "nested/file.bin".to_owned(),
                sha256: sha256_bytes(b"immutable attachment"),
                size: b"immutable attachment".len() as i64,
                bytes: b"immutable attachment".to_vec(),
            }],
        };
        assert!(stage_attachments(&snapshot, &root).is_err());
        assert!(!outside.join("file.bin").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn publish_rejects_canonical_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary publish directory");
        let path = directory.path().join("target.turso");
        let store = TursoStore::open(&path).await.expect("open target");
        store.initialize().await.expect("initialize target");
        let staging = directory.path().join("staging");
        let target = directory.path().join("canonical");
        fs::create_dir_all(&staging).expect("staging root");
        fs::create_dir_all(&target).expect("canonical root");
        let outside = directory.path().join("outside.bin");
        fs::write(&outside, b"immutable attachment").expect("outside file");
        let published = target.join("file.bin");
        symlink(&outside, &published).expect("canonical symlink");
        fs::write(staging.join("file.bin"), b"immutable attachment").expect("staging file");

        let attachment = Attachment {
            id: "a_test".to_owned(),
            rel_path: "file.bin".to_owned(),
            sha256: sha256_bytes(b"immutable attachment"),
            size: b"immutable attachment".len() as i64,
            bytes: b"immutable attachment".to_vec(),
        };
        let mut connection = store.connection().await.expect("target connection");
        let result = publish(&mut connection, "ij_test", &staging, &target, &[attachment]).await;
        assert!(result.is_err());
        assert!(
            fs::symlink_metadata(&published)
                .expect("published path")
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn source_fingerprint_is_bound_to_snapshot_bytes() {
        let attachments = vec![Attachment {
            id: "a_test".to_owned(),
            rel_path: "nested/file.bin".to_owned(),
            sha256: sha256_bytes(b"snapshot attachment"),
            size: b"snapshot attachment".len() as i64,
            bytes: b"snapshot attachment".to_vec(),
        }];
        let first = source_fingerprint(b"snapshot database", Some(b"snapshot wal"), &attachments);
        let second = source_fingerprint(b"snapshot database", Some(b"snapshot wal"), &attachments);
        assert_eq!(first, second);

        let mut changed = attachments;
        changed[0].bytes = b"changed source file".to_vec();
        assert_ne!(
            first,
            source_fingerprint(b"snapshot database", Some(b"snapshot wal"), &changed)
        );
    }

    #[cfg(feature = "test-support")]
    #[tokio::test]
    async fn repeated_import_is_idempotent_and_snapshot_survives_source_change() {
        let directory = tempfile::tempdir().expect("temporary source directory");
        let source = crate::adoption_test_support::make_legacy_source(directory.path())
            .expect("legacy source");
        let snapshot = read_snapshot(&source).expect("read snapshot");
        let fingerprint = snapshot.source_fingerprint.clone();
        let staging = directory.path().join("snapshot-staging");

        let source_connection = SqliteConnection::open(&source).expect("open source");
        source_connection
            .execute(
                "UPDATE tasks SET metadata_json='{\"changed\":true}' WHERE id='t_legacy'",
                [],
            )
            .expect("mutate source after snapshot");
        drop(source_connection);

        let staged = stage_attachments(&snapshot, &staging).expect("stage snapshot");
        assert_eq!(snapshot.source_fingerprint, fingerprint);
        assert_eq!(staged.len(), 1);
        assert_eq!(
            fs::read(staging.join("attachments/legacy.txt")).expect("read staged snapshot"),
            b"legacy\n"
        );

        let target_path = directory.path().join("target.turso");
        let target = TursoStore::open(&target_path).await.expect("open target");
        target.initialize().await.expect("initialize target");
        let first = import_into_store(
            &target,
            LegacyImportOptions::new(&source)
                .with_canonical_attachment_root(directory.path().join("canonical")),
        )
        .await
        .expect("first import");
        let second = import_into_store(
            &target,
            LegacyImportOptions::new(&source)
                .with_canonical_attachment_root(directory.path().join("canonical")),
        )
        .await
        .expect("repeated import");
        assert_ne!(first.source_fingerprint, fingerprint);
        assert_eq!(second.phase, "completed");
        assert!(!second.resumed);
        assert_eq!(first.journal_id, second.journal_id);
    }
}
