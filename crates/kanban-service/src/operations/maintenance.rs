//! Host-admin 诊断与维护的 service boundary。

use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, maintenance};

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

/// host-admin 维护操作直接落在 canonical Turso primitive 上。
///
/// 该入口固定使用 `KanbanService` 的 service-owned store，并在这里把 store records
/// 映射为 host-facing application records；`StoreError` 只在 service 内部转换。
impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn doctor(&self) -> Result<DoctorReportRecord> {
        self.store
            .doctor()
            .await
            .map_err(crate::error::store_error)
            .map(doctor_report)
    }

    pub async fn checkpoint(&self) -> Result<CheckpointReportRecord> {
        self.store
            .checkpoint()
            .await
            .map_err(crate::error::store_error)
            .map(checkpoint_report)
    }

    pub async fn backup(&self, path: &str) -> Result<BackupReportRecord> {
        validate_path(path)?;
        self.store
            .backup(path)
            .await
            .map_err(crate::error::store_error)
            .map(backup_report)
    }

    pub async fn export(&self, path: &str) -> Result<ExportReportRecord> {
        validate_path(path)?;
        self.store
            .export(path)
            .await
            .map_err(crate::error::store_error)
            .map(export_report)
    }

    pub async fn import(&self, path: &str, replace: bool) -> Result<ImportReportRecord> {
        validate_path(path)?;
        self.store
            .import(path, replace)
            .await
            .map_err(crate::error::store_error)
            .map(import_report)
    }

    /// 仅执行 replace 的 prepare/verify 阶段，并保留 restart/publish 证据。
    pub async fn prepare_import(&self, path: &str) -> Result<ImportReportRecord> {
        validate_path(path)?;
        self.store
            .prepare_import(path)
            .await
            .map_err(crate::error::store_error)
            .map(import_report)
    }

    pub async fn vacuum(&self) -> Result<VacuumReportRecord> {
        self.store
            .vacuum()
            .await
            .map_err(crate::error::store_error)
            .map(vacuum_report)
    }

    pub async fn maintenance_status(&self) -> Result<MaintenanceStatusRecord> {
        self.store
            .maintenance_status()
            .await
            .map_err(crate::error::store_error)
            .map(maintenance_status)
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
        self.store
            .maintenance_run(owner, action)
            .await
            .map_err(crate::error::store_error)
            .map(maintenance_run)
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
        self.store
            .import_legacy_sqlite_v30(options)
            .await
            .map_err(crate::error::store_error)
    }
}

fn doctor_report(value: maintenance::StoreDoctorReport) -> DoctorReportRecord {
    DoctorReportRecord {
        ok: value.ok,
        integrity_check: value.integrity_check,
        migration_version: value.migration_version,
        user_version: value.user_version,
        expired_running_tasks: value.expired_running_tasks,
        running_tasks_without_active_run: value.running_tasks_without_active_run,
        orphan_running_runs: value.orphan_running_runs,
        dependency_cycles: value.dependency_cycles,
        archived_dependency_edges: value.archived_dependency_edges,
        missing_run_logs: value.missing_run_logs,
        suspicious_run_log_paths: value.suspicious_run_log_paths,
        executable_dependency_violations: value.executable_dependency_violations,
        executable_spec_violations: value.executable_spec_violations,
        executable_schedule_violations: value.executable_schedule_violations,
        unplanned_active_tasks: value.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: value
            .active_parents_with_incomplete_required_steps,
        outbox_pending: value.outbox_pending,
        outbox_running: value.outbox_running,
        outbox_failed: value.outbox_failed,
        derived_dirty_stores: value.derived_dirty_stores,
        derived_error_stores: value.derived_error_stores,
        derived_stores: value
            .derived_stores
            .into_iter()
            .map(doctor_derived_store)
            .collect(),
        consistency_errors: value.consistency_errors,
        consistency_warnings: value.consistency_warnings,
        consistency_issues: value
            .consistency_issues
            .into_iter()
            .map(doctor_issue)
            .collect(),
        ontology_ledger_errors: value.ontology_ledger_errors,
        ontology_ledger_warnings: value.ontology_ledger_warnings,
        ontology_ledger_issues: value
            .ontology_ledger_issues
            .into_iter()
            .map(doctor_issue)
            .collect(),
    }
}

fn doctor_derived_store(value: maintenance::StoreDoctorDerivedStore) -> DoctorDerivedStoreRecord {
    DoctorDerivedStoreRecord {
        store_name: value.store_name,
        schema_version: value.schema_version,
        last_event_id: value.last_event_id,
        dirty: value.dirty,
        last_error: value.last_error,
        pending_outbox: value.pending_outbox,
        running_outbox: value.running_outbox,
        failed_outbox: value.failed_outbox,
    }
}

fn doctor_issue(value: maintenance::StoreDoctorIssue) -> DoctorIssueRecord {
    DoctorIssueRecord {
        severity: value.severity,
        code: value.code,
        message: value.message,
        record_ids: value.record_ids,
    }
}

fn checkpoint_report(value: maintenance::StoreCheckpointReport) -> CheckpointReportRecord {
    CheckpointReportRecord {
        busy: value.busy,
        log_frames: value.log_frames,
        checkpointed_frames: value.checkpointed_frames,
    }
}

fn backup_report(value: maintenance::StoreBackupReport) -> BackupReportRecord {
    BackupReportRecord {
        out_path: value.out_path,
        checksum_sha256: value.checksum_sha256,
        bytes: value.bytes,
        source_fingerprint: value.source_fingerprint,
    }
}

fn export_report(value: maintenance::StoreExportReport) -> ExportReportRecord {
    ExportReportRecord {
        out_path: value.out_path,
        checksum_sha256: value.checksum_sha256,
        bytes: value.bytes,
        record_count: value.record_count,
        source_fingerprint: value.source_fingerprint,
    }
}

fn import_report(value: maintenance::StoreImportReport) -> ImportReportRecord {
    ImportReportRecord {
        in_path: value.in_path,
        source_fingerprint: value.source_fingerprint,
        imported_records: value.imported_records,
        skipped_records: value.skipped_records,
        rebuild_jobs_enqueued: value.rebuild_jobs_enqueued,
        journal_id: value.journal_id,
        phase: value.phase,
        restart_required: value.restart_required,
        staged_database_path: value.staged_database_path,
        target_fingerprint_before: value.target_fingerprint_before,
        staged_fingerprint: value.staged_fingerprint,
        publish_preconditions: value.publish_preconditions,
    }
}

fn vacuum_report(value: maintenance::StoreVacuumReport) -> VacuumReportRecord {
    VacuumReportRecord {
        ok: value.ok,
        before_bytes: value.before_bytes,
        after_bytes: value.after_bytes,
        source_fingerprint: value.source_fingerprint,
    }
}

fn maintenance_owner(value: maintenance::StoreMaintenanceOwner) -> MaintenanceOwnerRecord {
    MaintenanceOwnerRecord {
        owner: value.owner,
        mode: value.mode,
        lease_expires_at: value.lease_expires_at,
        fence_epoch: value.fence_epoch,
        build_identity: value.build_identity,
        last_heartbeat_at: value.last_heartbeat_at,
        active: value.active,
    }
}

fn projection_status(value: maintenance::StoreProjectionStatus) -> ProjectionStatusRecord {
    ProjectionStatusRecord {
        store_name: value.store_name,
        active_generation: value.active_generation,
        active_fingerprint: value.active_fingerprint,
        previous_generation: value.previous_generation,
        building_generation: value.building_generation,
        lifecycle_status: value.lifecycle_status,
        fence_epoch: value.fence_epoch,
        last_event_id: value.last_event_id,
        dirty: value.dirty,
        pending: value.pending,
        running: value.running,
        failed: value.failed,
        last_error: value.last_error,
        phase: value.phase,
        degraded: value.degraded,
        errors: value.errors,
        updated_at: value.updated_at,
    }
}

fn maintenance_status(value: maintenance::StoreMaintenanceStatus) -> MaintenanceStatusRecord {
    MaintenanceStatusRecord {
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        owner: maintenance_owner(value.owner),
        stores: value.stores.into_iter().map(projection_status).collect(),
    }
}

fn maintenance_run(value: maintenance::StoreMaintenanceRun) -> MaintenanceRunRecord {
    MaintenanceRunRecord {
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        owner: value.owner,
        mode: value.mode,
        action: value.action,
        processed: value.processed,
        phase: value.phase,
        degraded: value.degraded,
        errors: value.errors,
        stores: value.stores.into_iter().map(projection_status).collect(),
    }
}

fn validate_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        Err(KanbanError::InvalidInput("path 不能为空".to_owned()))
    } else {
        Ok(())
    }
}
