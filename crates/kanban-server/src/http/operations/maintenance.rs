use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use kanban_application::LegacyImportOptionsRecord;
use kanban_contract::{
    BackupReport, BackupResponse, CheckpointResponse, DataEnvelope, DoctorReport, DoctorResponse,
    ExportReport, ExportResponse, ImportReport, ImportResponse, LegacyImportReport,
    LegacyImportRequest, LegacyImportResponse, MaintenanceImportRequest, MaintenanceOwnerStatus,
    MaintenancePathRequest, MaintenanceRunReport, MaintenanceRunRequest, MaintenanceRunResponse,
    MaintenanceStatusReport, MaintenanceStatusResponse, ProjectionStoreStatus, VacuumReport,
    VacuumResponse,
};

pub(crate) async fn doctor(
    State(state): State<AppState>,
) -> Result<Json<DoctorResponse>, ApiError> {
    let report = state.application().doctor().await?;
    Ok(Json(DataEnvelope::new(DoctorReport {
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
            .map(|store| kanban_contract::DoctorDerivedStore {
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
            .map(|issue| kanban_contract::DoctorIssue {
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
            .map(|issue| kanban_contract::DoctorIssue {
                severity: issue.severity,
                code: issue.code,
                message: issue.message,
                record_ids: issue.record_ids,
            })
            .collect(),
    })))
}

pub(crate) async fn checkpoint(
    State(state): State<AppState>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let report = state.application().checkpoint().await?;
    Ok(Json(DataEnvelope::new(kanban_contract::CheckpointReport {
        busy: report.busy,
        log_frames: report.log_frames,
        checkpointed_frames: report.checkpointed_frames,
    })))
}

pub(crate) async fn backup(
    State(state): State<AppState>,
    Json(request): Json<MaintenancePathRequest>,
) -> Result<(StatusCode, Json<BackupResponse>), ApiError> {
    let report = state.application().backup(&request.path).await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope::new(BackupReport {
            out_path: report.out_path,
            checksum_sha256: report.checksum_sha256,
            bytes: report.bytes,
            source_fingerprint: report.source_fingerprint,
        })),
    ))
}

pub(crate) async fn export(
    State(state): State<AppState>,
    Json(request): Json<MaintenancePathRequest>,
) -> Result<(StatusCode, Json<ExportResponse>), ApiError> {
    let report = state.application().export(&request.path).await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope::new(ExportReport {
            out_path: report.out_path,
            checksum_sha256: report.checksum_sha256,
            bytes: report.bytes,
            record_count: report.record_count,
            source_fingerprint: report.source_fingerprint,
        })),
    ))
}

pub(crate) async fn import(
    State(state): State<AppState>,
    Json(request): Json<MaintenanceImportRequest>,
) -> Result<(StatusCode, Json<ImportResponse>), ApiError> {
    let report = state
        .application()
        .import(&request.path, request.replace)
        .await?;
    Ok((
        StatusCode::OK,
        Json(DataEnvelope::new(ImportReport {
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
        })),
    ))
}

pub(crate) async fn import_legacy_sqlite_v30(
    State(state): State<AppState>,
    Json(request): Json<LegacyImportRequest>,
) -> Result<(StatusCode, Json<LegacyImportResponse>), ApiError> {
    let report = state
        .application()
        .import_legacy_sqlite_v30(LegacyImportOptionsRecord {
            source_path: request.path,
            canonical_attachment_root: request.canonical_attachment_root,
        })
        .await?;
    Ok((
        StatusCode::OK,
        Json(DataEnvelope::new(LegacyImportReport {
            journal_id: report.journal_id,
            phase: report.phase,
            source_path: report.source_path,
            source_fingerprint: report.source_fingerprint,
            schema_fingerprint: report.schema_fingerprint,
            resumed: report.resumed,
            attachment_count: report.attachment_count,
            table_counts: report
                .table_counts
                .into_iter()
                .map(|count| kanban_contract::LegacyImportTableCount {
                    table: count.table,
                    source_rows: count.source_rows,
                    target_rows: count.target_rows,
                })
                .collect(),
        })),
    ))
}

pub(crate) async fn vacuum(
    State(state): State<AppState>,
) -> Result<Json<VacuumResponse>, ApiError> {
    let report = state.application().vacuum().await?;
    Ok(Json(DataEnvelope::new(VacuumReport {
        ok: report.ok,
        before_bytes: report.before_bytes,
        after_bytes: report.after_bytes,
        source_fingerprint: report.source_fingerprint,
    })))
}

pub(crate) async fn maintenance_status(
    State(state): State<AppState>,
) -> Result<Json<MaintenanceStatusResponse>, ApiError> {
    let report = state.application().maintenance_status().await?;
    Ok(Json(DataEnvelope::new(MaintenanceStatusReport {
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
    })))
}

pub(crate) async fn maintenance_run(
    State(state): State<AppState>,
    Json(request): Json<MaintenanceRunRequest>,
) -> Result<Json<MaintenanceRunResponse>, ApiError> {
    let owner = request
        .owner
        .unwrap_or_else(|| state.default_actor().to_owned());
    let action = request.action.unwrap_or_else(|| "run".to_owned());
    let report = state.application().maintenance_run(&owner, &action).await?;
    Ok(Json(DataEnvelope::new(run_report(report))))
}

pub(crate) async fn maintenance_rebuild(
    State(state): State<AppState>,
    Json(request): Json<MaintenanceRunRequest>,
) -> Result<Json<MaintenanceRunResponse>, ApiError> {
    let owner = request
        .owner
        .unwrap_or_else(|| state.default_actor().to_owned());
    let report = state
        .application()
        .maintenance_run(&owner, "rebuild")
        .await?;
    Ok(Json(DataEnvelope::new(run_report(report))))
}

pub(crate) async fn maintenance_cleanup(
    State(state): State<AppState>,
    Json(request): Json<MaintenanceRunRequest>,
) -> Result<Json<MaintenanceRunResponse>, ApiError> {
    let owner = request
        .owner
        .unwrap_or_else(|| state.default_actor().to_owned());
    let report = state
        .application()
        .maintenance_run(&owner, "cleanup")
        .await?;
    Ok(Json(DataEnvelope::new(run_report(report))))
}

fn projection_status(value: kanban_application::ProjectionStatusRecord) -> ProjectionStoreStatus {
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
        updated_at: value.updated_at,
    }
}

fn run_report(value: kanban_application::MaintenanceRunRecord) -> MaintenanceRunReport {
    MaintenanceRunReport {
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        owner: value.owner,
        mode: value.mode,
        action: value.action,
        processed: value.processed,
        stores: value.stores.into_iter().map(projection_status).collect(),
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/maintenance/doctor", get(doctor))
        .route("/api/v1/maintenance/checkpoint", post(checkpoint))
        .route("/api/v1/maintenance/backup", post(backup))
        .route("/api/v1/maintenance/export", post(export))
        .route("/api/v1/maintenance/import", post(import))
        .route(
            "/api/v1/maintenance/import-v30",
            post(import_legacy_sqlite_v30),
        )
        .route("/api/v1/maintenance/vacuum", post(vacuum))
        .route("/api/v1/maintenance/status", get(maintenance_status))
        .route("/api/v1/maintenance/run", post(maintenance_run))
        .route("/api/v1/maintenance/rebuild", post(maintenance_rebuild))
        .route("/api/v1/maintenance/cleanup", post(maintenance_cleanup))
}
