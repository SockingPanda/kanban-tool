use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use turso::{Builder, Connection, Value, transaction::TransactionBehavior};

use crate::{error::StoreError, schema, shared::now_ms};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SchemaState {
    Fresh,
    CurrentV1,
    Upgraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationOutcome {
    pub state: SchemaState,
    pub from_version: i64,
    pub to_version: i64,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LedgerRow {
    name: String,
    checksum: String,
}

const V1_TABLES: &[&str] = &[
    "board_columns",
    "boards",
    "schema_migrations",
    "task_comments",
    "task_dependencies",
    "task_events",
    "task_execution_plans",
    "task_runs",
    "task_steps",
    "tasks",
];

const V1_EXACT_COLUMNS: &[(&str, &[&str])] = &[
    (
        "schema_migrations",
        &["version", "name", "checksum", "applied_at"],
    ),
    (
        "boards",
        &[
            "id",
            "slug",
            "name",
            "description",
            "created_at",
            "updated_at",
            "archived_at",
        ],
    ),
    (
        "board_columns",
        &[
            "id",
            "board_id",
            "status",
            "title",
            "position",
            "hidden",
            "wip_limit",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "tasks",
        &[
            "id",
            "board_id",
            "seq",
            "idempotency_key",
            "title",
            "description",
            "status",
            "status_reason",
            "assignee",
            "priority",
            "position",
            "scheduled_at",
            "due_at",
            "created_by",
            "created_at",
            "updated_at",
            "started_at",
            "completed_at",
            "archived_at",
            "claim_token",
            "claim_owner",
            "claim_expires_at",
            "last_heartbeat_at",
            "current_run_id",
            "retry_count",
            "max_retries",
            "result_summary",
            "result_json",
            "metadata_json",
            "lock_version",
        ],
    ),
    (
        "task_execution_plans",
        &[
            "board_id",
            "task_id",
            "state",
            "reason",
            "updated_by",
            "updated_at",
        ],
    ),
    (
        "task_steps",
        &[
            "id",
            "board_id",
            "parent_task_id",
            "idempotency_key",
            "position",
            "title",
            "body",
            "linked_task_id",
            "required",
            "status",
            "resolution_note",
            "resolved_by",
            "resolved_at",
            "created_by",
            "created_at",
            "updated_by",
            "updated_at",
        ],
    ),
    (
        "task_dependencies",
        &["board_id", "parent_task_id", "child_task_id", "created_at"],
    ),
    (
        "task_runs",
        &[
            "id",
            "board_id",
            "task_id",
            "status",
            "claim_token",
            "claim_owner",
            "claim_expires_at",
            "worker_profile",
            "worker_pid",
            "started_at",
            "last_heartbeat_at",
            "finished_at",
            "exit_code",
            "summary",
            "error",
            "log_path",
            "metadata_json",
        ],
    ),
    (
        "task_comments",
        &[
            "id",
            "board_id",
            "task_id",
            "idempotency_key",
            "author",
            "author_type",
            "agent_type",
            "body",
            "kind",
            "metadata_json",
            "created_at",
        ],
    ),
    (
        "task_events",
        &[
            "id",
            "event_id",
            "board_id",
            "task_id",
            "run_id",
            "kind",
            "actor",
            "payload_json",
            "created_at",
        ],
    ),
];

const FULL_EXACT_COLUMNS: &[(&str, &[&str])] = &[
    (
        "schema_identity",
        &[
            "singleton",
            "family",
            "lineage",
            "version",
            "fingerprint",
            "migration_checksum",
            "upgraded_at",
        ],
    ),
    (
        "schema_capabilities",
        &["capability", "available", "detail", "checked_at"],
    ),
    (
        "task_attachments",
        &[
            "id",
            "board_id",
            "task_id",
            "filename",
            "rel_path",
            "content_type",
            "size_bytes",
            "sha256",
            "created_by",
            "created_at",
        ],
    ),
    (
        "labels",
        &[
            "id",
            "board_id",
            "name",
            "color",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "task_labels",
        &["board_id", "task_id", "label_id", "created_at"],
    ),
    ("app_settings", &["key", "value_json", "updated_at"]),
    (
        "task_subtasks",
        &[
            "board_id",
            "parent_task_id",
            "child_task_id",
            "position",
            "required",
            "created_by",
            "created_at",
        ],
    ),
    (
        "entities",
        &[
            "uri",
            "kind",
            "source_table",
            "source_id",
            "board_id",
            "task_id",
            "title",
            "summary",
            "content_hash",
            "created_at",
            "updated_at",
            "archived_at",
        ],
    ),
    (
        "relation_predicates",
        &[
            "name",
            "domain_kind",
            "range_kind",
            "cardinality",
            "authoritative_store",
            "description",
            "created_at",
        ],
    ),
    (
        "entity_relations",
        &[
            "id",
            "subject_uri",
            "predicate",
            "object_uri",
            "graph_uri",
            "board_id",
            "authoritative_store",
            "source_table",
            "source_id",
            "source_event_id",
            "metadata_json",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "projection_jobs",
        &[
            "id",
            "board_id",
            "source_event_id",
            "target",
            "entity_uri",
            "dedupe_key",
            "operation",
            "payload_json",
            "status",
            "attempts",
            "max_attempts",
            "lease_owner",
            "lease_token",
            "lease_expires_at",
            "fence_epoch",
            "generation",
            "next_attempt_at",
            "last_error",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "projection_state",
        &[
            "projection",
            "lifecycle_status",
            "active_generation",
            "active_fingerprint",
            "previous_generation",
            "previous_fingerprint",
            "building_generation",
            "building_fingerprint",
            "provider",
            "provider_fingerprint",
            "corpus_schema",
            "corpus_fingerprint",
            "embedding_model",
            "embedding_dimensions",
            "last_event_id",
            "dirty",
            "lease_owner",
            "lease_token",
            "lease_expires_at",
            "fence_epoch",
            "last_success_at",
            "last_error",
            "updated_at",
        ],
    ),
    (
        "label_semantics",
        &[
            "label_id",
            "board_id",
            "description",
            "applies_when",
            "excludes_when",
            "positive_examples",
            "negative_examples",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "label_atoms",
        &[
            "id",
            "label_id",
            "board_id",
            "polarity",
            "kind",
            "text",
            "ordinal",
            "content_hash",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "label_atom_index_boards",
        &[
            "store_name",
            "board_id",
            "dirty",
            "last_rebuild_at",
            "last_error",
            "updated_at",
        ],
    ),
    (
        "label_semantic_proposals",
        &[
            "id",
            "board_id",
            "task_id",
            "status",
            "name",
            "description",
            "applies_when",
            "excludes_when",
            "positive_examples",
            "negative_examples",
            "heuristic_coverage",
            "heuristic_residual_norm",
            "heuristic_coverage_cosine",
            "top1_existing_label_id",
            "top1_existing_label_name",
            "diagnostics_json",
            "created_by",
            "decision_reason",
            "resolved_label_id",
            "created_at",
            "updated_at",
            "decided_at",
        ],
    ),
    (
        "label_ontology_observations",
        &[
            "id",
            "board_id",
            "task_id",
            "task_ref_snapshot",
            "task_snapshot_json",
            "agent_candidates_json",
            "suggestion_snapshot_json",
            "final_decision_json",
            "suggest_coverage",
            "suggest_coverage_cosine",
            "suggest_residual_norm",
            "suggest_needs_new_label",
            "suggest_degraded",
            "diagnostics_json",
            "capture_fingerprint",
            "suggest_input_hash",
            "created_by",
            "created_by_type",
            "agent_type",
            "created_at",
        ],
    ),
    (
        "label_ontology_signals",
        &[
            "id",
            "board_id",
            "observation_id",
            "kind",
            "status",
            "target_label_id",
            "target_label_name_snapshot",
            "related_labels_json",
            "proposed_action",
            "candidate_atom_polarity",
            "candidate_atom_kind",
            "candidate_text",
            "candidate_content_hash",
            "proposed_label_name",
            "proposed_label_name_normalized",
            "proposal_json",
            "agent_selected",
            "suggest_state",
            "suggest_score",
            "suggest_rank",
            "final_selected",
            "rationale",
            "confidence",
            "signal_key",
            "superseded_by_signal_id",
            "status_reason",
            "created_at",
            "updated_at",
            "reviewed_at",
            "closed_at",
        ],
    ),
    (
        "label_ontology_actions",
        &[
            "id",
            "board_id",
            "parent_action_id",
            "action_type",
            "reason",
            "target_label_id",
            "result_label_id",
            "result_atom_id",
            "result_atom_content_hash",
            "result_proposal_id",
            "canonical_before_hash",
            "canonical_after_hash",
            "change_json",
            "validation_status",
            "validation_json",
            "validation_requirement",
            "created_by",
            "created_by_type",
            "agent_type",
            "created_at",
        ],
    ),
    (
        "label_ontology_action_signals",
        &["board_id", "action_id", "signal_id", "created_at"],
    ),
    (
        "label_ontology_action_atom_effects",
        &[
            "board_id",
            "action_id",
            "label_id_snapshot",
            "atom_id_snapshot",
            "atom_content_hash",
            "polarity",
            "kind",
            "text",
            "effect",
            "created_at",
        ],
    ),
    (
        "signal_observations",
        &[
            "id",
            "board_id",
            "task_id",
            "run_id",
            "comment_id",
            "task_ref_snapshot",
            "actor",
            "agent_type",
            "source",
            "evidence_json",
            "created_at",
        ],
    ),
    (
        "signals",
        &[
            "id",
            "board_id",
            "observation_id",
            "kind",
            "title",
            "summary",
            "severity",
            "status",
            "dedupe_key",
            "superseded_by_signal_id",
            "reviewed_by",
            "reviewed_at",
            "review_reason",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "projection_maintenance_owner",
        &[
            "singleton",
            "owner",
            "lease_token",
            "mode",
            "lease_expires_at",
            "fence_epoch",
            "capabilities_json",
            "build_identity",
            "started_at",
            "last_heartbeat_at",
            "updated_at",
        ],
    ),
    (
        "retrieval_documents",
        &[
            "id",
            "board_id",
            "entity_uri",
            "source_kind",
            "content",
            "content_hash",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "retrieval_vectors",
        &[
            "id",
            "board_id",
            "entity_uri",
            "document_id",
            "embedding",
            "dimensions",
            "embedding_model",
            "content_hash",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "import_journal",
        &[
            "id",
            "source_kind",
            "source_path",
            "snapshot_fingerprint",
            "phase",
            "staged_database_path",
            "staged_attachment_root",
            "canonical_attachment_root",
            "manifest_json",
            "previous_identity_json",
            "error",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "attachment_staging",
        &[
            "id",
            "journal_id",
            "attachment_id",
            "source_rel_path",
            "staged_rel_path",
            "expected_sha256",
            "expected_size_bytes",
            "observed_sha256",
            "observed_size_bytes",
            "phase",
            "error",
            "created_at",
            "updated_at",
        ],
    ),
];

const FULL_REQUIRED_INDEXES: &[&str] = &[
    "idx_attachment_staging_phase",
    "idx_entities_board_kind",
    "idx_entity_relations_object",
    "idx_entity_relations_subject",
    "idx_import_journal_fingerprint",
    "idx_import_journal_phase",
    "idx_label_atoms_board_kind",
    "idx_label_atoms_label_ordinal",
    "idx_label_proposals_board_status",
    "idx_label_proposals_task_status",
    "idx_label_semantics_board_updated",
    "idx_ontology_action_atom_effects_hash",
    "idx_ontology_action_atom_effects_label",
    "idx_ontology_action_created",
    "idx_ontology_action_create_proposal",
    "idx_ontology_action_label",
    "idx_ontology_action_signals_signal",
    "idx_ontology_observation_task",
    "idx_ontology_signal_candidate_atom",
    "idx_ontology_signal_label_kind",
    "idx_ontology_signal_proposed_label",
    "idx_ontology_signal_status",
    "idx_projection_jobs_board",
    "idx_projection_jobs_dedupe",
    "idx_projection_jobs_lease",
    "idx_projection_jobs_ready",
    "idx_projection_state_dirty",
    "idx_retrieval_documents_board",
    "idx_retrieval_vectors_board",
    "idx_signal_observation_created",
    "idx_signal_observation_task",
    "idx_signals_dedupe_key",
    "idx_signals_observation",
    "idx_signals_status",
    "idx_subtasks_parent_position",
    "idx_task_attachments_task_created",
    "idx_task_labels_label",
];

const FULL_REQUIRED_TRIGGERS: &[&str] = &[
    "entity_relations_board_guard_insert",
    "entity_relations_board_guard_update",
    "label_ontology_actions_board_guard_insert",
    "label_ontology_actions_board_guard_update",
    "label_ontology_signals_board_guard_insert",
    "label_ontology_signals_board_guard_update",
    "label_semantic_proposals_board_guard_insert",
    "label_semantic_proposals_board_guard_update",
    "projection_jobs_board_guard_insert",
    "projection_jobs_board_guard_update",
    "retrieval_documents_board_guard_insert",
    "retrieval_documents_board_guard_update",
    "retrieval_vectors_board_guard_insert",
    "retrieval_vectors_board_guard_update",
    "signal_observations_board_guard_insert",
    "signal_observations_board_guard_update",
    "signals_board_guard_insert",
    "signals_board_guard_update",
    "task_attachments_path_guard_insert",
    "task_attachments_path_guard_update",
    "task_events_board_guard_insert",
    "task_events_board_guard_update",
];

const FULL_REQUIRED_SQL_FRAGMENTS: &[(&str, &[&str])] = &[
    (
        "task_comments",
        &["check(kindin('note','decision','signal'))"],
    ),
    (
        "projection_jobs",
        &[
            "check(targetin('fts','vector_tasks','vector_label_atoms','relations','all'))",
            "check((status='running')=(lease_ownerisnotnullandlease_tokenisnotnullandlease_expires_atisnotnull))",
        ],
    ),
    (
        "projection_state",
        &[
            "check(lifecycle_statusin('bootstrap_required','idle','rebuilding','ready','degraded','error'))",
            "check(embedding_dimensionsisnullorembedding_dimensions>0)",
        ],
    ),
    (
        "label_ontology_signals",
        &[
            "unique(observation_id,signal_key)",
            "check(proposed_actionin('observe','add_positive_atom','add_negative_atom','update_semantics','bootstrap_label','rename_label','split_label','merge_labels'))",
        ],
    ),
    (
        "label_ontology_actions",
        &[
            "'adopt_existing_atom'",
            "'revert_ontology_mutation'",
            "check(validation_statusin('not_required','pending','passed','failed','partial'))",
            "check(validation_requirementin('none','required','unsupported'))",
        ],
    ),
    (
        "signal_observations",
        &["check(json_valid(evidence_json)andjson_type(evidence_json)='object')"],
    ),
    (
        "signals",
        &[
            "check(statusin('open','confirmed','rejected','superseded','resolved'))",
            "check(id!=superseded_by_signal_id)",
        ],
    ),
    (
        "retrieval_vectors",
        &["check(dimensions>0)", "unique(document_id,embedding_model)"],
    ),
    (
        "import_journal",
        &[
            "check(source_kindin('jsonl','sqlite_v30'))",
            "check(phasein('prepared','staged','validated','published','completed','failed'))",
        ],
    ),
    (
        "attachment_staging",
        &[
            "check(expected_size_bytes>=0)",
            "check(phasein('planned','copied','verified','published','failed'))",
        ],
    ),
];

pub(crate) async fn apply(
    connection: &mut Connection,
    path: &Path,
    backup_hook: Option<&dyn crate::db::UpgradeBackupHook>,
) -> Result<MigrationOutcome, StoreError> {
    let state = inspect_state(connection).await?;
    let from_version = match state {
        SchemaState::Fresh => 0,
        SchemaState::CurrentV1 => schema::SCHEMA_VERSION,
        SchemaState::Upgraded => schema::FULL_SCHEMA_VERSION,
    };
    let fingerprint = match state {
        SchemaState::Fresh | SchemaState::CurrentV1 => {
            schema::CURRENT_V1_SCHEMA_FINGERPRINT.to_owned()
        }
        SchemaState::Upgraded => full_schema_fingerprint(),
    };

    if state == SchemaState::CurrentV1 {
        let backup_path = create_verified_backup(connection, path).await?;
        if let Some(hook) = backup_hook {
            hook.before_upgrade(&crate::db::UpgradeBackupRequest {
                source_path: path.to_path_buf(),
                backup_path,
                family: schema::SCHEMA_FAMILY.to_owned(),
                from_version,
                to_version: schema::FULL_SCHEMA_VERSION,
                fingerprint: fingerprint.clone(),
            })?;
        }
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;

    if state == SchemaState::Fresh {
        transaction.execute_batch(schema::CANONICAL_SCHEMA).await?;
    }

    ensure_schema_migrations_metadata(&transaction).await?;

    if state != SchemaState::Upgraded {
        rebuild_task_comments_for_full_schema(&transaction).await?;
        transaction.execute_batch(schema::FULL_SCHEMA).await?;
        validate_full_shape(&transaction).await?;
        let checksum = full_schema_fingerprint();
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at, schema_family) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(version) DO UPDATE SET name=excluded.name, checksum=excluded.checksum, schema_family=excluded.schema_family",
                (
                    schema::FULL_SCHEMA_VERSION,
                    schema::FULL_SCHEMA_NAME,
                    checksum.as_str(),
                    now_ms(),
                    schema::SCHEMA_FAMILY,
                ),
            )
            .await?;
        transaction
            .execute(
                "INSERT INTO schema_identity(singleton, family, lineage, version, fingerprint, migration_checksum, upgraded_at) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(singleton) DO UPDATE SET family=excluded.family, lineage=excluded.lineage, version=excluded.version, fingerprint=excluded.fingerprint, migration_checksum=excluded.migration_checksum, upgraded_at=excluded.upgraded_at",
                (
                    schema::SCHEMA_FAMILY,
                    schema::SCHEMA_LINEAGE,
                    schema::FULL_SCHEMA_VERSION,
                    full_schema_fingerprint(),
                    full_schema_fingerprint(),
                    now_ms(),
                ),
            )
            .await?;
    } else {
        validate_full_shape(&transaction).await?;
        validate_upgraded_ledger(&transaction).await?;
    }

    ensure_substrate_seeds(&transaction).await?;
    transaction.commit().await?;

    Ok(MigrationOutcome {
        state,
        from_version,
        to_version: schema::FULL_SCHEMA_VERSION,
        fingerprint,
    })
}

/// 使用 Turso 自身的 `VACUUM INTO` 生成 durable snapshot，并在开始 schema 事务前重新打开
/// 备份，验证 lineage、integrity 与逐表行数。验证失败时源库尚未发生任何 schema 写入。
async fn create_verified_backup(
    connection: &Connection,
    source_path: &Path,
) -> Result<PathBuf, StoreError> {
    let backup_path = next_backup_path(source_path)?;
    let backup_literal = sql_text_literal(
        backup_path
            .to_str()
            .ok_or_else(|| StoreError::BackupRequired("备份路径不是有效 UTF-8".to_owned()))?,
    );
    connection
        .execute(&format!("VACUUM INTO {backup_literal}"), ())
        .await
        .map_err(|error| StoreError::BackupRequired(format!("Turso 备份失败: {error}")))?;

    let metadata = std::fs::metadata(&backup_path)
        .map_err(|error| StoreError::BackupRequired(format!("读取备份文件失败: {error}")))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(StoreError::BackupRequired(format!(
            "Turso 备份不是非空普通文件: {}",
            backup_path.display()
        )));
    }

    let backup_database = Builder::new_local(
        backup_path
            .to_str()
            .ok_or_else(|| StoreError::BackupRequired("备份路径不是有效 UTF-8".to_owned()))?,
    )
    .build()
    .await
    .map_err(|error| StoreError::BackupRequired(format!("重新打开备份失败: {error}")))?;
    let backup = backup_database
        .connect()
        .map_err(|error| StoreError::BackupRequired(format!("连接备份失败: {error}")))?;
    if inspect_state(&backup).await? != SchemaState::CurrentV1 {
        return Err(StoreError::BackupRequired(
            "备份不是精确的 Turso v1 lineage".to_owned(),
        ));
    }
    verify_integrity(&backup).await?;
    for table in V1_TABLES {
        let source_count = table_row_count(connection, table).await?;
        let backup_count = table_row_count(&backup, table).await?;
        if source_count != backup_count {
            return Err(StoreError::BackupRequired(format!(
                "备份表 {table} 行数不一致: source={source_count}, backup={backup_count}"
            )));
        }
    }
    Ok(backup_path)
}

fn next_backup_path(source_path: &Path) -> Result<PathBuf, StoreError> {
    let parent = source_path.parent().ok_or_else(|| {
        StoreError::BackupRequired(format!("数据库路径没有父目录: {}", source_path.display()))
    })?;
    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            StoreError::BackupRequired(format!(
                "数据库文件名不是有效 UTF-8: {}",
                source_path.display()
            ))
        })?;
    let timestamp = now_ms();
    for sequence in 0_u16..=u16::MAX {
        let suffix = if sequence == 0 {
            String::new()
        } else {
            format!("-{sequence}")
        };
        let candidate = parent.join(format!(
            "{file_name}.pre-v2-{timestamp}{suffix}.turso-backup"
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(StoreError::BackupRequired(
        "无法为升级备份分配唯一文件名".to_owned(),
    ))
}

fn sql_text_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

async fn verify_integrity(connection: &Connection) -> Result<(), StoreError> {
    let mut rows = connection
        .query("PRAGMA integrity_check", ())
        .await
        .map_err(|error| {
            StoreError::BackupRequired(format!("备份 integrity_check 失败: {error}"))
        })?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| StoreError::BackupRequired(format!("读取备份完整性结果失败: {error}")))?
    else {
        return Err(StoreError::BackupRequired(
            "备份 integrity_check 没有返回结果".to_owned(),
        ));
    };
    match row
        .get_value(0)
        .map_err(|error| StoreError::BackupRequired(format!("读取备份完整性值失败: {error}")))?
    {
        Value::Text(value) if value.eq_ignore_ascii_case("ok") => Ok(()),
        value => Err(StoreError::BackupRequired(format!(
            "备份 integrity_check 未通过: {value:?}"
        ))),
    }
}

async fn table_row_count(connection: &Connection, table: &str) -> Result<i64, StoreError> {
    let mut rows = connection
        .query(&format!("SELECT COUNT(*) FROM {table}"), ())
        .await?;
    let Some(row) = rows.next().await? else {
        return Err(StoreError::BackupRequired(format!(
            "备份计数查询没有返回表 {table}"
        )));
    };
    match row.get_value(0)? {
        Value::Integer(value) => Ok(value),
        value => Err(StoreError::BackupRequired(format!(
            "表 {table} 的备份计数不是整数: {value:?}"
        ))),
    }
}

pub(crate) fn full_schema_fingerprint() -> String {
    format!(
        "sql-sha256:{:x}",
        Sha256::digest(schema::FULL_SCHEMA.as_bytes())
    )
}

async fn inspect_state(connection: &Connection) -> Result<SchemaState, StoreError> {
    let tables = table_names(connection).await?;
    if tables.is_empty() {
        return Ok(SchemaState::Fresh);
    }

    if tables.contains("schema_identity") || tables.contains("schema_capabilities") {
        if !tables.contains("schema_identity") || !tables.contains("schema_capabilities") {
            return Err(StoreError::SchemaMismatch(
                "partial Turso full-feature metadata tables".to_owned(),
            ));
        }
        validate_full_shape(connection).await?;
        validate_upgraded_ledger(connection).await?;
        return Ok(SchemaState::Upgraded);
    }

    if !tables.contains("schema_migrations") {
        return Err(StoreError::SchemaMismatch(
            "unknown database has tables but no schema_migrations ledger".to_owned(),
        ));
    }
    if tables
        .iter()
        .any(|table| !V1_TABLES.contains(&table.as_str()))
        || tables.len() != V1_TABLES.len()
    {
        return Err(StoreError::SchemaMismatch(
            "schema_migrations version resembles v1 but table set is not Turso v1".to_owned(),
        ));
    }
    validate_v1_shape(connection).await?;
    let mut version_rows = connection
        .query("SELECT version FROM schema_migrations", ())
        .await?;
    while let Some(version_row) = version_rows.next().await? {
        match version_row.get_value(0)? {
            Value::Integer(version) if version == schema::SCHEMA_VERSION => {}
            Value::Integer(version) => {
                return Err(StoreError::SchemaMismatch(format!(
                    "unknown migration version {version} in Turso v1 ledger"
                )));
            }
            _ => {
                return Err(StoreError::SchemaMismatch(
                    "Turso v1 ledger version is not integer".to_owned(),
                ));
            }
        }
    }
    let row = migration_row(connection, schema::SCHEMA_VERSION).await?;
    let Some(row) = row else {
        return Err(StoreError::SchemaMismatch(
            "Turso v1 table set has no version 1 ledger row".to_owned(),
        ));
    };
    if row.name != schema::SCHEMA_NAME {
        return Err(StoreError::SchemaMismatch(format!(
            "version 1 ledger name is {}, expected {}",
            row.name,
            schema::SCHEMA_NAME
        )));
    }
    if !row.checksum.is_empty() && row.checksum != schema::CURRENT_V1_SCHEMA_FINGERPRINT {
        return Err(StoreError::SchemaMismatch(format!(
            "version 1 checksum {} is not current Turso lineage {}",
            row.checksum,
            schema::CURRENT_V1_SCHEMA_FINGERPRINT
        )));
    }
    let columns = table_columns(connection, "schema_migrations").await?;
    if columns.contains("schema_family") {
        let mut family_rows = connection
            .query(
                "SELECT schema_family FROM schema_migrations WHERE version = ?1",
                [schema::SCHEMA_VERSION],
            )
            .await?;
        let Some(family_row) = family_rows.next().await? else {
            return Err(StoreError::SchemaMismatch(
                "Turso v1 schema family row is missing".to_owned(),
            ));
        };
        match family_row.get_value(0)? {
            Value::Text(value) if value == schema::SCHEMA_FAMILY => {}
            _ => {
                return Err(StoreError::SchemaMismatch(
                    "Turso v1 schema belongs to an unknown family".to_owned(),
                ));
            }
        }
    }
    Ok(SchemaState::CurrentV1)
}

async fn table_names(connection: &Connection) -> Result<BTreeSet<String>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_internal_%' ORDER BY name",
            (),
        )
        .await?;
    let mut names = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        let value = row.get_value(0)?;
        if let Value::Text(name) = value {
            names.insert(name);
        }
    }
    Ok(names)
}

async fn table_columns(
    connection: &Connection,
    table: &str,
) -> Result<BTreeSet<String>, StoreError> {
    let sql = format!("PRAGMA table_info('{table}')");
    let mut rows = connection.query(&sql, ()).await?;
    let mut columns = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        if let Value::Text(name) = row.get_value(1)? {
            columns.insert(name);
        }
    }
    Ok(columns)
}

async fn validate_v1_shape(connection: &Connection) -> Result<(), StoreError> {
    for (table, expected) in V1_EXACT_COLUMNS {
        validate_exact_columns(connection, "Turso v1", table, expected).await?;
    }
    Ok(())
}

async fn validate_full_shape(connection: &Connection) -> Result<(), StoreError> {
    let tables = table_names(connection).await?;
    let expected_tables = V1_EXACT_COLUMNS
        .iter()
        .chain(FULL_EXACT_COLUMNS.iter())
        .map(|(table, _)| (*table).to_owned())
        .collect::<BTreeSet<_>>();
    if tables != expected_tables {
        let missing = expected_tables
            .difference(&tables)
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = tables
            .difference(&expected_tables)
            .cloned()
            .collect::<Vec<_>>();
        return Err(StoreError::SchemaMismatch(format!(
            "完整 Turso schema 的 table fingerprint 不匹配: missing={missing:?}, unexpected={unexpected:?}"
        )));
    }

    for (table, expected) in V1_EXACT_COLUMNS {
        if *table == "schema_migrations" {
            let mut expected = expected.to_vec();
            expected.push("schema_family");
            validate_exact_columns(connection, "完整 Turso schema", table, &expected).await?;
        } else {
            validate_exact_columns(connection, "完整 Turso schema", table, expected).await?;
        }
    }

    for (table, expected) in FULL_EXACT_COLUMNS {
        validate_exact_columns(connection, "完整 Turso schema", table, expected).await?;
    }
    validate_required_objects(connection, "index", FULL_REQUIRED_INDEXES).await?;
    validate_required_objects(connection, "trigger", FULL_REQUIRED_TRIGGERS).await?;
    validate_required_sql_fragments(connection).await?;
    validate_board_isolation(connection).await?;
    validate_foreign_keys(connection).await?;
    Ok(())
}

async fn validate_exact_columns(
    connection: &Connection,
    lineage: &str,
    table: &str,
    expected: &[&str],
) -> Result<(), StoreError> {
    let columns = table_columns(connection, table).await?;
    let expected = expected
        .iter()
        .map(|column| (*column).to_owned())
        .collect::<BTreeSet<_>>();
    if columns == expected {
        return Ok(());
    }
    let missing = expected.difference(&columns).cloned().collect::<Vec<_>>();
    let unexpected = columns.difference(&expected).cloned().collect::<Vec<_>>();
    Err(StoreError::SchemaMismatch(format!(
        "{lineage} 表 {table} 的 column fingerprint 不匹配: missing={missing:?}, unexpected={unexpected:?}"
    )))
}

async fn validate_required_objects(
    connection: &Connection,
    object_type: &str,
    required: &[&str],
) -> Result<(), StoreError> {
    let mut rows = connection
        .query(
            "SELECT name FROM sqlite_master WHERE type = ?1 ORDER BY name",
            [object_type],
        )
        .await?;
    let mut actual = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        if let Value::Text(name) = row.get_value(0)? {
            actual.insert(name);
        }
    }
    let missing = required
        .iter()
        .filter(|name| !actual.contains(**name))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(StoreError::SchemaMismatch(format!(
            "完整 Turso schema 缺少 {object_type}: {missing:?}"
        )))
    }
}

async fn validate_required_sql_fragments(connection: &Connection) -> Result<(), StoreError> {
    for (table, required_fragments) in FULL_REQUIRED_SQL_FRAGMENTS {
        let mut rows = connection
            .query(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [*table],
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Err(StoreError::SchemaMismatch(format!(
                "完整 Turso schema 缺少表定义: {table}"
            )));
        };
        let sql = match row.get_value(0)? {
            Value::Text(value) => value,
            _ => {
                return Err(StoreError::SchemaMismatch(format!(
                    "完整 Turso schema 的表定义不是文本: {table}"
                )));
            }
        };
        let compact = sql
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .flat_map(char::to_lowercase)
            .collect::<String>();
        let missing = required_fragments
            .iter()
            .filter(|fragment| !compact.contains(**fragment))
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(StoreError::SchemaMismatch(format!(
                "完整 Turso schema 的 constraint fingerprint 不匹配: table={table}, missing={missing:?}"
            )));
        }
    }
    Ok(())
}

async fn validate_foreign_keys(connection: &Connection) -> Result<(), StoreError> {
    let mut rows = connection.query("PRAGMA foreign_key_check", ()).await?;
    let Some(row) = rows.next().await? else {
        return Ok(());
    };
    Err(StoreError::SchemaMismatch(format!(
        "完整 Turso schema 的 foreign_key_check 失败: table={:?}, rowid={:?}",
        row.get_value(0)?,
        row.get_value(1)?
    )))
}

async fn validate_board_isolation(connection: &Connection) -> Result<(), StoreError> {
    let mut rows = connection
        .query(
            r#"
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
UNION ALL SELECT 'entity_relations.subject_uri' WHERE EXISTS (
  SELECT 1 FROM entity_relations child
  LEFT JOIN entities parent ON parent.uri=child.subject_uri AND parent.board_id IS child.board_id
  WHERE parent.uri IS NULL
)
UNION ALL SELECT 'entity_relations.object_uri' WHERE EXISTS (
  SELECT 1 FROM entity_relations child
  LEFT JOIN entities parent ON parent.uri=child.object_uri AND parent.board_id IS child.board_id
  WHERE parent.uri IS NULL
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
  LEFT JOIN labels parent
    ON parent.id=child.top1_existing_label_id AND parent.board_id=child.board_id
  WHERE child.top1_existing_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_semantic_proposals.resolved_label_id' WHERE EXISTS (
  SELECT 1 FROM label_semantic_proposals child
  LEFT JOIN labels parent
    ON parent.id=child.resolved_label_id AND parent.board_id=child.board_id
  WHERE child.resolved_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_observations.task_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_observations child
  LEFT JOIN tasks parent ON parent.id=child.task_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_signals.observation_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_signals child
  LEFT JOIN label_ontology_observations parent
    ON parent.id=child.observation_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_signals.target_label_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_signals child
  LEFT JOIN labels parent ON parent.id=child.target_label_id AND parent.board_id=child.board_id
  WHERE child.target_label_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_signals.superseded_by_signal_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_signals child
  LEFT JOIN label_ontology_signals parent
    ON parent.id=child.superseded_by_signal_id AND parent.board_id=child.board_id
  WHERE child.superseded_by_signal_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_actions.parent_action_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_actions child
  LEFT JOIN label_ontology_actions parent
    ON parent.id=child.parent_action_id AND parent.board_id=child.board_id
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
  LEFT JOIN label_semantic_proposals parent
    ON parent.id=child.result_proposal_id AND parent.board_id=child.board_id
  WHERE child.result_proposal_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_action_signals.action_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_action_signals child
  LEFT JOIN label_ontology_actions parent
    ON parent.id=child.action_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'label_ontology_action_signals.signal_id' WHERE EXISTS (
  SELECT 1 FROM label_ontology_action_signals child
  LEFT JOIN label_ontology_signals parent
    ON parent.id=child.signal_id AND parent.board_id=child.board_id
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
  LEFT JOIN signal_observations parent
    ON parent.id=child.observation_id AND parent.board_id=child.board_id
  WHERE parent.id IS NULL
)
UNION ALL SELECT 'signals.superseded_by_signal_id' WHERE EXISTS (
  SELECT 1 FROM signals child
  LEFT JOIN signals parent
    ON parent.id=child.superseded_by_signal_id AND parent.board_id=child.board_id
  WHERE child.superseded_by_signal_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'projection_jobs.source_event_id' WHERE EXISTS (
  SELECT 1 FROM projection_jobs child
  LEFT JOIN task_events parent
    ON parent.id=child.source_event_id AND parent.board_id IS child.board_id
  WHERE child.source_event_id IS NOT NULL AND parent.id IS NULL
)
UNION ALL SELECT 'projection_jobs.entity_uri' WHERE EXISTS (
  SELECT 1 FROM projection_jobs child
  LEFT JOIN entities parent ON parent.uri=child.entity_uri AND parent.board_id IS child.board_id
  WHERE child.entity_uri IS NOT NULL AND parent.uri IS NULL
)
UNION ALL SELECT 'retrieval_documents.entity_uri' WHERE EXISTS (
  SELECT 1 FROM retrieval_documents child
  LEFT JOIN entities parent ON parent.uri=child.entity_uri AND parent.board_id IS child.board_id
  WHERE child.entity_uri IS NOT NULL AND parent.uri IS NULL
)
UNION ALL SELECT 'retrieval_vectors.document_id' WHERE EXISTS (
  SELECT 1 FROM retrieval_vectors child
  LEFT JOIN retrieval_documents parent
    ON parent.id=child.document_id AND parent.board_id IS child.board_id
  WHERE child.document_id IS NOT NULL AND parent.id IS NULL
)
LIMIT 1
"#,
            (),
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(());
    };
    let relation = match row.get_value(0)? {
        Value::Text(value) => value,
        _ => "unknown".to_owned(),
    };
    Err(StoreError::SchemaMismatch(format!(
        "完整 Turso schema 的 board isolation preflight 失败: {relation}"
    )))
}

async fn migration_row(
    connection: &Connection,
    version: i64,
) -> Result<Option<LedgerRow>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
            [version],
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    let name = match row.get_value(0)? {
        Value::Text(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "ledger name is not text".to_owned(),
            ));
        }
    };
    let checksum = match row.get_value(1)? {
        Value::Text(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "ledger checksum is not text".to_owned(),
            ));
        }
    };
    Ok(Some(LedgerRow { name, checksum }))
}

async fn ensure_schema_migrations_metadata(connection: &Connection) -> Result<(), StoreError> {
    let columns = table_columns(connection, "schema_migrations").await?;
    if !columns.contains("schema_family") {
        connection
            .execute(
                "ALTER TABLE schema_migrations ADD COLUMN schema_family TEXT NOT NULL DEFAULT 'kanban.turso'",
                (),
            )
            .await?;
    }
    connection
        .execute(
            "UPDATE schema_migrations SET schema_family=?1 WHERE schema_family IS NULL OR trim(schema_family) = ''",
            [schema::SCHEMA_FAMILY],
        )
        .await?;
    connection
        .execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at, schema_family) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(version) DO UPDATE SET name=excluded.name, checksum=excluded.checksum, schema_family=excluded.schema_family",
            (
                schema::SCHEMA_VERSION,
                schema::SCHEMA_NAME,
                schema::CURRENT_V1_SCHEMA_FINGERPRINT,
                now_ms(),
                schema::SCHEMA_FAMILY,
            ),
        )
        .await?;
    Ok(())
}

async fn rebuild_task_comments_for_full_schema(connection: &Connection) -> Result<(), StoreError> {
    connection
        .execute_batch(
            r#"
CREATE TABLE task_comments_new (
  id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL,
  idempotency_key TEXT,
  author TEXT NOT NULL,
  author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
  agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL),
  body TEXT NOT NULL CHECK(length(trim(body)) > 0),
  kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision', 'signal')),
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
  created_at INTEGER NOT NULL,
  UNIQUE(id, board_id),
  FOREIGN KEY(task_id, board_id) REFERENCES tasks(id, board_id) ON DELETE CASCADE,
  UNIQUE(task_id, idempotency_key)
);
"#,
        )
        .await?;
    connection
        .execute(
            "INSERT INTO task_comments_new(id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at) SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, CASE WHEN kind IN ('note', 'decision', 'signal') THEN kind ELSE 'note' END, metadata_json, created_at FROM task_comments",
            (),
        )
        .await?;
    connection
        .execute_batch(
            "DROP TABLE task_comments; ALTER TABLE task_comments_new RENAME TO task_comments;",
        )
        .await?;
    connection
        .execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_task_comments_idempotency ON task_comments(task_id, idempotency_key) WHERE idempotency_key IS NOT NULL;",
        )
        .await?;
    Ok(())
}

async fn validate_upgraded_ledger(connection: &Connection) -> Result<(), StoreError> {
    let migration_columns = table_columns(connection, "schema_migrations").await?;
    if !migration_columns.contains("schema_family") {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema ledger has no schema_family column".to_owned(),
        ));
    }
    let mut version_rows = connection
        .query("SELECT version FROM schema_migrations", ())
        .await?;
    while let Some(version_row) = version_rows.next().await? {
        match version_row.get_value(0)? {
            Value::Integer(version)
                if version == schema::SCHEMA_VERSION || version == schema::FULL_SCHEMA_VERSION => {}
            Value::Integer(version) => {
                return Err(StoreError::SchemaMismatch(format!(
                    "unknown migration version {version} in full Turso ledger"
                )));
            }
            _ => {
                return Err(StoreError::SchemaMismatch(
                    "full Turso ledger version is not integer".to_owned(),
                ));
            }
        }
    }
    let mut family_rows = connection
        .query(
            "SELECT schema_family FROM schema_migrations WHERE version = ?1",
            [schema::FULL_SCHEMA_VERSION],
        )
        .await?;
    let Some(family_row) = family_rows.next().await? else {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema has no version 2 family row".to_owned(),
        ));
    };
    match family_row.get_value(0)? {
        Value::Text(value) if value == schema::SCHEMA_FAMILY => {}
        _ => {
            return Err(StoreError::SchemaMismatch(
                "full Turso schema belongs to an unknown family".to_owned(),
            ));
        }
    }
    let Some(row) = migration_row(connection, schema::FULL_SCHEMA_VERSION).await? else {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema has no version 2 ledger row".to_owned(),
        ));
    };
    if row.name != schema::FULL_SCHEMA_NAME || row.checksum != full_schema_fingerprint() {
        return Err(StoreError::SchemaMismatch(format!(
            "full Turso migration ledger mismatch: {} / {}",
            row.name, row.checksum
        )));
    }
    let Some(v1_row) = migration_row(connection, schema::SCHEMA_VERSION).await? else {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema has no version 1 lineage row".to_owned(),
        ));
    };
    if v1_row.name != schema::SCHEMA_NAME
        || (!v1_row.checksum.is_empty() && v1_row.checksum != schema::CURRENT_V1_SCHEMA_FINGERPRINT)
    {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema version 1 lineage row does not match this binary".to_owned(),
        ));
    }
    let mut v1_family_rows = connection
        .query(
            "SELECT schema_family FROM schema_migrations WHERE version = ?1",
            [schema::SCHEMA_VERSION],
        )
        .await?;
    let Some(v1_family_row) = v1_family_rows.next().await? else {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema version 1 family row is missing".to_owned(),
        ));
    };
    match v1_family_row.get_value(0)? {
        Value::Text(value) if value == schema::SCHEMA_FAMILY => {}
        _ => {
            return Err(StoreError::SchemaMismatch(
                "full Turso schema version 1 belongs to an unknown family".to_owned(),
            ));
        }
    }
    let mut identity_rows = connection
        .query(
            "SELECT family, lineage, version, fingerprint, migration_checksum FROM schema_identity WHERE singleton = 1",
            (),
        )
        .await?;
    let Some(identity) = identity_rows.next().await? else {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema identity row is missing".to_owned(),
        ));
    };
    let identity_family = match identity.get_value(0)? {
        Value::Text(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "schema identity family is not text".to_owned(),
            ));
        }
    };
    let identity_lineage = match identity.get_value(1)? {
        Value::Text(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "schema identity lineage is not text".to_owned(),
            ));
        }
    };
    let identity_version = match identity.get_value(2)? {
        Value::Integer(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "schema identity version is not integer".to_owned(),
            ));
        }
    };
    let identity_fingerprint = match identity.get_value(3)? {
        Value::Text(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "schema identity fingerprint is not text".to_owned(),
            ));
        }
    };
    let identity_checksum = match identity.get_value(4)? {
        Value::Text(value) => value,
        _ => {
            return Err(StoreError::SchemaMismatch(
                "schema identity checksum is not text".to_owned(),
            ));
        }
    };
    let checksum = full_schema_fingerprint();
    if identity_family != schema::SCHEMA_FAMILY
        || identity_lineage != schema::SCHEMA_LINEAGE
        || identity_version != schema::FULL_SCHEMA_VERSION
        || identity_fingerprint != checksum
        || identity_checksum != checksum
    {
        return Err(StoreError::SchemaMismatch(
            "full Turso schema identity does not match this binary".to_owned(),
        ));
    }
    Ok(())
}

async fn ensure_substrate_seeds(connection: &Connection) -> Result<(), StoreError> {
    let now = now_ms();
    for projection in ["fts", "vector_tasks", "vector_label_atoms", "relations"] {
        connection
            .execute(
                "INSERT OR IGNORE INTO projection_state(projection, lifecycle_status, last_event_id, dirty, fence_epoch, updated_at) VALUES (?1, 'bootstrap_required', 0, 1, 0, ?2)",
                (projection, now),
            )
            .await?;
    }
    connection
        .execute(
            "INSERT OR IGNORE INTO projection_maintenance_owner(singleton, capabilities_json, fence_epoch, updated_at) VALUES (1, '[]', 0, ?1)",
            [now],
        )
        .await?;
    for (name, domain, range) in [
        ("belongs_to_board", "task", "board"),
        ("depends_on", "task", "task"),
        ("mentions", "task", "task"),
    ] {
        connection
            .execute(
                "INSERT OR IGNORE INTO relation_predicates(name, domain_kind, range_kind, cardinality, authoritative_store, created_at) VALUES (?1, ?2, ?3, 'many', 'turso', ?4)",
                (name, domain, range, now),
            )
            .await?;
    }
    Ok(())
}
