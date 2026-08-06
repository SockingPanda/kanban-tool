//! Host-admin 诊断与维护的 application boundary。

use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{
    ApplicationService, ApplicationStore, KanbanService, StoreBackupReport, StoreCheckpointReport,
    StoreDoctorReport, StoreExportReport, StoreImportReport, StoreMaintenanceRun,
    StoreMaintenanceStatus, StoreVacuumReport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorDerivedStoreRecord {
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
pub struct DoctorIssueRecord {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub record_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReportRecord {
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
    pub derived_stores: Vec<DoctorDerivedStoreRecord>,
    pub consistency_errors: i64,
    pub consistency_warnings: i64,
    pub consistency_issues: Vec<DoctorIssueRecord>,
    pub ontology_ledger_errors: i64,
    pub ontology_ledger_warnings: i64,
    pub ontology_ledger_issues: Vec<DoctorIssueRecord>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointReportRecord {
    pub busy: i64,
    pub log_frames: i64,
    pub checkpointed_frames: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReportRecord {
    pub out_path: String,
    pub checksum_sha256: String,
    pub bytes: u64,
    pub source_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReportRecord {
    pub out_path: String,
    pub checksum_sha256: String,
    pub bytes: u64,
    pub record_count: u64,
    pub source_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReportRecord {
    pub in_path: String,
    pub source_fingerprint: String,
    pub imported_records: u64,
    pub skipped_records: u64,
    pub rebuild_jobs_enqueued: u64,
    pub journal_id: String,
    pub phase: String,
    pub restart_required: bool,
    pub staged_database_path: Option<String>,
    pub target_fingerprint_before: Option<String>,
    pub staged_fingerprint: Option<String>,
    pub publish_preconditions: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VacuumReportRecord {
    pub ok: bool,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub source_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceOwnerRecord {
    pub owner: Option<String>,
    pub mode: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub fence_epoch: i64,
    pub build_identity: Option<String>,
    pub last_heartbeat_at: Option<i64>,
    pub active: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionStatusRecord {
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
    pub phase: String,
    pub degraded: bool,
    pub errors: Vec<String>,
    pub updated_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceStatusRecord {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: MaintenanceOwnerRecord,
    pub stores: Vec<ProjectionStatusRecord>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceRunRecord {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: String,
    pub mode: String,
    pub action: String,
    pub processed: u64,
    pub phase: String,
    pub degraded: bool,
    pub errors: Vec<String>,
    pub stores: Vec<ProjectionStatusRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportOptionsRecord {
    pub source_path: String,
    pub canonical_attachment_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportTableCountRecord {
    pub table: String,
    pub source_rows: u64,
    pub target_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportResultRecord {
    pub journal_id: String,
    pub phase: String,
    pub source_path: String,
    pub source_fingerprint: String,
    pub schema_fingerprint: String,
    pub resumed: bool,
    pub attachment_count: u64,
    pub table_counts: Vec<LegacyImportTableCountRecord>,
}

/// host-admin 维护操作直接落在 canonical Turso primitive 上。
///
/// 该入口固定使用 `KanbanService` 的 service-owned store，保留统一的输入校验与
/// `StoreError` 到 `KanbanError` 的映射；不再为每个调用创建 application store trait。
impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn doctor(&self) -> Result<StoreDoctorReport> {
        self.application
            .store
            .store
            .doctor()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn checkpoint(&self) -> Result<StoreCheckpointReport> {
        self.application
            .store
            .store
            .checkpoint()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn backup(&self, path: &str) -> Result<StoreBackupReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .backup(path)
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn export(&self, path: &str) -> Result<StoreExportReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .export(path)
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn import(&self, path: &str, replace: bool) -> Result<StoreImportReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .import(path, replace)
            .await
            .map_err(crate::adapter::store_error)
    }

    /// 仅执行 replace 的 prepare/verify 阶段，并保留 restart/publish 证据。
    pub async fn prepare_import(&self, path: &str) -> Result<StoreImportReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .prepare_import(path)
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn vacuum(&self) -> Result<StoreVacuumReport> {
        self.application
            .store
            .store
            .vacuum()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn maintenance_status(&self) -> Result<StoreMaintenanceStatus> {
        self.application
            .store
            .store
            .maintenance_status()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn maintenance_run(&self, owner: &str, action: &str) -> Result<StoreMaintenanceRun> {
        if owner.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "maintenance owner 不能为空".to_owned(),
            ));
        }
        let action = action.trim();
        if !matches!(action, "run" | "rebuild" | "cleanup" | "compact") {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported maintenance action: {action}"
            )));
        }
        self.application
            .store
            .store
            .maintenance_run(owner, action)
            .await
            .map_err(crate::adapter::store_error)
    }

    #[cfg(feature = "legacy-sqlite-import")]
    pub async fn import_legacy_sqlite_v30(
        &self,
        options: crate::LegacyImportOptions,
    ) -> Result<crate::LegacyImportResult> {
        let source_path = options.source_path.to_string_lossy();
        validate_path(&source_path)?;
        if let Some(root) = options.canonical_attachment_root.as_deref() {
            let root = root.to_string_lossy();
            validate_path(&root)?;
        }
        self.application
            .store
            .store
            .import_legacy_sqlite_v30(options)
            .await
            .map_err(crate::adapter::store_error)
    }
}

pub trait MaintenanceQuery: ApplicationStore {
    fn doctor(&self) -> impl Future<Output = Result<DoctorReportRecord>> + Send;
    fn checkpoint(&self) -> impl Future<Output = Result<CheckpointReportRecord>> + Send;
    fn backup(&self, path: &str) -> impl Future<Output = Result<BackupReportRecord>> + Send;
    fn export(&self, path: &str) -> impl Future<Output = Result<ExportReportRecord>> + Send;
    fn import(
        &self,
        path: &str,
        replace: bool,
    ) -> impl Future<Output = Result<ImportReportRecord>> + Send;
    /// 供 host lifecycle 调用的 replace prepare/verify typed seam；不直接暴露为 MCP。
    fn prepare_import(&self, path: &str)
        -> impl Future<Output = Result<ImportReportRecord>> + Send;
    fn vacuum(&self) -> impl Future<Output = Result<VacuumReportRecord>> + Send;
    fn maintenance_status(&self) -> impl Future<Output = Result<MaintenanceStatusRecord>> + Send;
    fn maintenance_run(
        &self,
        owner: &str,
        action: &str,
    ) -> impl Future<Output = Result<MaintenanceRunRecord>> + Send;
    fn import_legacy_sqlite_v30(
        &self,
        _options: LegacyImportOptionsRecord,
    ) -> impl Future<Output = Result<LegacyImportResultRecord>> + Send {
        async {
            Err(KanbanError::FeatureNotAvailable(
                "legacy sqlite v30 importer is not enabled".to_owned(),
            ))
        }
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: MaintenanceQuery,
    C: Clock,
{
    pub async fn doctor(&self) -> Result<DoctorReportRecord> {
        self.store.doctor().await
    }
    pub async fn checkpoint(&self) -> Result<CheckpointReportRecord> {
        self.store.checkpoint().await
    }
    pub async fn backup(&self, path: &str) -> Result<BackupReportRecord> {
        validate_path(path)?;
        self.store.backup(path).await
    }
    pub async fn export(&self, path: &str) -> Result<ExportReportRecord> {
        validate_path(path)?;
        self.store.export(path).await
    }
    pub async fn import(&self, path: &str, replace: bool) -> Result<ImportReportRecord> {
        validate_path(path)?;
        self.store.import(path, replace).await
    }
    pub async fn prepare_import(&self, path: &str) -> Result<ImportReportRecord> {
        validate_path(path)?;
        self.store.prepare_import(path).await
    }
    pub async fn vacuum(&self) -> Result<VacuumReportRecord> {
        self.store.vacuum().await
    }
    pub async fn maintenance_status(&self) -> Result<MaintenanceStatusRecord> {
        self.store.maintenance_status().await
    }
    pub async fn maintenance_run(&self, owner: &str, action: &str) -> Result<MaintenanceRunRecord> {
        if owner.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "maintenance owner 不能为空".to_owned(),
            ));
        }
        let action = action.trim();
        if !matches!(action, "run" | "rebuild" | "cleanup" | "compact") {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported maintenance action: {action}"
            )));
        }
        self.store.maintenance_run(owner, action).await
    }
    pub async fn import_legacy_sqlite_v30(
        &self,
        options: LegacyImportOptionsRecord,
    ) -> Result<LegacyImportResultRecord> {
        validate_path(&options.source_path)?;
        if let Some(root) = options.canonical_attachment_root.as_deref() {
            validate_path(root)?;
        }
        self.store.import_legacy_sqlite_v30(options).await
    }
}

fn validate_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        Err(KanbanError::InvalidInput("path 不能为空".to_owned()))
    } else {
        Ok(())
    }
}
