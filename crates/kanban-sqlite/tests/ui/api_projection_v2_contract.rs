#![allow(unused_imports)]

use kanban_sqlite::api::provider::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionCorpusMetadata, ProjectionDelivery,
    ProjectionDestructiveAuthority, ProjectionGenerationBinding, ProjectionGenerationRole,
    ProjectionPublishReceipt, ProjectionSnapshot, ProjectionSnapshotRecord,
    ProjectionStoreBackend, ProjectionStoreDescriptor,
    begin_projection_generation, prepare_projection_snapshot_with,
    publish_projection_generation_with, reconcile_projection_generation_with,
    recover_projection_generation_with,
    run_projection_batch_with,
};
use kanban_sqlite::api::{
    MaintenanceLegacyCleanupAction, MaintenanceLegacyCleanupReport,
    MaintenanceLegacyCleanupRoot, MaintenanceMode, MaintenanceRebuildIntent,
    MaintenanceRunOptions, MaintenanceRunReport, MaintenanceSession, PROJECTION_PROTOCOL_VERSION,
    ProjectionLease, ProjectionStatus, ProjectionStoreStatus, abort_projection_generation,
    acquire_projection_lease, maintenance_apply_legacy_projection_cleanup,
    maintenance_inventory_legacy_projections, maintenance_plan_rebuild_all,
    maintenance_plan_rebuild_store, maintenance_rebuild_all, maintenance_rebuild_store,
    maintenance_restore_legacy_projection_cleanup, maintenance_resume_rebuild_store,
    maintenance_run_once, maintenance_status, maintenance_verify_legacy_projection_cleanup,
    projection_status, release_projection_lease, renew_projection_lease,
};

fn main() {}

fn projection_v2_maintenance_contract_path_compiles(
    path: &std::path::Path,
    backup_dir: &std::path::Path,
    owner: &str,
    inventory_digest: &str,
) {
    let options = MaintenanceRunOptions::default();
    let _ = maintenance_plan_rebuild_store(
        path,
        owner,
        "tantivy",
        MaintenanceRebuildIntent::Fresh,
    );
    let _ = maintenance_plan_rebuild_store(
        path,
        owner,
        "tantivy",
        MaintenanceRebuildIntent::Resume,
    );
    let _ = maintenance_plan_rebuild_all(path, owner);
    let _ = maintenance_resume_rebuild_store(path, owner, "tantivy", options.clone());

    let _ = maintenance_inventory_legacy_projections(path);
    let _ = maintenance_apply_legacy_projection_cleanup(
        path,
        owner,
        inventory_digest,
        backup_dir,
        false,
        options.clone(),
    );
    let _ =
        maintenance_verify_legacy_projection_cleanup(path, owner, backup_dir, options.clone());
    let _ = maintenance_restore_legacy_projection_cleanup(path, owner, backup_dir, options);
}

fn projection_v2_maintenance_dtos_are_stable(
    _intent: MaintenanceRebuildIntent,
    _action: MaintenanceLegacyCleanupAction,
    _root: MaintenanceLegacyCleanupRoot,
    _report: MaintenanceLegacyCleanupReport,
) {
}
