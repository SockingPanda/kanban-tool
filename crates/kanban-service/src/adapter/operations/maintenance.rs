use crate::{
    BackupReportRecord, CheckpointReportRecord, DoctorDerivedStoreRecord, DoctorIssueRecord,
    DoctorReportRecord, ExportReportRecord, ImportReportRecord, MaintenanceOwnerRecord,
    MaintenanceQuery, MaintenanceRunRecord, MaintenanceStatusRecord, ProjectionStatusRecord,
    VacuumReportRecord,
};
#[cfg(feature = "legacy-sqlite-import")]
use crate::{LegacyImportOptionsRecord, LegacyImportResultRecord, LegacyImportTableCountRecord};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

impl MaintenanceQuery for TursoApplicationStore {
    async fn doctor(&self) -> Result<DoctorReportRecord> {
        self.store
            .doctor()
            .await
            .map_err(store_error)
            .map(|value| DoctorReportRecord {
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
                    .map(|store| DoctorDerivedStoreRecord {
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
                consistency_errors: value.consistency_errors,
                consistency_warnings: value.consistency_warnings,
                consistency_issues: value
                    .consistency_issues
                    .into_iter()
                    .map(|issue| DoctorIssueRecord {
                        severity: issue.severity,
                        code: issue.code,
                        message: issue.message,
                        record_ids: issue.record_ids,
                    })
                    .collect(),
                ontology_ledger_errors: value.ontology_ledger_errors,
                ontology_ledger_warnings: value.ontology_ledger_warnings,
                ontology_ledger_issues: value
                    .ontology_ledger_issues
                    .into_iter()
                    .map(|issue| DoctorIssueRecord {
                        severity: issue.severity,
                        code: issue.code,
                        message: issue.message,
                        record_ids: issue.record_ids,
                    })
                    .collect(),
            })
    }

    async fn checkpoint(&self) -> Result<CheckpointReportRecord> {
        self.store
            .checkpoint()
            .await
            .map_err(store_error)
            .map(|value| CheckpointReportRecord {
                busy: value.busy,
                log_frames: value.log_frames,
                checkpointed_frames: value.checkpointed_frames,
            })
    }

    async fn backup(&self, path: &str) -> Result<BackupReportRecord> {
        self.store
            .backup(path)
            .await
            .map_err(store_error)
            .map(|value| BackupReportRecord {
                out_path: value.out_path,
                checksum_sha256: value.checksum_sha256,
                bytes: value.bytes,
                source_fingerprint: value.source_fingerprint,
            })
    }

    async fn export(&self, path: &str) -> Result<ExportReportRecord> {
        self.store
            .export(path)
            .await
            .map_err(store_error)
            .map(|value| ExportReportRecord {
                out_path: value.out_path,
                checksum_sha256: value.checksum_sha256,
                bytes: value.bytes,
                record_count: value.record_count,
                source_fingerprint: value.source_fingerprint,
            })
    }

    async fn import(&self, path: &str, replace: bool) -> Result<ImportReportRecord> {
        self.store
            .import(path, replace)
            .await
            .map_err(store_error)
            .map(import_record)
    }

    async fn prepare_import(&self, path: &str) -> Result<ImportReportRecord> {
        self.store
            .prepare_import(path)
            .await
            .map_err(store_error)
            .map(import_record)
    }

    async fn vacuum(&self) -> Result<VacuumReportRecord> {
        self.store
            .vacuum()
            .await
            .map_err(store_error)
            .map(|value| VacuumReportRecord {
                ok: value.ok,
                before_bytes: value.before_bytes,
                after_bytes: value.after_bytes,
                source_fingerprint: value.source_fingerprint,
            })
    }

    async fn maintenance_status(&self) -> Result<MaintenanceStatusRecord> {
        self.store
            .maintenance_status()
            .await
            .map_err(store_error)
            .map(status_record)
    }

    async fn maintenance_run(&self, owner: &str, action: &str) -> Result<MaintenanceRunRecord> {
        self.store
            .maintenance_run(owner, action)
            .await
            .map_err(store_error)
            .map(|value| MaintenanceRunRecord {
                database_instance_id: value.database_instance_id,
                protocol_version: value.protocol_version,
                owner: value.owner,
                mode: value.mode,
                action: value.action,
                processed: value.processed,
                phase: value.phase,
                degraded: value.degraded,
                errors: value.errors,
                stores: value.stores.into_iter().map(projection_record).collect(),
            })
    }

    #[cfg(feature = "legacy-sqlite-import")]
    async fn import_legacy_sqlite_v30(
        &self,
        options: LegacyImportOptionsRecord,
    ) -> Result<LegacyImportResultRecord> {
        let options = crate::LegacyImportOptions {
            source_path: options.source_path.into(),
            canonical_attachment_root: options.canonical_attachment_root.map(Into::into),
        };
        self.store
            .import_legacy_sqlite_v30(options)
            .await
            .map_err(store_error)
            .map(|value| LegacyImportResultRecord {
                journal_id: value.journal_id,
                phase: value.phase,
                source_path: value.source_path.to_string_lossy().into_owned(),
                source_fingerprint: value.source_fingerprint,
                schema_fingerprint: value.schema_fingerprint,
                resumed: value.resumed,
                attachment_count: value.attachment_count,
                table_counts: value
                    .table_counts
                    .into_iter()
                    .map(|count| LegacyImportTableCountRecord {
                        table: count.table,
                        source_rows: count.source_rows,
                        target_rows: count.target_rows,
                    })
                    .collect(),
            })
    }
}

fn import_record(value: crate::StoreImportReport) -> ImportReportRecord {
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

fn status_record(value: crate::StoreMaintenanceStatus) -> MaintenanceStatusRecord {
    MaintenanceStatusRecord {
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        owner: MaintenanceOwnerRecord {
            owner: value.owner.owner,
            mode: value.owner.mode,
            lease_expires_at: value.owner.lease_expires_at,
            fence_epoch: value.owner.fence_epoch,
            build_identity: value.owner.build_identity,
            last_heartbeat_at: value.owner.last_heartbeat_at,
            active: value.owner.active,
        },
        stores: value.stores.into_iter().map(projection_record).collect(),
    }
}

fn projection_record(value: crate::StoreProjectionStatus) -> ProjectionStatusRecord {
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
