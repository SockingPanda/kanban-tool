use std::{collections::BTreeSet, path::Path};

use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_indexer::{
    DERIVED_STORE_SCHEMA_VERSION, DERIVED_STORE_SEEDS, LANCEDB_CHUNKS_STORE,
    LANCEDB_LABEL_ATOMS_STORE, OXIGRAPH_RELATIONS_STORE, TANTIVY_TASKS_STORE,
};
use kanban_local::{
    LegacyProjectionBackupManifest, LegacyProjectionCleanupError, LegacyProjectionCleanupInventory,
    LegacyProjectionRootInventory, acquire_legacy_projection_cleanup_guard,
    apply_legacy_projection_cleanup_with_resume_decision, inventory_legacy_projection_roots,
    restore_legacy_projection_backup, verify_legacy_projection_backup,
};
use kanban_vector::{
    LABEL_ATOMS_CORPUS_SCHEMA, TASK_CHUNKS_CORPUS_SCHEMA, corpus_provider_fingerprint,
    embedding_provider_fingerprint,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use super::{MaintenanceMode, MaintenanceRunOptions, MaintenanceSession};

#[cfg(feature = "tantivy-backend")]
const TANTIVY_PROVIDER: &str = super::tantivy_projection::TANTIVY_PROJECTION_PROVIDER;
#[cfg(not(feature = "tantivy-backend"))]
const TANTIVY_PROVIDER: &str = "tantivy";
#[cfg(feature = "tantivy-backend")]
const TANTIVY_PROVIDER_FINGERPRINT: &str =
    super::tantivy_projection::TANTIVY_PROJECTION_PROVIDER_FINGERPRINT;
#[cfg(not(feature = "tantivy-backend"))]
const TANTIVY_PROVIDER_FINGERPRINT: &str = "tantivy-tasks-v2";

#[cfg(feature = "oxigraph-backend")]
const OXIGRAPH_PROVIDER: &str = super::oxigraph_projection::OXIGRAPH_PROJECTION_PROVIDER;
#[cfg(not(feature = "oxigraph-backend"))]
const OXIGRAPH_PROVIDER: &str = "oxigraph";
#[cfg(feature = "oxigraph-backend")]
const OXIGRAPH_PROVIDER_FINGERPRINT: &str =
    super::oxigraph_projection::OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT;
#[cfg(not(feature = "oxigraph-backend"))]
const OXIGRAPH_PROVIDER_FINGERPRINT: &str = "oxigraph-relations-v2";

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

/// Produces the strictly read-only cleanup inventory without requiring Linux
/// `renameat2`.
pub fn maintenance_inventory_legacy_projections(
    path: impl AsRef<Path>,
) -> Result<MaintenanceLegacyCleanupReport> {
    let path = path.as_ref();
    let conn = super::maintenance::connect_existing_database_quiescent_read_only(path)?;
    let preflight = legacy_cleanup_preflight(&conn, LegacyCleanupOwnerExpectation::Idle)?;
    let database_instance_id = preflight.database_instance_id;
    let inventory =
        inventory_legacy_projection_roots(path, &database_instance_id).map_err(local_error)?;
    drop(conn);
    report_from_inventory(inventory)
}

/// Applies the exact inventory using Linux fd-bound `renameat2`.
///
/// Non-Linux platforms return `KanbanError::InvalidInput` before cleanup
/// journal creation/update or any root move.
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
    // Capture the database identity and reject active maintenance/store leases
    // on one read-only snapshot before any writable SQLite setup pragma or
    // owner CAS. A structurally valid expired owner remains available for the
    // session-start CAS to take over.
    let initial_conn = super::maintenance::connect_existing_database_read_only(path)?;
    let initial =
        legacy_cleanup_preflight(&initial_conn, LegacyCleanupOwnerExpectation::Available)?;
    let database_instance_id = initial.database_instance_id;
    drop(initial_conn);
    let session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let outcome = session.run_with_owner_heartbeat(|| {
        session.renew_and_validate_database_identity(&database_instance_id)?;
        let preflight_conn = super::maintenance::connect_existing_database_read_only(path)?;
        let owner_identity = legacy_cleanup_preflight(
            &preflight_conn,
            LegacyCleanupOwnerExpectation::Active {
                owner,
                expected: None,
            },
        )?
        .owner_identity
        .ok_or_else(|| {
            KanbanError::Storage("active cleanup owner identity was not captured".to_owned())
        })?;
        drop(preflight_conn);
        let guard = acquire_legacy_projection_cleanup_guard(path).map_err(local_error)?;
        session.renew_and_validate_database_identity(&database_instance_id)?;
        // This hook intentionally runs after the final renew and immediately
        // before the exact owner preflight. Tests use it to model a same-owner
        // lease rollover/identity change in that TOCTOU window.
        post_guard_hook()?;
        let preflight_conn = super::maintenance::connect_existing_database_read_only(path)?;
        validate_legacy_cleanup_database_identity(&preflight_conn, &database_instance_id)?;
        legacy_cleanup_preflight(
            &preflight_conn,
            LegacyCleanupOwnerExpectation::Active {
                owner,
                expected: Some(&owner_identity),
            },
        )?;
        drop(preflight_conn);
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

/// Re-hashes an existing cleanup backup without requiring Linux `renameat2`.
pub fn maintenance_verify_legacy_projection_cleanup(
    path: impl AsRef<Path>,
    owner: &str,
    backup_dir: impl AsRef<Path>,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceLegacyCleanupReport> {
    let path = path.as_ref();
    let backup_dir = backup_dir.as_ref();
    maintenance_verify_legacy_projection_cleanup_with_post_renew_hook(
        path,
        owner,
        backup_dir,
        options,
        || Ok(()),
    )
}

fn maintenance_verify_legacy_projection_cleanup_with_post_renew_hook(
    path: &Path,
    owner: &str,
    backup_dir: &Path,
    options: MaintenanceRunOptions,
    post_renew_hook: impl FnOnce() -> Result<()>,
) -> Result<MaintenanceLegacyCleanupReport> {
    let initial_conn = super::maintenance::connect_existing_database_read_only(path)?;
    let initial =
        legacy_cleanup_preflight(&initial_conn, LegacyCleanupOwnerExpectation::Available)?;
    let database_instance_id = initial.database_instance_id;
    drop(initial_conn);
    let session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let manifest = session.run_with_owner_heartbeat(|| {
        session.renew_and_validate_database_identity(&database_instance_id)?;
        let preflight_conn = super::maintenance::connect_existing_database_read_only(path)?;
        let owner_identity = legacy_cleanup_preflight(
            &preflight_conn,
            LegacyCleanupOwnerExpectation::Active {
                owner,
                expected: None,
            },
        )?
        .owner_identity
        .ok_or_else(|| {
            KanbanError::Storage("active cleanup owner identity was not captured".to_owned())
        })?;
        drop(preflight_conn);
        session.renew_and_validate_database_identity(&database_instance_id)?;
        post_renew_hook()?;
        let preflight_conn = super::maintenance::connect_existing_database_read_only(path)?;
        validate_legacy_cleanup_database_identity(&preflight_conn, &database_instance_id)?;
        legacy_cleanup_preflight(
            &preflight_conn,
            LegacyCleanupOwnerExpectation::Active {
                owner,
                expected: Some(&owner_identity),
            },
        )?;
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

/// Restores a completed backup using Linux fd-bound `renameat2`.
///
/// Non-Linux platforms return `KanbanError::InvalidInput` before cleanup
/// journal creation/update or any root move.
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
    let initial_conn = super::maintenance::connect_existing_database_read_only(path)?;
    let initial =
        legacy_cleanup_preflight(&initial_conn, LegacyCleanupOwnerExpectation::Available)?;
    let database_instance_id = initial.database_instance_id;
    drop(initial_conn);
    let session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let outcome = session.run_with_owner_heartbeat(|| {
        session.renew_and_validate_database_identity(&database_instance_id)?;
        let preflight_conn = super::maintenance::connect_existing_database_read_only(path)?;
        let owner_identity = legacy_cleanup_preflight(
            &preflight_conn,
            LegacyCleanupOwnerExpectation::Active {
                owner,
                expected: None,
            },
        )?
        .owner_identity
        .ok_or_else(|| {
            KanbanError::Storage("active cleanup owner identity was not captured".to_owned())
        })?;
        drop(preflight_conn);
        let guard = acquire_legacy_projection_cleanup_guard(path).map_err(local_error)?;
        session.renew_and_validate_database_identity(&database_instance_id)?;
        post_guard_hook()?;
        let preflight_conn = super::maintenance::connect_existing_database_read_only(path)?;
        validate_legacy_cleanup_database_identity(&preflight_conn, &database_instance_id)?;
        legacy_cleanup_preflight(
            &preflight_conn,
            LegacyCleanupOwnerExpectation::Active {
                owner,
                expected: Some(&owner_identity),
            },
        )?;
        drop(preflight_conn);
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

fn validate_legacy_cleanup_database_identity(
    conn: &rusqlite::Connection,
    expected_database_instance_id: &str,
) -> Result<()> {
    let actual = projection_database_instance_id(conn)?;
    if actual != expected_database_instance_id {
        return Err(KanbanError::Conflict(format!(
            "projection database identity changed while legacy cleanup was active: expected {expected_database_instance_id}, got {actual}"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyCleanupOwnerIdentity {
    owner: String,
    lease_token: String,
    capabilities_json: String,
    build_identity: String,
}

type LegacyCleanupOwnerRow = (
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    String,
    Option<String>,
);

#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyCleanupPreflight {
    database_instance_id: String,
    owner_identity: Option<LegacyCleanupOwnerIdentity>,
}

#[derive(Debug, Clone, Copy)]
enum LegacyCleanupOwnerExpectation<'owner> {
    Idle,
    Available,
    Active {
        owner: &'owner str,
        expected: Option<&'owner LegacyCleanupOwnerIdentity>,
    },
}

#[derive(Debug, Clone, Default)]
struct LegacyCleanupRoleBinding {
    generation: Option<String>,
    fingerprint: Option<String>,
    fence_epoch: Option<i64>,
    snapshot_cursor: Option<i64>,
    provider: Option<String>,
    provider_fingerprint: Option<String>,
    canonical_count: Option<i64>,
    canonical_digest: Option<String>,
    delivery_count: Option<i64>,
    delivery_digest: Option<String>,
    corpus_schema: Option<String>,
    corpus_fingerprint: Option<String>,
    embedding_model: Option<String>,
    embedding_dimensions: Option<i64>,
}

impl LegacyCleanupRoleBinding {
    fn from_row(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Self> {
        Ok(Self {
            generation: row.get(offset)?,
            fingerprint: row.get(offset + 1)?,
            fence_epoch: row.get(offset + 2)?,
            snapshot_cursor: row.get(offset + 3)?,
            provider: row.get(offset + 4)?,
            provider_fingerprint: row.get(offset + 5)?,
            canonical_count: row.get(offset + 6)?,
            canonical_digest: row.get(offset + 7)?,
            delivery_count: row.get(offset + 8)?,
            delivery_digest: row.get(offset + 9)?,
            corpus_schema: row.get(offset + 10)?,
            corpus_fingerprint: row.get(offset + 11)?,
            embedding_model: row.get(offset + 12)?,
            embedding_dimensions: row.get(offset + 13)?,
        })
    }

    fn from_building_row(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Self> {
        let mut binding = Self::from_row(row, offset)?;
        let building_phase: Option<String> = row.get(offset + 14)?;
        binding.snapshot_cursor = match building_phase.as_deref() {
            // `snapshot_cursor` at this row offset is store-wide. A
            // snapshotting building generation has no role cursor yet.
            Some("snapshotting") => None,
            // Prepared and store-published generations deliberately bind to
            // the store-wide cursor captured when the snapshot began.
            Some("prepared" | "store_published") => binding.snapshot_cursor,
            // An absent generation must not inherit ordinary store metadata.
            None if binding.generation.is_none() => None,
            _ => binding.snapshot_cursor,
        };
        Ok(binding)
    }

    fn is_empty(&self) -> bool {
        self.generation.is_none()
            && self.fingerprint.is_none()
            && self.fence_epoch.is_none()
            && self.snapshot_cursor.is_none()
            && self.provider.is_none()
            && self.provider_fingerprint.is_none()
            && self.canonical_count.is_none()
            && self.canonical_digest.is_none()
            && self.delivery_count.is_none()
            && self.delivery_digest.is_none()
            && self.corpus_schema.is_none()
            && self.corpus_fingerprint.is_none()
            && self.embedding_model.is_none()
            && self.embedding_dimensions.is_none()
    }

    fn corpus_is_empty(&self) -> bool {
        self.corpus_schema.is_none()
            && self.corpus_fingerprint.is_none()
            && self.embedding_model.is_none()
            && self.embedding_dimensions.is_none()
    }
}

#[derive(Debug, Clone)]
struct LegacyCleanupStoreRow {
    store_name: String,
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    control_plane: String,
    active: LegacyCleanupRoleBinding,
    previous: LegacyCleanupRoleBinding,
    building: LegacyCleanupRoleBinding,
    building_phase: Option<String>,
    snapshot_cursor: i64,
    checkpoint_cursor: i64,
    legacy_checkpoint_cursor: i64,
    lifecycle_status: String,
    lease_expires_at: Option<i64>,
}

impl LegacyCleanupStoreRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            store_name: row.get(0)?,
            database_instance_id: row.get(1)?,
            protocol_version: row.get(2)?,
            schema_version: row.get(3)?,
            control_plane: row.get(4)?,
            active: LegacyCleanupRoleBinding::from_row(row, 5)?,
            previous: LegacyCleanupRoleBinding::from_row(row, 19)?,
            building: LegacyCleanupRoleBinding::from_building_row(row, 33)?,
            building_phase: row.get(47)?,
            snapshot_cursor: row.get(48)?,
            checkpoint_cursor: row.get(49)?,
            legacy_checkpoint_cursor: row.get(50)?,
            lifecycle_status: row.get(51)?,
            lease_expires_at: row.get(52)?,
        })
    }
}

fn legacy_cleanup_preflight(
    conn: &rusqlite::Connection,
    owner_expectation: LegacyCleanupOwnerExpectation<'_>,
) -> Result<LegacyCleanupPreflight> {
    // Keep database identity, owner, store leases, generation roles, and
    // delivery coverage on one SQLite read snapshot. The connection remains
    // lifecycle-guarded by the caller; this transaction prevents the
    // preflight itself from mixing rows across concurrent writer commits.
    let transaction = conn.unchecked_transaction().map_err(super::storage)?;
    let conn = &*transaction;
    let (database_instance_id, protocol_version): (String, i64) = conn
        .query_row(
            "SELECT database_instance_id,protocol_version
             FROM projection_database WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(super::storage)?;
    if protocol_version != 2 {
        return Err(KanbanError::InvalidInput(format!(
            "legacy cleanup requires Projection v2 protocol 2, found {protocol_version}"
        )));
    }
    if database_instance_id.trim().is_empty() {
        return Err(KanbanError::InvalidInput(
            "legacy cleanup found an empty Projection v2 database identity".to_owned(),
        ));
    }

    let owner_identity = validate_maintenance_owner(conn, owner_expectation)?;

    let mut statement = conn
        .prepare(
            "SELECT store_name,database_instance_id,protocol_version,schema_version,
                    control_plane,
                    active_generation,active_fingerprint,active_fence_epoch,
                    active_snapshot_cursor,active_provider,active_provider_fingerprint,
                    active_canonical_count,active_canonical_digest,
                    active_delivery_count,active_delivery_digest,
                    active_corpus_schema,active_corpus_fingerprint,
                    active_embedding_model,active_embedding_dimensions,
                    previous_generation,previous_fingerprint,previous_fence_epoch,
                    previous_snapshot_cursor,previous_provider,previous_provider_fingerprint,
                    previous_canonical_count,previous_canonical_digest,
                    previous_delivery_count,previous_delivery_digest,
                    previous_corpus_schema,previous_corpus_fingerprint,
                    previous_embedding_model,previous_embedding_dimensions,
                    building_generation,building_fingerprint,building_fence_epoch,
                    snapshot_cursor,
                    building_provider,building_provider_fingerprint,
                    building_canonical_count,building_canonical_digest,
                    building_delivery_count,building_delivery_digest,
                    building_corpus_schema,building_corpus_fingerprint,
                    building_embedding_model,building_embedding_dimensions,
                    building_phase,
                    snapshot_cursor,checkpoint_cursor,legacy_checkpoint_cursor,
                    lifecycle_status,lease_expires_at
             FROM projection_store_state ORDER BY store_name",
        )
        .map_err(super::storage)?;
    let rows = statement
        .query_map([], LegacyCleanupStoreRow::from_row)
        .map_err(super::storage)?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let row = row.map_err(super::storage)?;
        if !seen.insert(row.store_name.clone()) {
            return Err(legacy_cleanup_metadata_error(format!(
                "duplicate projection store row {}",
                row.store_name
            )));
        }
        validate_cleanup_store_row(&row, &database_instance_id, conn)?;
    }
    let expected = DERIVED_STORE_SEEDS
        .iter()
        .map(|seed| seed.store_name.to_owned())
        .collect::<BTreeSet<_>>();
    if seen != expected {
        let missing = expected.difference(&seen).cloned().collect::<Vec<_>>();
        let unexpected = seen
            .difference(&expected)
            .map(String::as_str)
            .collect::<Vec<_>>();
        return Err(legacy_cleanup_metadata_error(format!(
            "projection store set is not exactly the four approved stores (missing: {}; unexpected: {})",
            if missing.is_empty() {
                "none".to_owned()
            } else {
                missing.join(",")
            },
            if unexpected.is_empty() {
                "none".to_owned()
            } else {
                unexpected.join(",")
            }
        )));
    }
    Ok(LegacyCleanupPreflight {
        database_instance_id,
        owner_identity,
    })
}

fn validate_maintenance_owner(
    conn: &rusqlite::Connection,
    expectation: LegacyCleanupOwnerExpectation<'_>,
) -> Result<Option<LegacyCleanupOwnerIdentity>> {
    let (
        owner,
        lease_token,
        lease_expires_at,
        mode,
        started_at,
        last_heartbeat_at,
        capabilities_json,
        build_identity,
    ): LegacyCleanupOwnerRow = conn
        .query_row(
            "SELECT owner,lease_token,lease_expires_at,mode,started_at,
                    last_heartbeat_at,capabilities_json,build_identity
             FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .map_err(super::storage)?;

    match expectation {
        LegacyCleanupOwnerExpectation::Idle => {
            if owner.is_some()
                || lease_token.is_some()
                || lease_expires_at.is_some()
                || mode.is_some()
                || started_at.is_some()
                || last_heartbeat_at.is_some()
                || capabilities_json.trim() != "[]"
                || build_identity.is_some()
            {
                return Err(legacy_cleanup_runtime_error(
                    "inventory requires an idle maintenance owner".to_owned(),
                ));
            }
            Ok(None)
        }
        LegacyCleanupOwnerExpectation::Available => {
            let exactly_idle = owner.is_none()
                && lease_token.is_none()
                && lease_expires_at.is_none()
                && mode.is_none()
                && started_at.is_none()
                && last_heartbeat_at.is_none()
                && capabilities_json.trim() == "[]"
                && build_identity.is_none();
            if exactly_idle {
                return Ok(None);
            }

            let now = SystemClock.now_ms();
            let structurally_valid_expired_owner = owner
                .as_deref()
                .is_some_and(|owner| !owner.trim().is_empty())
                && lease_token
                    .as_deref()
                    .is_some_and(|lease_token| !lease_token.trim().is_empty())
                && lease_expires_at.is_some_and(|expires_at| expires_at <= now)
                && mode
                    .as_deref()
                    .is_some_and(|mode| matches!(mode, "once" | "continuous"))
                && started_at.is_some()
                && last_heartbeat_at.is_some()
                && build_identity
                    .as_deref()
                    .is_some_and(|build_identity| !build_identity.trim().is_empty());
            if !structurally_valid_expired_owner {
                return Err(legacy_cleanup_runtime_error(
                    "action requires an idle or structurally valid expired maintenance owner"
                        .to_owned(),
                ));
            }
            validate_capabilities_json(&capabilities_json, false)?;
            Ok(Some(LegacyCleanupOwnerIdentity {
                owner: owner.unwrap_or_default(),
                lease_token: lease_token.unwrap_or_default(),
                capabilities_json,
                build_identity: build_identity.unwrap_or_default(),
            }))
        }
        LegacyCleanupOwnerExpectation::Active {
            owner: expected_owner,
            expected,
        } => {
            let now = SystemClock.now_ms();
            if expected_owner.trim().is_empty()
                || owner.as_deref() != Some(expected_owner)
                || lease_token.as_deref().is_none_or(str::is_empty)
                || lease_expires_at.is_none_or(|expires_at| expires_at <= now)
                || mode.as_deref() != Some("once")
                || started_at.is_none()
                || last_heartbeat_at.is_none()
                || build_identity.as_deref().is_none_or(str::is_empty)
            {
                return Err(legacy_cleanup_runtime_error(
                    "action requires the exact active maintenance session owner".to_owned(),
                ));
            }
            let actual = LegacyCleanupOwnerIdentity {
                owner: owner.clone().unwrap_or_default(),
                lease_token: lease_token.clone().unwrap_or_default(),
                capabilities_json: capabilities_json.clone(),
                build_identity: build_identity.clone().unwrap_or_default(),
            };
            if let Some(expected) = expected
                && &actual != expected
            {
                return Err(KanbanError::Conflict(
                    "projection maintenance owner lease is stale or identity changed".to_owned(),
                ));
            }
            validate_capabilities_json(&capabilities_json, false)?;
            Ok(Some(actual))
        }
    }
}

fn validate_capabilities_json(capabilities_json: &str, idle: bool) -> Result<()> {
    let capabilities: Vec<String> = serde_json::from_str(capabilities_json).map_err(|error| {
        legacy_cleanup_runtime_error(format!(
            "maintenance runtime capabilities are invalid: {error}"
        ))
    })?;
    if capabilities
        .iter()
        .any(|capability| capability.trim().is_empty())
        || capabilities.windows(2).any(|pair| pair[0] >= pair[1])
        || (!idle && capabilities.is_empty())
    {
        return Err(legacy_cleanup_runtime_error(
            "maintenance runtime capabilities are not a canonical non-empty set".to_owned(),
        ));
    }
    Ok(())
}

fn validate_cleanup_store_row(
    row: &LegacyCleanupStoreRow,
    database_instance_id: &str,
    conn: &rusqlite::Connection,
) -> Result<()> {
    let now = SystemClock.now_ms();
    if row
        .lease_expires_at
        .is_some_and(|expires_at| expires_at > now)
    {
        return Err(legacy_cleanup_runtime_error(format!(
            "projection store {} has an active lease",
            row.store_name
        )));
    }
    if row.database_instance_id != database_instance_id
        || row.protocol_version != 2
        || row.schema_version != DERIVED_STORE_SCHEMA_VERSION
        || row.snapshot_cursor < 0
        || row.checkpoint_cursor < 0
        || row.legacy_checkpoint_cursor < 0
        || !matches!(
            row.lifecycle_status.as_str(),
            "bootstrap_required" | "idle" | "rebuilding" | "ready" | "error"
        )
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "incompatible Projection v2 store metadata for {}",
            row.store_name
        )));
    }
    if row.control_plane == "legacy" {
        if !row.active.is_empty()
            || !row.previous.is_empty()
            || !row.building.is_empty()
            || row.building_phase.is_some()
        {
            return Err(legacy_cleanup_metadata_error(format!(
                "legacy control plane has v2 generation metadata for {}",
                row.store_name
            )));
        }
        return Ok(());
    }
    if row.control_plane != "v2"
        || (row.active.is_empty() && row.previous.is_empty() && row.building.is_empty())
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {} has no valid v2 generation or has an unknown control plane",
            row.store_name
        )));
    }
    validate_cleanup_role(&row.store_name, "active", &row.active, None)?;
    validate_cleanup_role(&row.store_name, "previous", &row.previous, None)?;
    validate_cleanup_role(
        &row.store_name,
        "building",
        &row.building,
        row.building_phase.as_deref(),
    )?;
    if row.active.generation.as_deref().is_some_and(|generation| {
        row.previous.generation.as_deref() == Some(generation)
            || row.building.generation.as_deref() == Some(generation)
    }) || row
        .previous
        .generation
        .as_deref()
        .is_some_and(|generation| row.building.generation.as_deref() == Some(generation))
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {} reuses a generation across roles",
            row.store_name
        )));
    }
    if row.building.generation.is_some() {
        let (delivery_count, delivery_digest, max_delivery_cursor) =
            cleanup_delivery_snapshot_coverage(conn, &row.store_name, row.snapshot_cursor)?;
        if row.snapshot_cursor != max_delivery_cursor {
            return Err(legacy_cleanup_metadata_error(format!(
                "store {} building snapshot cursor {} drifted from delivery cursor {}",
                row.store_name, row.snapshot_cursor, max_delivery_cursor
            )));
        }
        if row.building.delivery_count != Some(delivery_count)
            || row.building.delivery_digest.as_deref() != Some(delivery_digest.as_str())
        {
            return Err(legacy_cleanup_metadata_error(format!(
                "store {} building snapshot cursor coverage changed",
                row.store_name
            )));
        }
    }
    Ok(())
}

fn cleanup_delivery_snapshot_coverage(
    conn: &rusqlite::Connection,
    store_name: &str,
    snapshot_cursor: i64,
) -> Result<(i64, String, i64)> {
    let max_delivery_cursor: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(cursor),0)
             FROM projection_deliveries WHERE store_name=?1",
            [store_name],
            |row| row.get(0),
        )
        .map_err(super::storage)?;
    let mut statement = conn
        .prepare(
            "SELECT id,outbox_id,board_id,source_event_id,cursor,action,
                    entity_uri,payload_json
             FROM projection_deliveries
             WHERE store_name=?1 AND cursor<=?2
             ORDER BY cursor,id",
        )
        .map_err(super::storage)?;
    let rows = statement
        .query_map(rusqlite::params![store_name, snapshot_cursor], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(super::storage)?;
    let mut count = 0_i64;
    let mut hash = 0xcbf29ce484222325_u64;
    for row in rows {
        let (id, outbox_id, board_id, source_event_id, cursor, action, entity_uri, payload_json) =
            row.map_err(super::storage)?;
        count += 1;
        cleanup_coverage_hash_bytes(&mut hash, &id.to_le_bytes());
        cleanup_coverage_hash_bytes(&mut hash, &outbox_id.to_le_bytes());
        cleanup_coverage_hash_bytes(&mut hash, board_id.as_bytes());
        match source_event_id {
            Some(value) => {
                cleanup_coverage_hash_bytes(&mut hash, &[1]);
                cleanup_coverage_hash_bytes(&mut hash, &value.to_le_bytes());
            }
            None => cleanup_coverage_hash_bytes(&mut hash, &[0]),
        }
        cleanup_coverage_hash_bytes(&mut hash, &cursor.to_le_bytes());
        cleanup_coverage_hash_bytes(&mut hash, action.as_bytes());
        cleanup_coverage_hash_bytes(&mut hash, entity_uri.as_bytes());
        cleanup_coverage_hash_bytes(&mut hash, payload_json.as_bytes());
    }
    Ok((count, format!("fnv64:{hash:016x}"), max_delivery_cursor))
}

fn cleanup_coverage_hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn validate_cleanup_role(
    store_name: &str,
    role: &str,
    binding: &LegacyCleanupRoleBinding,
    building_phase: Option<&str>,
) -> Result<()> {
    let Some(generation) = binding.generation.as_deref() else {
        if !binding.is_empty() || building_phase.is_some() {
            return Err(legacy_cleanup_metadata_error(format!(
                "store {store_name} has orphan {role} generation metadata"
            )));
        }
        return Ok(());
    };
    if generation.trim().is_empty() || !generation.starts_with("gen_") {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has an invalid {role} generation id"
        )));
    }
    let snapshotting = building_phase == Some("snapshotting");
    if role == "building"
        && !matches!(
            building_phase,
            Some("snapshotting" | "prepared" | "store_published")
        )
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has an unknown building phase"
        )));
    }
    if role != "building" && building_phase.is_some() {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has a building phase without building metadata"
        )));
    }
    if (!snapshotting && binding.fingerprint.is_none())
        || binding.fence_epoch.is_none()
        || (!snapshotting && binding.snapshot_cursor.is_none())
        || binding.provider.is_none()
        || binding.provider_fingerprint.is_none()
        || binding.canonical_count.is_none()
        || binding.canonical_digest.is_none()
        || binding.delivery_count.is_none()
        || binding.delivery_digest.is_none()
        || (snapshotting && (binding.fingerprint.is_some() || binding.snapshot_cursor.is_some()))
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has incomplete {role} generation manifest"
        )));
    }
    if binding.fence_epoch.is_some_and(|value| value < 0)
        || binding.snapshot_cursor.is_some_and(|value| value < 0)
        || binding.canonical_count.is_some_and(|value| value < 0)
        || binding.delivery_count.is_some_and(|value| value < 0)
        || binding.fingerprint.as_deref().is_some_and(str::is_empty)
        || binding.provider.as_deref().is_some_and(str::is_empty)
        || binding
            .provider_fingerprint
            .as_deref()
            .is_some_and(str::is_empty)
        || binding
            .canonical_digest
            .as_deref()
            .is_some_and(str::is_empty)
        || binding
            .delivery_digest
            .as_deref()
            .is_some_and(str::is_empty)
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has invalid {role} generation manifest values"
        )));
    }
    let provider = binding.provider.as_deref().unwrap_or_default();
    let provider_fingerprint = binding.provider_fingerprint.as_deref().unwrap_or_default();
    let expected = match store_name {
        TANTIVY_TASKS_STORE => Some((TANTIVY_PROVIDER, TANTIVY_PROVIDER_FINGERPRINT)),
        OXIGRAPH_RELATIONS_STORE => Some((OXIGRAPH_PROVIDER, OXIGRAPH_PROVIDER_FINGERPRINT)),
        LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE => None,
        _ => {
            return Err(legacy_cleanup_metadata_error(format!(
                "unknown projection store {store_name}"
            )));
        }
    };
    if expected.is_some_and(|(expected_provider, expected_fingerprint)| {
        provider != expected_provider || provider_fingerprint != expected_fingerprint
    }) {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has an incompatible {role} provider binding"
        )));
    }
    validate_cleanup_corpus_binding(store_name, role, binding)
}

fn validate_cleanup_corpus_binding(
    store_name: &str,
    role: &str,
    binding: &LegacyCleanupRoleBinding,
) -> Result<()> {
    let corpus_presence = [
        binding.corpus_schema.is_some(),
        binding.corpus_fingerprint.is_some(),
        binding.embedding_model.is_some(),
        binding.embedding_dimensions.is_some(),
    ];
    if corpus_presence.iter().any(|present| *present)
        && !corpus_presence.iter().all(|present| *present)
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has incomplete {role} corpus binding"
        )));
    }
    if binding.corpus_is_empty() {
        return match store_name {
            TANTIVY_TASKS_STORE | OXIGRAPH_RELATIONS_STORE => Ok(()),
            LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE => Err(
                legacy_cleanup_corpus_binding_upgrade_error(store_name, role),
            ),
            _ => Err(legacy_cleanup_metadata_error(format!(
                "unknown projection store {store_name}"
            ))),
        };
    }
    let expected_schema = match store_name {
        LANCEDB_CHUNKS_STORE => TASK_CHUNKS_CORPUS_SCHEMA,
        LANCEDB_LABEL_ATOMS_STORE => LABEL_ATOMS_CORPUS_SCHEMA,
        _ => {
            return Err(legacy_cleanup_metadata_error(format!(
                "store {store_name} has an unexpected {role} corpus binding"
            )));
        }
    };
    let schema = binding.corpus_schema.as_deref().unwrap_or_default();
    let provider = binding.provider.as_deref().unwrap_or_default();
    let provider_fingerprint = binding.provider_fingerprint.as_deref().unwrap_or_default();
    let model = binding.embedding_model.as_deref().unwrap_or_default();
    let dimensions = binding.embedding_dimensions.unwrap_or_default();
    let dimensions = usize::try_from(dimensions).ok();
    let Some(dimensions) = dimensions.filter(|dimensions| *dimensions > 0) else {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has invalid {role} embedding dimensions"
        )));
    };
    let expected_corpus_fingerprint =
        corpus_provider_fingerprint(expected_schema, provider_fingerprint);
    let expected_provider_fingerprint = embedding_provider_fingerprint(provider, model, dimensions);
    if schema != expected_schema
        || binding.corpus_fingerprint.as_deref() != Some(expected_corpus_fingerprint.as_str())
        || provider.trim().is_empty()
        || model.trim().is_empty()
        || provider_fingerprint != expected_provider_fingerprint
    {
        return Err(legacy_cleanup_metadata_error(format!(
            "store {store_name} has an incompatible {role} provider/corpus binding"
        )));
    }
    Ok(())
}

fn legacy_cleanup_metadata_error(message: String) -> KanbanError {
    KanbanError::InvalidInput(format!("legacy cleanup found {message}"))
}

fn legacy_cleanup_corpus_binding_upgrade_error(store_name: &str, role: &str) -> KanbanError {
    KanbanError::InvalidInput(format!(
        "corpus_binding_upgrade_required: legacy cleanup refuses {role} generation without a complete LanceDB corpus binding for {store_name}"
    ))
}

fn legacy_cleanup_runtime_error(message: String) -> KanbanError {
    KanbanError::InvalidInput(format!("legacy cleanup found incompatible {message}"))
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
        LegacyProjectionCleanupError::UnsupportedMutationPlatform
        | LegacyProjectionCleanupError::UnsafePath { .. }
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

    #[derive(Debug, PartialEq, Eq)]
    struct CleanupSqliteSnapshot {
        sqlite_authority: Vec<(String, Vec<Vec<String>>)>,
        database_authority: Vec<Vec<String>>,
        store_authority: Vec<Vec<String>>,
        owner_authority: Vec<Vec<String>>,
        outbox: Vec<Vec<String>>,
        deliveries: Vec<Vec<String>>,
        dirty_state: Vec<Vec<String>>,
        label_dirty_state: Vec<Vec<String>>,
        watermarks: Vec<Vec<String>>,
    }

    fn cleanup_sqlite_snapshot(path: &Path) -> anyhow::Result<CleanupSqliteSnapshot> {
        let conn = connect_file(path)?;
        Ok(CleanupSqliteSnapshot {
            sqlite_authority: sqlite_authority_snapshot(&conn)?,
            database_authority: query_snapshot(
                &conn,
                "SELECT * FROM projection_database ORDER BY singleton",
            )?,
            store_authority: query_snapshot(
                &conn,
                "SELECT * FROM projection_store_state ORDER BY store_name",
            )?,
            // start/heartbeat/updated timestamps are intentional action lease
            // bookkeeping. The semantic owner authority must return to idle.
            owner_authority: query_snapshot(
                &conn,
                "SELECT owner,lease_token,lease_expires_at,mode,
                        capabilities_json,build_identity
                 FROM projection_maintenance_owner ORDER BY singleton",
            )?,
            outbox: query_snapshot(&conn, "SELECT * FROM index_outbox ORDER BY id")?,
            deliveries: query_snapshot(&conn, "SELECT * FROM projection_deliveries ORDER BY id")?,
            dirty_state: query_snapshot(
                &conn,
                "SELECT * FROM derived_store_state ORDER BY store_name",
            )?,
            label_dirty_state: query_snapshot(
                &conn,
                "SELECT * FROM label_atom_index_boards ORDER BY store_name,board_id",
            )?,
            watermarks: query_snapshot(
                &conn,
                "SELECT store_name,snapshot_cursor,checkpoint_cursor,
                        legacy_checkpoint_cursor
                 FROM projection_store_state ORDER BY store_name",
            )?,
        })
    }

    fn sqlite_authority_snapshot(
        conn: &rusqlite::Connection,
    ) -> anyhow::Result<Vec<(String, Vec<Vec<String>>)>> {
        let mut statement = conn.prepare(
            "SELECT name FROM sqlite_master
             WHERE type='table' AND name!='projection_maintenance_owner'
             ORDER BY name",
        )?;
        let tables = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut snapshot = Vec::with_capacity(tables.len());
        for table in tables {
            let quoted_table = table.replace('"', "\"\"");
            let mut rows = query_snapshot(conn, &format!("SELECT * FROM \"{quoted_table}\""))?;
            rows.sort();
            snapshot.push((table, rows));
        }
        Ok(snapshot)
    }

    fn query_snapshot(conn: &rusqlite::Connection, sql: &str) -> anyhow::Result<Vec<Vec<String>>> {
        let mut statement = conn.prepare(sql)?;
        let column_count = statement.column_count();
        Ok(statement
            .query_map([], |row| sqlite_row_snapshot(row, column_count))?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn sqlite_row_snapshot(
        row: &rusqlite::Row<'_>,
        column_count: usize,
    ) -> rusqlite::Result<Vec<String>> {
        use rusqlite::types::ValueRef;

        (0..column_count)
            .map(|index| {
                Ok(match row.get_ref(index)? {
                    ValueRef::Null => "null".to_owned(),
                    ValueRef::Integer(value) => format!("integer:{value}"),
                    ValueRef::Real(value) => format!("real:{value:?}"),
                    ValueRef::Text(value) => {
                        format!("text:{}", String::from_utf8_lossy(value))
                    }
                    ValueRef::Blob(value) => format!("blob:{value:?}"),
                })
            })
            .collect()
    }

    fn cleanup_physical_snapshot(
        database_root: &Path,
        backup_dir: &Path,
    ) -> anyhow::Result<Vec<(String, String, Vec<u8>)>> {
        let mut snapshot = Vec::new();
        collect_physical_tree(
            &database_root.join("index"),
            "database-index",
            &mut snapshot,
        )?;
        collect_physical_tree(backup_dir, "backup", &mut snapshot)?;
        snapshot.sort();
        Ok(snapshot)
    }

    fn cleanup_database_file_snapshot(
        database_path: &Path,
    ) -> anyhow::Result<Vec<(String, Option<Vec<u8>>)>> {
        ["", "-journal", "-wal", "-shm"]
            .into_iter()
            .map(|suffix| {
                let path = std::path::PathBuf::from(format!(
                    "{}{suffix}",
                    database_path.to_string_lossy()
                ));
                let contents = match std::fs::read(&path) {
                    Ok(contents) => Some(contents),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                Ok((suffix.to_owned(), contents))
            })
            .collect()
    }

    fn read_only_journal_mode(path: &Path) -> anyhow::Result<String> {
        let conn = super::super::maintenance::connect_existing_database_read_only(path)?;
        Ok(conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    fn set_delete_journal_mode(path: &Path) -> anyhow::Result<()> {
        let conn = connect_file(path)?;
        let _: (i64, i64, i64) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mode: String = conn.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))?;
        anyhow::ensure!(mode == "delete", "fixture must enter DELETE journal mode");
        drop(conn);
        Ok(())
    }

    fn seed_maintenance_owner(
        path: &Path,
        owner: &str,
        lease_expires_at: i64,
    ) -> anyhow::Result<()> {
        let now = SystemClock.now_ms();
        connect_file(path)?.execute(
            "UPDATE projection_maintenance_owner
             SET owner=?1,lease_token='pmlease_fixture',
                 lease_expires_at=?2,mode='once',started_at=?3,
                 last_heartbeat_at=?3,
                 capabilities_json='[\"legacy_cleanup\"]',
                 build_identity='legacy-cleanup-fixture-build',
                 updated_at=?3
             WHERE singleton=1",
            rusqlite::params![owner, lease_expires_at, now],
        )?;
        Ok(())
    }

    fn rewrite_completed_cleanup_journal_as_applying(backup_dir: &Path) -> anyhow::Result<()> {
        let journal_path = backup_dir.join("journal.toml");
        let completed = std::fs::read_to_string(&journal_path)?;
        let applying = completed.replacen("phase = \"completed\"", "phase = \"applying\"", 1);
        anyhow::ensure!(
            applying != completed,
            "fixture must rewrite the completed journal phase"
        );
        std::fs::write(journal_path, applying)?;
        Ok(())
    }

    fn collect_physical_tree(
        path: &Path,
        label: &str,
        snapshot: &mut Vec<(String, String, Vec<u8>)>,
    ) -> anyhow::Result<()> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                snapshot.push((label.to_owned(), "missing".to_owned(), Vec::new()));
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        if metadata.is_file() {
            snapshot.push((label.to_owned(), "file".to_owned(), std::fs::read(path)?));
            return Ok(());
        }
        if !metadata.is_dir() {
            snapshot.push((label.to_owned(), "unsupported".to_owned(), Vec::new()));
            return Ok(());
        }
        snapshot.push((label.to_owned(), "directory".to_owned(), Vec::new()));
        let mut children = std::fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_label = format!("{label}/{}", child.file_name().to_string_lossy());
            collect_physical_tree(&child.path(), &child_label, snapshot)?;
        }
        Ok(())
    }

    fn seed_cleanup_control_evidence(path: &Path, store_name: &str) -> anyhow::Result<()> {
        let conn = connect_file(path)?;
        let board_id: String =
            conn.query_row("SELECT id FROM boards ORDER BY id LIMIT 1", [], |row| {
                row.get(0)
            })?;
        conn.execute(
            "UPDATE derived_store_state
             SET dirty=1,last_event_id=17,last_sync_at=19,
                 last_error='preserve-dirty-evidence',updated_at=23
             WHERE store_name=?1",
            [store_name],
        )?;
        conn.execute(
            "UPDATE projection_store_state
             SET snapshot_cursor=11,checkpoint_cursor=7,
                 legacy_checkpoint_cursor=5
             WHERE store_name=?1",
            [store_name],
        )?;
        conn.execute(
            "INSERT INTO label_atom_index_boards(
                 store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
             ) VALUES (
                 ?1,?2,1,NULL,'preserve-label-dirty-evidence',29
             )",
            rusqlite::params![LANCEDB_LABEL_ATOMS_STORE, board_id],
        )?;
        Ok(())
    }

    fn seed_unbound_v29_lance_role(
        path: &Path,
        store_name: &str,
        role: &str,
    ) -> anyhow::Result<()> {
        let conn = connect_file(path)?;
        let guard = match role {
            "active" => "projection_active_corpus_generation_guard",
            "previous" => "projection_previous_corpus_generation_guard",
            "building" => "projection_building_corpus_generation_guard",
            _ => unreachable!("fixed cleanup role"),
        };
        // Migration 030 deliberately carries forward already-unbound v29
        // rows, while its update trigger prevents manufacturing a new one.
        // Dropping the phase guard in this disposable fixture recreates that
        // exact persisted v29 row shape.
        conn.execute_batch(&format!("DROP TRIGGER {guard};"))?;
        match role {
            "active" | "previous" => {
                let sql = format!(
                    "UPDATE projection_store_state
                     SET control_plane='v2',lifecycle_status='ready',snapshot_cursor=11,
                         {role}_generation=?1,{role}_fingerprint=?2,
                         {role}_fence_epoch=31,{role}_snapshot_cursor=11,
                         {role}_provider='legacy-lance-provider',
                         {role}_provider_fingerprint='legacy-provider-fingerprint',
                         {role}_canonical_count=3,
                         {role}_canonical_digest='fnv64:v29-canonical',
                         {role}_delivery_count=2,
                         {role}_delivery_digest='fnv64:v29-delivery'
                     WHERE store_name=?3"
                );
                conn.execute(
                    &sql,
                    rusqlite::params![
                        format!("gen_v29_{role}"),
                        format!("sha256:v29-{role}"),
                        store_name
                    ],
                )?;
            }
            "building" => {
                conn.execute(
                    "UPDATE projection_store_state
                     SET control_plane='v2',lifecycle_status='rebuilding',
                         building_generation='gen_v29_building',
                         building_fingerprint='sha256:v29-building',
                         building_fence_epoch=31,
                         building_provider='legacy-lance-provider',
                         building_provider_fingerprint='legacy-provider-fingerprint',
                         building_canonical_count=3,
                         building_canonical_digest='fnv64:v29-canonical',
                         building_delivery_count=2,
                         building_delivery_digest='fnv64:v29-delivery',
                         building_phase='prepared'
                     WHERE store_name=?1",
                    [store_name],
                )?;
            }
            _ => unreachable!("fixed cleanup role"),
        }
        Ok(())
    }

    fn seed_bound_lance_role(
        path: &Path,
        store_name: &str,
        role: &str,
        building_phase: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = connect_file(path)?;
        let provider = "deterministic-test-provider";
        let model = "deterministic-test-model";
        let dimensions = 8_usize;
        let provider_fingerprint = embedding_provider_fingerprint(provider, model, dimensions);
        let corpus_schema = match store_name {
            LANCEDB_CHUNKS_STORE => TASK_CHUNKS_CORPUS_SCHEMA,
            LANCEDB_LABEL_ATOMS_STORE => LABEL_ATOMS_CORPUS_SCHEMA,
            _ => unreachable!("fixed LanceDB store"),
        };
        let corpus_fingerprint = corpus_provider_fingerprint(corpus_schema, &provider_fingerprint);
        match role {
            "active" | "previous" => {
                let sql = format!(
                    "UPDATE projection_store_state
                     SET control_plane='v2',lifecycle_status='ready',
                         {role}_generation=?1,{role}_fingerprint=?2,
                         {role}_fence_epoch=37,{role}_snapshot_cursor=11,
                         {role}_provider=?3,{role}_provider_fingerprint=?4,
                         {role}_canonical_count=3,
                         {role}_canonical_digest='fnv64:bound-canonical',
                         {role}_delivery_count=2,
                         {role}_delivery_digest='fnv64:bound-delivery',
                         {role}_corpus_schema=?5,{role}_corpus_fingerprint=?6,
                         {role}_embedding_model=?7,{role}_embedding_dimensions=?8
                     WHERE store_name=?9"
                );
                conn.execute(
                    &sql,
                    rusqlite::params![
                        format!("gen_bound_{role}"),
                        format!("sha256:bound-{role}"),
                        provider,
                        provider_fingerprint,
                        corpus_schema,
                        corpus_fingerprint,
                        model,
                        i64::try_from(dimensions)?,
                        store_name,
                    ],
                )?;
            }
            "building" => {
                let phase = building_phase.expect("building phase");
                let fingerprint = (phase != "snapshotting").then_some("sha256:bound-building");
                conn.execute(
                    "UPDATE projection_store_state
                     SET control_plane='v2',lifecycle_status='rebuilding',snapshot_cursor=0,
                         building_generation='gen_bound_building',
                         building_fingerprint=?1,building_fence_epoch=37,
                         building_provider=?2,building_provider_fingerprint=?3,
                         building_canonical_count=3,
                         building_canonical_digest='fnv64:bound-canonical',
                         building_delivery_count=0,
                         building_delivery_digest='fnv64:cbf29ce484222325',
                         building_corpus_schema=?4,
                         building_corpus_fingerprint=?5,
                         building_embedding_model=?6,
                         building_embedding_dimensions=?7,
                         building_phase=?8
                     WHERE store_name=?9",
                    rusqlite::params![
                        fingerprint,
                        provider,
                        provider_fingerprint,
                        corpus_schema,
                        corpus_fingerprint,
                        model,
                        i64::try_from(dimensions)?,
                        phase,
                        store_name,
                    ],
                )?;
            }
            _ => unreachable!("fixed cleanup role"),
        }
        Ok(())
    }

    fn seed_bound_snapshotting_role(path: &Path, store_name: &str) -> anyhow::Result<()> {
        let conn = connect_file(path)?;
        let (provider, provider_fingerprint) = match store_name {
            TANTIVY_TASKS_STORE => (TANTIVY_PROVIDER, TANTIVY_PROVIDER_FINGERPRINT),
            OXIGRAPH_RELATIONS_STORE => (OXIGRAPH_PROVIDER, OXIGRAPH_PROVIDER_FINGERPRINT),
            _ => unreachable!("non-Lance snapshotting fixture store"),
        };
        conn.execute(
            "UPDATE projection_store_state
             SET control_plane='v2',lifecycle_status='rebuilding',snapshot_cursor=0,
                 building_generation='gen_bound_snapshotting',
                 building_fingerprint=NULL,building_fence_epoch=37,
                 building_provider=?1,building_provider_fingerprint=?2,
                 building_canonical_count=0,
                 building_canonical_digest='fnv64:cbf29ce484222325',
                 building_delivery_count=0,
                 building_delivery_digest='fnv64:cbf29ce484222325',
                 building_phase='snapshotting'
             WHERE store_name=?3",
            rusqlite::params![provider, provider_fingerprint, store_name],
        )?;
        Ok(())
    }

    fn run_cleanup_action(
        action: MaintenanceLegacyCleanupAction,
        database_path: &Path,
        digest: &str,
        backup_dir: &Path,
    ) -> Result<MaintenanceLegacyCleanupReport> {
        match action {
            MaintenanceLegacyCleanupAction::Inventory => {
                maintenance_inventory_legacy_projections(database_path)
            }
            MaintenanceLegacyCleanupAction::Apply => maintenance_apply_legacy_projection_cleanup(
                database_path,
                "cleanup-preflight-test-owner",
                digest,
                backup_dir,
                false,
                MaintenanceRunOptions::default(),
            ),
            MaintenanceLegacyCleanupAction::Verify => maintenance_verify_legacy_projection_cleanup(
                database_path,
                "cleanup-preflight-test-owner",
                backup_dir,
                MaintenanceRunOptions::default(),
            ),
            MaintenanceLegacyCleanupAction::Restore => {
                maintenance_restore_legacy_projection_cleanup(
                    database_path,
                    "cleanup-preflight-test-owner",
                    backup_dir,
                    MaintenanceRunOptions::default(),
                )
            }
        }
    }

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
    fn cleanup_verify_revalidates_exact_owner_identity_after_final_renew_before_primitive()
    -> anyhow::Result<()> {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, _digest) =
            cleanup_fixture("verify-owner-identity-revalidation")?;
        let error = maintenance_verify_legacy_projection_cleanup_with_post_renew_hook(
            &database_path,
            "cleanup-owner",
            &backup_dir,
            MaintenanceRunOptions::default(),
            || {
                connect_file(&database_path)?
                    .execute(
                        "UPDATE projection_maintenance_owner
                     SET capabilities_json='[\"successor-store\"]',
                         build_identity='successor-build'
                     WHERE singleton=1",
                        [],
                    )
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                Ok(())
            },
        )
        .expect_err("same-owner capability/build identity rollover must abort verify");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(error.to_string().contains("owner lease is stale"));
        assert!(legacy_file.is_file(), "verify must not move legacy roots");
        assert!(
            !backup_dir.exists(),
            "verify must not create backup evidence"
        );
        Ok(())
    }

    #[test]
    fn cleanup_apply_restore_verify_reject_each_same_owner_identity_takeover_before_primitive()
    -> anyhow::Result<()> {
        for action in [
            MaintenanceLegacyCleanupAction::Apply,
            MaintenanceLegacyCleanupAction::Restore,
            MaintenanceLegacyCleanupAction::Verify,
        ] {
            for (identity_kind, update_sql) in [
                (
                    "token",
                    "UPDATE projection_maintenance_owner
                     SET lease_token='successor-token' WHERE singleton=1",
                ),
                (
                    "capabilities",
                    "UPDATE projection_maintenance_owner
                     SET capabilities_json='[\"successor-store\"]'
                     WHERE singleton=1",
                ),
                (
                    "build",
                    "UPDATE projection_maintenance_owner
                     SET build_identity='successor-build' WHERE singleton=1",
                ),
            ] {
                let fixture_name = format!("{action:?}-owner-{identity_kind}");
                let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, digest) =
                    cleanup_fixture(&fixture_name)?;
                let result = match action {
                    MaintenanceLegacyCleanupAction::Apply => {
                        maintenance_apply_legacy_projection_cleanup_with_post_guard_hook(
                            &database_path,
                            "cleanup-owner",
                            &digest,
                            &backup_dir,
                            false,
                            MaintenanceRunOptions::default(),
                            || {
                                connect_file(&database_path)?
                                    .execute(update_sql, [])
                                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                                Ok(())
                            },
                        )
                    }
                    MaintenanceLegacyCleanupAction::Restore => {
                        maintenance_restore_legacy_projection_cleanup_with_post_guard_hook(
                            &database_path,
                            "cleanup-owner",
                            &backup_dir,
                            MaintenanceRunOptions::default(),
                            || {
                                connect_file(&database_path)?
                                    .execute(update_sql, [])
                                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                                Ok(())
                            },
                        )
                    }
                    MaintenanceLegacyCleanupAction::Verify => {
                        maintenance_verify_legacy_projection_cleanup_with_post_renew_hook(
                            &database_path,
                            "cleanup-owner",
                            &backup_dir,
                            MaintenanceRunOptions::default(),
                            || {
                                connect_file(&database_path)?
                                    .execute(update_sql, [])
                                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                                Ok(())
                            },
                        )
                    }
                    MaintenanceLegacyCleanupAction::Inventory => unreachable!("not exercised"),
                };
                let error = result.expect_err("same-owner identity takeover must abort cleanup");
                assert!(
                    matches!(error, KanbanError::Conflict(_)),
                    "{action:?}/{identity_kind} must return Conflict, got {error:?}"
                );
                assert!(
                    error.to_string().contains("owner lease is stale"),
                    "{action:?}/{identity_kind} must preserve stale-owner diagnostic"
                );
                assert!(
                    legacy_file.is_file(),
                    "{action:?} must not move legacy roots"
                );
                assert!(
                    !backup_dir.exists(),
                    "{action:?} must not create backup evidence"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn cleanup_error_mapping_preserves_validation_and_corruption_classes() {
        let unsupported = local_error(LegacyProjectionCleanupError::UnsupportedMutationPlatform);
        assert!(matches!(
            unsupported,
            KanbanError::InvalidInput(message)
                if message
                    == "legacy projection cleanup apply/restore is unsupported on this platform: requires Linux fd-bound renameat2"
        ));

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

    #[test]
    fn cleanup_inventory_requires_the_exact_idle_owner_state_without_side_effects()
    -> anyhow::Result<()> {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, _digest) =
            cleanup_fixture("protocol-preflight")?;
        connect_file(&database_path)?.execute(
            "UPDATE projection_maintenance_owner
             SET capabilities_json='[]', build_identity='incompatible-build'
             WHERE singleton=1",
            [],
        )?;

        let error = maintenance_inventory_legacy_projections(&database_path)
            .expect_err("legacy cleanup must reject incompatible runtime capabilities");
        assert!(
            matches!(
                error,
                KanbanError::InvalidInput(ref message)
                    if message
                        == "legacy cleanup found incompatible inventory requires an idle maintenance owner"
            ),
            "idle-owner mismatch must remain the exact typed preflight error"
        );
        assert!(
            legacy_file.is_file(),
            "preflight must not move legacy files"
        );
        assert!(
            !backup_dir.exists(),
            "preflight must not create cleanup evidence"
        );
        Ok(())
    }

    #[test]
    fn cleanup_inventory_rejects_incompatible_generation_binding_without_side_effects()
    -> anyhow::Result<()> {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, _digest) =
            cleanup_fixture("generation-preflight")?;
        connect_file(&database_path)?.execute_batch(
            "UPDATE projection_store_state
             SET control_plane='v2',
                 active_generation='gen_incompatible',
                 active_fingerprint='sha256:active', active_fence_epoch=1,
                 active_snapshot_cursor=0, active_provider='incompatible-provider',
                 active_provider_fingerprint='provider-v1', active_canonical_count=0,
                 active_canonical_digest='fnv64:active-canonical', active_delivery_count=0,
                 active_delivery_digest='fnv64:active-delivery'
             WHERE store_name='tantivy_tasks';",
        )?;

        let error = maintenance_inventory_legacy_projections(&database_path)
            .expect_err("legacy cleanup must reject incompatible active generation evidence");
        assert!(
            matches!(
                error,
                KanbanError::InvalidInput(ref message)
                    if message
                        == "legacy cleanup found store tantivy_tasks has an incompatible active provider binding"
            ),
            "incompatible generation must remain a typed metadata error"
        );
        assert!(
            legacy_file.is_file(),
            "preflight must not move legacy files"
        );
        assert!(
            !backup_dir.exists(),
            "preflight must not create cleanup evidence"
        );
        Ok(())
    }

    #[test]
    fn cleanup_actions_reject_unbound_v29_lance_roles_without_any_projection_or_physical_mutation()
    -> anyhow::Result<()> {
        for store_name in [LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE] {
            for role in ["active", "previous", "building"] {
                let fixture_name = format!("v29-unbound-{store_name}-{role}");
                let (database_temp, _backup_temp, database_path, _legacy_file, backup_dir, digest) =
                    cleanup_fixture(&fixture_name)?;
                seed_cleanup_control_evidence(&database_path, store_name)?;
                seed_unbound_v29_lance_role(&database_path, store_name, role)?;
                crate::service::checkpoint_database(&database_path)?;

                let expected_error = format!(
                    "corpus_binding_upgrade_required: legacy cleanup refuses {role} generation without a complete LanceDB corpus binding for {store_name}"
                );
                for action in [
                    MaintenanceLegacyCleanupAction::Inventory,
                    MaintenanceLegacyCleanupAction::Apply,
                    MaintenanceLegacyCleanupAction::Verify,
                    MaintenanceLegacyCleanupAction::Restore,
                ] {
                    let sqlite_before = cleanup_sqlite_snapshot(&database_path)?;
                    let physical_before =
                        cleanup_physical_snapshot(database_temp.path(), &backup_dir)?;

                    let error = run_cleanup_action(action, &database_path, &digest, &backup_dir)
                        .expect_err("unbound LanceDB generation must hard-stop cleanup");
                    assert!(
                        matches!(
                            error,
                            KanbanError::InvalidInput(ref message)
                                if message == &expected_error
                        ),
                        "{action:?} must return the stable corpus-binding upgrade error for {store_name}/{role}"
                    );
                    assert_eq!(
                        cleanup_sqlite_snapshot(&database_path)?,
                        sqlite_before,
                        "{action:?} must preserve SQLite authority, outbox, dirty state, and watermarks for {store_name}/{role}"
                    );
                    assert_eq!(
                        cleanup_physical_snapshot(database_temp.path(), &backup_dir)?,
                        physical_before,
                        "{action:?} must preserve the physical projection and backup trees for {store_name}/{role}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn cleanup_inventory_accepts_deterministic_lance_bindings_for_every_generation_role()
    -> anyhow::Result<()> {
        for store_name in [LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE] {
            for role in ["active", "previous", "building"] {
                let fixture_name = format!("bound-{store_name}-{role}");
                let (
                    _database_temp,
                    _backup_temp,
                    database_path,
                    _legacy_file,
                    _backup_dir,
                    _digest,
                ) = cleanup_fixture(&fixture_name)?;
                seed_bound_lance_role(
                    &database_path,
                    store_name,
                    role,
                    (role == "building").then_some("prepared"),
                )?;
                crate::service::checkpoint_database(&database_path)?;

                let control_plane: String = connect_file(&database_path)?.query_row(
                    "SELECT control_plane FROM projection_store_state WHERE store_name=?1",
                    [store_name],
                    |row| row.get(0),
                )?;
                assert_eq!(control_plane, "v2");
                let report = maintenance_inventory_legacy_projections(&database_path)?;
                assert_eq!(report.action, MaintenanceLegacyCleanupAction::Inventory);
            }
        }
        Ok(())
    }

    #[test]
    fn cleanup_inventory_accepts_legal_snapshotting_generation_for_each_store() -> anyhow::Result<()>
    {
        for store_name in [
            TANTIVY_TASKS_STORE,
            OXIGRAPH_RELATIONS_STORE,
            LANCEDB_CHUNKS_STORE,
            LANCEDB_LABEL_ATOMS_STORE,
        ] {
            let fixture_name = format!("bound-{store_name}-snapshotting");
            let (_database_temp, _backup_temp, database_path, _legacy_file, _backup_dir, _digest) =
                cleanup_fixture(&fixture_name)?;
            match store_name {
                LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE => {
                    seed_bound_lance_role(
                        &database_path,
                        store_name,
                        "building",
                        Some("snapshotting"),
                    )?;
                }
                TANTIVY_TASKS_STORE | OXIGRAPH_RELATIONS_STORE => {
                    seed_bound_snapshotting_role(&database_path, store_name)?;
                }
                _ => unreachable!("fixed cleanup store"),
            }
            crate::service::checkpoint_database(&database_path)?;

            let (control_plane, phase, fingerprint): (String, String, Option<String>) =
                connect_file(&database_path)?.query_row(
                    "SELECT control_plane,building_phase,building_fingerprint
                     FROM projection_store_state WHERE store_name=?1",
                    [store_name],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            assert_eq!(control_plane, "v2");
            assert_eq!(phase, "snapshotting");
            assert_eq!(fingerprint, None);
            let report = maintenance_inventory_legacy_projections(&database_path)?;
            assert_eq!(report.action, MaintenanceLegacyCleanupAction::Inventory);
        }
        Ok(())
    }

    #[test]
    fn cleanup_actions_reject_store_cursor_drift_for_snapshotting_generations_without_mutation()
    -> anyhow::Result<()> {
        for store_name in [
            TANTIVY_TASKS_STORE,
            OXIGRAPH_RELATIONS_STORE,
            LANCEDB_CHUNKS_STORE,
            LANCEDB_LABEL_ATOMS_STORE,
        ] {
            let fixture_name = format!("snapshotting-cursor-drift-{store_name}");
            let (database_temp, _backup_temp, database_path, _legacy_file, backup_dir, digest) =
                cleanup_fixture(&fixture_name)?;
            match store_name {
                LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE => {
                    seed_bound_lance_role(
                        &database_path,
                        store_name,
                        "building",
                        Some("snapshotting"),
                    )?;
                }
                TANTIVY_TASKS_STORE | OXIGRAPH_RELATIONS_STORE => {
                    seed_bound_snapshotting_role(&database_path, store_name)?;
                }
                _ => unreachable!("fixed cleanup store"),
            }
            crate::service::checkpoint_database(&database_path)?;
            connect_file(&database_path)?.execute(
                "UPDATE projection_store_state
                 SET snapshot_cursor=1 WHERE store_name=?1",
                [store_name],
            )?;
            let sqlite_before = cleanup_sqlite_snapshot(&database_path)?;
            let physical_before = cleanup_physical_snapshot(database_temp.path(), &backup_dir)?;
            for action in [
                MaintenanceLegacyCleanupAction::Inventory,
                MaintenanceLegacyCleanupAction::Apply,
                MaintenanceLegacyCleanupAction::Verify,
                MaintenanceLegacyCleanupAction::Restore,
            ] {
                let error = run_cleanup_action(action, &database_path, &digest, &backup_dir)
                    .expect_err("store-wide cursor drift must fail closed");
                assert!(
                    matches!(error, KanbanError::InvalidInput(_)),
                    "{action:?} must reject snapshotting cursor drift for {store_name}, got {error:?}"
                );
                assert_eq!(
                    cleanup_sqlite_snapshot(&database_path)?,
                    sqlite_before,
                    "{action:?} must not mutate SQLite authority, outbox, delivery, dirty state, or watermark"
                );
                assert_eq!(
                    cleanup_physical_snapshot(database_temp.path(), &backup_dir)?,
                    physical_before,
                    "{action:?} must not mutate physical projection or backup trees"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn cleanup_actions_reject_any_active_store_lease_before_guard_or_mutation() -> anyhow::Result<()>
    {
        for store_name in [
            TANTIVY_TASKS_STORE,
            OXIGRAPH_RELATIONS_STORE,
            LANCEDB_CHUNKS_STORE,
            LANCEDB_LABEL_ATOMS_STORE,
        ] {
            let fixture_name = format!("active-store-lease-{store_name}");
            let (database_temp, _backup_temp, database_path, _legacy_file, backup_dir, digest) =
                cleanup_fixture(&fixture_name)?;
            let lease_expires_at = SystemClock.now_ms() + 60_000;
            connect_file(&database_path)?.execute(
                "UPDATE projection_store_state
                 SET lease_owner='projection-worker',lease_token='please_active',
                     lease_expires_at=?1 WHERE store_name=?2",
                rusqlite::params![lease_expires_at, store_name],
            )?;
            let sqlite_before = cleanup_sqlite_snapshot(&database_path)?;
            let physical_before = cleanup_physical_snapshot(database_temp.path(), &backup_dir)?;
            for action in [
                MaintenanceLegacyCleanupAction::Inventory,
                MaintenanceLegacyCleanupAction::Apply,
                MaintenanceLegacyCleanupAction::Verify,
                MaintenanceLegacyCleanupAction::Restore,
            ] {
                let error = run_cleanup_action(action, &database_path, &digest, &backup_dir)
                    .expect_err("active store lease must hard-stop cleanup");
                assert!(
                    matches!(error, KanbanError::InvalidInput(_)),
                    "{action:?} must reject active {store_name} lease, got {error:?}"
                );
                assert_eq!(
                    cleanup_sqlite_snapshot(&database_path)?,
                    sqlite_before,
                    "{action:?} must preserve SQLite authority, outbox, delivery, dirty state, and watermarks"
                );
                assert_eq!(
                    cleanup_physical_snapshot(database_temp.path(), &backup_dir)?,
                    physical_before,
                    "{action:?} must preserve physical projection and backup trees"
                );
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cleanup_actions_take_over_a_structurally_valid_expired_owner_and_finish_journal_work()
    -> anyhow::Result<()> {
        let (_database_temp, _backup_temp, database_path, legacy_file, backup_dir, digest) =
            cleanup_fixture("expired-owner-takeover")?;

        maintenance_apply_legacy_projection_cleanup(
            &database_path,
            "initial-cleanup-owner",
            &digest,
            &backup_dir,
            false,
            MaintenanceRunOptions::default(),
        )?;
        assert!(!legacy_file.exists());
        rewrite_completed_cleanup_journal_as_applying(&backup_dir)?;

        seed_maintenance_owner(
            &database_path,
            "crashed-apply-owner",
            SystemClock.now_ms() - 60_000,
        )?;
        let apply = maintenance_apply_legacy_projection_cleanup(
            &database_path,
            "successor-apply-owner",
            &digest,
            &backup_dir,
            true,
            MaintenanceRunOptions::default(),
        )?;
        assert_eq!(apply.action, MaintenanceLegacyCleanupAction::Apply);
        assert!(apply.resumed, "applying journal must be resumed");
        assert!(
            std::fs::read_to_string(backup_dir.join("journal.toml"))?
                .contains("phase = \"completed\""),
            "resumed apply must durably complete the journal"
        );

        seed_maintenance_owner(
            &database_path,
            "crashed-verify-owner",
            SystemClock.now_ms() - 60_000,
        )?;
        let verify = maintenance_verify_legacy_projection_cleanup(
            &database_path,
            "successor-verify-owner",
            &backup_dir,
            MaintenanceRunOptions::default(),
        )?;
        assert_eq!(verify.action, MaintenanceLegacyCleanupAction::Verify);

        seed_maintenance_owner(
            &database_path,
            "crashed-restore-owner",
            SystemClock.now_ms() - 60_000,
        )?;
        let restore = maintenance_restore_legacy_projection_cleanup(
            &database_path,
            "successor-restore-owner",
            &backup_dir,
            MaintenanceRunOptions::default(),
        )?;
        assert_eq!(restore.action, MaintenanceLegacyCleanupAction::Restore);
        assert!(
            legacy_file.is_file(),
            "restore must replace the legacy root"
        );

        let owner = super::super::maintenance::connect_existing_database_read_only(&database_path)?
            .query_row(
                "SELECT owner,lease_token,lease_expires_at,mode,
                        capabilities_json,build_identity
                 FROM projection_maintenance_owner WHERE singleton=1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )?;
        assert_eq!(owner, (None, None, None, None, "[]".to_owned(), None));
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn cleanup_mutation_is_unsupported_without_any_primitive_mutation() -> anyhow::Result<()> {
        let (database_temp, _backup_temp, database_path, legacy_file, backup_dir, digest) =
            cleanup_fixture("non-linux-unsupported")?;
        let database_instance_id =
            super::super::maintenance::connect_existing_database_read_only(&database_path)?
                .query_row(
                    "SELECT database_instance_id FROM projection_database WHERE singleton=1",
                    [],
                    |row| row.get::<_, String>(0),
                )?;
        let guard = acquire_legacy_projection_cleanup_guard(&database_path).map_err(local_error)?;

        let sqlite_before = cleanup_sqlite_snapshot(&database_path)?;
        let database_files_before = cleanup_database_file_snapshot(&database_path)?;
        let physical_before = cleanup_physical_snapshot(database_temp.path(), &backup_dir)?;

        let error = apply_legacy_projection_cleanup_with_resume_decision(
            &guard,
            &database_path,
            &database_instance_id,
            &digest,
            &backup_dir,
            false,
        )
        .expect_err("non-Linux apply must fail before publishing cleanup evidence");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsupportedMutationPlatform
        ));
        assert!(
            !backup_dir.exists(),
            "apply must not create a cleanup journal"
        );
        assert!(legacy_file.is_file(), "apply must not move a legacy root");
        assert_eq!(
            cleanup_sqlite_snapshot(&database_path)?,
            sqlite_before,
            "apply must preserve SQLite authority and owner state"
        );
        assert_eq!(
            cleanup_database_file_snapshot(&database_path)?,
            database_files_before,
            "apply must preserve the database and SQLite sidecars"
        );
        assert_eq!(
            cleanup_physical_snapshot(database_temp.path(), &backup_dir)?,
            physical_before,
            "apply must preserve legacy roots and backup evidence"
        );

        std::fs::create_dir(&backup_dir)?;
        let sqlite_before = cleanup_sqlite_snapshot(&database_path)?;
        let database_files_before = cleanup_database_file_snapshot(&database_path)?;
        let physical_before = cleanup_physical_snapshot(database_temp.path(), &backup_dir)?;

        let error = restore_legacy_projection_backup(
            &guard,
            &database_path,
            &database_instance_id,
            &backup_dir,
        )
        .expect_err("non-Linux restore must fail before reading or updating a journal");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsupportedMutationPlatform
        ));
        assert_eq!(
            std::fs::read_dir(&backup_dir)?.count(),
            0,
            "restore must not create or update cleanup journal evidence"
        );
        assert!(legacy_file.is_file(), "restore must not move a legacy root");
        assert_eq!(
            cleanup_sqlite_snapshot(&database_path)?,
            sqlite_before,
            "restore must preserve SQLite authority and owner state"
        );
        assert_eq!(
            cleanup_database_file_snapshot(&database_path)?,
            database_files_before,
            "restore must preserve the database and SQLite sidecars"
        );
        assert_eq!(
            cleanup_physical_snapshot(database_temp.path(), &backup_dir)?,
            physical_before,
            "restore must preserve legacy roots and backup evidence"
        );
        Ok(())
    }

    #[test]
    fn cleanup_hard_stops_preserve_delete_journal_database_and_sidecars_byte_for_byte()
    -> anyhow::Result<()> {
        for blocker in ["active-owner", "active-store-lease"] {
            for action in [
                MaintenanceLegacyCleanupAction::Inventory,
                MaintenanceLegacyCleanupAction::Apply,
                MaintenanceLegacyCleanupAction::Verify,
                MaintenanceLegacyCleanupAction::Restore,
            ] {
                let fixture_name = format!("delete-journal-{blocker}-{action:?}");
                let (database_temp, _backup_temp, database_path, _legacy_file, backup_dir, digest) =
                    cleanup_fixture(&fixture_name)?;
                match blocker {
                    "active-owner" => seed_maintenance_owner(
                        &database_path,
                        "active-maintenance-owner",
                        SystemClock.now_ms() + 60_000,
                    )?,
                    "active-store-lease" => {
                        connect_file(&database_path)?.execute(
                            "UPDATE projection_store_state
                             SET lease_owner='projection-worker',
                                 lease_token='please_active',
                                 lease_expires_at=?1
                             WHERE store_name=?2",
                            rusqlite::params![SystemClock.now_ms() + 60_000, TANTIVY_TASKS_STORE],
                        )?;
                    }
                    _ => unreachable!("fixed blocker"),
                }
                set_delete_journal_mode(&database_path)?;
                assert_eq!(read_only_journal_mode(&database_path)?, "delete");
                let database_files_before = cleanup_database_file_snapshot(&database_path)?;
                let physical_before = cleanup_physical_snapshot(database_temp.path(), &backup_dir)?;

                let error = run_cleanup_action(action, &database_path, &digest, &backup_dir)
                    .expect_err("active projection authority must hard-stop cleanup");
                assert!(
                    matches!(error, KanbanError::InvalidInput(_)),
                    "{action:?}/{blocker} must return InvalidInput, got {error:?}"
                );
                assert_eq!(
                    cleanup_database_file_snapshot(&database_path)?,
                    database_files_before,
                    "{action:?}/{blocker} must preserve the database and SQLite sidecars byte-for-byte"
                );
                assert_eq!(
                    read_only_journal_mode(&database_path)?,
                    "delete",
                    "{action:?}/{blocker} must not persist journal_mode=WAL"
                );
                assert_eq!(
                    cleanup_physical_snapshot(database_temp.path(), &backup_dir)?,
                    physical_before,
                    "{action:?}/{blocker} must preserve projection and backup roots"
                );
            }
        }
        Ok(())
    }
}
