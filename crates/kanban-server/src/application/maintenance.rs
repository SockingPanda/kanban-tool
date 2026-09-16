use crate::{error::ApiError, state::AppState};
#[cfg(feature = "legacy-sqlite-import")]
use kanban_protocol::LegacyImportReport;
use kanban_protocol::{
    BackupReport, BackupResponse, CheckpointResponse, DataEnvelope, DoctorReport, DoctorResponse,
    ExportReport, ExportResponse, ImportReport, ImportResponse, LegacyImportRequest,
    LegacyImportResponse, MaintenanceImportRequest, MaintenanceOwnerStatus, MaintenancePathRequest,
    MaintenanceRunReport, MaintenanceRunRequest, MaintenanceRunResponse, MaintenanceStatusReport,
    MaintenanceStatusResponse, ProjectionStoreStatus, VacuumReport, VacuumResponse,
};
#[cfg(feature = "legacy-sqlite-import")]
use kanban_service::LegacyImportOptions;
pub(crate) async fn doctor(state: AppState) -> Result<DoctorResponse, ApiError> {
    let report = state.application().doctor().await?;
    Ok(DataEnvelope::new(DoctorReport {
        ok: report.ok,
        integrity_check: report.integrity_check,
        migration_version: report.migration_version,
        user_version: report.user_version,
        expired_running_tasks: report.expired_running_tasks,
        running_tasks_without_active_run: report.running_tasks_without_active_run,
        orphan_running_runs: report.orphan_running_runs,
        dependency_cycles: report.dependency_cycles,
        archived_dependency_edges: report.archived_dependency_edges,
        missing_run_logs: report.missing_run_logs,
        suspicious_run_log_paths: report.suspicious_run_log_paths,
        executable_dependency_violations: report.executable_dependency_violations,
        executable_spec_violations: report.executable_spec_violations,
        executable_schedule_violations: report.executable_schedule_violations,
        unplanned_active_tasks: report.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: report
            .active_parents_with_incomplete_required_steps,
        outbox_pending: report.outbox_pending,
        outbox_running: report.outbox_running,
        outbox_failed: report.outbox_failed,
        derived_dirty_stores: report.derived_dirty_stores,
        derived_error_stores: report.derived_error_stores,
        derived_stores: report
            .derived_stores
            .into_iter()
            .map(|store| kanban_protocol::DoctorDerivedStore {
                store_name: store.store_name,
                schema_version: store.schema_version,
                last_event_id: store.last_event_id,
                dirty: store.dirty,
                last_error: store.last_error,
                pending_outbox: store.pending_outbox,
                running_outbox: store.running_outbox,
                failed_outbox: store.failed_outbox,
            })
            .collect(),
        consistency_errors: report.consistency_errors,
        consistency_warnings: report.consistency_warnings,
        consistency_issues: report
            .consistency_issues
            .into_iter()
            .map(|issue| kanban_protocol::DoctorIssue {
                severity: issue.severity,
                code: issue.code,
                message: issue.message,
                record_ids: issue.record_ids,
            })
            .collect(),
        ontology_ledger_errors: report.ontology_ledger_errors,
        ontology_ledger_warnings: report.ontology_ledger_warnings,
        ontology_ledger_issues: report
            .ontology_ledger_issues
            .into_iter()
            .map(|issue| kanban_protocol::DoctorIssue {
                severity: issue.severity,
                code: issue.code,
                message: issue.message,
                record_ids: issue.record_ids,
            })
            .collect(),
    }))
}
pub(crate) async fn checkpoint(state: AppState) -> Result<CheckpointResponse, ApiError> {
    let report = state.application().checkpoint().await?;
    Ok(DataEnvelope::new(kanban_protocol::CheckpointReport {
        busy: report.busy,
        log_frames: report.log_frames,
        checkpointed_frames: report.checkpointed_frames,
    }))
}
pub(crate) async fn backup(
    state: AppState,
    request: MaintenancePathRequest,
) -> Result<BackupResponse, ApiError> {
    let report = state.application().backup(&request.path).await?;
    Ok(DataEnvelope::new(BackupReport {
        out_path: report.out_path,
        checksum_sha256: report.checksum_sha256,
        bytes: report.bytes,
        source_fingerprint: report.source_fingerprint,
    }))
}
pub(crate) async fn export(
    state: AppState,
    request: MaintenancePathRequest,
) -> Result<ExportResponse, ApiError> {
    let report = state.application().export(&request.path).await?;
    Ok(DataEnvelope::new(ExportReport {
        out_path: report.out_path,
        checksum_sha256: report.checksum_sha256,
        bytes: report.bytes,
        record_count: report.record_count,
        source_fingerprint: report.source_fingerprint,
    }))
}
pub(crate) async fn import(
    state: AppState,
    request: MaintenanceImportRequest,
) -> Result<ImportResponse, ApiError> {
    let report = state
        .application()
        .import(&request.path, request.replace)
        .await?;
    Ok(DataEnvelope::new(ImportReport {
        in_path: report.in_path,
        source_fingerprint: report.source_fingerprint,
        imported_records: report.imported_records,
        skipped_records: report.skipped_records,
        rebuild_jobs_enqueued: report.rebuild_jobs_enqueued,
        journal_id: report.journal_id,
        phase: report.phase,
        restart_required: report.restart_required,
        staged_database_path: report.staged_database_path,
        target_fingerprint_before: report.target_fingerprint_before,
        staged_fingerprint: report.staged_fingerprint,
        publish_preconditions: report.publish_preconditions,
    }))
}
#[cfg(feature = "legacy-sqlite-import")]
pub(crate) async fn import_legacy_sqlite_v30(
    state: AppState,
    request: LegacyImportRequest,
) -> Result<LegacyImportResponse, ApiError> {
    let report = state
        .application()
        .import_legacy_sqlite_v30(LegacyImportOptions {
            source_path: request.path.into(),
            canonical_attachment_root: request.canonical_attachment_root.map(Into::into),
        })
        .await?;
    Ok(DataEnvelope::new(LegacyImportReport {
        journal_id: report.journal_id,
        phase: report.phase,
        source_path: report.source_path.to_string_lossy().into_owned(),
        source_fingerprint: report.source_fingerprint,
        schema_fingerprint: report.schema_fingerprint,
        resumed: report.resumed,
        attachment_count: report.attachment_count,
        table_counts: report
            .table_counts
            .into_iter()
            .map(|count| kanban_protocol::LegacyImportTableCount {
                table: count.table,
                source_rows: count.source_rows,
                target_rows: count.target_rows,
            })
            .collect(),
    }))
}
#[cfg(not(feature = "legacy-sqlite-import"))]
pub(crate) async fn import_legacy_sqlite_v30(
    _state: AppState,
    request: LegacyImportRequest,
) -> Result<LegacyImportResponse, ApiError> {
    if request.path.trim().is_empty()
        || request
            .canonical_attachment_root
            .as_deref()
            .is_some_and(|root| root.trim().is_empty())
    {
        return Err(ApiError(kanban_service::KanbanError::InvalidInput(
            "path 不能为空".to_owned(),
        )));
    }
    Err(ApiError(kanban_service::KanbanError::FeatureNotAvailable(
        "legacy sqlite v30 importer is not enabled".to_owned(),
    )))
}
pub(crate) async fn vacuum(state: AppState) -> Result<VacuumResponse, ApiError> {
    let report = state.application().vacuum().await?;
    Ok(DataEnvelope::new(VacuumReport {
        ok: report.ok,
        before_bytes: report.before_bytes,
        after_bytes: report.after_bytes,
        source_fingerprint: report.source_fingerprint,
    }))
}
pub(crate) async fn maintenance_status(
    state: AppState,
) -> Result<MaintenanceStatusResponse, ApiError> {
    let report = state.application().maintenance_status().await?;
    Ok(DataEnvelope::new(MaintenanceStatusReport {
        database_instance_id: report.database_instance_id,
        protocol_version: report.protocol_version,
        owner: MaintenanceOwnerStatus {
            owner: report.owner.owner,
            mode: report.owner.mode,
            lease_expires_at: report.owner.lease_expires_at,
            fence_epoch: report.owner.fence_epoch,
            build_identity: report.owner.build_identity,
            last_heartbeat_at: report.owner.last_heartbeat_at,
            active: report.owner.active,
        },
        stores: report.stores.into_iter().map(projection_status).collect(),
    }))
}
pub(crate) async fn maintenance_run(
    state: AppState,
    request: MaintenanceRunRequest,
) -> Result<MaintenanceRunResponse, ApiError> {
    let owner = request
        .owner
        .unwrap_or_else(|| state.default_actor().to_owned());
    let action = request.action.unwrap_or_else(|| "run".to_owned());
    let report = state.application().maintenance_run(&owner, &action).await?;
    Ok(DataEnvelope::new(run_report(report)))
}
pub(crate) async fn maintenance_rebuild(
    state: AppState,
    request: MaintenanceRunRequest,
) -> Result<MaintenanceRunResponse, ApiError> {
    let owner = request
        .owner
        .unwrap_or_else(|| state.default_actor().to_owned());
    let report = state
        .application()
        .maintenance_run(&owner, "rebuild")
        .await?;
    Ok(DataEnvelope::new(run_report(report)))
}
pub(crate) async fn maintenance_cleanup(
    state: AppState,
    request: MaintenanceRunRequest,
) -> Result<MaintenanceRunResponse, ApiError> {
    let owner = request
        .owner
        .unwrap_or_else(|| state.default_actor().to_owned());
    let report = state
        .application()
        .maintenance_run(&owner, "cleanup")
        .await?;
    Ok(DataEnvelope::new(run_report(report)))
}
fn projection_status(value: kanban_service::ProjectionStatusRecord) -> ProjectionStoreStatus {
    ProjectionStoreStatus {
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
fn run_report(value: kanban_service::MaintenanceRunRecord) -> MaintenanceRunReport {
    MaintenanceRunReport {
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
