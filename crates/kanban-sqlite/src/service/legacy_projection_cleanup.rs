use std::path::Path;

use kanban_core::{KanbanError, Result};
use kanban_local::{
    LegacyProjectionBackupManifest, LegacyProjectionCleanupError, LegacyProjectionCleanupInventory,
    LegacyProjectionRootInventory, acquire_legacy_projection_cleanup_guard,
    apply_legacy_projection_cleanup_with_resume_decision, inventory_legacy_projection_roots,
    restore_legacy_projection_backup, verify_legacy_projection_backup,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use super::{MaintenanceMode, MaintenanceRunOptions, MaintenanceSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceLegacyCleanupAction {
    Inventory,
    Apply,
    Verify,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceLegacyCleanupRoot {
    pub kind: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub present: bool,
    pub file_count: u64,
    pub directory_count: u64,
    pub byte_count: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceLegacyCleanupReport {
    pub action: MaintenanceLegacyCleanupAction,
    pub dry_run: bool,
    pub resumed: bool,
    pub format_version: u32,
    pub database_instance_id: String,
    pub database_path: String,
    pub backup_dir: Option<String>,
    pub inventory_digest: String,
    pub roots: Vec<MaintenanceLegacyCleanupRoot>,
}

pub fn maintenance_inventory_legacy_projections(
    path: impl AsRef<Path>,
) -> Result<MaintenanceLegacyCleanupReport> {
    let path = path.as_ref();
    let conn = super::maintenance::connect_existing_database_quiescent_read_only(path)?;
    let database_instance_id = projection_database_instance_id(&conn)?;
    let inventory =
        inventory_legacy_projection_roots(path, &database_instance_id).map_err(local_error)?;
    drop(conn);
    report_from_inventory(inventory)
}

pub fn maintenance_apply_legacy_projection_cleanup(
    path: impl AsRef<Path>,
    owner: &str,
    expected_inventory_digest: &str,
    backup_dir: impl AsRef<Path>,
    resume: bool,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceLegacyCleanupReport> {
    let path = path.as_ref();
    let backup_dir = backup_dir.as_ref();
    maintenance_apply_legacy_projection_cleanup_with_post_guard_hook(
        path,
        owner,
        expected_inventory_digest,
        backup_dir,
        resume,
        options,
        || Ok(()),
    )
}

fn maintenance_apply_legacy_projection_cleanup_with_post_guard_hook(
    path: &Path,
    owner: &str,
    expected_inventory_digest: &str,
    backup_dir: &Path,
    resume: bool,
    options: MaintenanceRunOptions,
    post_guard_hook: impl FnOnce() -> Result<()>,
) -> Result<MaintenanceLegacyCleanupReport> {
    let session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let database_instance_id = database_instance_id(path)?;
    let outcome = session.run_with_owner_heartbeat(|| {
        let guard = acquire_legacy_projection_cleanup_guard(path).map_err(local_error)?;
        post_guard_hook()?;
        session.renew_and_validate_database_identity(&database_instance_id)?;
        let outcome = apply_legacy_projection_cleanup_with_resume_decision(
            &guard,
            path,
            &database_instance_id,
            expected_inventory_digest,
            backup_dir,
            resume,
        )
        .map_err(local_error)?;
        let verified = verify_legacy_projection_backup(path, &database_instance_id, backup_dir)
            .map_err(local_error)?;
        if outcome.manifest != verified {
            return Err(KanbanError::Storage(
                "legacy projection cleanup verification disagrees with the applied manifest"
                    .to_owned(),
            ));
        }
        Ok((verified, outcome.resumed))
    })?;
    session.finish()?;
    report_from_manifest(
        MaintenanceLegacyCleanupAction::Apply,
        outcome.0,
        Some(backup_dir),
        outcome.1,
    )
}

pub fn maintenance_verify_legacy_projection_cleanup(
    path: impl AsRef<Path>,
    owner: &str,
    backup_dir: impl AsRef<Path>,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceLegacyCleanupReport> {
    let path = path.as_ref();
    let backup_dir = backup_dir.as_ref();
    let session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let database_instance_id = database_instance_id(path)?;
    let manifest = session.run_with_owner_heartbeat(|| {
        verify_legacy_projection_backup(path, &database_instance_id, backup_dir)
            .map_err(local_error)
    })?;
    session.finish()?;
    report_from_manifest(
        MaintenanceLegacyCleanupAction::Verify,
        manifest,
        Some(backup_dir),
        false,
    )
}

pub fn maintenance_restore_legacy_projection_cleanup(
    path: impl AsRef<Path>,
    owner: &str,
    backup_dir: impl AsRef<Path>,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceLegacyCleanupReport> {
    let path = path.as_ref();
    let backup_dir = backup_dir.as_ref();
    maintenance_restore_legacy_projection_cleanup_with_post_guard_hook(
        path,
        owner,
        backup_dir,
        options,
        || Ok(()),
    )
}

fn maintenance_restore_legacy_projection_cleanup_with_post_guard_hook(
    path: &Path,
    owner: &str,
    backup_dir: &Path,
    options: MaintenanceRunOptions,
    post_guard_hook: impl FnOnce() -> Result<()>,
) -> Result<MaintenanceLegacyCleanupReport> {
    let session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let database_instance_id = database_instance_id(path)?;
    let outcome = session.run_with_owner_heartbeat(|| {
        let guard = acquire_legacy_projection_cleanup_guard(path).map_err(local_error)?;
        post_guard_hook()?;
        session.renew_and_validate_database_identity(&database_instance_id)?;
        restore_legacy_projection_backup(&guard, path, &database_instance_id, backup_dir)
            .map_err(local_error)
    })?;
    session.finish()?;
    report_from_manifest(
        MaintenanceLegacyCleanupAction::Restore,
        outcome.manifest,
        Some(backup_dir),
        outcome.resumed,
    )
}

fn database_instance_id(path: &Path) -> Result<String> {
    let conn = super::maintenance::connect_existing_database(path)?;
    projection_database_instance_id(&conn)
}

fn projection_database_instance_id(conn: &rusqlite::Connection) -> Result<String> {
    conn.query_row(
        "SELECT database_instance_id
         FROM projection_database
         WHERE singleton=1",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(super::storage)?
    .ok_or_else(|| KanbanError::Storage("Projection v2 database identity is missing".to_owned()))
}

fn report_from_inventory(
    inventory: LegacyProjectionCleanupInventory,
) -> Result<MaintenanceLegacyCleanupReport> {
    Ok(MaintenanceLegacyCleanupReport {
        action: MaintenanceLegacyCleanupAction::Inventory,
        dry_run: true,
        resumed: false,
        format_version: inventory.format_version,
        database_instance_id: inventory.database_instance_id,
        database_path: path_string(&inventory.database_path)?,
        backup_dir: None,
        inventory_digest: inventory.inventory_digest,
        roots: inventory
            .roots
            .into_iter()
            .map(root_report)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn report_from_manifest(
    action: MaintenanceLegacyCleanupAction,
    manifest: LegacyProjectionBackupManifest,
    backup_dir: Option<&Path>,
    resumed: bool,
) -> Result<MaintenanceLegacyCleanupReport> {
    Ok(MaintenanceLegacyCleanupReport {
        action,
        dry_run: false,
        resumed,
        format_version: manifest.format_version,
        database_instance_id: manifest.database_instance_id,
        database_path: path_string(&manifest.database_path)?,
        backup_dir: backup_dir.map(path_string).transpose()?,
        inventory_digest: manifest.inventory_digest,
        roots: manifest
            .roots
            .into_iter()
            .map(root_report)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn root_report(root: LegacyProjectionRootInventory) -> Result<MaintenanceLegacyCleanupRoot> {
    Ok(MaintenanceLegacyCleanupRoot {
        kind: serde_json::to_value(root.kind)
            .map_err(|error| KanbanError::Storage(error.to_string()))?
            .as_str()
            .ok_or_else(|| {
                KanbanError::Storage("legacy projection root kind is not a string".to_owned())
            })?
            .to_owned(),
        relative_path: root.relative_path,
        absolute_path: path_string(&root.absolute_path)?,
        present: root.present,
        file_count: root.file_count,
        directory_count: root.directory_count,
        byte_count: root.byte_count,
        digest: root.digest,
    })
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "legacy projection cleanup path is not valid UTF-8: {}",
            path.display()
        ))
    })
}

fn local_error(error: LegacyProjectionCleanupError) -> KanbanError {
    let message = error.to_string();
    match error {
        LegacyProjectionCleanupError::UnsafePath { .. }
        | LegacyProjectionCleanupError::UnsupportedEntry(_)
        | LegacyProjectionCleanupError::DigestMismatch { .. }
        | LegacyProjectionCleanupError::Overlap(_)
        | LegacyProjectionCleanupError::CrossFilesystem { .. }
        | LegacyProjectionCleanupError::ResumeDecision(_) => KanbanError::InvalidInput(message),
        LegacyProjectionCleanupError::Io(_)
        | LegacyProjectionCleanupError::BackupConflict(_)
        | LegacyProjectionCleanupError::JournalConflict(_)
        | LegacyProjectionCleanupError::ManifestConflict(_)
        | LegacyProjectionCleanupError::JournalEncode(_)
        | LegacyProjectionCleanupError::JournalDecode(_) => KanbanError::Storage(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{db::connect_file, init::init_database};

    fn cleanup_fixture(
        name: &str,
    ) -> anyhow::Result<(
        tempfile::TempDir,
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
        String,
    )> {
        let database_temp = tempfile::Builder::new()
            .prefix(&format!("kb-cleanup-service-{name}-"))
            .tempdir()?;
        let backup_temp = tempfile::Builder::new()
            .prefix(&format!("kb-cleanup-service-backup-{name}-"))
            .tempdir()?;
        let database_path = database_temp.path().join("kb.db");
        init_database(&database_path, "tester")?;
        let legacy_file = database_temp.path().join("index/v1/tasks/segment/doc");
        std::fs::create_dir_all(
            legacy_file
                .parent()
                .expect("legacy fixture path has a parent"),
        )?;
        std::fs::write(&legacy_file, b"legacy-task-index")?;
        let backup_dir = backup_temp.path().join("projection-v1-backup");
        crate::service::checkpoint_database(&database_path)?;
        let inventory = maintenance_inventory_legacy_projections(&database_path)?;
        Ok((
            database_temp,
            backup_temp,
            database_path,
            legacy_file,
            backup_dir,
            inventory.inventory_digest,
        ))
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cleanup_apply_revalidates_owner_after_physical_guard_before_any_move() -> anyhow::Result<()>
    {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, digest) =
            cleanup_fixture("apply-owner-revalidation")?;

        let error = maintenance_apply_legacy_projection_cleanup_with_post_guard_hook(
            &database_path,
            "cleanup-owner",
            &digest,
            &backup_dir,
            false,
            MaintenanceRunOptions::default(),
            || {
                connect_file(&database_path)?
                    .execute(
                        "UPDATE projection_maintenance_owner
                     SET lease_token='pmlease_replaced_after_guard'
                     WHERE singleton=1",
                        [],
                    )
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                Ok(())
            },
        )
        .expect_err("stale owner must abort cleanup");

        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(error.to_string().contains("owner lease is stale"));
        assert!(legacy_file.is_file(), "no legacy root may move");
        assert!(!backup_dir.exists(), "no cleanup journal may be published");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cleanup_apply_revalidates_database_identity_after_physical_guard_before_any_move()
    -> anyhow::Result<()> {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, digest) =
            cleanup_fixture("apply-database-identity-revalidation")?;

        let error = maintenance_apply_legacy_projection_cleanup_with_post_guard_hook(
            &database_path,
            "cleanup-owner",
            &digest,
            &backup_dir,
            false,
            MaintenanceRunOptions::default(),
            || {
                connect_file(&database_path)?
                    .execute_batch(
                        "PRAGMA foreign_keys=OFF;
                         UPDATE projection_store_state
                         SET database_instance_id='db_replaced_after_guard';
                         UPDATE projection_database
                         SET database_instance_id='db_replaced_after_guard'
                         WHERE singleton=1;
                         PRAGMA foreign_keys=ON;",
                    )
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                Ok(())
            },
        )
        .expect_err("rebound database identity must abort cleanup");

        assert!(
            matches!(error, KanbanError::Conflict(_)),
            "expected identity fence conflict, got {error:?}"
        );
        assert!(error.to_string().contains("database identity changed"));
        assert!(legacy_file.is_file(), "no legacy root may move");
        assert!(!backup_dir.exists(), "no cleanup journal may be published");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cleanup_restore_revalidates_owner_after_physical_guard_before_any_move() -> anyhow::Result<()>
    {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, digest) =
            cleanup_fixture("restore-owner-revalidation")?;
        maintenance_apply_legacy_projection_cleanup(
            &database_path,
            "setup-owner",
            &digest,
            &backup_dir,
            false,
            MaintenanceRunOptions::default(),
        )?;
        assert!(!legacy_file.exists());
        let backed_up = backup_dir.join("roots/tantivy_v1/segment/doc");
        assert!(backed_up.is_file());

        let error = maintenance_restore_legacy_projection_cleanup_with_post_guard_hook(
            &database_path,
            "cleanup-owner",
            &backup_dir,
            MaintenanceRunOptions::default(),
            || {
                connect_file(&database_path)?
                    .execute(
                        "UPDATE projection_maintenance_owner
                     SET lease_token='pmlease_replaced_after_guard'
                     WHERE singleton=1",
                        [],
                    )
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                Ok(())
            },
        )
        .expect_err("stale owner must abort restore");

        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(error.to_string().contains("owner lease is stale"));
        assert!(!legacy_file.exists(), "no backup root may be restored");
        assert!(backed_up.is_file(), "backup evidence must remain intact");
        Ok(())
    }

    #[test]
    fn cleanup_error_mapping_preserves_validation_and_corruption_classes() {
        for error in [
            LegacyProjectionCleanupError::DigestMismatch {
                expected: "sha256:expected".to_owned(),
                actual: "sha256:actual".to_owned(),
            },
            LegacyProjectionCleanupError::ResumeDecision("use --resume".to_owned()),
            LegacyProjectionCleanupError::Overlap("/managed/backup".into()),
            LegacyProjectionCleanupError::CrossFilesystem {
                source_path: "/managed".into(),
                backup: "/backup".into(),
            },
        ] {
            assert!(
                matches!(local_error(error), KanbanError::InvalidInput(_)),
                "operator-correctable validation must remain typed invalid input"
            );
        }
        for error in [
            LegacyProjectionCleanupError::JournalConflict("journal binding is corrupt".to_owned()),
            LegacyProjectionCleanupError::ManifestConflict(
                "manifest binding is corrupt".to_owned(),
            ),
            LegacyProjectionCleanupError::BackupConflict("/backup/journal.toml".into()),
            LegacyProjectionCleanupError::Io(std::io::Error::other("disk failure")),
        ] {
            assert!(
                matches!(local_error(error), KanbanError::Storage(_)),
                "corruption and I/O must remain typed storage errors"
            );
        }
    }
}
