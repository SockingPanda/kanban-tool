//! Host-owned doctor/checkpoint/backup/import/compaction primitives.
//!
//! 所有方法只在 `kanban-server` 的 canonical Turso owner 内调用。portable JSONL
//! 只包含 canonical facts；projection、FTS、vector 和 graph 表属于可重建派生物。

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use turso::{Connection, Value, params_from_iter, transaction::TransactionBehavior};

use crate::{TursoStore, error::StoreError, migration, schema, shared::now_ms};

/// 外部快照中的 canonical facts。派生 projection 不得被导出为事实。
pub(crate) const PORTABLE_TABLES: &[&str] = &[
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
    "label_atom_index_boards",
    "label_semantic_proposals",
    "label_ontology_observations",
    "label_ontology_signals",
    "label_ontology_actions",
    "label_ontology_action_signals",
    "label_ontology_action_atom_effects",
    "signal_observations",
    "signals",
];

/// replace 时必须先删除子表，再删除父表。该顺序与 `PORTABLE_TABLES` 的导入顺序
/// 相反，保留外部 schema、migration 和 host 治理表不动。
const PORTABLE_REPLACE_DELETE_TABLES: &[&str] = &[
    "signals",
    "signal_observations",
    "label_ontology_action_atom_effects",
    "label_ontology_action_signals",
    "label_ontology_actions",
    "label_ontology_signals",
    "label_ontology_observations",
    "label_semantic_proposals",
    "label_atom_index_boards",
    "label_atoms",
    "label_semantics",
    "entity_relations",
    "relation_predicates",
    "entities",
    "task_subtasks",
    "app_settings",
    "task_labels",
    "labels",
    "task_attachments",
    "task_events",
    "task_comments",
    "task_runs",
    "task_dependencies",
    "task_steps",
    "task_execution_plans",
    "tasks",
    "board_columns",
    "boards",
];

/// 诊断所依赖的 schema/identity 表清单；它们不属于 portable business facts。
#[allow(dead_code)]
const SCHEMA_TABLES: &[&str] = &[
    "boards",
    "board_columns",
    "tasks",
    "task_execution_plans",
    "task_steps",
    "task_dependencies",
    "task_runs",
    "task_comments",
    "task_events",
    "schema_migrations",
    "schema_identity",
    "schema_capabilities",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDoctorReport {
    pub ok: bool,
    pub integrity_check: String,
    pub migration_version: Option<i64>,
    pub user_version: i64,
    pub expired_running_tasks: i64,
    pub running_tasks_without_active_run: i64,
    pub orphan_running_runs: i64,
    pub dependency_cycles: i64,
    pub archived_dependency_edges: i64,
    pub missing_run_logs: i64,
    pub suspicious_run_log_paths: i64,
    pub executable_dependency_violations: i64,
    pub executable_spec_violations: i64,
    pub executable_schedule_violations: i64,
    pub unplanned_active_tasks: i64,
    pub active_parents_with_incomplete_required_steps: i64,
    pub outbox_pending: i64,
    pub outbox_running: i64,
    pub outbox_failed: i64,
    pub derived_dirty_stores: i64,
    pub derived_error_stores: i64,
    pub derived_stores: Vec<StoreDoctorDerivedStore>,
    pub consistency_errors: i64,
    pub consistency_warnings: i64,
    pub consistency_issues: Vec<StoreDoctorIssue>,
    pub ontology_ledger_errors: i64,
    pub ontology_ledger_warnings: i64,
    pub ontology_ledger_issues: Vec<StoreDoctorIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDoctorDerivedStore {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_error: Option<String>,
    pub pending_outbox: i64,
    pub running_outbox: i64,
    pub failed_outbox: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDoctorIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreCheckpointReport {
    pub busy: i64,
    pub log_frames: i64,
    pub checkpointed_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreBackupReport {
    pub out_path: String,
    pub checksum_sha256: String,
    pub bytes: u64,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreExportReport {
    pub out_path: String,
    pub checksum_sha256: String,
    pub bytes: u64,
    pub record_count: u64,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreImportReport {
    pub in_path: String,
    pub source_fingerprint: String,
    pub imported_records: u64,
    pub skipped_records: u64,
    pub rebuild_jobs_enqueued: u64,
    pub journal_id: String,
    /// `completed` 表示 canonical 已提交；`validated` 表示 replace staging 已校验，
    /// 仍需停止 host 后由 lifecycle owner 发布。
    pub phase: String,
    pub restart_required: bool,
    pub staged_database_path: Option<String>,
    pub target_fingerprint_before: Option<String>,
    pub staged_fingerprint: Option<String>,
    pub publish_preconditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreVacuumReport {
    pub ok: bool,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreMaintenanceOwner {
    pub owner: Option<String>,
    pub mode: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub fence_epoch: i64,
    pub build_identity: Option<String>,
    pub last_heartbeat_at: Option<i64>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreProjectionStatus {
    pub store_name: String,
    pub active_generation: Option<String>,
    pub active_fingerprint: Option<String>,
    pub previous_generation: Option<String>,
    pub building_generation: Option<String>,
    pub lifecycle_status: String,
    pub fence_epoch: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
    pub last_error: Option<String>,
    /// 当前 projection 的可审计阶段；它来自实际派生状态而不是请求动作。
    pub phase: String,
    /// provider 或 job 失败时保持降级，不能把 pending projection 报成 ready。
    pub degraded: bool,
    /// 当前阶段所有可见错误；`last_error` 保留兼容字段。
    pub errors: Vec<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreMaintenanceStatus {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: StoreMaintenanceOwner,
    pub stores: Vec<StoreProjectionStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreMaintenanceRun {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: String,
    pub mode: String,
    pub action: String,
    pub processed: u64,
    pub phase: String,
    pub degraded: bool,
    pub errors: Vec<String>,
    pub stores: Vec<StoreProjectionStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PortableHeader {
    format: String,
    version: u32,
    schema_family: String,
    schema_lineage: String,
    schema_version: i64,
    schema_fingerprint: String,
    source_fingerprint: String,
    canonical_tables: Vec<String>,
    table_counts: BTreeMap<String, u64>,
    record_count: u64,
    payload_checksum_sha256: String,
    manifest_checksum_sha256: String,
    /// 当前 JSONL 只携带 `task_attachments` metadata；二进制文件需独立 attachment
    /// staging/publish 协议，不能在导入时静默声称已迁移。
    attachments_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PortableLine {
    #[serde(rename = "type")]
    table: String,
    data: serde_json::Map<String, serde_json::Value>,
}

/// 已完成预检的 portable 快照。解析和校验在任何 canonical 写入之前完成。
struct PortableSnapshot {
    header: PortableHeader,
    records: Vec<PortableLine>,
    payload_checksum_sha256: String,
    table_counts: BTreeMap<String, u64>,
}

impl TursoStore {
    pub async fn doctor(&self) -> Result<StoreDoctorReport, StoreError> {
        doctor_connection(&self.connection().await?).await
    }

    pub async fn checkpoint(&self) -> Result<StoreCheckpointReport, StoreError> {
        let lease = self
            .acquire_maintenance_lease("backup", "host-admin")
            .await?;
        let report = self.checkpoint_inner().await?;
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    async fn checkpoint_inner(&self) -> Result<StoreCheckpointReport, StoreError> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::InvalidInput("checkpoint 没有返回结果".to_owned()))?;
        Ok(StoreCheckpointReport {
            busy: integer_value(row.get_value(0)?, "checkpoint.busy")?,
            log_frames: integer_value(row.get_value(1)?, "checkpoint.log_frames")?,
            checkpointed_frames: integer_value(
                row.get_value(2)?,
                "checkpoint.checkpointed_frames",
            )?,
        })
    }

    pub async fn backup(
        &self,
        out_path: impl AsRef<Path>,
    ) -> Result<StoreBackupReport, StoreError> {
        let out_path = checked_target(out_path.as_ref(), "backup")?;
        let lease = self
            .acquire_maintenance_lease("backup", "host-admin")
            .await?;
        let _ = self.checkpoint_inner().await?;
        let source_fingerprint = self.database_fingerprint().await?;
        let temp = temporary_sibling(&out_path, "backup")?;
        vacuum_into(&self.connection().await?, &temp).await?;
        verify_database_file(&temp).await?;
        durable_rename(&temp, &out_path)?;
        let (checksum_sha256, bytes) = file_digest(&out_path)?;
        let report = StoreBackupReport {
            out_path: out_path.display().to_string(),
            checksum_sha256,
            bytes,
            source_fingerprint,
        };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    pub async fn export(
        &self,
        out_path: impl AsRef<Path>,
    ) -> Result<StoreExportReport, StoreError> {
        let out_path = checked_target(out_path.as_ref(), "export")?;
        let lease = self
            .acquire_maintenance_lease("backup", "host-admin")
            .await?;
        let source_fingerprint = self.database_fingerprint().await?;
        let temp = temporary_sibling(&out_path, "export")?;
        let mut writer = BufWriter::new(File::create(&temp).map_err(io_error)?);
        let connection = self.connection().await?;
        let (schema_family, schema_lineage, schema_version, schema_fingerprint) =
            portable_schema_identity(&connection).await?;
        let records = collect_portable_records(&connection).await?;
        let (payload, table_counts) = serialize_portable_records(&records)?;
        let mut header = PortableHeader {
            format: "kanban.portable.jsonl".to_owned(),
            version: 2,
            schema_family,
            schema_lineage,
            schema_version,
            schema_fingerprint,
            source_fingerprint: source_fingerprint.clone(),
            canonical_tables: PORTABLE_TABLES
                .iter()
                .map(|table| (*table).to_owned())
                .collect(),
            record_count: records.len() as u64,
            payload_checksum_sha256: digest_bytes(&payload),
            table_counts,
            manifest_checksum_sha256: String::new(),
            attachments_mode: "metadata_only".to_owned(),
        };
        header.manifest_checksum_sha256 = manifest_checksum(&header)?;
        serde_json::to_writer(&mut writer, &header).map_err(json_error)?;
        writer.write_all(b"\n").map_err(io_error)?;
        writer.write_all(&payload).map_err(io_error)?;
        writer.flush().map_err(io_error)?;
        writer
            .into_inner()
            .map_err(|error| io_error(error.into_error()))?
            .sync_all()
            .map_err(io_error)?;
        durable_rename(&temp, &out_path)?;
        let (checksum_sha256, bytes) = file_digest(&out_path)?;
        let report = StoreExportReport {
            out_path: out_path.display().to_string(),
            checksum_sha256,
            bytes,
            record_count: records.len() as u64,
            source_fingerprint,
        };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    /// 当前 host 已持有 Turso handle，replace 不直接替换旧 inode。非 replace 导入
    /// 只允许空 canonical target，并通过 `import_journal` 记录可恢复阶段。
    pub async fn import(
        &self,
        in_path: impl AsRef<Path>,
        replace: bool,
    ) -> Result<StoreImportReport, StoreError> {
        let in_path = in_path.as_ref();
        if !fs::symlink_metadata(in_path)
            .map(|metadata| metadata.file_type().is_file())
            .unwrap_or(false)
        {
            return Err(StoreError::InvalidInput(format!(
                "portable import source 不是普通文件: {}",
                in_path.display()
            )));
        }
        let lease = self
            .acquire_maintenance_lease("import", "host-admin")
            .await?;
        let result = self
            .import_with_lease(in_path, replace, &lease, false)
            .await;
        let release = self.release_maintenance_lease(&lease).await;
        match (result, release) {
            (Ok(report), Ok(())) => Ok(report),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    /// 仅执行 replace 的 prepare/verify 阶段并返回 typed 状态。
    ///
    /// 供 host lifecycle 预检使用的 typed seam。普通 HTTP/CLI `import --replace` 不走此
    /// 预检路径，而是在当前 host-owned Turso handle 内完成真实事务替换。
    pub async fn prepare_import(
        &self,
        in_path: impl AsRef<Path>,
    ) -> Result<StoreImportReport, StoreError> {
        let in_path = in_path.as_ref();
        if !fs::symlink_metadata(in_path)
            .map(|metadata| metadata.file_type().is_file())
            .unwrap_or(false)
        {
            return Err(StoreError::InvalidInput(format!(
                "portable import source 不是普通文件: {}",
                in_path.display()
            )));
        }
        let lease = self
            .acquire_maintenance_lease("import", "host-admin")
            .await?;
        let result = self.import_with_lease(in_path, true, &lease, true).await;
        let release = self.release_maintenance_lease(&lease).await;
        match (result, release) {
            (Ok(report), Ok(())) => Ok(report),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    async fn import_with_lease(
        &self,
        in_path: &Path,
        replace: bool,
        _lease: &MaintenanceLease,
        return_prepared: bool,
    ) -> Result<StoreImportReport, StoreError> {
        let snapshot = read_portable(in_path)?;
        validate_portable_snapshot(&snapshot.header, &snapshot)?;
        let manifest = serde_json::to_string(&snapshot.header).map_err(json_error)?;
        let snapshot_fingerprint = portable_snapshot_fingerprint(&snapshot.header);
        let mut connection = self.connection().await?;
        validate_portable_columns(&connection, &snapshot).await?;
        let target_fingerprint_before = self.database_fingerprint().await?;

        let journal = find_portable_journal(&connection, &snapshot_fingerprint).await?;
        if let Some(journal) = &journal {
            match journal.phase.as_str() {
                "completed" => {
                    verify_imported_target(&connection, &snapshot.header).await?;
                    let jobs = count_rebuild_jobs(&connection, &snapshot_fingerprint).await?;
                    return Ok(import_report(
                        in_path,
                        &snapshot.header,
                        journal.id.clone(),
                        snapshot.header.record_count,
                        jobs,
                        ImportReportDetails {
                            phase: "completed",
                            restart_required: false,
                            staged_database_path: None,
                            target_fingerprint_before,
                            staged_fingerprint: None,
                            publish_preconditions: Vec::new(),
                        },
                    ));
                }
                "failed" => {}
                "validated" if replace => {}
                "published" if replace => {}
                phase => {
                    return Err(StoreError::MaintenanceBusy(format!(
                        "portable import journal {} 处于不可恢复阶段 {phase}，请先完成 recovery",
                        journal.id
                    )));
                }
            }
        }

        let existing = canonical_record_count(&connection).await?;
        if replace && !return_prepared {
            return self
                .replace_import_transaction(
                    in_path,
                    &snapshot,
                    &manifest,
                    &snapshot_fingerprint,
                    &mut connection,
                    (journal.as_ref(), &target_fingerprint_before),
                )
                .await;
        }

        if replace && return_prepared {
            if let Some(journal) = journal.as_ref()
                && journal.phase == "validated"
            {
                let staged_path = journal.staged_database_path.clone().ok_or_else(|| {
                    StoreError::InvalidInput(
                        "portable replace journal 缺少 staged database path".to_owned(),
                    )
                })?;
                let (staged_fingerprint, jobs) =
                    verify_journal_staging(journal, &staged_path, &snapshot).await?;
                return Ok(prepared_import_report(
                    in_path,
                    &snapshot.header,
                    journal.id.clone(),
                    staged_path,
                    target_fingerprint_before,
                    staged_fingerprint,
                    jobs,
                ));
            }
            let journal_id = format!(
                "ij_{}",
                &snapshot_fingerprint[..snapshot_fingerprint.len().min(32)]
            );
            let staged_path = temporary_sibling(self.database_path(), "portable-replace")?;
            insert_import_journal(
                &connection,
                &journal_id,
                in_path,
                &snapshot_fingerprint,
                "prepared",
                &manifest,
                (None, Some(&target_fingerprint_before)),
            )
            .await?;
            let prepared = self
                .prepare_replacement(
                    &snapshot,
                    in_path,
                    &snapshot_fingerprint,
                    &manifest,
                    (&journal_id, staged_path.as_path()),
                    &target_fingerprint_before,
                )
                .await;
            match prepared {
                Ok((staged_fingerprint, jobs)) => {
                    if let Err(error) = update_import_journal_validated(
                        &connection,
                        &journal_id,
                        &staged_path,
                        &target_fingerprint_before,
                        &staged_fingerprint,
                    )
                    .await
                    {
                        let _ = mark_import_journal_failed(
                            &connection,
                            &journal_id,
                            error.to_string().as_str(),
                        )
                        .await;
                        cleanup_staged_database(&staged_path);
                        return Err(error);
                    }
                    let report = prepared_import_report(
                        in_path,
                        &snapshot.header,
                        journal_id.clone(),
                        staged_path.clone(),
                        target_fingerprint_before.clone(),
                        staged_fingerprint,
                        jobs,
                    );
                    return Ok(report);
                }
                Err(error) => {
                    let _ = mark_import_journal_failed(
                        &connection,
                        &journal_id,
                        error.to_string().as_str(),
                    )
                    .await;
                    cleanup_staged_database(&staged_path);
                    return Err(error);
                }
            }
        }

        if existing > 0 {
            return Err(StoreError::InvalidInput(
                "import target 非空；需要 replace=true 才能替换 canonical facts".to_owned(),
            ));
        }
        let journal_id = deterministic_journal_id(&snapshot_fingerprint);
        insert_import_journal(
            &connection,
            &journal_id,
            in_path,
            &snapshot_fingerprint,
            "prepared",
            &manifest,
            (None, Some(&target_fingerprint_before)),
        )
        .await?;
        let imported_records =
            match import_records_into_connection(&mut connection, &snapshot).await {
                Ok(count) => count,
                Err(error) => {
                    let _ = mark_import_journal_failed(
                        &connection,
                        &journal_id,
                        error.to_string().as_str(),
                    )
                    .await;
                    return Err(error);
                }
            };
        let jobs = enqueue_rebuild_jobs(&connection, &snapshot_fingerprint).await?;
        verify_imported_target(&connection, &snapshot.header).await?;
        connection
            .execute(
                "UPDATE import_journal SET phase='completed', updated_at=?1 WHERE id=?2",
                (now_ms(), journal_id.as_str()),
            )
            .await?;
        Ok(import_report(
            in_path,
            &snapshot.header,
            journal_id,
            imported_records,
            jobs,
            ImportReportDetails {
                phase: "completed",
                restart_required: false,
                staged_database_path: None,
                target_fingerprint_before,
                staged_fingerprint: None,
                publish_preconditions: Vec::new(),
            },
        ))
    }

    /// 在当前 canonical Turso handle 内完成 replace。`BEGIN IMMEDIATE` 会阻止新的
    /// writer 进入，事务失败时由 Turso 默认 rollback，旧 canonical facts 保持可启动。
    async fn replace_import_transaction(
        &self,
        in_path: &Path,
        snapshot: &PortableSnapshot,
        manifest: &str,
        snapshot_fingerprint: &str,
        connection: &mut Connection,
        import_state: (Option<&PortableJournal>, &str),
    ) -> Result<StoreImportReport, StoreError> {
        let (journal, target_fingerprint_before) = import_state;
        let journal_id = journal
            .map(|journal| journal.id.clone())
            .unwrap_or_else(|| deterministic_journal_id(snapshot_fingerprint));

        if let Some(journal) = journal
            && journal.phase == "validated"
        {
            verify_journal_manifest(journal, &snapshot.header)?;
            let staged_path = journal.staged_database_path.clone().ok_or_else(|| {
                StoreError::InvalidInput(
                    "portable replace journal 缺少 staged database path".to_owned(),
                )
            })?;
            let _ = verify_journal_staging(journal, &staged_path, snapshot).await?;
        }

        // 已提交但尚未完成 journal 的恢复只重跑验证和 rebuild enqueue，不再次清空事实。
        if let Some(journal) = journal
            && journal.phase == "published"
        {
            let jobs = enqueue_rebuild_jobs(connection, snapshot_fingerprint).await?;
            verify_imported_target(connection, &snapshot.header).await?;
            let doctor = doctor_connection(connection).await?;
            if !doctor_replace_safe(&doctor) {
                return Err(StoreError::InvalidInput(
                    "portable replace published target doctor 校验未通过".to_owned(),
                ));
            }
            connection
                .execute(
                    "UPDATE import_journal SET phase='completed', error=NULL, updated_at=?1 WHERE id=?2",
                    (now_ms(), journal.id.as_str()),
                )
                .await?;
            return Ok(import_report(
                in_path,
                &snapshot.header,
                journal.id.clone(),
                snapshot.header.record_count,
                jobs,
                ImportReportDetails {
                    phase: "completed",
                    restart_required: false,
                    staged_database_path: None,
                    target_fingerprint_before: target_fingerprint_before.to_owned(),
                    staged_fingerprint: None,
                    publish_preconditions: Vec::new(),
                },
            ));
        }

        if journal.is_none() || journal.is_some_and(|journal| journal.phase == "failed") {
            insert_import_journal(
                connection,
                &journal_id,
                in_path,
                snapshot_fingerprint,
                "prepared",
                manifest,
                (None, Some(target_fingerprint_before)),
            )
            .await?;
        }

        // backup 在事务前完成并验证；路径与 digest 写进 journal，便于失败恢复和审计。
        let backup = match self.create_verified_replace_backup().await {
            Ok(backup) => backup,
            Err(error) => {
                let _ =
                    mark_import_journal_failed(connection, &journal_id, &error.to_string()).await;
                return Err(error);
            }
        };
        update_import_journal_backup(connection, &journal_id, target_fingerprint_before, &backup)
            .await?;

        let transaction_result = async {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .await?;
            let imported_records = replace_records_in_transaction(&transaction, snapshot).await?;
            verify_imported_target(&transaction, &snapshot.header).await?;
            let doctor = doctor_connection(&transaction).await?;
            if !doctor_replace_safe(&doctor) {
                return Err(StoreError::InvalidInput(format!(
                    "portable replace transaction doctor 校验未通过: {doctor:?}"
                )));
            }
            transaction
                .execute(
                    "UPDATE import_journal SET phase='published', error=NULL, updated_at=?1 WHERE id=?2",
                    (now_ms(), journal_id.as_str()),
                )
                .await?;
            transaction.commit().await?;
            Ok::<u64, StoreError>(imported_records)
        }
        .await;

        let imported_records = match transaction_result {
            Ok(imported_records) => imported_records,
            Err(error) => {
                let _ =
                    mark_import_journal_failed(connection, &journal_id, &error.to_string()).await;
                return Err(error);
            }
        };

        // rebuild job 是 derived projection，不参与 facts 清空；在 canonical commit 后入队。
        // enqueue 失败时保留 `published` journal，下一次相同 fingerprint 请求可安全恢复。
        let jobs = enqueue_rebuild_jobs(connection, snapshot_fingerprint).await?;
        verify_imported_target(connection, &snapshot.header).await?;
        let doctor = doctor_connection(connection).await?;
        if !doctor_replace_safe(&doctor) {
            return Err(StoreError::InvalidInput(
                "portable replace committed target doctor 校验未通过".to_owned(),
            ));
        }
        connection
            .execute(
                "UPDATE import_journal SET phase='completed', error=NULL, updated_at=?1 WHERE id=?2",
                (now_ms(), journal_id.as_str()),
            )
            .await?;
        Ok(import_report(
            in_path,
            &snapshot.header,
            journal_id,
            imported_records,
            jobs,
            ImportReportDetails {
                phase: "completed",
                restart_required: false,
                staged_database_path: None,
                target_fingerprint_before: target_fingerprint_before.to_owned(),
                staged_fingerprint: None,
                publish_preconditions: Vec::new(),
            },
        ))
    }

    async fn create_verified_replace_backup(&self) -> Result<VerifiedReplaceBackup, StoreError> {
        let backup_path = temporary_sibling(self.database_path(), "portable-replace-backup")?;
        let connection = self.connection().await?;
        // `VACUUM INTO` 直接从 canonical handle 生成一致快照；不要先做 WAL
        // checkpoint，因为并发 reader 可能让 Turso 返回带 NULL frame 字段，
        // 从而把本可验证的 backup 误判成失败。
        vacuum_into(&connection, &backup_path).await?;
        verify_database_file(&backup_path).await?;
        let (checksum_sha256, bytes) = file_digest(&backup_path)?;
        Ok(VerifiedReplaceBackup {
            path: backup_path,
            checksum_sha256,
            bytes,
        })
    }

    async fn prepare_replacement(
        &self,
        snapshot: &PortableSnapshot,
        in_path: &Path,
        snapshot_fingerprint: &str,
        manifest: &str,
        staging: (&str, &Path),
        target_fingerprint_before: &str,
    ) -> Result<(String, u64), StoreError> {
        let (journal_id, staged_path) = staging;
        let staged = TursoStore::open(staged_path).await?;
        staged.initialize().await?;
        let mut staged_connection = staged.connection().await?;
        insert_import_journal(
            &staged_connection,
            journal_id,
            in_path,
            snapshot_fingerprint,
            "staged",
            manifest,
            (Some(staged_path), Some(target_fingerprint_before)),
        )
        .await?;
        import_records_into_connection(&mut staged_connection, snapshot).await?;
        let jobs = enqueue_rebuild_jobs(&staged_connection, snapshot_fingerprint).await?;
        verify_imported_target(&staged_connection, &snapshot.header).await?;
        drop(staged_connection);
        drop(staged);
        verify_database_file(staged_path).await?;
        let staged_fingerprint = database_file_fingerprint(staged_path)?;
        Ok((staged_fingerprint, jobs))
    }

    pub async fn vacuum(&self) -> Result<StoreVacuumReport, StoreError> {
        let before_bytes = fs::metadata(self.database_path()).map_err(io_error)?.len();
        let lease = self
            .acquire_maintenance_lease("compact", "host-admin")
            .await?;
        let source_fingerprint = self.database_fingerprint().await?;
        let _ = self.checkpoint_inner().await?;
        self.connection().await?.execute("VACUUM", ()).await?;
        let after_bytes = fs::metadata(self.database_path()).map_err(io_error)?.len();
        let report = StoreVacuumReport {
            ok: true,
            before_bytes,
            after_bytes,
            source_fingerprint,
        };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    pub async fn maintenance_status(&self) -> Result<StoreMaintenanceStatus, StoreError> {
        maintenance_status_connection(&self.connection().await?).await
    }

    pub async fn maintenance_run(
        &self,
        owner: &str,
        action: &str,
    ) -> Result<StoreMaintenanceRun, StoreError> {
        let action = action.trim();
        if !matches!(action, "run" | "rebuild" | "cleanup" | "compact") {
            return Err(StoreError::InvalidInput(format!(
                "unsupported maintenance action: {action}"
            )));
        }
        let mode = if matches!(action, "cleanup" | "compact") {
            "compact"
        } else {
            "rebuild"
        };
        let lease = self.acquire_maintenance_lease(mode, owner).await?;
        let result = self.maintenance_run_inner(owner.trim(), action, mode).await;

        if let Err(error) = &result {
            // 跨阶段的硬失败也必须留下可 resume 的 dirty/error 证据；不能因
            // 返回 transport error 而把上一次 active generation 伪装成 ready。
            let _ = self.mark_maintenance_failure(&error.to_string()).await;
        }

        // 维护阶段可能跨多个 projection。无论哪个阶段失败，都先把失败写入
        // projection_state，再释放 owner lease，供下一次 run resume。
        let release_result = self.release_maintenance_lease(&lease).await;
        match (result, release_result) {
            (Ok(report), Ok(())) => Ok(report),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(release_error)) => Err(StoreError::InvalidInput(format!(
                "maintenance failed: {error}; lease release failed: {release_error}"
            ))),
        }
    }

    async fn maintenance_run_inner(
        &self,
        owner: &str,
        action: &str,
        mode: &str,
    ) -> Result<StoreMaintenanceRun, StoreError> {
        let mut errors = Vec::new();
        let mut processed = 0_u64;
        let mut phase = if matches!(action, "cleanup" | "compact") {
            "cleanup"
        } else if action == "rebuild" {
            "rebuild"
        } else {
            "sync"
        }
        .to_owned();

        if matches!(action, "cleanup" | "compact") {
            processed = self.cleanup_rebuildable().await?;
        } else {
            let connection = self.connection().await?;
            let boards = board_ids(&connection).await?;
            drop(connection);

            for board in boards {
                // 每个 capability 独立提交；一个 provider 故障不能回滚其它派生层，
                // 也不能把 vector 的 pending 误报成整体 ready。
                let search = if action == "rebuild" {
                    self.rebuild_search_index(&board).await
                } else {
                    self.sync_search_index(&board).await
                };
                match search {
                    Ok(status) => {
                        processed = processed.saturating_add(
                            self.count_board_documents(&board).await.unwrap_or_default(),
                        );
                        if status.stale || status.fallback_reason.is_some() {
                            let message = status
                                .fallback_reason
                                .unwrap_or_else(|| "fts projection 未 ready".to_owned());
                            errors.push(format!("fts[{board}]: {message}"));
                        }
                    }
                    Err(error) => {
                        let message = error.to_string();
                        errors.push(format!("fts[{board}]: {message}"));
                        let _ = self.mark_projection_error("fts", &message).await;
                    }
                }

                match if action == "rebuild" {
                    self.graph_rebuild(&board).await
                } else {
                    self.graph_sync(&board).await
                } {
                    Ok(graph) => {
                        processed = processed.saturating_add(
                            u64::try_from(
                                graph
                                    .validated_tasks
                                    .saturating_add(graph.validated_entities)
                                    .saturating_add(graph.validated_relations),
                            )
                            .unwrap_or_default(),
                        );
                    }
                    Err(error) => {
                        let message = error.to_string();
                        errors.push(format!("relations[{board}]: {message}"));
                        let _ = self.mark_projection_error("relations", &message).await;
                    }
                }

                match self
                    .enqueue_vector_projection_jobs(&board, action == "rebuild")
                    .await
                {
                    Ok(count) => processed = processed.saturating_add(count),
                    Err(error) => {
                        let message = error.to_string();
                        errors.push(format!("vector[{board}]: {message}"));
                        let _ = self.mark_projection_error("vector_tasks", &message).await;
                        let _ = self
                            .mark_projection_error("vector_label_atoms", &message)
                            .await;
                    }
                }
            }
            // `projection_state` 目前以 projection 为粒度保存 cursor，而 FTS
            // 文档本身按 board 隔离。全量阶段结束后发布全库最大 event cursor，
            // 避免最后一个 board 的 cursor 让其它 board 被误判为 stale。
            if !errors.iter().any(|error| error.starts_with("fts[")) {
                self.refresh_fts_global_cursor().await?;
            }
            phase = "complete".to_owned();
        }

        let connection = self.connection().await?;
        let status = maintenance_status_connection(&connection).await?;
        let stores = status.stores;
        let mut store_errors = stores
            .iter()
            .flat_map(|store| store.errors.iter().cloned())
            .collect::<Vec<_>>();
        errors.append(&mut store_errors);
        errors.sort();
        errors.dedup();
        let degraded = !errors.is_empty()
            || stores.iter().any(|store| {
                store.degraded
                    || store.dirty
                    || store.pending > 0
                    || store.running > 0
                    || store.failed > 0
            });
        if degraded && phase == "complete" {
            phase = "degraded".to_owned();
        }
        Ok(StoreMaintenanceRun {
            database_instance_id: status.database_instance_id,
            protocol_version: status.protocol_version,
            owner: owner.to_owned(),
            mode: mode.to_owned(),
            action: action.to_owned(),
            processed,
            phase,
            degraded,
            errors,
            stores,
        })
    }

    async fn count_board_documents(&self, board_id: &str) -> Result<u64, StoreError> {
        let connection = self.connection().await?;
        let count = scalar_integer_params(
            &connection,
            "SELECT COUNT(*) FROM retrieval_documents WHERE board_id=?1 AND source_kind='task'",
            [board_id],
            "retrieval_documents.count",
        )
        .await?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    async fn mark_projection_error(
        &self,
        projection: &str,
        message: &str,
    ) -> Result<(), StoreError> {
        let connection = self.connection().await?;
        connection
            .execute(
                "UPDATE projection_state SET lifecycle_status='degraded', dirty=1, last_error=?1, updated_at=?2 WHERE projection=?3",
                (message, now_ms(), projection),
            )
            .await?;
        Ok(())
    }

    async fn mark_maintenance_failure(&self, message: &str) -> Result<(), StoreError> {
        let connection = self.connection().await?;
        connection
            .execute(
                "UPDATE projection_state SET lifecycle_status='degraded', dirty=1, last_error=?1, updated_at=?2",
                (message, now_ms()),
            )
            .await?;
        Ok(())
    }

    async fn refresh_fts_global_cursor(&self) -> Result<(), StoreError> {
        let connection = self.connection().await?;
        let cursor = scalar_integer(
            &connection,
            "SELECT COALESCE(MAX(id), 0) FROM task_events",
            "task_events.global_last_event_id",
        )
        .await?;
        let mut rows = connection
            .query(
                "SELECT board_id, id, content_hash FROM retrieval_documents WHERE source_kind='task' ORDER BY board_id, id",
                (),
            )
            .await?;
        let mut digest = Sha256::new();
        while let Some(row) = rows.next().await? {
            digest.update(text_value(
                row.get_value(0)?,
                "retrieval_documents.board_id",
            )?);
            digest.update(b"\0");
            digest.update(text_value(row.get_value(1)?, "retrieval_documents.id")?);
            digest.update(b"\0");
            digest.update(text_value(
                row.get_value(2)?,
                "retrieval_documents.content_hash",
            )?);
            digest.update(b"\n");
        }
        let fingerprint = format!("sha256:{:x}", digest.finalize());
        let generation = format!("fts-{fingerprint}");
        connection
            .execute(
                "UPDATE projection_state SET active_generation=?1, active_fingerprint=?2, corpus_fingerprint=?2, last_event_id=?3, updated_at=?4 WHERE projection='fts' AND lifecycle_status='ready' AND dirty=0",
                (generation.as_str(), fingerprint.as_str(), cursor, now_ms()),
            )
            .await?;
        Ok(())
    }

    /// 只清理派生 job/cache；canonical facts 和事件永远不在此路径删除。
    async fn cleanup_rebuildable(&self) -> Result<u64, StoreError> {
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let cutoff = now_ms().saturating_sub(60 * 60 * 1_000);
        let mut processed = transaction
            .execute(
                "DELETE FROM projection_jobs WHERE status='done' AND updated_at < ?1",
                [cutoff],
            )
            .await? as u64;
        processed = processed.saturating_add(
            transaction
                .execute(
                    "DELETE FROM retrieval_documents WHERE source_kind='task' AND id LIKE 'doc_task_%' AND NOT EXISTS (SELECT 1 FROM tasks WHERE tasks.id = substr(retrieval_documents.id, 10) AND tasks.board_id IS retrieval_documents.board_id)",
                    (),
                )
                .await? as u64,
        );
        processed = processed.saturating_add(
            transaction
                .execute(
                    "DELETE FROM retrieval_documents WHERE source_kind='label_atom' AND NOT EXISTS (SELECT 1 FROM label_atoms WHERE label_atoms.board_id IS retrieval_documents.board_id AND label_atoms.content_hash = retrieval_documents.content_hash)",
                    (),
                )
                .await? as u64,
        );
        transaction.commit().await?;
        Ok(processed)
    }

    async fn acquire_maintenance_lease(
        &self,
        mode: &str,
        owner: &str,
    ) -> Result<MaintenanceLease, StoreError> {
        let owner = owner.trim();
        if owner.is_empty() {
            return Err(StoreError::InvalidInput(
                "maintenance owner 不能为空".to_owned(),
            ));
        }
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let now = now_ms();
        let mut rows = transaction.query("SELECT owner, lease_expires_at, fence_epoch FROM projection_maintenance_owner WHERE singleton=1", ()).await?;
        let (current_owner, expires, epoch) = if let Some(row) = rows.next().await? {
            (
                optional_text(row.get_value(0)?)?,
                optional_integer(row.get_value(1)?)?,
                integer_value(row.get_value(2)?, "maintenance.fence_epoch")?,
            )
        } else {
            (None, None, 0)
        };
        if current_owner.is_some() && expires.unwrap_or(0) > now {
            return Err(StoreError::MaintenanceBusy(format!(
                "maintenance owner {} holds lease until {}",
                current_owner.unwrap_or_default(),
                expires.unwrap_or_default()
            )));
        }
        let token = format!("mt_{}", unique_suffix());
        let expires = now.saturating_add(60_000);
        transaction.execute("INSERT INTO projection_maintenance_owner(singleton, owner, lease_token, mode, lease_expires_at, fence_epoch, capabilities_json, build_identity, started_at, last_heartbeat_at, updated_at) VALUES (1, ?1, ?2, ?3, ?4, ?5, '[]', ?6, ?7, ?7, ?7) ON CONFLICT(singleton) DO UPDATE SET owner=excluded.owner, lease_token=excluded.lease_token, mode=excluded.mode, lease_expires_at=excluded.lease_expires_at, fence_epoch=excluded.fence_epoch, build_identity=excluded.build_identity, started_at=excluded.started_at, last_heartbeat_at=excluded.last_heartbeat_at, updated_at=excluded.updated_at", (owner, token.as_str(), mode, expires, epoch.saturating_add(1), env!("CARGO_PKG_VERSION"), now)).await?;
        transaction.commit().await?;
        Ok(MaintenanceLease { token })
    }

    async fn release_maintenance_lease(&self, lease: &MaintenanceLease) -> Result<(), StoreError> {
        let connection = self.connection().await?;
        connection.execute("UPDATE projection_maintenance_owner SET owner=NULL, lease_token=NULL, mode=NULL, lease_expires_at=NULL, last_heartbeat_at=?1, updated_at=?1 WHERE singleton=1 AND lease_token=?2", (now_ms(), lease.token.as_str())).await?;
        Ok(())
    }

    async fn database_fingerprint(&self) -> Result<String, StoreError> {
        let metadata = fs::metadata(self.database_path()).map_err(io_error)?;
        let connection = self.connection().await?;
        let schema_version = scalar_integer(&connection, "PRAGMA schema_version", "schema_version")
            .await
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "unknown".to_owned());
        Ok(format!(
            "turso:{}:{}:{}",
            metadata.len(),
            metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos())
                .unwrap_or(0),
            schema_version
        ))
    }
}

struct MaintenanceLease {
    token: String,
}

async fn doctor_connection(connection: &Connection) -> Result<StoreDoctorReport, StoreError> {
    let integrity_check =
        scalar_text(connection, "PRAGMA integrity_check", "integrity_check").await?;
    let user_version = scalar_integer(connection, "PRAGMA user_version", "user_version").await?;
    let migration_version = if table_exists(connection, "schema_migrations").await? {
        scalar_optional_integer(
            connection,
            "SELECT MAX(version) FROM schema_migrations",
            "migration_version",
        )
        .await?
    } else {
        None
    };
    let now = now_ms();
    let expired_running_tasks = scalar_integer_params(
        connection,
        "SELECT COUNT(*) FROM tasks WHERE status='running' AND claim_expires_at <= ?1",
        [now],
        "expired_running_tasks",
    )
    .await?;
    let running_tasks_without_active_run = scalar_integer(connection, "SELECT COUNT(*) FROM tasks t WHERE t.status='running' AND (t.current_run_id IS NULL OR NOT EXISTS (SELECT 1 FROM task_runs r WHERE r.id=t.current_run_id AND r.task_id=t.id AND r.status='running' AND r.claim_token=t.claim_token))", "running_tasks_without_active_run").await?;
    let orphan_running_runs = scalar_integer(connection, "SELECT COUNT(*) FROM task_runs r WHERE r.status='running' AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=r.task_id AND t.status='running' AND t.current_run_id=r.id AND t.claim_token=r.claim_token)", "orphan_running_runs").await?;
    let archived_dependency_edges = scalar_integer(connection, "SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id JOIN tasks c ON c.id=d.child_task_id WHERE c.status='archived' AND p.status!='archived'", "archived_dependency_edges").await?;
    let unplanned_active_tasks = scalar_integer(connection, "SELECT COUNT(*) FROM tasks t LEFT JOIN task_execution_plans p ON p.task_id=t.id WHERE t.status NOT IN ('done','archived') AND COALESCE(p.state,'unplanned')='unplanned'", "unplanned_active_tasks").await?;
    let active_parents_with_incomplete_required_steps = scalar_integer(connection, "SELECT COUNT(*) FROM tasks t WHERE t.status NOT IN ('done','archived') AND EXISTS (SELECT 1 FROM task_steps s WHERE s.parent_task_id=t.id AND s.required=1 AND s.status NOT IN ('done','skipped'))", "active_parents_with_incomplete_required_steps").await?;
    let (outbox_pending, outbox_running, outbox_failed) =
        if table_exists(connection, "projection_jobs").await? {
            (
                scalar_integer(
                    connection,
                    "SELECT COUNT(*) FROM projection_jobs WHERE status='pending'",
                    "outbox_pending",
                )
                .await?,
                scalar_integer(
                    connection,
                    "SELECT COUNT(*) FROM projection_jobs WHERE status='running'",
                    "outbox_running",
                )
                .await?,
                scalar_integer(
                    connection,
                    "SELECT COUNT(*) FROM projection_jobs WHERE status='failed'",
                    "outbox_failed",
                )
                .await?,
            )
        } else {
            (0, 0, 0)
        };
    let mut consistency_issues = Vec::new();
    let mut foreign_keys = connection.query("PRAGMA foreign_key_check", ()).await?;
    let mut foreign_key_violations = 0;
    while foreign_keys.next().await?.is_some() {
        foreign_key_violations += 1;
    }
    if foreign_key_violations > 0 {
        consistency_issues.push(StoreDoctorIssue {
            severity: "error".to_owned(),
            code: "foreign_key_violation".to_owned(),
            message: format!("检测到 {foreign_key_violations} 个外键违规"),
            record_ids: Vec::new(),
        });
    }
    let consistency_errors = consistency_issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .count() as i64;
    let derived_stores = if table_exists(connection, "projection_state").await? {
        projection_state_reports(connection).await?
    } else {
        Vec::new()
    };
    let derived_dirty_stores = derived_stores.iter().filter(|store| store.dirty).count() as i64;
    let derived_error_stores = derived_stores
        .iter()
        .filter(|store| store.last_error.is_some() || store.failed > 0)
        .count() as i64;
    let ok = integrity_check.eq_ignore_ascii_case("ok")
        && migration_version == Some(user_version)
        && expired_running_tasks == 0
        && running_tasks_without_active_run == 0
        && orphan_running_runs == 0
        && archived_dependency_edges == 0
        && outbox_failed == 0
        && consistency_errors == 0
        && derived_error_stores == 0;
    let derived_stores = derived_stores
        .into_iter()
        .map(|store| StoreDoctorDerivedStore {
            store_name: store.store_name,
            schema_version: 2,
            last_event_id: store.last_event_id,
            dirty: store.dirty,
            last_error: store.last_error,
            pending_outbox: store.pending,
            running_outbox: store.running,
            failed_outbox: store.failed,
        })
        .collect();
    Ok(StoreDoctorReport {
        ok,
        integrity_check,
        migration_version,
        user_version,
        expired_running_tasks,
        running_tasks_without_active_run,
        orphan_running_runs,
        dependency_cycles: 0,
        archived_dependency_edges,
        missing_run_logs: 0,
        suspicious_run_log_paths: 0,
        executable_dependency_violations: 0,
        executable_spec_violations: 0,
        executable_schedule_violations: 0,
        unplanned_active_tasks,
        active_parents_with_incomplete_required_steps,
        outbox_pending,
        outbox_running,
        outbox_failed,
        derived_dirty_stores,
        derived_error_stores,
        derived_stores,
        consistency_errors,
        consistency_warnings: 0,
        consistency_issues,
        ontology_ledger_errors: 0,
        ontology_ledger_warnings: 0,
        ontology_ledger_issues: Vec::new(),
    })
}

/// doctor 的 `ok` 还包含现有产品的 migration/user_version 和计划提醒；replace 只允许
/// canonical 完整性、引用完整性和 derived error 通过，不能把正常提醒误判为事务失败。
fn doctor_replace_safe(report: &StoreDoctorReport) -> bool {
    report.integrity_check.eq_ignore_ascii_case("ok")
        && report.consistency_errors == 0
        && report.outbox_failed == 0
        && report.derived_error_stores == 0
}

async fn maintenance_status_connection(
    connection: &Connection,
) -> Result<StoreMaintenanceStatus, StoreError> {
    let (owner, mode, lease_expires_at, fence_epoch, build_identity, last_heartbeat_at) =
        if table_exists(connection, "projection_maintenance_owner").await? {
            let mut rows = connection.query("SELECT owner, mode, lease_expires_at, fence_epoch, build_identity, last_heartbeat_at FROM projection_maintenance_owner WHERE singleton=1", ()).await?;
            if let Some(row) = rows.next().await? {
                (
                    optional_text(row.get_value(0)?)?,
                    optional_text(row.get_value(1)?)?,
                    optional_integer(row.get_value(2)?)?,
                    integer_value(row.get_value(3)?, "maintenance.fence_epoch")?,
                    optional_text(row.get_value(4)?)?,
                    optional_integer(row.get_value(5)?)?,
                )
            } else {
                (None, None, None, 0, None, None)
            }
        } else {
            (None, None, None, 0, None, None)
        };
    let stores = if table_exists(connection, "projection_state").await? {
        projection_state_reports(connection).await?
    } else {
        Vec::new()
    };
    let database_instance_id = scalar_text(connection, "SELECT family || ':' || lineage || ':' || fingerprint FROM schema_identity WHERE singleton=1", "database_instance_id").await.unwrap_or_else(|_| "turso:unknown".to_owned());
    Ok(StoreMaintenanceStatus {
        database_instance_id,
        protocol_version: 2,
        owner: StoreMaintenanceOwner {
            active: owner.is_some() && lease_expires_at.unwrap_or(0) > now_ms(),
            owner,
            mode,
            lease_expires_at,
            fence_epoch,
            build_identity,
            last_heartbeat_at,
        },
        stores,
    })
}

async fn board_ids(connection: &Connection) -> Result<Vec<String>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT id FROM boards WHERE archived_at IS NULL ORDER BY id",
            (),
        )
        .await?;
    let mut boards = Vec::new();
    while let Some(row) = rows.next().await? {
        boards.push(text_value(row.get_value(0)?, "boards.id")?);
    }
    Ok(boards)
}

async fn projection_state_reports(
    connection: &Connection,
) -> Result<Vec<StoreProjectionStatus>, StoreError> {
    let mut rows = connection.query("SELECT projection, active_generation, active_fingerprint, previous_generation, building_generation, lifecycle_status, fence_epoch, last_event_id, dirty, last_error, updated_at FROM projection_state ORDER BY projection", ()).await?;
    let mut values = Vec::new();
    while let Some(row) = rows.next().await? {
        let projection = text_value(row.get_value(0)?, "projection_state.projection")?;
        let pending = scalar_integer_params(
            connection,
            "SELECT COUNT(*) FROM projection_jobs WHERE target=?1 AND status='pending'",
            [projection.as_str()],
            "projection_jobs.pending",
        )
        .await
        .unwrap_or(0);
        let running = scalar_integer_params(
            connection,
            "SELECT COUNT(*) FROM projection_jobs WHERE target=?1 AND status='running'",
            [projection.as_str()],
            "projection_jobs.running",
        )
        .await
        .unwrap_or(0);
        let failed = scalar_integer_params(
            connection,
            "SELECT COUNT(*) FROM projection_jobs WHERE target=?1 AND status='failed'",
            [projection.as_str()],
            "projection_jobs.failed",
        )
        .await
        .unwrap_or(0);
        let last_error = optional_text(row.get_value(9)?)?;
        let degraded = matches!(
            text_value(row.get_value(5)?, "projection_state.lifecycle_status")?.as_str(),
            "degraded" | "error"
        ) || failed > 0;
        let phase = if degraded {
            "degraded"
        } else if pending > 0
            || running > 0
            || integer_value(row.get_value(8)?, "projection_state.dirty")? != 0
        {
            "pending"
        } else {
            "ready"
        };
        let errors = last_error.clone().into_iter().collect();
        values.push(StoreProjectionStatus {
            store_name: projection,
            active_generation: optional_text(row.get_value(1)?)?,
            active_fingerprint: optional_text(row.get_value(2)?)?,
            previous_generation: optional_text(row.get_value(3)?)?,
            building_generation: optional_text(row.get_value(4)?)?,
            lifecycle_status: text_value(row.get_value(5)?, "projection_state.lifecycle_status")?,
            fence_epoch: integer_value(row.get_value(6)?, "projection_state.fence_epoch")?,
            last_event_id: integer_value(row.get_value(7)?, "projection_state.last_event_id")?,
            dirty: integer_value(row.get_value(8)?, "projection_state.dirty")? != 0,
            last_error,
            phase: phase.to_owned(),
            degraded,
            errors,
            updated_at: integer_value(row.get_value(10)?, "projection_state.updated_at")?,
            pending,
            running,
            failed,
        });
    }
    Ok(values)
}

#[derive(Debug, Clone)]
struct DeferredPortableValue {
    table: &'static str,
    column: &'static str,
    id: String,
    value: Value,
}

async fn import_records_into_connection(
    connection: &mut Connection,
    snapshot: &PortableSnapshot,
) -> Result<u64, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;
    // 初始化会创建 bootstrap board/columns；它们不是导入事实。先在同一事务删除，
    // 再使用严格 INSERT，避免主键冲突被静默吞掉。
    transaction
        .execute(
            "DELETE FROM boards WHERE id='b_default' AND slug='default'",
            (),
        )
        .await?;
    transaction
        .execute("DELETE FROM relation_predicates", ())
        .await?;
    let imported_records = import_records_into_transaction(&transaction, snapshot).await?;
    transaction.commit().await?;
    Ok(imported_records)
}

/// 在已持有的 canonical transaction 中按 FK 逆序清空事实表，再严格恢复 snapshot。
async fn replace_records_in_transaction(
    transaction: &turso::transaction::Transaction<'_>,
    snapshot: &PortableSnapshot,
) -> Result<u64, StoreError> {
    for table in PORTABLE_REPLACE_DELETE_TABLES {
        transaction
            .execute(format!("DELETE FROM {table}"), ())
            .await?;
    }
    import_records_into_transaction(transaction, snapshot).await
}

async fn import_records_into_transaction(
    transaction: &turso::transaction::Transaction<'_>,
    snapshot: &PortableSnapshot,
) -> Result<u64, StoreError> {
    let mut deferred = Vec::new();
    for record in &snapshot.records {
        insert_portable_record(transaction, record, &mut deferred).await?;
    }
    for value in deferred {
        let sql = format!(
            "UPDATE {} SET \"{}\"=?1 WHERE id=?2",
            value.table, value.column
        );
        let changed = transaction
            .execute(sql, (value.value, value.id.as_str()))
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidInput(format!(
                "portable deferred reference {}.{}={} 未能恢复",
                value.table, value.column, value.id
            )));
        }
    }
    Ok(snapshot.records.len() as u64)
}

async fn insert_portable_record(
    transaction: &turso::transaction::Transaction<'_>,
    record: &PortableLine,
    deferred: &mut Vec<DeferredPortableValue>,
) -> Result<(), StoreError> {
    if record.data.is_empty() {
        return Err(StoreError::InvalidInput(format!(
            "portable {} record 不能为空",
            record.table
        )));
    }
    if !PORTABLE_TABLES.contains(&record.table.as_str()) {
        return Err(StoreError::InvalidInput(format!(
            "portable record type 不受支持: {}",
            record.table
        )));
    }
    let columns = record.data.keys().cloned().collect::<Vec<_>>();
    let quoted = columns
        .iter()
        .map(|column| format!("\"{}\"", column.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({quoted}) VALUES ({placeholders})",
        record.table
    );
    let id = record
        .data
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let mut params = Vec::with_capacity(columns.len());
    for column in &columns {
        let value = record.data.get(column).expect("portable column");
        if let Some((table, deferred_column)) = deferred_column(&record.table, column)
            && !value.is_null()
        {
            let id = id.clone().ok_or_else(|| {
                StoreError::InvalidInput(format!("portable {} record 缺少 id", record.table))
            })?;
            deferred.push(DeferredPortableValue {
                table,
                column: deferred_column,
                id,
                value: json_to_value(value),
            });
            params.push(Value::Null);
        } else {
            params.push(json_to_value(value));
        }
    }
    let changed = transaction.execute(sql, params_from_iter(params)).await?;
    if changed != 1 {
        return Err(StoreError::InvalidInput(format!(
            "portable {} record 未能插入",
            record.table
        )));
    }
    Ok(())
}

fn deferred_column(table: &str, column: &str) -> Option<(&'static str, &'static str)> {
    match (table, column) {
        ("label_ontology_actions", "parent_action_id") => {
            Some(("label_ontology_actions", "parent_action_id"))
        }
        ("label_ontology_signals", "superseded_by_signal_id") => {
            Some(("label_ontology_signals", "superseded_by_signal_id"))
        }
        ("signals", "superseded_by_signal_id") => Some(("signals", "superseded_by_signal_id")),
        _ => None,
    }
}

fn read_portable(path: &Path) -> Result<PortableSnapshot, StoreError> {
    let bytes = fs::read(path).map_err(io_error)?;
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
    let header_bytes = lines
        .next()
        .ok_or_else(|| StoreError::InvalidInput("portable export 为空".to_owned()))?;
    let header_line = std::str::from_utf8(header_bytes)
        .map_err(|error| StoreError::InvalidInput(format!("portable header 不是 UTF-8: {error}")))?
        .trim();
    let header = serde_json::from_str::<PortableHeader>(header_line).map_err(json_error)?;
    let mut records = Vec::new();
    let mut payload = Vec::new();
    let mut table_counts = BTreeMap::new();
    for line_bytes in lines {
        let line = std::str::from_utf8(line_bytes).map_err(|error| {
            StoreError::InvalidInput(format!("portable record 不是 UTF-8: {error}"))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        payload.extend_from_slice(line_bytes);
        let line = line.trim_end_matches(['\n', '\r']);
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str::<PortableLine>(line).map_err(json_error)?;
        *table_counts.entry(record.table.clone()).or_insert(0) += 1;
        records.push(record);
    }
    Ok(PortableSnapshot {
        header,
        records,
        payload_checksum_sha256: digest_bytes(&payload),
        table_counts,
    })
}

async fn portable_schema_identity(
    connection: &Connection,
) -> Result<(String, String, i64, String), StoreError> {
    let mut rows = connection
        .query(
            "SELECT family, lineage, version, fingerprint FROM schema_identity WHERE singleton=1",
            (),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| StoreError::SchemaMismatch("缺少 schema_identity singleton".to_owned()))?;
    let family = text_value(row.get_value(0)?, "schema_identity.family")?;
    let lineage = text_value(row.get_value(1)?, "schema_identity.lineage")?;
    let version = integer_value(row.get_value(2)?, "schema_identity.version")?;
    let fingerprint = text_value(row.get_value(3)?, "schema_identity.fingerprint")?;
    Ok((family, lineage, version, fingerprint))
}

async fn validate_portable_columns(
    connection: &Connection,
    snapshot: &PortableSnapshot,
) -> Result<(), StoreError> {
    let mut expected_columns = BTreeMap::<String, Vec<String>>::new();
    for table in PORTABLE_TABLES {
        let mut rows = connection
            .query(&format!("PRAGMA table_info('{table}')"), ())
            .await?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await? {
            columns.push(text_value(row.get_value(1)?, "portable.table_info.name")?);
        }
        expected_columns.insert((*table).to_owned(), columns);
    }
    for record in &snapshot.records {
        let expected = expected_columns.get(&record.table).ok_or_else(|| {
            StoreError::InvalidInput(format!("未知 portable 表 {}", record.table))
        })?;
        let mut actual = record.data.keys().cloned().collect::<Vec<_>>();
        actual.sort();
        let mut expected = expected.clone();
        expected.sort();
        if actual != expected {
            return Err(StoreError::InvalidInput(format!(
                "portable 表 {} 的列清单不匹配: expected={expected:?}, actual={actual:?}",
                record.table
            )));
        }
    }
    Ok(())
}

async fn collect_portable_records(
    connection: &Connection,
) -> Result<Vec<PortableLine>, StoreError> {
    let mut records = Vec::new();
    for table in PORTABLE_TABLES {
        let mut rows = connection
            .query(format!("SELECT * FROM {table}"), ())
            .await?;
        let columns = rows.column_names();
        while let Some(row) = rows.next().await? {
            let mut data = serde_json::Map::new();
            for (index, column) in columns.iter().enumerate() {
                data.insert(column.clone(), value_to_json(row.get_value(index)?));
            }
            scrub_portable_record(table, &mut data, now_ms());
            records.push(PortableLine {
                table: (*table).to_owned(),
                data,
            });
        }
    }
    Ok(records)
}

/// live claim 和绝对 run-log 路径不属于可移植事实。导出时将其转换为可重放的终态，
/// 与旧 SQLite portable exporter 的语义保持一致，并保留 task/run 主键和历史时间。
fn scrub_portable_record(
    table: &str,
    data: &mut serde_json::Map<String, serde_json::Value>,
    export_now: i64,
) {
    if table == "tasks"
        && data
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status == "running")
    {
        data.insert(
            "status".to_owned(),
            serde_json::Value::String("ready".to_owned()),
        );
        for column in [
            "claim_token",
            "claim_owner",
            "claim_expires_at",
            "last_heartbeat_at",
            "current_run_id",
            "started_at",
        ] {
            data.insert(column.to_owned(), serde_json::Value::Null);
        }
    }
    if table == "task_runs" {
        data.insert("log_path".to_owned(), serde_json::Value::Null);
        if data
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status == "running")
        {
            data.insert(
                "status".to_owned(),
                serde_json::Value::String("canceled".to_owned()),
            );
            data.insert(
                "finished_at".to_owned(),
                serde_json::Value::Number(export_now.into()),
            );
            data.insert(
                "error".to_owned(),
                serde_json::Value::String(
                    "canceled by portable export; claim is not portable".to_owned(),
                ),
            );
        }
    }
}

fn serialize_portable_records(
    records: &[PortableLine],
) -> Result<(Vec<u8>, BTreeMap<String, u64>), StoreError> {
    let mut payload = Vec::new();
    let mut table_counts = BTreeMap::new();
    for record in records {
        serde_json::to_writer(&mut payload, record).map_err(json_error)?;
        payload.push(b'\n');
        *table_counts.entry(record.table.clone()).or_insert(0) += 1;
    }
    Ok((payload, table_counts))
}

fn validate_portable_snapshot(
    header: &PortableHeader,
    snapshot: &PortableSnapshot,
) -> Result<(), StoreError> {
    if header.format != "kanban.portable.jsonl" || header.version != 2 {
        return Err(StoreError::InvalidInput(
            "不支持的 portable export 格式或版本（需要 kanban.portable.jsonl v2）".to_owned(),
        ));
    }
    if header.schema_family != schema::SCHEMA_FAMILY
        || header.schema_lineage != schema::SCHEMA_LINEAGE
        || header.schema_version != schema::FULL_SCHEMA_VERSION
        || header.schema_fingerprint != migration::full_schema_fingerprint()
    {
        return Err(StoreError::SchemaMismatch(format!(
            "portable schema lineage 不匹配: family={}, lineage={}, version={}, fingerprint={}",
            header.schema_family,
            header.schema_lineage,
            header.schema_version,
            header.schema_fingerprint
        )));
    }
    if header.attachments_mode != "metadata_only" {
        return Err(StoreError::InvalidInput(format!(
            "不支持的 portable attachments_mode: {}",
            header.attachments_mode
        )));
    }
    let expected_tables = PORTABLE_TABLES
        .iter()
        .map(|table| (*table).to_owned())
        .collect::<Vec<_>>();
    if header.canonical_tables != expected_tables {
        return Err(StoreError::InvalidInput(
            "portable canonical_tables 与当前 Turso schema 不一致".to_owned(),
        ));
    }
    if header.record_count != snapshot.records.len() as u64 {
        return Err(StoreError::InvalidInput(format!(
            "portable record_count 不匹配: manifest={}, observed={}",
            header.record_count,
            snapshot.records.len()
        )));
    }
    if header.table_counts != snapshot.table_counts {
        return Err(StoreError::InvalidInput(
            "portable table_counts 与实际 JSONL 不一致".to_owned(),
        ));
    }
    if header.payload_checksum_sha256 != snapshot.payload_checksum_sha256 {
        return Err(StoreError::InvalidInput(format!(
            "portable payload checksum 不匹配: manifest={}, observed={}",
            header.payload_checksum_sha256, snapshot.payload_checksum_sha256
        )));
    }
    if header.manifest_checksum_sha256 != manifest_checksum(header)? {
        return Err(StoreError::InvalidInput(
            "portable manifest checksum 不匹配".to_owned(),
        ));
    }
    for record in &snapshot.records {
        if !PORTABLE_TABLES.contains(&record.table.as_str()) {
            return Err(StoreError::InvalidInput(format!(
                "portable record type 不受支持: {}",
                record.table
            )));
        }
    }
    if snapshot
        .table_counts
        .get("boards")
        .copied()
        .unwrap_or_default()
        == 0
    {
        return Err(StoreError::InvalidInput(
            "portable snapshot 至少需要一个 board".to_owned(),
        ));
    }
    let board_ids = snapshot
        .records
        .iter()
        .filter(|record| record.table == "boards")
        .filter_map(|record| record.data.get("id").and_then(serde_json::Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    let column_board_ids = snapshot
        .records
        .iter()
        .filter(|record| record.table == "board_columns")
        .filter_map(|record| {
            record
                .data
                .get("board_id")
                .and_then(serde_json::Value::as_str)
        })
        .collect::<std::collections::BTreeSet<_>>();
    if board_ids
        .iter()
        .any(|board| !column_board_ids.contains(board))
    {
        return Err(StoreError::InvalidInput(
            "portable snapshot 存在没有 board_columns 的 board".to_owned(),
        ));
    }
    Ok(())
}

async fn verify_imported_target(
    connection: &Connection,
    header: &PortableHeader,
) -> Result<(), StoreError> {
    let integrity = scalar_text(connection, "PRAGMA integrity_check", "integrity_check").await?;
    if !integrity.eq_ignore_ascii_case("ok") {
        return Err(StoreError::InvalidInput(format!(
            "portable import integrity_check 未通过: {integrity}"
        )));
    }
    let mut foreign_keys = connection.query("PRAGMA foreign_key_check", ()).await?;
    if foreign_keys.next().await?.is_some() {
        return Err(StoreError::InvalidInput(
            "portable import foreign_key_check 未通过".to_owned(),
        ));
    }
    let (family, lineage, version, fingerprint) = portable_schema_identity(connection).await?;
    if family != header.schema_family
        || lineage != header.schema_lineage
        || version != header.schema_version
        || fingerprint != header.schema_fingerprint
    {
        return Err(StoreError::SchemaMismatch(
            "portable import target schema lineage 不匹配".to_owned(),
        ));
    }
    for table in PORTABLE_TABLES {
        let expected = header.table_counts.get(*table).copied().unwrap_or_default();
        let actual = scalar_integer(
            connection,
            &format!("SELECT COUNT(*) FROM {table}"),
            "portable.import.table_count",
        )
        .await? as u64;
        if actual != expected {
            return Err(StoreError::InvalidInput(format!(
                "portable import 表 {table} 行数不一致: expected={expected}, actual={actual}"
            )));
        }
    }
    let boards_without_columns = scalar_integer(
        connection,
        "SELECT COUNT(*) FROM boards b WHERE NOT EXISTS (SELECT 1 FROM board_columns c WHERE c.board_id=b.id)",
        "portable.import.boards_without_columns",
    )
    .await?;
    if boards_without_columns != 0 {
        return Err(StoreError::InvalidInput(
            "portable import 存在没有 board_columns 的 board".to_owned(),
        ));
    }
    Ok(())
}

async fn verify_staged_database(
    path: &Path,
    header: &PortableHeader,
) -> Result<(String, u64), StoreError> {
    if !fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
    {
        return Err(StoreError::InvalidInput(format!(
            "portable replace staged database 不存在: {}",
            path.display()
        )));
    }
    let database = turso::Builder::new_local(path.to_str().ok_or(StoreError::InvalidPath)?)
        .experimental_index_method(true)
        .experimental_vacuum(true)
        .build()
        .await?;
    let connection = database.connect()?;
    connection.execute("PRAGMA foreign_keys = ON", ()).await?;
    verify_imported_target(&connection, header).await?;
    let jobs = count_rebuild_jobs(&connection, &portable_snapshot_fingerprint(header)).await?;
    drop(connection);
    drop(database);
    Ok((database_file_fingerprint(path)?, jobs))
}

async fn enqueue_rebuild_jobs(
    connection: &Connection,
    snapshot_fingerprint: &str,
) -> Result<u64, StoreError> {
    let now = now_ms();
    let mut boards = connection
        .query("SELECT id FROM boards ORDER BY id", ())
        .await?;
    let mut board_ids = Vec::new();
    while let Some(row) = boards.next().await? {
        board_ids.push(text_value(row.get_value(0)?, "boards.id")?);
    }
    let mut enqueued = 0;
    for board_id in board_ids {
        for target in ["fts", "vector_tasks", "vector_label_atoms", "relations"] {
            let dedupe = format!("portable-rebuild:{snapshot_fingerprint}:{target}:{board_id}");
            let changed = connection
                .execute(
                    "INSERT OR IGNORE INTO projection_jobs(board_id, source_event_id, target, entity_uri, dedupe_key, operation, payload_json, status, attempts, max_attempts, next_attempt_at, created_at, updated_at) VALUES (?1, NULL, ?2, NULL, ?3, 'rebuild', '{}', 'pending', 0, 10, ?4, ?4, ?4)",
                    (board_id.as_str(), target, dedupe.as_str(), now),
                )
                .await?;
            enqueued += changed;
        }
    }
    connection
        .execute(
            "UPDATE projection_state SET dirty=1, lifecycle_status='bootstrap_required', last_error=NULL, updated_at=?1",
            [now],
        )
        .await?;
    Ok(enqueued)
}

async fn count_rebuild_jobs(
    connection: &Connection,
    snapshot_fingerprint: &str,
) -> Result<u64, StoreError> {
    let count = scalar_integer_params(
        connection,
        "SELECT COUNT(*) FROM projection_jobs WHERE operation='rebuild' AND dedupe_key LIKE ?1",
        [format!("portable-rebuild:{snapshot_fingerprint}:%")],
        "portable.import.rebuild_jobs",
    )
    .await?;
    Ok(count.max(0) as u64)
}

fn portable_snapshot_fingerprint(header: &PortableHeader) -> String {
    format!(
        "{}:{}",
        header.source_fingerprint, header.payload_checksum_sha256
    )
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn manifest_checksum(header: &PortableHeader) -> Result<String, StoreError> {
    let mut unsigned = header.clone();
    unsigned.manifest_checksum_sha256.clear();
    let bytes = serde_json::to_vec(&unsigned).map_err(json_error)?;
    Ok(digest_bytes(&bytes))
}

fn database_file_fingerprint(path: &Path) -> Result<String, StoreError> {
    let (checksum, bytes) = file_digest(path)?;
    Ok(format!("{checksum}:{bytes}"))
}

#[derive(Debug, Clone)]
struct VerifiedReplaceBackup {
    path: PathBuf,
    checksum_sha256: String,
    bytes: u64,
}

#[derive(Debug)]
struct PortableJournal {
    id: String,
    phase: String,
    staged_database_path: Option<PathBuf>,
    manifest_json: String,
    previous_identity_json: Option<String>,
}

struct ImportReportDetails<'a> {
    phase: &'a str,
    restart_required: bool,
    staged_database_path: Option<PathBuf>,
    target_fingerprint_before: String,
    staged_fingerprint: Option<String>,
    publish_preconditions: Vec<String>,
}

async fn find_portable_journal(
    connection: &Connection,
    snapshot_fingerprint: &str,
) -> Result<Option<PortableJournal>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT id, phase, staged_database_path, manifest_json, previous_identity_json FROM import_journal WHERE source_kind='jsonl' AND snapshot_fingerprint=?1 ORDER BY updated_at DESC LIMIT 1",
            [snapshot_fingerprint],
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    let id = text_value(row.get_value(0)?, "import_journal.id")?;
    let phase = text_value(row.get_value(1)?, "import_journal.phase")?;
    let staged_database_path = match row.get_value(2)? {
        Value::Null => None,
        Value::Text(value) => Some(PathBuf::from(value)),
        _ => {
            return Err(StoreError::InvalidStoredValue {
                field: "import_journal.staged_database_path",
            });
        }
    };
    let manifest_json = text_value(row.get_value(3)?, "import_journal.manifest_json")?;
    let previous_identity_json = match row.get_value(4)? {
        Value::Null => None,
        Value::Text(value) => Some(value),
        _ => {
            return Err(StoreError::InvalidStoredValue {
                field: "import_journal.previous_identity_json",
            });
        }
    };
    Ok(Some(PortableJournal {
        id,
        phase,
        staged_database_path,
        manifest_json,
        previous_identity_json,
    }))
}

async fn insert_import_journal(
    connection: &Connection,
    journal_id: &str,
    in_path: &Path,
    snapshot_fingerprint: &str,
    phase: &str,
    manifest: &str,
    staging: (Option<&Path>, Option<&str>),
) -> Result<(), StoreError> {
    let (staged_database_path, target_fingerprint_before) = staging;
    let previous_identity = target_fingerprint_before
        .map(|fingerprint| serde_json::json!({"target_fingerprint": fingerprint}).to_string());
    connection
        .execute(
            "INSERT INTO import_journal(id, source_kind, source_path, snapshot_fingerprint, phase, staged_database_path, manifest_json, previous_identity_json, created_at, updated_at) VALUES (?1, 'jsonl', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8) ON CONFLICT(id) DO UPDATE SET source_kind=excluded.source_kind, source_path=excluded.source_path, snapshot_fingerprint=excluded.snapshot_fingerprint, phase=excluded.phase, staged_database_path=excluded.staged_database_path, manifest_json=excluded.manifest_json, previous_identity_json=excluded.previous_identity_json, error=NULL, updated_at=excluded.updated_at",
            (
                journal_id,
                in_path.to_string_lossy().as_ref(),
                snapshot_fingerprint,
                phase,
                staged_database_path.map(|path| path.to_string_lossy().into_owned()),
                manifest,
                previous_identity,
                now_ms(),
            ),
        )
        .await?;
    Ok(())
}

fn deterministic_journal_id(snapshot_fingerprint: &str) -> String {
    format!(
        "ij_{}",
        &snapshot_fingerprint[..snapshot_fingerprint.len().min(32)]
    )
}

fn journal_identity_value(journal: &PortableJournal) -> Result<serde_json::Value, StoreError> {
    let Some(value) = journal.previous_identity_json.as_deref() else {
        return Ok(serde_json::json!({}));
    };
    serde_json::from_str(value).map_err(json_error)
}

fn verify_journal_manifest(
    journal: &PortableJournal,
    header: &PortableHeader,
) -> Result<(), StoreError> {
    let expected = serde_json::to_value(header).map_err(json_error)?;
    let observed =
        serde_json::from_str::<serde_json::Value>(&journal.manifest_json).map_err(json_error)?;
    if observed != expected {
        return Err(StoreError::InvalidInput(
            "portable replace journal manifest 与 source 不一致".to_owned(),
        ));
    }
    Ok(())
}

async fn verify_journal_staging(
    journal: &PortableJournal,
    staged_path: &Path,
    snapshot: &PortableSnapshot,
) -> Result<(String, u64), StoreError> {
    let (staged_fingerprint, jobs) = verify_staged_database(staged_path, &snapshot.header).await?;
    if let Some(expected) = journal_identity_value(journal)?
        .get("staged_fingerprint")
        .and_then(serde_json::Value::as_str)
        && expected != staged_fingerprint
    {
        return Err(StoreError::InvalidInput(
            "portable replace staged fingerprint 与 journal 不一致".to_owned(),
        ));
    }
    Ok((staged_fingerprint, jobs))
}

async fn update_import_journal_backup(
    connection: &Connection,
    journal_id: &str,
    target_fingerprint_before: &str,
    backup: &VerifiedReplaceBackup,
) -> Result<(), StoreError> {
    let mut rows = connection
        .query(
            "SELECT previous_identity_json FROM import_journal WHERE id=?1",
            [journal_id],
        )
        .await?;
    let mut identity = if let Some(row) = rows.next().await? {
        match row.get_value(0)? {
            Value::Null => serde_json::json!({}),
            Value::Text(value) => serde_json::from_str(&value).map_err(json_error)?,
            _ => {
                return Err(StoreError::InvalidStoredValue {
                    field: "import_journal.previous_identity_json",
                });
            }
        }
    } else {
        return Err(StoreError::InvalidInput(format!(
            "portable import journal 不存在: {journal_id}"
        )));
    };
    let object = identity.as_object_mut().ok_or_else(|| {
        StoreError::InvalidInput("portable import journal identity 不是 JSON object".to_owned())
    })?;
    object.insert(
        "target_fingerprint".to_owned(),
        serde_json::Value::String(target_fingerprint_before.to_owned()),
    );
    object.insert(
        "backup_path".to_owned(),
        serde_json::Value::String(backup.path.display().to_string()),
    );
    object.insert(
        "backup_checksum_sha256".to_owned(),
        serde_json::Value::String(backup.checksum_sha256.clone()),
    );
    object.insert(
        "backup_bytes".to_owned(),
        serde_json::Value::Number(backup.bytes.into()),
    );
    let identity = identity.to_string();
    connection
        .execute(
            "UPDATE import_journal SET previous_identity_json=?1, updated_at=?2 WHERE id=?3",
            (identity.as_str(), now_ms(), journal_id),
        )
        .await?;
    Ok(())
}

async fn update_import_journal_validated(
    connection: &Connection,
    journal_id: &str,
    staged_path: &Path,
    target_fingerprint_before: &str,
    staged_fingerprint: &str,
) -> Result<(), StoreError> {
    let previous = format!(
        r#"{{"target_fingerprint":"{}","staged_fingerprint":"{}"}}"#,
        target_fingerprint_before, staged_fingerprint
    );
    connection
        .execute(
            "UPDATE import_journal SET phase='validated', staged_database_path=?1, previous_identity_json=?2, updated_at=?3 WHERE id=?4",
            (
                staged_path.to_string_lossy().as_ref(),
                previous.as_str(),
                now_ms(),
                journal_id,
            ),
        )
        .await?;
    Ok(())
}

async fn mark_import_journal_failed(
    connection: &Connection,
    journal_id: &str,
    error: &str,
) -> Result<(), StoreError> {
    connection
        .execute(
            "UPDATE import_journal SET phase='failed', error=?1, updated_at=?2 WHERE id=?3",
            (error, now_ms(), journal_id),
        )
        .await?;
    Ok(())
}

fn import_report(
    in_path: &Path,
    header: &PortableHeader,
    journal_id: String,
    imported_records: u64,
    rebuild_jobs_enqueued: u64,
    details: ImportReportDetails<'_>,
) -> StoreImportReport {
    let ImportReportDetails {
        phase,
        restart_required,
        staged_database_path,
        target_fingerprint_before,
        staged_fingerprint,
        publish_preconditions,
    } = details;
    StoreImportReport {
        in_path: in_path.display().to_string(),
        source_fingerprint: header.source_fingerprint.clone(),
        imported_records,
        skipped_records: 0,
        rebuild_jobs_enqueued,
        journal_id,
        phase: phase.to_owned(),
        restart_required,
        staged_database_path: staged_database_path.map(|path| path.display().to_string()),
        target_fingerprint_before: Some(target_fingerprint_before),
        staged_fingerprint,
        publish_preconditions,
    }
}

fn prepared_import_report(
    in_path: &Path,
    header: &PortableHeader,
    journal_id: String,
    staged_database_path: PathBuf,
    target_fingerprint_before: String,
    staged_fingerprint: String,
    rebuild_jobs_enqueued: u64,
) -> StoreImportReport {
    import_report(
        in_path,
        header,
        journal_id,
        header.record_count,
        rebuild_jobs_enqueued,
        ImportReportDetails {
            phase: "validated",
            restart_required: true,
            staged_database_path: Some(staged_database_path),
            target_fingerprint_before,
            staged_fingerprint: Some(staged_fingerprint),
            publish_preconditions: vec![
                "停止 kanban serve/dispatcher，获得 host lifecycle 独占".to_owned(),
                "校验 canonical path 与 target_fingerprint_before 一致".to_owned(),
                "同文件系统原子发布 staged_database_path 到 canonical path".to_owned(),
                "重新打开 TursoStore，校验 integrity/schema/counts 并将 journal 标为 completed"
                    .to_owned(),
                "attachments_mode=metadata_only：二进制附件需独立 staging/publish，不在本 JSONL 中静默迁移"
                    .to_owned(),
            ],
        },
    )
}

fn cleanup_staged_database(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
    let _ = fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
}

async fn canonical_record_count(connection: &Connection) -> Result<i64, StoreError> {
    // `initialize()` seeds the default board/columns and relation predicates. They are
    // host bootstrap metadata, not imported facts, so a fresh target must remain importable.
    let mut total = 0;
    for table in PORTABLE_TABLES
        .iter()
        .filter(|table| !matches!(**table, "boards" | "board_columns" | "relation_predicates"))
    {
        total += scalar_integer(
            connection,
            &format!("SELECT COUNT(*) FROM {table}"),
            "canonical_record_count",
        )
        .await
        .unwrap_or(0);
    }
    Ok(total)
}

async fn table_exists(connection: &Connection, table: &str) -> Result<bool, StoreError> {
    Ok(scalar_integer_params(
        connection,
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        "table_exists",
    )
    .await?
        != 0)
}
async fn scalar_text(
    connection: &Connection,
    sql: &str,
    field: &'static str,
) -> Result<String, StoreError> {
    let mut rows = connection.query(sql, ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or(StoreError::InvalidStoredValue { field })?;
    text_value(row.get_value(0)?, field)
}
async fn scalar_integer(
    connection: &Connection,
    sql: &str,
    field: &'static str,
) -> Result<i64, StoreError> {
    scalar_integer_params(connection, sql, Vec::<Value>::new(), field).await
}
async fn scalar_optional_integer(
    connection: &Connection,
    sql: &str,
    field: &'static str,
) -> Result<Option<i64>, StoreError> {
    let mut rows = connection.query(sql, ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or(StoreError::InvalidStoredValue { field })?;
    optional_integer(row.get_value(0)?)
}
async fn scalar_integer_params<T: turso::IntoParams>(
    connection: &Connection,
    sql: &str,
    params: T,
    field: &'static str,
) -> Result<i64, StoreError> {
    let mut rows = connection.query(sql, params).await?;
    let row = rows
        .next()
        .await?
        .ok_or(StoreError::InvalidStoredValue { field })?;
    integer_value(row.get_value(0)?, field)
}
fn integer_value(value: Value, field: &'static str) -> Result<i64, StoreError> {
    match value {
        Value::Integer(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}
fn optional_integer(value: Value) -> Result<Option<i64>, StoreError> {
    match value {
        Value::Null => Ok(None),
        Value::Integer(value) => Ok(Some(value)),
        _ => Err(StoreError::InvalidStoredValue {
            field: "nullable_integer",
        }),
    }
}
fn text_value(value: Value, field: &'static str) -> Result<String, StoreError> {
    match value {
        Value::Text(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}
fn optional_text(value: Value) -> Result<Option<String>, StoreError> {
    match value {
        Value::Null => Ok(None),
        Value::Text(value) => Ok(Some(value)),
        _ => Err(StoreError::InvalidStoredValue {
            field: "nullable_text",
        }),
    }
}

fn value_to_json(value: Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Integer(value) => value.into(),
        Value::Real(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Text(value) => value.into(),
        Value::Blob(value) => serde_json::Value::String(format!("hex:{}", hex_encode(&value))),
    }
}

fn json_to_value(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Integer(i64::from(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(Value::Integer)
            .or_else(|| value.as_f64().map(Value::Real))
            .unwrap_or(Value::Null),
        serde_json::Value::String(value) if value.starts_with("hex:") => {
            Value::Blob(hex_decode(&value[5..]).unwrap_or_default())
        }
        serde_json::Value::String(value) => Value::Text(value.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Value::Text(value.to_string())
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars = value.as_bytes();
    for pair in chars.chunks_exact(2) {
        let high = (pair[0] as char).to_digit(16)? as u8;
        let low = (pair[1] as char).to_digit(16)? as u8;
        bytes.push((high << 4) | low);
    }
    Some(bytes)
}

fn checked_target(path: &Path, kind: &str) -> Result<PathBuf, StoreError> {
    if path.as_os_str().is_empty() {
        return Err(StoreError::InvalidInput(format!("{kind} target path 为空")));
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err(StoreError::InvalidInput(format!(
            "{kind} target already exists: {}",
            path.display()
        )));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    Ok(path.to_path_buf())
}
fn temporary_sibling(path: &Path, kind: &str) -> Result<PathBuf, StoreError> {
    Ok(path.with_file_name(format!(
        ".{}.{}.{:?}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("kanban"),
        kind,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0)
    )))
}
async fn vacuum_into(connection: &Connection, path: &Path) -> Result<(), StoreError> {
    let escaped = path.to_string_lossy().replace('\'', "''");
    connection
        .execute(format!("VACUUM INTO '{escaped}'"), ())
        .await?;
    Ok(())
}
async fn verify_database_file(path: &Path) -> Result<(), StoreError> {
    let path = path.to_str().ok_or(StoreError::InvalidPath)?;
    let database = turso::Builder::new_local(path)
        .experimental_index_method(true)
        .experimental_vacuum(true)
        .build()
        .await?;
    let connection = database.connect()?;
    let value = scalar_text(&connection, "PRAGMA integrity_check", "integrity_check").await?;
    if value != "ok" {
        return Err(StoreError::InvalidInput(format!(
            "backup integrity check failed: {value}"
        )));
    }
    Ok(())
}
fn durable_rename(source: &Path, target: &Path) -> Result<(), StoreError> {
    OpenOptions::new()
        .read(true)
        .open(source)
        .map_err(io_error)?
        .sync_all()
        .map_err(io_error)?;
    fs::rename(source, target).map_err(io_error)?;
    if let Some(parent) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        File::open(parent)
            .map_err(io_error)?
            .sync_all()
            .map_err(io_error)?;
    }
    Ok(())
}
fn file_digest(path: &Path) -> Result<(String, u64), StoreError> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut digest = Sha256::new();
    let mut total = 0;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        total += read as u64;
    }
    Ok((format!("sha256:{:x}", digest.finalize()), total))
}
fn unique_suffix() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0)
    )
}
fn io_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidInput(format!("filesystem operation failed: {error}"))
}
fn json_error(error: serde_json::Error) -> StoreError {
    StoreError::InvalidInput(format!("portable JSONL 无效: {error}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use turso::transaction::TransactionBehavior;

    use super::{integer_value, text_value};
    use crate::test_support::{create_input, store};
    use crate::{StoreError, maintenance::scalar_integer_params, shared::now_ms};

    #[tokio::test]
    async fn maintenance_status_and_run_release_owner_lease() {
        let (_directory, store, _path) = store("maintenance-status").await;
        store.initialize().await.expect("initialize");

        let status = store.maintenance_status().await.expect("status");
        assert!(!status.owner.active);
        let run = store
            .maintenance_run("test-owner", "rebuild")
            .await
            .expect("rebuild");
        assert_eq!(run.owner, "test-owner");
        assert!(
            run.stores
                .iter()
                .filter(|store| store.store_name == "fts" || store.store_name == "relations")
                .all(|store| store.active_generation.is_some())
        );
        assert!(
            run.stores
                .iter()
                .filter(|store| store.store_name.starts_with("vector_"))
                .all(|store| store.degraded)
        );
        let status = store.maintenance_status().await.expect("released status");
        assert!(
            !status.owner.active,
            "successful maintenance must release its lease"
        );

        let checkpoint = store.checkpoint().await.expect("checkpoint");
        assert!(checkpoint.busy >= 0);
        let doctor = store.doctor().await.expect("doctor");
        assert_eq!(doctor.integrity_check, "ok");
    }

    #[tokio::test]
    async fn maintenance_rebuild_executes_search_graph_and_leaves_unavailable_vector_pending() {
        let (_directory, store, _path) = store("maintenance-orchestrator").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_maintenance_orchestrator", None, "Orchestrated task"),
            )
            .await
            .expect("create task");

        let first = store
            .maintenance_run("orchestrator", "rebuild")
            .await
            .expect("maintenance rebuild");
        assert!(first.processed > 0, "maintenance must report real work");
        assert!(first.degraded, "missing vector provider must be degraded");
        assert_eq!(first.phase, "degraded");
        assert!(first.errors.iter().any(|error| error.contains("vector")));

        let connection = store.connection().await.expect("connection");
        let fts_documents = scalar_integer_params(
            &connection,
            "SELECT COUNT(*) FROM retrieval_documents WHERE board_id='b_default' AND source_kind='task'",
            (),
            "fts documents",
        )
        .await
        .expect("fts documents count");
        let task_entities = scalar_integer_params(
            &connection,
            "SELECT COUNT(*) FROM entities WHERE board_id='b_default' AND task_id='t_maintenance_orchestrator'",
            (),
            "task entities",
        )
        .await
        .expect("task entities count");
        let vector_jobs = scalar_integer_params(
            &connection,
            "SELECT COUNT(*) FROM projection_jobs WHERE board_id='b_default' AND target IN ('vector_tasks','vector_label_atoms') AND status='pending'",
            (),
            "vector jobs",
        )
        .await
        .expect("vector jobs count");
        assert_eq!(fts_documents, 1);
        assert!(task_entities > 0);
        assert!(vector_jobs > 0, "vector outage must retain resumable jobs");

        let second = store
            .maintenance_run("orchestrator", "rebuild")
            .await
            .expect("idempotent maintenance rebuild");
        let first_status = first
            .stores
            .iter()
            .find(|store| store.store_name == "fts")
            .expect("fts first status");
        let second_status = second
            .stores
            .iter()
            .find(|store| store.store_name == "fts")
            .expect("fts second status");
        assert_eq!(
            first_status.active_fingerprint, second_status.active_fingerprint,
            "same canonical corpus must publish a stable fingerprint"
        );
    }

    #[tokio::test]
    async fn maintenance_cleanup_does_not_delete_canonical_facts() {
        let (_directory, store, _path) = store("maintenance-cleanup-safe").await;
        store.initialize().await.expect("initialize");
        store
            .create_task("default", create_input("t_cleanup_fact", None, "fact"))
            .await
            .expect("create fact");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO projection_jobs(board_id, target, entity_uri, operation, payload_json, status, created_at, updated_at) VALUES ('b_default', 'fts', 'kb://task/t_cleanup_fact', 'upsert', '{}', 'done', 1, 1)",
                (),
            )
            .await
            .expect("old derived job");
        let before = scalar_integer_params(
            &connection,
            "SELECT COUNT(*) FROM tasks WHERE id='t_cleanup_fact'",
            (),
            "canonical task",
        )
        .await
        .expect("task count");
        drop(connection);

        let report = store
            .maintenance_run("cleanup-owner", "cleanup")
            .await
            .expect("cleanup");
        assert!(
            report.processed > 0,
            "cleanup must report deleted derived rows"
        );
        let connection = store.connection().await.expect("connection");
        let after = scalar_integer_params(
            &connection,
            "SELECT COUNT(*) FROM tasks WHERE id='t_cleanup_fact'",
            (),
            "canonical task",
        )
        .await
        .expect("task count");
        assert_eq!(before, after, "cleanup must preserve canonical facts");
    }

    #[tokio::test]
    async fn maintenance_lease_competition_is_fail_closed() {
        let (_directory, store, _path) = store("maintenance-lease-competition").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE projection_maintenance_owner SET owner='other-owner', lease_token='held', mode='rebuild', lease_expires_at=?1 WHERE singleton=1",
                [now_ms().saturating_add(60_000)],
            )
            .await
            .expect("hold lease");
        let error = store
            .maintenance_run("losing-owner", "run")
            .await
            .expect_err("active owner must win");
        assert!(matches!(error, StoreError::MaintenanceBusy(_)));
    }

    #[tokio::test]
    async fn verified_backup_and_portable_export_import_roundtrip() {
        let (source_directory, source, _source_path) = store("maintenance-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_maintenance", None, "Maintenance fixture"),
            )
            .await
            .expect("fixture task");

        let backup_path = source_directory.path().join("verified.db");
        let backup = source.backup(&backup_path).await.expect("verified backup");
        assert!(backup_path.is_file());
        assert!(backup.bytes > 0);
        assert!(backup.checksum_sha256.starts_with("sha256:"));

        let export_path = source_directory.path().join("portable.jsonl");
        let export = source.export(&export_path).await.expect("portable export");
        assert!(export.record_count > 0);
        assert!(export_path.is_file());

        let (_target_directory, target, _target_path) = store("maintenance-target").await;
        target.initialize().await.expect("initialize target");
        let import = target
            .import(&export_path, false)
            .await
            .expect("portable import");
        assert!(import.imported_records > 0);
        assert_eq!(import.phase, "completed");
        assert!(!import.restart_required);
        assert!(import.rebuild_jobs_enqueued > 0);
        assert!(
            !target
                .maintenance_status()
                .await
                .expect("target status")
                .owner
                .active
        );
        let tasks = target
            .list_tasks("default", crate::StoreTaskListOptions::default())
            .await
            .expect("imported tasks");
        assert!(tasks.tasks.iter().any(|task| task.id == "t_maintenance"));

        let repeated = target
            .import(&export_path, false)
            .await
            .expect("repeated portable import is idempotent");
        assert_eq!(repeated.journal_id, import.journal_id);
        assert_eq!(repeated.phase, "completed");
        assert_eq!(
            target
                .list_tasks("default", crate::StoreTaskListOptions::default())
                .await
                .expect("repeated tasks")
                .tasks
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn portable_checksum_failure_leaves_empty_target_bootstrap_intact() {
        let (source_directory, source, _source_path) = store("maintenance-checksum-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_checksum", None, "Checksum fixture"),
            )
            .await
            .expect("fixture task");
        let export_path = source_directory.path().join("portable.jsonl");
        source.export(&export_path).await.expect("portable export");
        let tampered_path = source_directory.path().join("tampered.jsonl");
        let tampered = fs::read_to_string(&export_path)
            .expect("export text")
            .replace("Checksum fixture", "Tampered fixture");
        fs::write(&tampered_path, tampered).expect("tampered export");

        let (_target_directory, target, _target_path) = store("maintenance-checksum-target").await;
        target.initialize().await.expect("initialize target");
        let error = target
            .import(&tampered_path, false)
            .await
            .expect_err("tampered payload must be rejected");
        assert!(error.to_string().contains("checksum"));
        let tasks = target
            .list_tasks("default", crate::StoreTaskListOptions::default())
            .await
            .expect("target tasks");
        assert!(tasks.tasks.is_empty());
    }

    #[tokio::test]
    async fn portable_import_preserves_explicit_ids_times_and_relations() {
        let (source_directory, source, _source_path) = store("maintenance-facts-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task("default", create_input("t_parent", None, "Parent fixture"))
            .await
            .expect("parent task");
        source
            .create_task("default", create_input("t_child", None, "Child fixture"))
            .await
            .expect("child task");
        let source_connection = source.connection().await.expect("source connection");
        source_connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', 't_parent', 't_child', 424242)",
                (),
            )
            .await
            .expect("dependency");
        let export_path = source_directory.path().join("facts.jsonl");
        source.export(&export_path).await.expect("portable export");

        let (_target_directory, target, _target_path) = store("maintenance-facts-target").await;
        target.initialize().await.expect("initialize target");
        target
            .import(&export_path, false)
            .await
            .expect("portable import");
        let target_connection = target.connection().await.expect("target connection");
        let mut rows = target_connection
            .query(
                "SELECT parent_task_id, child_task_id, created_at FROM task_dependencies",
                (),
            )
            .await
            .expect("dependency query");
        let row = rows
            .next()
            .await
            .expect("dependency row result")
            .expect("dependency row");
        assert_eq!(
            text_value(row.get_value(0).expect("parent"), "parent").expect("parent text"),
            "t_parent"
        );
        assert_eq!(
            text_value(row.get_value(1).expect("child"), "child").expect("child text"),
            "t_child"
        );
        assert_eq!(
            integer_value(row.get_value(2).expect("created_at"), "created_at")
                .expect("created_at integer"),
            424242
        );
    }

    #[tokio::test]
    async fn replace_consumes_validated_staging_and_commits_canonical_transaction() {
        let (source_directory, source, _source_path) = store("maintenance-replace-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_incoming", None, "Incoming fixture"),
            )
            .await
            .expect("incoming task");
        let export_path = source_directory.path().join("portable.jsonl");
        source.export(&export_path).await.expect("portable export");

        let (_target_directory, target, target_path) = store("maintenance-replace-target").await;
        target.initialize().await.expect("initialize target");
        target
            .create_task(
                "default",
                create_input("t_existing", None, "Existing fixture"),
            )
            .await
            .expect("existing task");
        let prepared = target
            .prepare_import(&export_path)
            .await
            .expect("prepare replace staging");
        assert_eq!(prepared.phase, "validated");
        assert!(prepared.restart_required);
        let tasks = target
            .list_tasks("default", crate::StoreTaskListOptions::default())
            .await
            .expect("target tasks");
        assert!(tasks.tasks.iter().any(|task| task.id == "t_existing"));
        assert!(!tasks.tasks.iter().any(|task| task.id == "t_incoming"));

        let connection = target.connection().await.expect("journal connection");
        let mut rows = connection
            .query(
                "SELECT id, phase, staged_database_path FROM import_journal ORDER BY updated_at DESC LIMIT 1",
                (),
            )
            .await
            .expect("journal query");
        let row = rows
            .next()
            .await
            .expect("journal row result")
            .expect("journal row");
        let journal_id =
            text_value(row.get_value(0).expect("journal id value"), "id").expect("journal id");
        assert_eq!(
            text_value(row.get_value(1).expect("phase value"), "phase").expect("phase"),
            "validated"
        );
        let staged = text_value(
            row.get_value(2).expect("staged path value"),
            "staged_database_path",
        )
        .expect("staged path");
        assert!(Path::new(&staged).is_file(), "staged path {staged}");
        assert!(target_path.is_file());
        drop(rows);
        drop(row);

        let resumed = target
            .import(&export_path, true)
            .await
            .expect("validated portable replace commits in host transaction");
        assert_eq!(resumed.phase, "completed");
        assert!(!resumed.restart_required);
        assert_eq!(resumed.journal_id, journal_id);
        let tasks = target
            .list_tasks("default", crate::StoreTaskListOptions::default())
            .await
            .expect("replaced tasks");
        assert!(tasks.tasks.iter().any(|task| task.id == "t_incoming"));
        assert!(!tasks.tasks.iter().any(|task| task.id == "t_existing"));
        assert!(target_path.is_file());

        let mut rows = connection
            .query(
                "SELECT phase, previous_identity_json FROM import_journal WHERE id=?1",
                [journal_id.as_str()],
            )
            .await
            .expect("completed journal query");
        let row = rows
            .next()
            .await
            .expect("completed journal result")
            .expect("completed journal row");
        assert_eq!(
            text_value(row.get_value(0).expect("completed phase"), "phase")
                .expect("completed phase text"),
            "completed"
        );
        let identity = text_value(
            row.get_value(1).expect("backup identity"),
            "previous_identity_json",
        )
        .expect("backup identity text");
        let identity = serde_json::from_str::<serde_json::Value>(&identity).expect("identity json");
        let backup_path = identity
            .get("backup_path")
            .and_then(serde_json::Value::as_str)
            .expect("verified backup path");
        assert!(
            Path::new(backup_path).is_file(),
            "backup path {backup_path}"
        );
        assert_eq!(
            super::scalar_integer(
                &connection,
                "SELECT COUNT(*) FROM schema_identity WHERE singleton=1",
                "schema identity",
            )
            .await
            .expect("schema identity count"),
            1
        );
        assert!(
            super::scalar_integer(
                &connection,
                "SELECT COUNT(*) FROM schema_migrations",
                "schema migrations",
            )
            .await
            .expect("schema migration count")
                > 0
        );
        assert!(
            !target
                .maintenance_status()
                .await
                .expect("maintenance status")
                .owner
                .active
        );
    }

    #[tokio::test]
    async fn replace_on_logically_empty_target_imports_without_inode_publish() {
        let (source_directory, source, _source_path) =
            store("maintenance-replace-empty-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_replace_empty", None, "Replace empty fixture"),
            )
            .await
            .expect("fixture task");
        let export_path = source_directory.path().join("portable.jsonl");
        source.export(&export_path).await.expect("portable export");

        let (_target_directory, target, _target_path) =
            store("maintenance-replace-empty-target").await;
        target.initialize().await.expect("initialize target");
        let report = target
            .import(&export_path, true)
            .await
            .expect("empty target replace");
        assert_eq!(report.phase, "completed");
        assert!(!report.restart_required);
        assert!(
            target
                .list_tasks("default", crate::StoreTaskListOptions::default())
                .await
                .expect("imported tasks")
                .tasks
                .iter()
                .any(|task| task.id == "t_replace_empty")
        );
    }

    #[tokio::test]
    async fn replace_failure_rolls_back_old_facts_and_records_failed_journal() {
        let (source_directory, source, _source_path) =
            store("maintenance-replace-failure-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_incoming_invalid", None, "Invalid incoming fixture"),
            )
            .await
            .expect("incoming task");
        let export_path = source_directory.path().join("portable.jsonl");
        source.export(&export_path).await.expect("portable export");
        let mut snapshot = super::read_portable(&export_path).expect("read portable");
        snapshot
            .records
            .iter_mut()
            .find(|record| record.table == "tasks")
            .expect("task record")
            .data
            .insert(
                "board_id".to_owned(),
                serde_json::Value::String("b_missing".to_owned()),
            );
        let (payload, table_counts) = super::serialize_portable_records(&snapshot.records)
            .expect("serialize invalid payload");
        snapshot.header.payload_checksum_sha256 = super::digest_bytes(&payload);
        snapshot.header.table_counts = table_counts;
        snapshot.header.record_count = snapshot.records.len() as u64;
        snapshot.header.manifest_checksum_sha256 =
            super::manifest_checksum(&snapshot.header).expect("manifest checksum");
        let invalid_path = source_directory.path().join("portable-invalid.jsonl");
        let mut bytes = serde_json::to_vec(&snapshot.header).expect("header json");
        bytes.push(b'\n');
        bytes.extend_from_slice(&payload);
        fs::write(&invalid_path, bytes).expect("invalid export");

        let (_target_directory, target, _target_path) =
            store("maintenance-replace-failure-target").await;
        target.initialize().await.expect("initialize target");
        target
            .create_task(
                "default",
                create_input("t_existing_safe", None, "Existing safe fixture"),
            )
            .await
            .expect("existing task");
        let error = target
            .import(&invalid_path, true)
            .await
            .expect_err("foreign key failure must rollback replace");
        assert!(error.to_string().contains("turso") || error.to_string().contains("foreign"));
        let tasks = target
            .list_tasks("default", crate::StoreTaskListOptions::default())
            .await
            .expect("target tasks after rollback");
        assert!(tasks.tasks.iter().any(|task| task.id == "t_existing_safe"));
        assert!(
            !tasks
                .tasks
                .iter()
                .any(|task| task.id == "t_incoming_invalid")
        );
        let connection = target.connection().await.expect("journal connection");
        let mut rows = connection
            .query(
                "SELECT phase FROM import_journal WHERE source_kind='jsonl' ORDER BY updated_at DESC LIMIT 1",
                (),
            )
            .await
            .expect("failed journal query");
        let row = rows
            .next()
            .await
            .expect("failed journal result")
            .expect("failed journal row");
        assert_eq!(
            text_value(row.get_value(0).expect("failed phase"), "phase")
                .expect("failed phase text"),
            "failed"
        );
    }

    #[tokio::test]
    async fn replace_preserves_attachment_metadata_and_is_idempotent() {
        let (source_directory, source, _source_path) =
            store("maintenance-replace-attachment-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_attachment", None, "Attachment fixture"),
            )
            .await
            .expect("attachment task");
        source
            .connection()
            .await
            .expect("source connection")
            .execute(
                "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, content_type, size_bytes, sha256, created_by, created_at) VALUES ('a_fixture', 'b_default', 't_attachment', 'report.txt', 't_attachment/report.txt', 'text/plain', 12, 'sha256:fixture', 'tester', 424242)",
                (),
            )
            .await
            .expect("attachment metadata");
        let export_path = source_directory.path().join("portable.jsonl");
        source.export(&export_path).await.expect("portable export");

        let (_target_directory, target, _target_path) =
            store("maintenance-replace-attachment-target").await;
        target.initialize().await.expect("initialize target");
        let first = target
            .import(&export_path, true)
            .await
            .expect("replace import");
        assert_eq!(first.phase, "completed");
        let connection = target.connection().await.expect("target connection");
        let mut rows = connection
            .query(
                "SELECT filename, rel_path, content_type, size_bytes, sha256, created_at FROM task_attachments WHERE id='a_fixture'",
                (),
            )
            .await
            .expect("attachment query");
        let row = rows
            .next()
            .await
            .expect("attachment result")
            .expect("attachment row");
        assert_eq!(
            text_value(row.get_value(0).expect("filename"), "filename").expect("filename text"),
            "report.txt"
        );
        assert_eq!(
            text_value(row.get_value(1).expect("rel path"), "rel_path").expect("rel path text"),
            "t_attachment/report.txt"
        );
        assert_eq!(
            text_value(row.get_value(2).expect("content type"), "content_type")
                .expect("content type text"),
            "text/plain"
        );
        assert_eq!(
            integer_value(row.get_value(3).expect("size"), "size").expect("size integer"),
            12
        );
        assert_eq!(
            text_value(row.get_value(4).expect("sha"), "sha").expect("sha text"),
            "sha256:fixture"
        );
        assert_eq!(
            integer_value(row.get_value(5).expect("created"), "created").expect("created integer"),
            424242
        );
        let repeated = target
            .import(&export_path, true)
            .await
            .expect("idempotent replace");
        assert_eq!(repeated.phase, "completed");
        assert_eq!(repeated.journal_id, first.journal_id);
        let count = super::scalar_integer(
            &connection,
            "SELECT COUNT(*) FROM task_attachments",
            "attachments",
        )
        .await
        .expect("attachment count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn replace_respects_existing_writer_lock_without_mutating_old_facts() {
        let (source_directory, source, _source_path) =
            store("maintenance-replace-lock-source").await;
        source.initialize().await.expect("initialize source");
        source
            .create_task(
                "default",
                create_input("t_lock_incoming", None, "Lock incoming"),
            )
            .await
            .expect("incoming task");
        let export_path = source_directory.path().join("portable.jsonl");
        source.export(&export_path).await.expect("portable export");
        let (_target_directory, target, _target_path) =
            store("maintenance-replace-lock-target").await;
        target.initialize().await.expect("initialize target");
        target
            .create_task(
                "default",
                create_input("t_lock_existing", None, "Lock existing"),
            )
            .await
            .expect("existing task");
        let mut connection = target.connection().await.expect("lock connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .expect("writer lock");
        let error = target
            .import(&export_path, true)
            .await
            .expect_err("existing writer must block replace");
        drop(transaction);
        assert!(error.to_string().contains("turso") || error.to_string().contains("busy"));
        let tasks = target
            .list_tasks("default", crate::StoreTaskListOptions::default())
            .await
            .expect("tasks after writer lock");
        assert!(tasks.tasks.iter().any(|task| task.id == "t_lock_existing"));
        assert!(!tasks.tasks.iter().any(|task| task.id == "t_lock_incoming"));
    }
}
