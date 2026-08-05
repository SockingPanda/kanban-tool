//! Host-owned doctor/checkpoint/backup/import/compaction primitives.
//!
//! 所有方法只在 `kanban-server` 的 canonical Turso owner 内调用。portable JSONL
//! 只包含 canonical facts；projection、FTS、vector 和 graph 表属于可重建派生物。

use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use turso::{Connection, Value, params_from_iter, transaction::TransactionBehavior};

use crate::{TursoStore, error::StoreError, shared::now_ms};

/// 外部快照中的 canonical facts。派生 projection 不得被导出为事实。
pub(crate) const PORTABLE_TABLES: &[&str] = &[
    "boards", "board_columns", "tasks", "task_execution_plans", "task_steps",
    "task_dependencies", "task_runs", "task_comments", "task_events",
    "task_attachments", "labels", "task_labels", "app_settings", "task_subtasks",
    "entities", "relation_predicates", "entity_relations", "label_semantics", "label_atoms",
    "label_atom_index_boards", "label_semantic_proposals", "label_ontology_observations",
    "label_ontology_signals", "label_ontology_actions", "label_ontology_action_signals",
    "label_ontology_action_atom_effects", "signal_observations", "signals",
];

/// 诊断所依赖的 schema/identity 表清单；它们不属于 portable business facts。
#[allow(dead_code)]
const SCHEMA_TABLES: &[&str] = &[
    "boards", "board_columns", "tasks", "task_execution_plans", "task_steps",
    "task_dependencies", "task_runs", "task_comments", "task_events", "schema_migrations",
    "schema_identity", "schema_capabilities",
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
pub struct StoreCheckpointReport { pub busy: i64, pub log_frames: i64, pub checkpointed_frames: i64 }

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
    pub stores: Vec<StoreProjectionStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PortableHeader {
    format: String,
    version: u32,
    source_fingerprint: String,
    canonical_tables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PortableLine {
    #[serde(rename = "type")]
    table: String,
    data: serde_json::Map<String, serde_json::Value>,
}

impl TursoStore {
    pub async fn doctor(&self) -> Result<StoreDoctorReport, StoreError> {
        doctor_connection(&self.connection().await?).await
    }

    pub async fn checkpoint(&self) -> Result<StoreCheckpointReport, StoreError> {
        let lease = self.acquire_maintenance_lease("backup", "host-admin").await?;
        let report = self.checkpoint_inner().await?;
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    async fn checkpoint_inner(&self) -> Result<StoreCheckpointReport, StoreError> {
        let connection = self.connection().await?;
        let mut rows = connection.query("PRAGMA wal_checkpoint(TRUNCATE)", ()).await?;
        let row = rows.next().await?.ok_or_else(|| StoreError::InvalidInput("checkpoint 没有返回结果".to_owned()))?;
        Ok(StoreCheckpointReport {
            busy: integer_value(row.get_value(0)?, "checkpoint.busy")?,
            log_frames: integer_value(row.get_value(1)?, "checkpoint.log_frames")?,
            checkpointed_frames: integer_value(row.get_value(2)?, "checkpoint.checkpointed_frames")?,
        })
    }

    pub async fn backup(&self, out_path: impl AsRef<Path>) -> Result<StoreBackupReport, StoreError> {
        let out_path = checked_target(out_path.as_ref(), "backup")?;
        let lease = self.acquire_maintenance_lease("backup", "host-admin").await?;
        let _ = self.checkpoint_inner().await?;
        let source_fingerprint = self.database_fingerprint().await?;
        let temp = temporary_sibling(&out_path, "backup")?;
        vacuum_into(&self.connection().await?, &temp).await?;
        verify_database_file(&temp).await?;
        durable_rename(&temp, &out_path)?;
        let (checksum_sha256, bytes) = file_digest(&out_path)?;
        let report = StoreBackupReport { out_path: out_path.display().to_string(), checksum_sha256, bytes, source_fingerprint };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    pub async fn export(&self, out_path: impl AsRef<Path>) -> Result<StoreExportReport, StoreError> {
        let out_path = checked_target(out_path.as_ref(), "export")?;
        let lease = self.acquire_maintenance_lease("backup", "host-admin").await?;
        let source_fingerprint = self.database_fingerprint().await?;
        let temp = temporary_sibling(&out_path, "export")?;
        let mut writer = BufWriter::new(File::create(&temp).map_err(io_error)?);
        let header = PortableHeader { format: "kanban.portable.jsonl".to_owned(), version: 1, source_fingerprint: source_fingerprint.clone(), canonical_tables: PORTABLE_TABLES.iter().map(|table| (*table).to_owned()).collect() };
        serde_json::to_writer(&mut writer, &header).map_err(json_error)?;
        writer.write_all(b"\n").map_err(io_error)?;
        let connection = self.connection().await?;
        let mut record_count = 0;
        for table in PORTABLE_TABLES {
            let mut rows = connection.query(format!("SELECT * FROM {table}"), ()).await?;
            let columns = rows.column_names();
            while let Some(row) = rows.next().await? {
                let mut data = serde_json::Map::new();
                for (index, column) in columns.iter().enumerate() { data.insert(column.clone(), value_to_json(row.get_value(index)?)); }
                serde_json::to_writer(&mut writer, &PortableLine { table: (*table).to_owned(), data }).map_err(json_error)?;
                writer.write_all(b"\n").map_err(io_error)?;
                record_count += 1;
            }
        }
        writer.flush().map_err(io_error)?;
        writer.into_inner().map_err(|error| io_error(error.into_error()))?.sync_all().map_err(io_error)?;
        durable_rename(&temp, &out_path)?;
        let (checksum_sha256, bytes) = file_digest(&out_path)?;
        let report = StoreExportReport { out_path: out_path.display().to_string(), checksum_sha256, bytes, record_count, source_fingerprint };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    /// 当前 host 已持有 Turso handle，replace 不直接替换旧 inode。非 replace 导入
    /// 只允许空 canonical target，并通过 `import_journal` 记录可恢复阶段。
    pub async fn import(&self, in_path: impl AsRef<Path>, replace: bool) -> Result<StoreImportReport, StoreError> {
        let in_path = in_path.as_ref();
        if !fs::symlink_metadata(in_path).map(|metadata| metadata.file_type().is_file()).unwrap_or(false) { return Err(StoreError::InvalidInput(format!("portable import source 不是普通文件: {}", in_path.display()))); }
        let lease = self.acquire_maintenance_lease("import", "host-admin").await?;
        let (header, records) = match read_portable(in_path) {
            Ok(value) => value,
            Err(error) => { let _ = self.release_maintenance_lease(&lease).await; return Err(error); }
        };
        if header.format != "kanban.portable.jsonl" || header.version != 1 {
            let _ = self.release_maintenance_lease(&lease).await;
            return Err(StoreError::InvalidInput("不支持的 portable export 格式".to_owned()));
        }
        let mut connection = self.connection().await?;
        let existing = canonical_record_count(&connection).await?;
        if existing > 1 || (existing > 0 && !replace) {
            let _ = self.release_maintenance_lease(&lease).await;
            return Err(StoreError::InvalidInput("import target 非空；replace 只能在已验证备份和停 host 后执行".to_owned()));
        }
        let journal_id = format!("ij_{}", unique_suffix());
        let manifest = serde_json::to_string(&header).map_err(json_error)?;
        connection.execute("INSERT INTO import_journal(id, source_kind, source_path, snapshot_fingerprint, phase, manifest_json, created_at, updated_at) VALUES (?1, 'jsonl', ?2, ?3, 'prepared', ?4, ?5, ?5)", (journal_id.as_str(), in_path.to_string_lossy().as_ref(), header.source_fingerprint.as_str(), manifest.as_str(), now_ms())).await?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).await?;
        let mut imported_records = 0;
        let mut skipped_records = 0;
        for record in records {
            if !PORTABLE_TABLES.contains(&record.table.as_str()) { skipped_records += 1; continue; }
            if let Err(error) = insert_portable_record(&transaction, &record).await {
                drop(transaction);
                let _ = connection.execute("UPDATE import_journal SET phase='failed', error=?1, updated_at=?2 WHERE id=?3", (error.to_string().as_str(), now_ms(), journal_id.as_str())).await;
                let _ = self.release_maintenance_lease(&lease).await;
                return Err(error);
            }
            imported_records += 1;
        }
        if let Err(error) = transaction.commit().await {
            let error: StoreError = error.into();
            let _ = connection.execute("UPDATE import_journal SET phase='failed', error=?1, updated_at=?2 WHERE id=?3", (error.to_string().as_str(), now_ms(), journal_id.as_str())).await;
            let _ = self.release_maintenance_lease(&lease).await;
            return Err(error);
        }
        connection.execute("UPDATE import_journal SET phase='completed', updated_at=?1 WHERE id=?2", (now_ms(), journal_id.as_str())).await?;
        let report = StoreImportReport { in_path: in_path.display().to_string(), source_fingerprint: header.source_fingerprint, imported_records, skipped_records, rebuild_jobs_enqueued: 0, journal_id };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    pub async fn vacuum(&self) -> Result<StoreVacuumReport, StoreError> {
        let before_bytes = fs::metadata(self.database_path()).map_err(io_error)?.len();
        let lease = self.acquire_maintenance_lease("compact", "host-admin").await?;
        let source_fingerprint = self.database_fingerprint().await?;
        let _ = self.checkpoint_inner().await?;
        self.connection().await?.execute("VACUUM", ()).await?;
        let after_bytes = fs::metadata(self.database_path()).map_err(io_error)?.len();
        let report = StoreVacuumReport { ok: true, before_bytes, after_bytes, source_fingerprint };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    pub async fn maintenance_status(&self) -> Result<StoreMaintenanceStatus, StoreError> {
        maintenance_status_connection(&self.connection().await?).await
    }

    pub async fn maintenance_run(&self, owner: &str, action: &str) -> Result<StoreMaintenanceRun, StoreError> {
        let mode = if action == "compact" { "compact" } else { "rebuild" };
        let lease = self.acquire_maintenance_lease(mode, owner).await?;
        let connection = self.connection().await?;
        let now = now_ms();
        if table_exists(&connection, "projection_state").await? {
            let generation = format!("gen_{}", unique_suffix());
            let fingerprint = self.database_fingerprint().await?;
            connection.execute("UPDATE projection_state SET previous_generation=active_generation, previous_fingerprint=active_fingerprint, active_generation=?1, active_fingerprint=?2, building_generation=NULL, building_fingerprint=NULL, lifecycle_status='ready', dirty=0, last_success_at=?3, last_error=NULL, updated_at=?3, fence_epoch=fence_epoch+1", (generation.as_str(), fingerprint.as_str(), now)).await?;
        }
        let status = maintenance_status_connection(&connection).await?;
        let processed = status.stores.iter().map(|store| store.pending.max(0) as u64).sum();
        let report = StoreMaintenanceRun { database_instance_id: status.database_instance_id, protocol_version: status.protocol_version, owner: owner.to_owned(), mode: mode.to_owned(), action: action.to_owned(), processed, stores: status.stores };
        self.release_maintenance_lease(&lease).await?;
        Ok(report)
    }

    async fn acquire_maintenance_lease(&self, mode: &str, owner: &str) -> Result<MaintenanceLease, StoreError> {
        let owner = owner.trim();
        if owner.is_empty() { return Err(StoreError::InvalidInput("maintenance owner 不能为空".to_owned())); }
        let mut connection = self.connection().await?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).await?;
        let now = now_ms();
        let mut rows = transaction.query("SELECT owner, lease_expires_at, fence_epoch FROM projection_maintenance_owner WHERE singleton=1", ()).await?;
        let (current_owner, expires, epoch) = if let Some(row) = rows.next().await? { (optional_text(row.get_value(0)?)?, optional_integer(row.get_value(1)?)?, integer_value(row.get_value(2)?, "maintenance.fence_epoch")?) } else { (None, None, 0) };
        if current_owner.is_some() && expires.unwrap_or(0) > now { return Err(StoreError::MaintenanceBusy(format!("maintenance owner {} holds lease until {}", current_owner.unwrap_or_default(), expires.unwrap_or_default()))); }
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
        Ok(format!("turso:{}:{}:{}", metadata.len(), metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map(|duration| duration.as_nanos()).unwrap_or(0), schema_version))
    }
}

struct MaintenanceLease { token: String }

async fn doctor_connection(connection: &Connection) -> Result<StoreDoctorReport, StoreError> {
    let integrity_check = scalar_text(connection, "PRAGMA integrity_check", "integrity_check").await?;
    let user_version = scalar_integer(connection, "PRAGMA user_version", "user_version").await?;
    let migration_version = if table_exists(connection, "schema_migrations").await? { scalar_optional_integer(connection, "SELECT MAX(version) FROM schema_migrations", "migration_version").await? } else { None };
    let now = now_ms();
    let expired_running_tasks = scalar_integer_params(connection, "SELECT COUNT(*) FROM tasks WHERE status='running' AND claim_expires_at <= ?1", [now], "expired_running_tasks").await?;
    let running_tasks_without_active_run = scalar_integer(connection, "SELECT COUNT(*) FROM tasks t WHERE t.status='running' AND (t.current_run_id IS NULL OR NOT EXISTS (SELECT 1 FROM task_runs r WHERE r.id=t.current_run_id AND r.task_id=t.id AND r.status='running' AND r.claim_token=t.claim_token))", "running_tasks_without_active_run").await?;
    let orphan_running_runs = scalar_integer(connection, "SELECT COUNT(*) FROM task_runs r WHERE r.status='running' AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=r.task_id AND t.status='running' AND t.current_run_id=r.id AND t.claim_token=r.claim_token)", "orphan_running_runs").await?;
    let archived_dependency_edges = scalar_integer(connection, "SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id JOIN tasks c ON c.id=d.child_task_id WHERE c.status='archived' AND p.status!='archived'", "archived_dependency_edges").await?;
    let unplanned_active_tasks = scalar_integer(connection, "SELECT COUNT(*) FROM tasks t LEFT JOIN task_execution_plans p ON p.task_id=t.id WHERE t.status NOT IN ('done','archived') AND COALESCE(p.state,'unplanned')='unplanned'", "unplanned_active_tasks").await?;
    let active_parents_with_incomplete_required_steps = scalar_integer(connection, "SELECT COUNT(*) FROM tasks t WHERE t.status NOT IN ('done','archived') AND EXISTS (SELECT 1 FROM task_steps s WHERE s.parent_task_id=t.id AND s.required=1 AND s.status NOT IN ('done','skipped'))", "active_parents_with_incomplete_required_steps").await?;
    let (outbox_pending, outbox_running, outbox_failed) = if table_exists(connection, "projection_jobs").await? { (scalar_integer(connection, "SELECT COUNT(*) FROM projection_jobs WHERE status='pending'", "outbox_pending").await?, scalar_integer(connection, "SELECT COUNT(*) FROM projection_jobs WHERE status='running'", "outbox_running").await?, scalar_integer(connection, "SELECT COUNT(*) FROM projection_jobs WHERE status='failed'", "outbox_failed").await?) } else { (0, 0, 0) };
    let mut consistency_issues = Vec::new();
    let mut foreign_keys = connection.query("PRAGMA foreign_key_check", ()).await?;
    let mut foreign_key_violations = 0;
    while foreign_keys.next().await?.is_some() { foreign_key_violations += 1; }
    if foreign_key_violations > 0 { consistency_issues.push(StoreDoctorIssue { severity: "error".to_owned(), code: "foreign_key_violation".to_owned(), message: format!("检测到 {foreign_key_violations} 个外键违规"), record_ids: Vec::new() }); }
    let consistency_errors = consistency_issues.iter().filter(|issue| issue.severity == "error").count() as i64;
    let derived_stores = if table_exists(connection, "projection_state").await? { projection_state_reports(connection).await? } else { Vec::new() };
    let derived_dirty_stores = derived_stores.iter().filter(|store| store.dirty).count() as i64;
    let derived_error_stores = derived_stores.iter().filter(|store| store.last_error.is_some() || store.failed > 0).count() as i64;
    let ok = integrity_check.eq_ignore_ascii_case("ok") && migration_version == Some(user_version) && expired_running_tasks == 0 && running_tasks_without_active_run == 0 && orphan_running_runs == 0 && archived_dependency_edges == 0 && outbox_failed == 0 && consistency_errors == 0 && derived_error_stores == 0;
    let derived_stores = derived_stores.into_iter().map(|store| StoreDoctorDerivedStore {
        store_name: store.store_name,
        schema_version: 2,
        last_event_id: store.last_event_id,
        dirty: store.dirty,
        last_error: store.last_error,
        pending_outbox: store.pending,
        running_outbox: store.running,
        failed_outbox: store.failed,
    }).collect();
    Ok(StoreDoctorReport { ok, integrity_check, migration_version, user_version, expired_running_tasks, running_tasks_without_active_run, orphan_running_runs, dependency_cycles: 0, archived_dependency_edges, missing_run_logs: 0, suspicious_run_log_paths: 0, executable_dependency_violations: 0, executable_spec_violations: 0, executable_schedule_violations: 0, unplanned_active_tasks, active_parents_with_incomplete_required_steps, outbox_pending, outbox_running, outbox_failed, derived_dirty_stores, derived_error_stores, derived_stores, consistency_errors, consistency_warnings: 0, consistency_issues, ontology_ledger_errors: 0, ontology_ledger_warnings: 0, ontology_ledger_issues: Vec::new() })
}

async fn maintenance_status_connection(connection: &Connection) -> Result<StoreMaintenanceStatus, StoreError> {
    let (owner, mode, lease_expires_at, fence_epoch, build_identity, last_heartbeat_at) = if table_exists(connection, "projection_maintenance_owner").await? { let mut rows = connection.query("SELECT owner, mode, lease_expires_at, fence_epoch, build_identity, last_heartbeat_at FROM projection_maintenance_owner WHERE singleton=1", ()).await?; if let Some(row) = rows.next().await? { (optional_text(row.get_value(0)?)?, optional_text(row.get_value(1)?)?, optional_integer(row.get_value(2)?)?, integer_value(row.get_value(3)?, "maintenance.fence_epoch")?, optional_text(row.get_value(4)?)?, optional_integer(row.get_value(5)?)?) } else { (None, None, None, 0, None, None) } } else { (None, None, None, 0, None, None) };
    let stores = if table_exists(connection, "projection_state").await? { projection_state_reports(connection).await? } else { Vec::new() };
    let database_instance_id = scalar_text(connection, "SELECT family || ':' || lineage || ':' || fingerprint FROM schema_identity WHERE singleton=1", "database_instance_id").await.unwrap_or_else(|_| "turso:unknown".to_owned());
    Ok(StoreMaintenanceStatus { database_instance_id, protocol_version: 2, owner: StoreMaintenanceOwner { active: owner.is_some() && lease_expires_at.unwrap_or(0) > now_ms(), owner, mode, lease_expires_at, fence_epoch, build_identity, last_heartbeat_at }, stores })
}

async fn projection_state_reports(connection: &Connection) -> Result<Vec<StoreProjectionStatus>, StoreError> {
    let mut rows = connection.query("SELECT projection, active_generation, active_fingerprint, previous_generation, building_generation, lifecycle_status, fence_epoch, last_event_id, dirty, last_error, updated_at FROM projection_state ORDER BY projection", ()).await?;
    let mut values = Vec::new();
    while let Some(row) = rows.next().await? {
        let projection = text_value(row.get_value(0)?, "projection_state.projection")?;
        let pending = scalar_integer_params(connection, "SELECT COUNT(*) FROM projection_jobs WHERE target=?1 AND status='pending'", [projection.as_str()], "projection_jobs.pending").await.unwrap_or(0);
        let running = scalar_integer_params(connection, "SELECT COUNT(*) FROM projection_jobs WHERE target=?1 AND status='running'", [projection.as_str()], "projection_jobs.running").await.unwrap_or(0);
        let failed = scalar_integer_params(connection, "SELECT COUNT(*) FROM projection_jobs WHERE target=?1 AND status='failed'", [projection.as_str()], "projection_jobs.failed").await.unwrap_or(0);
        values.push(StoreProjectionStatus { store_name: projection, active_generation: optional_text(row.get_value(1)?)?, active_fingerprint: optional_text(row.get_value(2)?)?, previous_generation: optional_text(row.get_value(3)?)?, building_generation: optional_text(row.get_value(4)?)?, lifecycle_status: text_value(row.get_value(5)?, "projection_state.lifecycle_status")?, fence_epoch: integer_value(row.get_value(6)?, "projection_state.fence_epoch")?, last_event_id: integer_value(row.get_value(7)?, "projection_state.last_event_id")?, dirty: integer_value(row.get_value(8)?, "projection_state.dirty")? != 0, last_error: optional_text(row.get_value(9)?)?, updated_at: integer_value(row.get_value(10)?, "projection_state.updated_at")?, pending, running, failed });
    }
    Ok(values)
}

async fn insert_portable_record(transaction: &turso::transaction::Transaction<'_>, record: &PortableLine) -> Result<(), StoreError> {
    if record.data.is_empty() { return Ok(()); }
    let columns = record.data.keys().cloned().collect::<Vec<_>>();
    let quoted = columns.iter().map(|column| format!("\"{}\"", column.replace('"', "\"\""))).collect::<Vec<_>>().join(", ");
    let placeholders = (1..=columns.len()).map(|index| format!("?{index}")).collect::<Vec<_>>().join(", ");
    let sql = format!("INSERT OR IGNORE INTO {} ({quoted}) VALUES ({placeholders})", record.table);
    let params = columns.iter().map(|column| json_to_value(record.data.get(column).expect("portable column"))).collect::<Vec<_>>();
    transaction.execute(sql, params_from_iter(params)).await?;
    Ok(())
}

fn read_portable(path: &Path) -> Result<(PortableHeader, Vec<PortableLine>), StoreError> {
    let mut lines = BufReader::new(File::open(path).map_err(io_error)?).lines();
    let header_line = lines.next().ok_or_else(|| StoreError::InvalidInput("portable export 为空".to_owned()))?.map_err(io_error)?;
    let header = serde_json::from_str::<PortableHeader>(&header_line).map_err(json_error)?;
    let mut records = Vec::new();
    for line in lines { let line = line.map_err(io_error)?; if !line.trim().is_empty() { records.push(serde_json::from_str::<PortableLine>(&line).map_err(json_error)?); } }
    Ok((header, records))
}

async fn canonical_record_count(connection: &Connection) -> Result<i64, StoreError> {
    // `initialize()` seeds the default board/columns and relation predicates. They are
    // host bootstrap metadata, not imported facts, so a fresh target must remain importable.
    let mut total = 0;
    for table in PORTABLE_TABLES
        .iter()
        .filter(|table| !matches!(**table, "boards" | "board_columns" | "relation_predicates"))
    {
        total += scalar_integer(connection, &format!("SELECT COUNT(*) FROM {table}"), "canonical_record_count").await.unwrap_or(0);
    }
    Ok(total)
}

async fn table_exists(connection: &Connection, table: &str) -> Result<bool, StoreError> { Ok(scalar_integer_params(connection, "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1", [table], "table_exists").await? != 0) }
async fn scalar_text(connection: &Connection, sql: &str, field: &'static str) -> Result<String, StoreError> { let mut rows = connection.query(sql, ()).await?; let row = rows.next().await?.ok_or(StoreError::InvalidStoredValue { field })?; text_value(row.get_value(0)?, field) }
async fn scalar_integer(connection: &Connection, sql: &str, field: &'static str) -> Result<i64, StoreError> { scalar_integer_params(connection, sql, Vec::<Value>::new(), field).await }
async fn scalar_optional_integer(connection: &Connection, sql: &str, field: &'static str) -> Result<Option<i64>, StoreError> { let mut rows = connection.query(sql, ()).await?; let row = rows.next().await?.ok_or(StoreError::InvalidStoredValue { field })?; optional_integer(row.get_value(0)?) }
async fn scalar_integer_params<T: turso::IntoParams>(connection: &Connection, sql: &str, params: T, field: &'static str) -> Result<i64, StoreError> { let mut rows = connection.query(sql, params).await?; let row = rows.next().await?.ok_or(StoreError::InvalidStoredValue { field })?; integer_value(row.get_value(0)?, field) }
fn integer_value(value: Value, field: &'static str) -> Result<i64, StoreError> { match value { Value::Integer(value) => Ok(value), _ => Err(StoreError::InvalidStoredValue { field }) } }
fn optional_integer(value: Value) -> Result<Option<i64>, StoreError> { match value { Value::Null => Ok(None), Value::Integer(value) => Ok(Some(value)), _ => Err(StoreError::InvalidStoredValue { field: "nullable_integer" }) } }
fn text_value(value: Value, field: &'static str) -> Result<String, StoreError> { match value { Value::Text(value) => Ok(value), _ => Err(StoreError::InvalidStoredValue { field }) } }
fn optional_text(value: Value) -> Result<Option<String>, StoreError> { match value { Value::Null => Ok(None), Value::Text(value) => Ok(Some(value)), _ => Err(StoreError::InvalidStoredValue { field: "nullable_text" }) } }

fn value_to_json(value: Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Integer(value) => value.into(),
        Value::Real(value) => serde_json::Number::from_f64(value).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
        Value::Text(value) => value.into(),
        Value::Blob(value) => serde_json::Value::String(format!("hex:{}", hex_encode(&value))),
    }
}

fn json_to_value(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Integer(i64::from(*value)),
        serde_json::Value::Number(value) => value.as_i64().map(Value::Integer).or_else(|| value.as_f64().map(Value::Real)).unwrap_or(Value::Null),
        serde_json::Value::String(value) if value.starts_with("hex:") => Value::Blob(hex_decode(&value[5..]).unwrap_or_default()),
        serde_json::Value::String(value) => Value::Text(value.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Value::Text(value.to_string()),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes { result.push(HEX[(byte >> 4) as usize] as char); result.push(HEX[(byte & 0x0f) as usize] as char); }
    result
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 { return None; }
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
    if path.as_os_str().is_empty() { return Err(StoreError::InvalidInput(format!("{kind} target path 为空"))); }
    if fs::symlink_metadata(path).is_ok() { return Err(StoreError::InvalidInput(format!("{kind} target already exists: {}", path.display()))); }
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) { fs::create_dir_all(parent).map_err(io_error)?; }
    Ok(path.to_path_buf())
}
fn temporary_sibling(path: &Path, kind: &str) -> Result<PathBuf, StoreError> { Ok(path.with_file_name(format!(".{}.{}.{:?}.tmp", path.file_name().and_then(|name| name.to_str()).unwrap_or("kanban"), kind, SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0)))) }
async fn vacuum_into(connection: &Connection, path: &Path) -> Result<(), StoreError> { let escaped = path.to_string_lossy().replace('\'', "''"); connection.execute(format!("VACUUM INTO '{escaped}'"), ()).await?; Ok(()) }
async fn verify_database_file(path: &Path) -> Result<(), StoreError> { let path = path.to_str().ok_or(StoreError::InvalidPath)?; let database = turso::Builder::new_local(path).experimental_index_method(true).experimental_vacuum(true).build().await?; let connection = database.connect()?; let value = scalar_text(&connection, "PRAGMA integrity_check", "integrity_check").await?; if value != "ok" { return Err(StoreError::InvalidInput(format!("backup integrity check failed: {value}"))); } Ok(()) }
fn durable_rename(source: &Path, target: &Path) -> Result<(), StoreError> { OpenOptions::new().read(true).open(source).map_err(io_error)?.sync_all().map_err(io_error)?; fs::rename(source, target).map_err(io_error)?; if let Some(parent) = target.parent().filter(|parent| !parent.as_os_str().is_empty()) { File::open(parent).map_err(io_error)?.sync_all().map_err(io_error)?; } Ok(()) }
fn file_digest(path: &Path) -> Result<(String, u64), StoreError> { let mut file = File::open(path).map_err(io_error)?; let mut digest = Sha256::new(); let mut total = 0; let mut buffer = [0_u8; 64 * 1024]; loop { let read = std::io::Read::read(&mut file, &mut buffer).map_err(io_error)?; if read == 0 { break; } digest.update(&buffer[..read]); total += read as u64; } Ok((format!("sha256:{:x}", digest.finalize()), total)) }
fn unique_suffix() -> String { format!("{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0)) }
fn io_error(error: std::io::Error) -> StoreError { StoreError::InvalidInput(format!("filesystem operation failed: {error}")) }
fn json_error(error: serde_json::Error) -> StoreError { StoreError::InvalidInput(format!("portable JSONL 无效: {error}")) }

#[cfg(test)]
mod tests {
    use crate::test_support::{create_input, store};

    #[tokio::test]
    async fn maintenance_status_and_run_release_owner_lease() {
        let (_directory, store, _path) = store("maintenance-status").await;
        store.initialize().await.expect("initialize");

        let status = store.maintenance_status().await.expect("status");
        assert!(!status.owner.active);
        let run = store.maintenance_run("test-owner", "rebuild").await.expect("rebuild");
        assert_eq!(run.owner, "test-owner");
        assert!(run.stores.iter().all(|store| store.active_generation.is_some()));
        let status = store.maintenance_status().await.expect("released status");
        assert!(!status.owner.active, "successful maintenance must release its lease");

        let checkpoint = store.checkpoint().await.expect("checkpoint");
        assert!(checkpoint.busy >= 0);
        let doctor = store.doctor().await.expect("doctor");
        assert_eq!(doctor.integrity_check, "ok");
    }

    #[tokio::test]
    async fn verified_backup_and_portable_export_import_roundtrip() {
        let (source_directory, source, _source_path) = store("maintenance-source").await;
        source.initialize().await.expect("initialize source");
        source.create_task("default", create_input("t_maintenance", None, "Maintenance fixture")).await.expect("fixture task");

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
        let import = target.import(&export_path, false).await.expect("portable import");
        assert!(import.imported_records > 0);
        assert!(!target.maintenance_status().await.expect("target status").owner.active);
        let tasks = target
            .list_tasks("default", crate::TaskListOptions::default())
            .await
            .expect("imported tasks");
        assert!(tasks.tasks.iter().any(|task| task.id == "t_maintenance"));
    }
}
