use std::path::Path;

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::connect_file;

use super::{storage, with_immediate_tx};

pub const PROJECTION_PROTOCOL_VERSION: i64 = 2;
const MAX_PROJECTION_BATCH: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionLease {
    pub store_name: String,
    pub owner: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub lease_expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionArtifactManifest {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub generation: String,
    pub fence_epoch: i64,
    pub snapshot_cursor: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub canonical_item_count: i64,
    pub canonical_digest: String,
    pub delivery_item_count: i64,
    pub delivery_digest: String,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionSnapshotRecord {
    pub board_id: String,
    pub identity: String,
    pub payload_json: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionSnapshot {
    pub manifest: ProjectionArtifactManifest,
    pub records: Vec<ProjectionSnapshotRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionArtifactEvidence {
    pub manifest: ProjectionArtifactManifest,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionDelivery {
    pub id: i64,
    pub outbox_id: i64,
    pub store_name: String,
    pub board_id: String,
    pub source_event_id: Option<i64>,
    pub cursor: i64,
    pub action: String,
    pub entity_uri: String,
    pub payload_json: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionBatch {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub owner: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub target_generation: String,
    pub claim_token: String,
    pub claim_expires_at: i64,
    pub items: Vec<ProjectionDelivery>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionBatchReceipt {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub target_generation: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub claim_token: String,
    pub applied_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionStoreDescriptor {
    pub store_name: String,
    pub provider: String,
    pub provider_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionPublishReceipt {
    pub active: ProjectionArtifactEvidence,
    pub retained_previous: Option<ProjectionArtifactEvidence>,
}

pub trait ProjectionStoreBackend {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor>;

    fn prepare_snapshot(&self, snapshot: &ProjectionSnapshot)
    -> Result<ProjectionArtifactEvidence>;

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt>;

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> Result<ProjectionPublishReceipt>;

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>>;

    fn inspect_generation(&self, generation: &str) -> Result<Option<ProjectionArtifactEvidence>>;

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend cannot quarantine generation {generation}"
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionStoreStatus {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub control_plane: String,
    pub active_generation: Option<String>,
    pub active_fingerprint: Option<String>,
    pub active_fence_epoch: Option<i64>,
    pub active_provider: Option<String>,
    pub active_provider_fingerprint: Option<String>,
    pub previous_generation: Option<String>,
    pub previous_fingerprint: Option<String>,
    pub previous_fence_epoch: Option<i64>,
    pub building_generation: Option<String>,
    pub building_fingerprint: Option<String>,
    pub building_fence_epoch: Option<i64>,
    pub building_provider: Option<String>,
    pub building_provider_fingerprint: Option<String>,
    pub building_phase: Option<String>,
    pub snapshot_cursor: i64,
    pub checkpoint_cursor: i64,
    pub legacy_checkpoint_cursor: i64,
    pub lifecycle_status: String,
    pub owner: Option<String>,
    pub fence_epoch: i64,
    pub lease_expires_at: Option<i64>,
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
    pub legacy_done: i64,
    pub pending_age_ms: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error: Option<String>,
    pub fallback_reason: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionMaintenanceOwnerStatus {
    pub owner: Option<String>,
    pub mode: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionStatus {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub maintenance_owner: ProjectionMaintenanceOwnerStatus,
    pub stores: Vec<ProjectionStoreStatus>,
}

pub fn projection_status(path: impl AsRef<Path>) -> Result<ProjectionStatus> {
    let now = SystemClock.now_ms();
    let conn = super::maintenance::connect_existing_database(path.as_ref())?;
    let (database_instance_id, protocol_version) = conn
        .query_row(
            "SELECT database_instance_id,protocol_version \
             FROM projection_database WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(storage)?;
    let maintenance_owner = conn
        .query_row(
            "SELECT owner,mode,lease_expires_at,last_heartbeat_at
             FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| {
                let owner: Option<String> = row.get(0)?;
                let mode: Option<String> = row.get(1)?;
                let lease_expires_at: Option<i64> = row.get(2)?;
                let last_heartbeat_at: Option<i64> = row.get(3)?;
                Ok(ProjectionMaintenanceOwnerStatus {
                    active: lease_expires_at.is_some_and(|expires_at| expires_at > now)
                        && owner.is_some(),
                    owner,
                    mode,
                    lease_expires_at,
                    last_heartbeat_at,
                })
            },
        )
        .map_err(storage)?;
    let mut statement = conn
        .prepare(
            "SELECT \
                 s.store_name,s.database_instance_id,s.protocol_version,s.schema_version,\
                 s.control_plane,s.active_generation,s.active_fingerprint,s.active_fence_epoch,\
                 s.previous_generation,s.previous_fingerprint,s.previous_fence_epoch,\
                 s.building_generation,s.building_fingerprint,s.building_fence_epoch,\
                 s.building_phase,s.snapshot_cursor,s.checkpoint_cursor,\
                 s.legacy_checkpoint_cursor,s.lifecycle_status,s.lease_owner,s.fence_epoch,\
                 s.lease_expires_at,\
                 SUM(CASE WHEN d.status='pending' THEN 1 ELSE 0 END),\
                 SUM(CASE WHEN d.status='running' THEN 1 ELSE 0 END),\
                 SUM(CASE WHEN d.status='failed' THEN 1 ELSE 0 END),\
                 SUM(CASE WHEN d.status='legacy_done' THEN 1 ELSE 0 END),\
                 MIN(CASE WHEN d.status IN ('pending','failed','legacy_done') \
                          THEN d.created_at END),\
                 s.last_success_at,\
                 COALESCE(s.last_error,(\
                   SELECT failed.last_error FROM projection_deliveries failed \
                   WHERE failed.store_name=s.store_name AND failed.status='failed' \
                   ORDER BY failed.updated_at DESC,failed.cursor LIMIT 1\
                 )),\
                 s.active_provider,s.active_provider_fingerprint,\
                 s.building_provider,s.building_provider_fingerprint,\
                 s.updated_at \
             FROM projection_store_state s \
             LEFT JOIN projection_deliveries d ON d.store_name=s.store_name \
             GROUP BY s.store_name ORDER BY s.store_name",
        )
        .map_err(storage)?;
    let stores = statement
        .query_map([], |row| projection_status_from_row(row, now))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    Ok(ProjectionStatus {
        database_instance_id,
        protocol_version,
        maintenance_owner,
        stores,
    })
}

pub fn acquire_projection_lease(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    ttl_ms: i64,
) -> Result<ProjectionLease> {
    validate_owner_and_ttl(owner, ttl_ms)?;
    let now = SystemClock.now_ms();
    let lease_expires_at = checked_expiry(now, ttl_ms, "projection lease")?;
    let lease_token = new_typed_id("please");
    let conn = connect_file(path.as_ref())?;
    let fence_epoch = with_immediate_tx(&conn, || {
        let changed = conn
            .execute(
                "UPDATE projection_store_state \
                 SET lease_owner=?1,lease_token=?2,lease_expires_at=?3,\
                     fence_epoch=fence_epoch+1,updated_at=?4 \
                 WHERE store_name=?5 \
                   AND (lease_token IS NULL OR lease_expires_at <= ?4)",
                params![owner, lease_token, lease_expires_at, now, store_name],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(projection_lease_conflict(store_name));
        }
        conn.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| row.get(0),
        )
        .map_err(storage)
    })?;
    Ok(ProjectionLease {
        store_name: store_name.to_owned(),
        owner: owner.to_owned(),
        lease_token,
        fence_epoch,
        lease_expires_at,
    })
}

pub fn renew_projection_lease(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    ttl_ms: i64,
) -> Result<ProjectionLease> {
    validate_owner_and_ttl(owner, ttl_ms)?;
    let now = SystemClock.now_ms();
    let lease_expires_at = checked_expiry(now, ttl_ms, "projection lease")?;
    let conn = connect_file(path.as_ref())?;
    let fence_epoch = with_immediate_tx(&conn, || {
        let changed = conn
            .execute(
                "UPDATE projection_store_state SET lease_expires_at=?1,updated_at=?2 \
                 WHERE store_name=?3 AND lease_owner=?4 AND lease_token=?5 \
                   AND lease_expires_at > ?2",
                params![lease_expires_at, now, store_name, owner, lease_token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(projection_lease_conflict(store_name));
        }
        conn.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| row.get(0),
        )
        .map_err(storage)
    })?;
    Ok(ProjectionLease {
        store_name: store_name.to_owned(),
        owner: owner.to_owned(),
        lease_token: lease_token.to_owned(),
        fence_epoch,
        lease_expires_at,
    })
}

pub fn release_projection_lease(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
) -> Result<()> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path.as_ref())?;
    with_immediate_tx(&conn, || {
        require_current_lease(&conn, store_name, owner, lease_token, now)?;
        conn.execute(
            "UPDATE projection_deliveries \
             SET status='pending',claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,\
                 claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,\
                 last_error=COALESCE(last_error,'claim released before acknowledgement'),\
                 updated_at=?1 \
             WHERE store_name=?2 AND status='running' AND claim_lease_token=?3",
            params![now, store_name, lease_token],
        )
        .map_err(storage)?;
        conn.execute(
            "UPDATE projection_store_state \
             SET lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL,updated_at=?1 \
             WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
            params![now, store_name, owner, lease_token],
        )
        .map_err(storage)?;
        Ok(())
    })
}

pub fn begin_projection_generation(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionArtifactManifest> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let descriptor = backend.descriptor()?;
    validate_store_descriptor(store_name, &descriptor)?;
    if store_name == "lancedb_label_atoms" {
        return Err(KanbanError::Conflict(
            "projection store lancedb_label_atoms cannot enter v2 before its mutation delivery protocol is available"
                .to_owned(),
        ));
    }
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let lease = current_lease(&conn, store_name, owner, lease_token, now)?;
        conn.execute(
            "UPDATE projection_deliveries
             SET status='pending',claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                 claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,
                 last_error=COALESCE(last_error,'claim expired before generation rebuild'),
                 updated_at=?1
             WHERE store_name=?2 AND status='running' AND claim_expires_at<=?1",
            params![now, store_name],
        )
        .map_err(storage)?;
        let running: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projection_deliveries
                 WHERE store_name=?1 AND status='running'",
                [store_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if running != 0 {
            return Err(KanbanError::Conflict(format!(
                "projection generation cannot begin while {running} delivery claim(s) are running"
            )));
        }
        let building: Option<String> = conn
            .query_row(
                "SELECT building_generation FROM projection_store_state WHERE store_name=?1",
                [store_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if let Some(building) = building {
            return Err(KanbanError::Conflict(format!(
                "projection generation {building} already building for store {store_name}"
            )));
        }
        let snapshot_cursor = conn
            .query_row(
                "SELECT COALESCE(MAX(cursor),0) FROM projection_deliveries WHERE store_name=?1",
                [store_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let records = canonical_snapshot_records(&conn, store_name)?;
        let (canonical_item_count, canonical_digest) = snapshot_record_coverage(&records);
        let (delivery_item_count, delivery_digest) =
            delivery_snapshot_coverage(&conn, store_name, snapshot_cursor)?;
        let generation = new_typed_id("gen");
        conn.execute(
            "UPDATE projection_store_state \
             SET building_generation=?1,building_fingerprint=NULL,building_fence_epoch=?2,\
                 building_provider=?3,building_provider_fingerprint=?4,\
                 building_canonical_count=?5,building_canonical_digest=?6,\
                 building_delivery_count=?7,building_delivery_digest=?8,\
                 building_phase='snapshotting',snapshot_cursor=?9,\
                 control_plane='v2',lifecycle_status='rebuilding',last_error=NULL,updated_at=?10 \
             WHERE store_name=?11",
            params![
                generation,
                lease.fence_epoch,
                descriptor.provider,
                descriptor.provider_fingerprint,
                canonical_item_count,
                canonical_digest,
                delivery_item_count,
                delivery_digest,
                snapshot_cursor,
                now,
                store_name
            ],
        )
        .map_err(storage)?;
        Ok(ProjectionArtifactManifest {
            store_name: store_name.to_owned(),
            database_instance_id: lease.database_instance_id,
            protocol_version: lease.protocol_version,
            schema_version: lease.schema_version,
            generation,
            fence_epoch: lease.fence_epoch,
            snapshot_cursor,
            provider: descriptor.provider,
            provider_fingerprint: descriptor.provider_fingerprint,
            canonical_item_count,
            canonical_digest,
            delivery_item_count,
            delivery_digest,
            fingerprint: None,
        })
    })
}

pub fn prepare_projection_snapshot_with(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionArtifactEvidence> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let manifest = building_manifest(path, store_name, owner, lease_token)?;
    validate_backend_binding(backend, &manifest)?;
    if manifest.fingerprint.is_some() {
        return Err(KanbanError::Conflict(format!(
            "projection snapshot already prepared for store {store_name}"
        )));
    }
    let conn = connect_file(path)?;
    let running: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_deliveries \
             WHERE store_name=?1 AND cursor<=?2 AND status='running'",
            params![store_name, manifest.snapshot_cursor],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if running != 0 {
        return Err(KanbanError::Conflict(format!(
            "projection snapshot cannot cover {running} running delivery item(s)"
        )));
    }
    let snapshot = match canonical_snapshot_for_manifest(&conn, &manifest).and_then(|snapshot| {
        validate_delivery_snapshot_coverage(&conn, &manifest)?;
        Ok(snapshot)
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            record_projection_error(path, store_name, &error.to_string())?;
            return Err(error);
        }
    };
    let evidence = match backend.prepare_snapshot(&snapshot).and_then(|evidence| {
        validate_artifact_evidence(&manifest, &evidence)?;
        Ok(evidence)
    }) {
        Ok(evidence) => evidence,
        Err(error) => {
            record_projection_error(path, store_name, &error.to_string())?;
            return Err(error);
        }
    };
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    if let Err(error) = with_immediate_tx(&conn, || {
        require_current_lease(&conn, store_name, owner, lease_token, now)?;
        let running: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projection_deliveries \
                 WHERE store_name=?1 AND cursor<=?2 AND status='running'",
                params![store_name, manifest.snapshot_cursor],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if running != 0 {
            return Err(KanbanError::Conflict(format!(
                "projection snapshot cannot cover {running} running delivery item(s)"
            )));
        }
        canonical_snapshot_for_manifest(&conn, &manifest)?;
        validate_delivery_snapshot_coverage(&conn, &manifest)?;
        let changed = conn
            .execute(
                "UPDATE projection_store_state \
                 SET building_fingerprint=?1,building_phase='prepared',updated_at=?2 \
                 WHERE store_name=?3 AND building_generation=?4 \
                   AND building_fence_epoch=?5 AND building_phase='snapshotting'",
                params![
                    evidence.fingerprint,
                    now,
                    store_name,
                    manifest.generation,
                    manifest.fence_epoch
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        conn.execute(
            "UPDATE projection_deliveries \
             SET status='done',published_generation=?1,last_error=NULL,updated_at=?2 \
             WHERE store_name=?3 AND cursor<=?4 AND status!='running'",
            params![
                manifest.generation,
                now,
                store_name,
                manifest.snapshot_cursor
            ],
        )
        .map_err(storage)?;
        advance_checkpoint(&conn, store_name, now)?;
        Ok(())
    }) {
        record_projection_error(path, store_name, &error.to_string())?;
        return Err(error);
    }
    Ok(evidence)
}

pub fn abort_projection_generation(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<()> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let manifest = building_manifest(path, store_name, owner, lease_token)?;
    validate_backend_binding(backend, &manifest)?;
    if backend
        .inspect_active()?
        .is_some_and(|active| active.manifest.generation == manifest.generation)
    {
        return Err(KanbanError::Conflict(format!(
            "projection generation {} is physically active and must be reconciled instead of aborted",
            manifest.generation
        )));
    }
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        require_current_lease(&conn, store_name, owner, lease_token, now)?;
        let building: Option<String> = conn
            .query_row(
                "SELECT building_generation FROM projection_store_state WHERE store_name=?1",
                [store_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let Some(building) = building else {
            return Err(KanbanError::Conflict(format!(
                "projection store {store_name} has no building generation to abort"
            )));
        };
        conn.execute(
            "UPDATE projection_deliveries
             SET status='pending',published_generation=NULL,
                 claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                 claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,
                 last_error='generation aborted before publish',updated_at=?1
             WHERE store_name=?2
               AND (published_generation=?3 OR claim_generation=?3)",
            params![now, store_name, building],
        )
        .map_err(storage)?;
        recompute_checkpoint(&conn, store_name, now)?;
        conn.execute(
            "UPDATE projection_store_state
             SET building_generation=NULL,building_fingerprint=NULL,building_fence_epoch=NULL,
                 building_provider=NULL,building_provider_fingerprint=NULL,
                 building_canonical_count=NULL,building_canonical_digest=NULL,
                 building_delivery_count=NULL,building_delivery_digest=NULL,
                 building_phase=NULL,
                 lifecycle_status=CASE
                   WHEN active_generation IS NULL THEN 'bootstrap_required'
                   ELSE 'ready'
                 END,
                 last_error=NULL,updated_at=?1
             WHERE store_name=?2 AND building_generation=?3",
            params![now, store_name, building],
        )
        .map_err(storage)?;
        Ok(())
    })
}

pub fn run_projection_batch_with(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    claim_ttl_ms: i64,
    limit: usize,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionBatch> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    validate_backend_for_target(path, store_name, backend)?;
    let batch = claim_projection_batch(path, store_name, owner, lease_token, claim_ttl_ms, limit)?;
    if batch.items.is_empty() {
        return Ok(batch);
    }
    match backend.apply_batch(&batch) {
        Ok(receipt) => {
            if let Err(error) = acknowledge_projection_batch(path, &batch, &receipt) {
                fail_projection_batch(path, &batch, &error.to_string(), claim_ttl_ms)?;
                return Err(error);
            }
            Ok(batch)
        }
        Err(error) => {
            fail_projection_batch(path, &batch, &error.to_string(), claim_ttl_ms)?;
            Err(error)
        }
    }
}

pub fn publish_projection_generation_with(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionArtifactEvidence> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let prepared = prepared_manifest(path, store_name, owner, lease_token)?;
    validate_backend_binding(backend, &prepared.manifest)?;
    let expected_active = active_artifact(path, store_name)?;
    let operation = (|| {
        let receipt = match backend.inspect_active()? {
            Some(active) if same_artifact(&prepared, &active) => ProjectionPublishReceipt {
                active,
                retained_previous: inspect_expected_previous(backend, expected_active.as_ref())?,
            },
            _ => backend.publish_generation(expected_active.as_ref(), &prepared)?,
        };
        validate_artifact_evidence(&prepared.manifest, &receipt.active)?;
        if receipt.retained_previous != expected_active {
            return Err(KanbanError::Storage(format!(
                "projection store did not retain the previous physical generation for {store_name}"
            )));
        }
        let active = backend.inspect_active()?.ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store did not expose active generation for {store_name}"
            ))
        })?;
        if !same_artifact(&receipt.active, &active) {
            return Err(KanbanError::Storage(format!(
                "projection active generation readback mismatch for {store_name}"
            )));
        }
        inspect_expected_previous(backend, expected_active.as_ref())?;
        confirm_published_generation(
            path,
            store_name,
            owner,
            lease_token,
            &active,
            expected_active.as_ref(),
        )?;
        Ok(active)
    })();
    if let Err(error) = &operation {
        record_projection_error(path, store_name, &error.to_string())?;
    }
    operation
}

pub fn recover_projection_generation_with(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionArtifactEvidence> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let prepared = prepared_manifest(path, store_name, owner, lease_token)?;
    validate_backend_binding(backend, &prepared.manifest)?;
    let missing_active = active_artifact(path, store_name)?.ok_or_else(|| {
        KanbanError::Conflict(format!(
            "projection recovery requires a logical active generation for {store_name}"
        ))
    })?;
    let expected_previous = previous_artifact(path, store_name)?;
    let operation = (|| {
        if backend
            .inspect_generation(&missing_active.manifest.generation)?
            .as_ref()
            .is_some_and(|artifact| same_artifact(artifact, &missing_active))
        {
            return Err(KanbanError::Conflict(format!(
                "projection recovery refused because logical active generation {} is still readable",
                missing_active.manifest.generation
            )));
        }
        let mut physical_active = backend.inspect_active()?;
        for _ in 0..1_024 {
            let Some(active) = physical_active.as_ref() else {
                break;
            };
            if expected_previous
                .as_ref()
                .is_some_and(|previous| same_artifact(previous, active))
            {
                break;
            }
            let quarantined = active.clone();
            backend.quarantine_generation(&quarantined.manifest.generation)?;
            physical_active = backend.inspect_active()?;
            if physical_active
                .as_ref()
                .is_some_and(|current| same_artifact(current, &quarantined))
            {
                return Err(KanbanError::Storage(format!(
                    "projection backend did not quarantine unexpected generation {}",
                    quarantined.manifest.generation
                )));
            }
        }
        if physical_active
            .as_ref()
            .is_some_and(|active| expected_previous.as_ref() != Some(active))
        {
            return Err(KanbanError::Conflict(format!(
                "projection recovery found too many unexpected physical generations for {store_name}"
            )));
        }
        let receipt = backend.publish_generation(physical_active.as_ref(), &prepared)?;
        validate_artifact_evidence(&prepared.manifest, &receipt.active)?;
        if receipt.retained_previous != physical_active {
            return Err(KanbanError::Storage(format!(
                "projection recovery did not retain the readable previous generation for {store_name}"
            )));
        }
        let active = backend.inspect_active()?.ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection recovery did not expose active generation for {store_name}"
            ))
        })?;
        if !same_artifact(&receipt.active, &active) {
            return Err(KanbanError::Storage(format!(
                "projection recovery active generation readback mismatch for {store_name}"
            )));
        }
        inspect_expected_previous(backend, physical_active.as_ref())?;
        confirm_published_generation(
            path,
            store_name,
            owner,
            lease_token,
            &active,
            physical_active.as_ref(),
        )?;
        Ok(active)
    })();
    if let Err(error) = &operation {
        record_projection_error(path, store_name, &error.to_string())?;
    }
    operation
}

pub fn reconcile_projection_generation_with(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionArtifactEvidence> {
    let path = path.as_ref();
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let prepared = prepared_manifest(path, store_name, owner, lease_token)?;
    validate_backend_binding(backend, &prepared.manifest)?;
    let operation = (|| {
        let active = backend.inspect_active()?.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "projection store has no published generation to reconcile for {store_name}"
            ))
        })?;
        if !same_artifact(&prepared, &active) {
            return Err(KanbanError::Conflict(format!(
                "projection store generation does not match SQLite building state for {store_name}"
            )));
        }
        let expected_previous = active_artifact(path, store_name)?;
        inspect_expected_previous(backend, expected_previous.as_ref())?;
        confirm_published_generation(
            path,
            store_name,
            owner,
            lease_token,
            &active,
            expected_previous.as_ref(),
        )?;
        Ok(active)
    })();
    if let Err(error) = &operation {
        record_projection_error(path, store_name, &error.to_string())?;
    }
    operation
}

fn claim_projection_batch(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    claim_ttl_ms: i64,
    limit: usize,
) -> Result<ProjectionBatch> {
    validate_owner_and_ttl(owner, claim_ttl_ms)?;
    if limit == 0 || limit > MAX_PROJECTION_BATCH {
        return Err(KanbanError::InvalidInput(format!(
            "projection claim limit must be between 1 and {MAX_PROJECTION_BATCH}"
        )));
    }
    let now = SystemClock.now_ms();
    let claim_expires_at = checked_expiry(now, claim_ttl_ms, "projection claim")?;
    let claim_token = new_typed_id("pclaim");
    let conn = connect_file(path.as_ref())?;
    let (lease, target_generation, provider, provider_fingerprint, items) =
        with_immediate_tx(&conn, || {
            let lease = current_lease(&conn, store_name, owner, lease_token, now)?;
            if claim_expires_at > lease.lease_expires_at {
                return Err(KanbanError::InvalidInput(
                    "projection claim TTL cannot exceed the current store lease".to_owned(),
                ));
            }
            conn.execute(
                "UPDATE projection_deliveries \
             SET status='pending',claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,\
                 claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,\
                 last_error=COALESCE(last_error,'claim expired before acknowledgement'),\
                 updated_at=?1 \
             WHERE store_name=?2 AND status='running' AND claim_expires_at<=?1",
                params![now, store_name],
            )
            .map_err(storage)?;
            let (target_generation, provider, provider_fingerprint) =
                target_generation_for_claim(&conn, store_name)?;
            let mut statement = conn
                .prepare(
                    "SELECT id,outbox_id,store_name,board_id,source_event_id,cursor,action,\
                        entity_uri,payload_json,attempts \
                 FROM projection_deliveries \
                 WHERE store_name=?1 AND status IN ('pending','failed') AND next_attempt_at<=?2 \
                   AND cursor<COALESCE((\
                     SELECT MIN(blocked.cursor) FROM projection_deliveries blocked \
                     WHERE blocked.store_name=?1 AND blocked.status NOT IN ('done','legacy_done') \
                       AND (blocked.status='running' OR blocked.next_attempt_at>?2)\
                   ),9223372036854775807) \
                 ORDER BY cursor LIMIT ?3",
                )
                .map_err(storage)?;
            let candidates = statement
                .query_map(
                    params![store_name, now, limit as i64],
                    projection_delivery_from_row,
                )
                .map_err(storage)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(storage)?;
            drop(statement);
            let mut claimed = Vec::with_capacity(candidates.len());
            for mut candidate in candidates {
                let changed = conn
                    .execute(
                        "UPDATE projection_deliveries \
                     SET status='running',attempts=attempts+1,claim_owner=?1,claim_token=?2,\
                         claim_lease_token=?3,claim_fence_epoch=?4,claim_generation=?5,\
                         claim_expires_at=?6,last_error=NULL,updated_at=?7 \
                     WHERE id=?8 AND status IN ('pending','failed')",
                        params![
                            owner,
                            claim_token,
                            lease_token,
                            lease.fence_epoch,
                            target_generation,
                            claim_expires_at,
                            now,
                            candidate.id
                        ],
                    )
                    .map_err(storage)?;
                if changed == 1 {
                    candidate.attempts += 1;
                    claimed.push(candidate);
                }
            }
            Ok((
                lease,
                target_generation,
                provider,
                provider_fingerprint,
                claimed,
            ))
        })?;
    Ok(ProjectionBatch {
        store_name: store_name.to_owned(),
        database_instance_id: lease.database_instance_id,
        protocol_version: lease.protocol_version,
        schema_version: lease.schema_version,
        provider,
        provider_fingerprint,
        owner: owner.to_owned(),
        lease_token: lease_token.to_owned(),
        fence_epoch: lease.fence_epoch,
        target_generation,
        claim_token,
        claim_expires_at,
        items,
    })
}

fn acknowledge_projection_batch(
    path: impl AsRef<Path>,
    batch: &ProjectionBatch,
    receipt: &ProjectionBatchReceipt,
) -> Result<i64> {
    validate_batch_receipt(batch, receipt)?;
    let now = SystemClock.now_ms();
    let conn = connect_file(path.as_ref())?;
    with_immediate_tx(&conn, || {
        let lease = current_lease(
            &conn,
            &batch.store_name,
            &batch.owner,
            &batch.lease_token,
            now,
        )?;
        if lease.fence_epoch != batch.fence_epoch {
            return Err(projection_lease_conflict(&batch.store_name));
        }
        let changed = conn
            .execute(
                "UPDATE projection_deliveries \
                 SET status='done',published_generation=claim_generation,\
                     claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,\
                     claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,\
                     last_error=NULL,updated_at=?1 \
                 WHERE store_name=?2 AND claim_owner=?3 AND claim_token=?4 \
                   AND claim_lease_token=?5 AND claim_fence_epoch=?6 \
                   AND claim_generation=?7 AND status='running' AND claim_expires_at>?1",
                params![
                    now,
                    batch.store_name,
                    batch.owner,
                    batch.claim_token,
                    batch.lease_token,
                    batch.fence_epoch,
                    batch.target_generation
                ],
            )
            .map_err(storage)?;
        if changed != batch.items.len() {
            return Err(KanbanError::Conflict(format!(
                "projection claim is stale or incomplete for store {}",
                batch.store_name
            )));
        }
        let checkpoint = advance_checkpoint(&conn, &batch.store_name, now)?;
        let targets_active: bool = conn
            .query_row(
                "SELECT COALESCE(active_generation=?1,0)
                 FROM projection_store_state WHERE store_name=?2",
                params![batch.target_generation, batch.store_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if targets_active {
            reconcile_legacy_outbox(&conn, &batch.store_name, now)?;
            reconcile_legacy_store_state(&conn, &batch.store_name, now)?;
        }
        conn.execute(
            "UPDATE projection_store_state
             SET lifecycle_status=CASE
                   WHEN building_generation IS NOT NULL THEN 'rebuilding'
                   WHEN active_generation IS NOT NULL THEN 'ready'
                   ELSE 'bootstrap_required'
                 END,
                 last_error=NULL,updated_at=?1
             WHERE store_name=?2",
            params![now, batch.store_name],
        )
        .map_err(storage)?;
        Ok(checkpoint)
    })
}

fn fail_projection_batch(
    path: impl AsRef<Path>,
    batch: &ProjectionBatch,
    error: &str,
    retry_delay_ms: i64,
) -> Result<()> {
    let error = error.trim();
    if error.is_empty() {
        return Err(KanbanError::InvalidInput(
            "projection failure message cannot be empty".to_owned(),
        ));
    }
    let now = SystemClock.now_ms();
    let retry_at = checked_expiry(now, retry_delay_ms.max(1), "projection retry")?;
    let conn = connect_file(path.as_ref())?;
    with_immediate_tx(&conn, || {
        let lease = current_lease(
            &conn,
            &batch.store_name,
            &batch.owner,
            &batch.lease_token,
            now,
        )?;
        if lease.fence_epoch != batch.fence_epoch {
            return Err(projection_lease_conflict(&batch.store_name));
        }
        let changed = conn
            .execute(
                "UPDATE projection_deliveries \
                 SET status='failed',next_attempt_at=?1,\
                     claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,\
                     claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,\
                     last_error=?2,updated_at=?3 \
                 WHERE store_name=?4 AND claim_owner=?5 AND claim_token=?6 \
                   AND claim_lease_token=?7 AND claim_fence_epoch=?8 \
                   AND status='running'",
                params![
                    retry_at,
                    error,
                    now,
                    batch.store_name,
                    batch.owner,
                    batch.claim_token,
                    batch.lease_token,
                    batch.fence_epoch
                ],
            )
            .map_err(storage)?;
        if changed != batch.items.len() {
            return Err(KanbanError::Conflict(format!(
                "projection claim is stale or incomplete for store {}",
                batch.store_name
            )));
        }
        conn.execute(
            "UPDATE projection_store_state \
             SET lifecycle_status='error',last_error=?1,updated_at=?2 WHERE store_name=?3",
            params![error, now, batch.store_name],
        )
        .map_err(storage)?;
        Ok(())
    })
}

fn record_projection_error(path: &Path, store_name: &str, error: &str) -> Result<()> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    conn.execute(
        "UPDATE projection_store_state
         SET lifecycle_status='error',last_error=?1,updated_at=?2
         WHERE store_name=?3",
        params![error, now, store_name],
    )
    .map_err(storage)?;
    Ok(())
}

fn confirm_published_generation(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    active: &ProjectionArtifactEvidence,
    retained_previous: Option<&ProjectionArtifactEvidence>,
) -> Result<()> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path.as_ref())?;
    with_immediate_tx(&conn, || {
        require_current_lease(&conn, store_name, owner, lease_token, now)?;
        let unfinished: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projection_deliveries \
                 WHERE store_name=?1 \
                   AND (status!='done' OR published_generation!=?2)",
                params![store_name, active.manifest.generation],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if unfinished != 0 {
            return Err(KanbanError::Conflict(format!(
                "projection generation catch-up is incomplete for store {store_name}: \
                 {unfinished} delivery item(s) lack generation coverage"
            )));
        }
        let previous = retained_previous.map(|evidence| &evidence.manifest);
        let changed = conn
            .execute(
                "UPDATE projection_store_state \
                 SET previous_generation=?6,previous_fingerprint=?7,\
                     previous_fence_epoch=?8,previous_snapshot_cursor=?9,\
                     previous_provider=?10,previous_provider_fingerprint=?11,\
                     previous_canonical_count=?12,previous_canonical_digest=?13,\
                     previous_delivery_count=?14,previous_delivery_digest=?15,\
                     active_generation=building_generation,\
                     active_fingerprint=building_fingerprint,\
                     active_fence_epoch=building_fence_epoch,\
                     active_snapshot_cursor=snapshot_cursor,\
                     active_provider=building_provider,\
                     active_provider_fingerprint=building_provider_fingerprint,\
                     active_canonical_count=building_canonical_count,\
                     active_canonical_digest=building_canonical_digest,\
                     active_delivery_count=building_delivery_count,\
                     active_delivery_digest=building_delivery_digest,\
                     building_generation=NULL,building_fingerprint=NULL,\
                     building_fence_epoch=NULL,building_provider=NULL,\
                     building_provider_fingerprint=NULL,building_canonical_count=NULL,\
                     building_canonical_digest=NULL,building_delivery_count=NULL,\
                     building_delivery_digest=NULL,building_phase=NULL,\
                     control_plane='v2',lifecycle_status='ready',\
                     last_success_at=?1,last_error=NULL,updated_at=?1 \
                 WHERE store_name=?2 AND building_generation=?3 \
                   AND building_fingerprint=?4 AND building_fence_epoch=?5 \
                   AND building_phase IN ('prepared','store_published')",
                params![
                    now,
                    store_name,
                    active.manifest.generation,
                    active.fingerprint,
                    active.manifest.fence_epoch,
                    previous.map(|manifest| manifest.generation.as_str()),
                    retained_previous.map(|evidence| evidence.fingerprint.as_str()),
                    previous.map(|manifest| manifest.fence_epoch),
                    previous.map(|manifest| manifest.snapshot_cursor),
                    previous.map(|manifest| manifest.provider.as_str()),
                    previous.map(|manifest| manifest.provider_fingerprint.as_str()),
                    previous.map(|manifest| manifest.canonical_item_count),
                    previous.map(|manifest| manifest.canonical_digest.as_str()),
                    previous.map(|manifest| manifest.delivery_item_count),
                    previous.map(|manifest| manifest.delivery_digest.as_str()),
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        reconcile_legacy_outbox(&conn, store_name, now)?;
        reconcile_legacy_store_state(&conn, store_name, now)?;
        Ok(())
    })
}

#[derive(Debug)]
struct CurrentLease {
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    fence_epoch: i64,
    lease_expires_at: i64,
}

fn current_lease(
    conn: &Connection,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    now: i64,
) -> Result<CurrentLease> {
    conn.query_row(
        "SELECT database_instance_id,protocol_version,schema_version,fence_epoch,\
                lease_expires_at \
         FROM projection_store_state \
         WHERE store_name=?1 AND lease_owner=?2 AND lease_token=?3 \
           AND lease_expires_at>?4",
        params![store_name, owner, lease_token, now],
        |row| {
            Ok(CurrentLease {
                database_instance_id: row.get(0)?,
                protocol_version: row.get(1)?,
                schema_version: row.get(2)?,
                fence_epoch: row.get(3)?,
                lease_expires_at: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| projection_lease_conflict(store_name))
}

fn require_current_lease(
    conn: &Connection,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    now: i64,
) -> Result<()> {
    current_lease(conn, store_name, owner, lease_token, now).map(|_| ())
}

fn building_manifest(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
) -> Result<ProjectionArtifactManifest> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let lease = current_lease(&conn, store_name, owner, lease_token, now)?;
    conn.query_row(
        "SELECT building_generation,building_fence_epoch,snapshot_cursor,\
                building_provider,building_provider_fingerprint,\
                building_canonical_count,building_canonical_digest,\
                building_delivery_count,building_delivery_digest,building_fingerprint \
         FROM projection_store_state WHERE store_name=?1",
        [store_name],
        |row| {
            Ok(ProjectionArtifactManifest {
                store_name: store_name.to_owned(),
                database_instance_id: lease.database_instance_id.clone(),
                protocol_version: lease.protocol_version,
                schema_version: lease.schema_version,
                generation: row.get(0)?,
                fence_epoch: row.get(1)?,
                snapshot_cursor: row.get(2)?,
                provider: row.get(3)?,
                provider_fingerprint: row.get(4)?,
                canonical_item_count: row.get(5)?,
                canonical_digest: row.get(6)?,
                delivery_item_count: row.get(7)?,
                delivery_digest: row.get(8)?,
                fingerprint: row.get(9)?,
            })
        },
    )
    .map_err(storage)
}

fn prepared_manifest(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
) -> Result<ProjectionArtifactEvidence> {
    let manifest = building_manifest(path, store_name, owner, lease_token)?;
    let fingerprint = manifest.fingerprint.clone().ok_or_else(|| {
        KanbanError::Conflict(format!(
            "projection generation is not snapshot-prepared for store {store_name}"
        ))
    })?;
    Ok(ProjectionArtifactEvidence {
        manifest,
        fingerprint,
    })
}

pub(crate) fn active_artifact(
    path: &Path,
    store_name: &str,
) -> Result<Option<ProjectionArtifactEvidence>> {
    let conn = connect_file(path)?;
    let row = conn
        .query_row(
            "SELECT database_instance_id,protocol_version,schema_version,
                active_generation,active_fence_epoch,active_snapshot_cursor,
                active_provider,active_provider_fingerprint,
                active_canonical_count,active_canonical_digest,
                active_delivery_count,active_delivery_digest,active_fingerprint
         FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            },
        )
        .map_err(storage)?;
    let (
        database_instance_id,
        protocol_version,
        schema_version,
        generation,
        fence_epoch,
        snapshot_cursor,
        provider,
        provider_fingerprint,
        canonical_item_count,
        canonical_digest,
        delivery_item_count,
        delivery_digest,
        fingerprint,
    ) = row;
    let Some(generation) = generation else {
        return Ok(None);
    };
    let manifest = ProjectionArtifactManifest {
        store_name: store_name.to_owned(),
        database_instance_id,
        protocol_version,
        schema_version,
        generation,
        fence_epoch: required_artifact_field(fence_epoch, store_name, "active_fence_epoch")?,
        snapshot_cursor: required_artifact_field(
            snapshot_cursor,
            store_name,
            "active_snapshot_cursor",
        )?,
        provider: required_artifact_field(provider, store_name, "active_provider")?,
        provider_fingerprint: required_artifact_field(
            provider_fingerprint,
            store_name,
            "active_provider_fingerprint",
        )?,
        canonical_item_count: required_artifact_field(
            canonical_item_count,
            store_name,
            "active_canonical_count",
        )?,
        canonical_digest: required_artifact_field(
            canonical_digest,
            store_name,
            "active_canonical_digest",
        )?,
        delivery_item_count: required_artifact_field(
            delivery_item_count,
            store_name,
            "active_delivery_count",
        )?,
        delivery_digest: required_artifact_field(
            delivery_digest,
            store_name,
            "active_delivery_digest",
        )?,
        fingerprint: fingerprint.clone(),
    };
    Ok(Some(ProjectionArtifactEvidence {
        manifest,
        fingerprint: required_artifact_field(fingerprint, store_name, "active_fingerprint")?,
    }))
}

fn previous_artifact(path: &Path, store_name: &str) -> Result<Option<ProjectionArtifactEvidence>> {
    let conn = connect_file(path)?;
    let row = conn
        .query_row(
            "SELECT database_instance_id,protocol_version,schema_version,
                    previous_generation,previous_fence_epoch,previous_snapshot_cursor,
                    previous_provider,previous_provider_fingerprint,
                    previous_canonical_count,previous_canonical_digest,
                    previous_delivery_count,previous_delivery_digest,previous_fingerprint
             FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            },
        )
        .map_err(storage)?;
    let (
        database_instance_id,
        protocol_version,
        schema_version,
        generation,
        fence_epoch,
        snapshot_cursor,
        provider,
        provider_fingerprint,
        canonical_item_count,
        canonical_digest,
        delivery_item_count,
        delivery_digest,
        fingerprint,
    ) = row;
    let Some(generation) = generation else {
        return Ok(None);
    };
    let manifest = ProjectionArtifactManifest {
        store_name: store_name.to_owned(),
        database_instance_id,
        protocol_version,
        schema_version,
        generation,
        fence_epoch: required_artifact_field(fence_epoch, store_name, "previous_fence_epoch")?,
        snapshot_cursor: required_artifact_field(
            snapshot_cursor,
            store_name,
            "previous_snapshot_cursor",
        )?,
        provider: required_artifact_field(provider, store_name, "previous_provider")?,
        provider_fingerprint: required_artifact_field(
            provider_fingerprint,
            store_name,
            "previous_provider_fingerprint",
        )?,
        canonical_item_count: required_artifact_field(
            canonical_item_count,
            store_name,
            "previous_canonical_count",
        )?,
        canonical_digest: required_artifact_field(
            canonical_digest,
            store_name,
            "previous_canonical_digest",
        )?,
        delivery_item_count: required_artifact_field(
            delivery_item_count,
            store_name,
            "previous_delivery_count",
        )?,
        delivery_digest: required_artifact_field(
            delivery_digest,
            store_name,
            "previous_delivery_digest",
        )?,
        fingerprint: fingerprint.clone(),
    };
    Ok(Some(ProjectionArtifactEvidence {
        manifest,
        fingerprint: required_artifact_field(fingerprint, store_name, "previous_fingerprint")?,
    }))
}

fn required_artifact_field<T>(value: Option<T>, store_name: &str, field: &str) -> Result<T> {
    value.ok_or_else(|| {
        KanbanError::Storage(format!(
            "projection store {store_name} has incomplete {field}"
        ))
    })
}

fn inspect_expected_previous(
    backend: &(impl ProjectionStoreBackend + ?Sized),
    expected: Option<&ProjectionArtifactEvidence>,
) -> Result<Option<ProjectionArtifactEvidence>> {
    let Some(expected) = expected else {
        return Ok(None);
    };
    let actual = backend
        .inspect_generation(&expected.manifest.generation)?
        .ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection previous physical generation {} is missing",
                expected.manifest.generation
            ))
        })?;
    if !same_artifact(expected, &actual) {
        return Err(KanbanError::Storage(format!(
            "projection previous physical generation {} readback mismatch",
            expected.manifest.generation
        )));
    }
    Ok(Some(actual))
}

fn target_generation_for_claim(
    conn: &Connection,
    store_name: &str,
) -> Result<(String, String, String)> {
    struct GenerationCandidates {
        active: Option<String>,
        active_provider: Option<String>,
        active_provider_fingerprint: Option<String>,
        building: Option<String>,
        building_provider: Option<String>,
        building_provider_fingerprint: Option<String>,
        phase: Option<String>,
    }

    let candidates = conn
        .query_row(
            "SELECT active_generation,active_provider,active_provider_fingerprint,
                    building_generation,building_provider,building_provider_fingerprint,
                    building_phase \
             FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| {
                Ok(GenerationCandidates {
                    active: row.get(0)?,
                    active_provider: row.get(1)?,
                    active_provider_fingerprint: row.get(2)?,
                    building: row.get(3)?,
                    building_provider: row.get(4)?,
                    building_provider_fingerprint: row.get(5)?,
                    phase: row.get(6)?,
                })
            },
        )
        .map_err(storage)?;
    match (
        candidates.building,
        candidates.phase.as_deref(),
        candidates.active,
    ) {
        (Some(building), Some("prepared" | "store_published"), _) => Ok((
            building,
            required_artifact_field(
                candidates.building_provider,
                store_name,
                "building_provider",
            )?,
            required_artifact_field(
                candidates.building_provider_fingerprint,
                store_name,
                "building_provider_fingerprint",
            )?,
        )),
        (Some(_), Some("snapshotting"), _) => Err(KanbanError::Conflict(format!(
            "projection snapshot is still building for store {store_name}"
        ))),
        (None, _, Some(active)) => Ok((
            active,
            required_artifact_field(candidates.active_provider, store_name, "active_provider")?,
            required_artifact_field(
                candidates.active_provider_fingerprint,
                store_name,
                "active_provider_fingerprint",
            )?,
        )),
        _ => Err(KanbanError::Conflict(format!(
            "projection store {store_name} requires a generation rebuild"
        ))),
    }
}

fn validate_store_descriptor(
    store_name: &str,
    descriptor: &ProjectionStoreDescriptor,
) -> Result<()> {
    if descriptor.store_name != store_name
        || descriptor.provider.trim().is_empty()
        || descriptor.provider_fingerprint.trim().is_empty()
    {
        return Err(KanbanError::InvalidInput(format!(
            "projection backend descriptor does not match store {store_name}"
        )));
    }
    Ok(())
}

fn validate_backend_binding(
    backend: &(impl ProjectionStoreBackend + ?Sized),
    manifest: &ProjectionArtifactManifest,
) -> Result<()> {
    let descriptor = backend.descriptor()?;
    validate_store_descriptor(&manifest.store_name, &descriptor)?;
    validate_descriptor_binding(
        &manifest.store_name,
        &manifest.provider,
        &manifest.provider_fingerprint,
        &descriptor,
    )
}

fn validate_backend_for_target(
    path: &Path,
    store_name: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<()> {
    let descriptor = backend.descriptor()?;
    validate_store_descriptor(store_name, &descriptor)?;
    let conn = connect_file(path)?;
    let (_, provider, provider_fingerprint) = target_generation_for_claim(&conn, store_name)?;
    validate_descriptor_binding(store_name, &provider, &provider_fingerprint, &descriptor)
}

fn validate_descriptor_binding(
    store_name: &str,
    expected_provider: &str,
    expected_provider_fingerprint: &str,
    descriptor: &ProjectionStoreDescriptor,
) -> Result<()> {
    if descriptor.provider != expected_provider
        || descriptor.provider_fingerprint != expected_provider_fingerprint
    {
        return Err(KanbanError::Conflict(format!(
            "projection backend provider binding does not match generation for store {store_name}"
        )));
    }
    Ok(())
}

fn canonical_snapshot_for_manifest(
    conn: &Connection,
    manifest: &ProjectionArtifactManifest,
) -> Result<ProjectionSnapshot> {
    let records = canonical_snapshot_records(conn, &manifest.store_name)?;
    let coverage = snapshot_record_coverage(&records);
    if coverage
        != (
            manifest.canonical_item_count,
            manifest.canonical_digest.clone(),
        )
    {
        return Err(KanbanError::Conflict(format!(
            "projection canonical snapshot coverage changed for store {}",
            manifest.store_name
        )));
    }
    Ok(ProjectionSnapshot {
        manifest: manifest.clone(),
        records,
    })
}

fn canonical_snapshot_records(
    conn: &Connection,
    store_name: &str,
) -> Result<Vec<ProjectionSnapshotRecord>> {
    match store_name {
        "tantivy_tasks" | "lancedb_chunks" => canonical_task_records(conn),
        "oxigraph_relations" => canonical_relation_records(conn),
        "lancedb_label_atoms" => canonical_label_atom_records(conn),
        _ => Err(KanbanError::InvalidInput(format!(
            "unknown projection store: {store_name}"
        ))),
    }
}

fn canonical_task_records(conn: &Connection) -> Result<Vec<ProjectionSnapshotRecord>> {
    let mut statement = conn
        .prepare(
            "SELECT t.board_id,t.id,t.seq,t.status,t.assignee,t.priority,t.created_at,t.updated_at,
                    t.due_at,t.title,t.description,
                    COALESCE((SELECT group_concat(c.body, char(10)) FROM task_comments c
                              WHERE c.board_id=t.board_id AND c.task_id=t.id
                              ORDER BY c.created_at,c.id),''),
                    COALESCE((SELECT group_concat(COALESCE(r.summary,'') || ' ' ||
                                                  COALESCE(r.error,''), char(10))
                              FROM task_runs r
                              WHERE r.board_id=t.board_id AND r.task_id=t.id
                              ORDER BY r.started_at,r.id),''),
                    COALESCE((SELECT group_concat(e.kind || ' ' || e.payload_json, char(10))
                              FROM task_events e
                              WHERE e.board_id=t.board_id AND e.task_id=t.id
                              ORDER BY e.id),'')
             FROM tasks t
             ORDER BY t.board_id,t.seq,t.id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([], |row| {
            let board_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let payload_json = serde_json::json!({
                "board_id": board_id,
                "task_id": task_id,
                "seq": row.get::<_, i64>(2)?,
                "status": row.get::<_, String>(3)?,
                "assignee": row.get::<_, Option<String>>(4)?,
                "priority": row.get::<_, i64>(5)?,
                "created_at": row.get::<_, i64>(6)?,
                "updated_at": row.get::<_, i64>(7)?,
                "due_at": row.get::<_, Option<i64>>(8)?,
                "title": row.get::<_, String>(9)?,
                "description": row.get::<_, Option<String>>(10)?,
                "comments": row.get::<_, String>(11)?,
                "run_text": row.get::<_, String>(12)?,
                "event_text": row.get::<_, String>(13)?,
            })
            .to_string();
            Ok(snapshot_record(
                board_id,
                format!("kb://task/{task_id}"),
                payload_json,
            ))
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn canonical_relation_records(conn: &Connection) -> Result<Vec<ProjectionSnapshotRecord>> {
    let cross_board_relation: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT r.subject_uri,r.object_uri,subject.board_id,object.board_id
             FROM entity_relations r
             JOIN entities subject ON subject.uri=r.subject_uri
             JOIN entities object ON object.uri=r.object_uri
             WHERE subject.board_id IS NOT NULL
               AND object.board_id IS NOT NULL
               AND subject.board_id!=object.board_id
             ORDER BY r.subject_uri,r.predicate,r.object_uri,r.graph_uri
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(storage)?;
    if let Some((subject, object, subject_board, object_board)) = cross_board_relation {
        return Err(KanbanError::Conflict(format!(
            "projection snapshot contains cross-board relation {subject} ({subject_board}) -> {object} ({object_board})"
        )));
    }
    let mut statement = conn
        .prepare(
            "SELECT COALESCE(subject.board_id,object.board_id),r.subject_uri,r.predicate,
                    r.object_uri,r.graph_uri,r.authoritative_store,r.source_table,r.source_id,
                    r.source_event_id,r.metadata_json,r.created_at,r.updated_at
             FROM entity_relations r
             LEFT JOIN entities subject ON subject.uri=r.subject_uri
             LEFT JOIN entities object ON object.uri=r.object_uri
             WHERE COALESCE(subject.board_id,object.board_id) IS NOT NULL
             ORDER BY 1,r.subject_uri,r.predicate,r.object_uri,r.graph_uri",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([], |row| {
            let board_id: String = row.get(0)?;
            let subject_uri: String = row.get(1)?;
            let predicate: String = row.get(2)?;
            let object_uri: String = row.get(3)?;
            let graph_uri: String = row.get(4)?;
            let payload_json = serde_json::json!({
                "subject_uri": subject_uri,
                "predicate": predicate,
                "object_uri": object_uri,
                "graph_uri": graph_uri,
                "authoritative_store": row.get::<_, String>(5)?,
                "source_table": row.get::<_, Option<String>>(6)?,
                "source_id": row.get::<_, Option<String>>(7)?,
                "source_event_id": row.get::<_, Option<i64>>(8)?,
                "metadata_json": row.get::<_, String>(9)?,
                "created_at": row.get::<_, i64>(10)?,
                "updated_at": row.get::<_, i64>(11)?,
            })
            .to_string();
            Ok(snapshot_record(
                board_id,
                format!("{subject_uri}|{predicate}|{object_uri}|{graph_uri}"),
                payload_json,
            ))
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn canonical_label_atom_records(conn: &Connection) -> Result<Vec<ProjectionSnapshotRecord>> {
    let mut statement = conn
        .prepare(
            "SELECT a.board_id,a.id,a.label_id,l.name,a.polarity,a.kind,a.text,a.ordinal,
                    a.content_hash,a.created_at,a.updated_at
             FROM label_atoms a
             JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id
             ORDER BY a.board_id,a.id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([], |row| {
            let board_id: String = row.get(0)?;
            let atom_id: String = row.get(1)?;
            let payload_json = serde_json::json!({
                "atom_id": atom_id,
                "label_id": row.get::<_, String>(2)?,
                "label_name": row.get::<_, String>(3)?,
                "polarity": row.get::<_, String>(4)?,
                "kind": row.get::<_, String>(5)?,
                "text": row.get::<_, String>(6)?,
                "ordinal": row.get::<_, i64>(7)?,
                "content_hash": row.get::<_, String>(8)?,
                "created_at": row.get::<_, i64>(9)?,
                "updated_at": row.get::<_, i64>(10)?,
            })
            .to_string();
            Ok(snapshot_record(
                board_id,
                format!("kb://label-atom/{atom_id}"),
                payload_json,
            ))
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn snapshot_record(
    board_id: String,
    identity: String,
    payload_json: String,
) -> ProjectionSnapshotRecord {
    let content_hash = stable_bytes_hash(payload_json.as_bytes());
    ProjectionSnapshotRecord {
        board_id,
        identity,
        payload_json,
        content_hash,
    }
}

fn snapshot_record_coverage(records: &[ProjectionSnapshotRecord]) -> (i64, String) {
    let mut hash = 0xcbf29ce484222325_u64;
    for record in records {
        coverage_hash_bytes(&mut hash, record.board_id.as_bytes());
        coverage_hash_bytes(&mut hash, record.identity.as_bytes());
        coverage_hash_bytes(&mut hash, record.payload_json.as_bytes());
        coverage_hash_bytes(&mut hash, record.content_hash.as_bytes());
    }
    (records.len() as i64, format!("fnv64:{hash:016x}"))
}

fn stable_bytes_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    coverage_hash_bytes(&mut hash, bytes);
    format!("fnv64:{hash:016x}")
}

fn validate_delivery_snapshot_coverage(
    conn: &Connection,
    manifest: &ProjectionArtifactManifest,
) -> Result<()> {
    let coverage =
        delivery_snapshot_coverage(conn, &manifest.store_name, manifest.snapshot_cursor)?;
    if coverage
        != (
            manifest.delivery_item_count,
            manifest.delivery_digest.clone(),
        )
    {
        return Err(KanbanError::Conflict(format!(
            "projection delivery snapshot coverage changed for store {}",
            manifest.store_name
        )));
    }
    Ok(())
}

fn delivery_snapshot_coverage(
    conn: &Connection,
    store_name: &str,
    snapshot_cursor: i64,
) -> Result<(i64, String)> {
    let mut statement = conn
        .prepare(
            "SELECT id,outbox_id,board_id,source_event_id,cursor,action,entity_uri,payload_json
             FROM projection_deliveries
             WHERE store_name=?1 AND cursor<=?2
             ORDER BY cursor,id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map(params![store_name, snapshot_cursor], |row| {
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
        .map_err(storage)?;
    let mut count = 0_i64;
    let mut hash = 0xcbf29ce484222325_u64;
    for row in rows {
        let (id, outbox_id, board_id, source_event_id, cursor, action, entity_uri, payload_json) =
            row.map_err(storage)?;
        count += 1;
        coverage_hash_bytes(&mut hash, &id.to_le_bytes());
        coverage_hash_bytes(&mut hash, &outbox_id.to_le_bytes());
        coverage_hash_bytes(&mut hash, board_id.as_bytes());
        match source_event_id {
            Some(value) => {
                coverage_hash_bytes(&mut hash, &[1]);
                coverage_hash_bytes(&mut hash, &value.to_le_bytes());
            }
            None => coverage_hash_bytes(&mut hash, &[0]),
        }
        coverage_hash_bytes(&mut hash, &cursor.to_le_bytes());
        coverage_hash_bytes(&mut hash, action.as_bytes());
        coverage_hash_bytes(&mut hash, entity_uri.as_bytes());
        coverage_hash_bytes(&mut hash, payload_json.as_bytes());
    }
    Ok((count, format!("fnv64:{hash:016x}")))
}

fn coverage_hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn validate_artifact_evidence(
    expected: &ProjectionArtifactManifest,
    evidence: &ProjectionArtifactEvidence,
) -> Result<()> {
    if evidence.fingerprint.trim().is_empty()
        || evidence.manifest.store_name != expected.store_name
        || evidence.manifest.database_instance_id != expected.database_instance_id
        || evidence.manifest.protocol_version != expected.protocol_version
        || evidence.manifest.schema_version != expected.schema_version
        || evidence.manifest.generation != expected.generation
        || evidence.manifest.fence_epoch != expected.fence_epoch
        || evidence.manifest.snapshot_cursor != expected.snapshot_cursor
        || evidence.manifest.provider != expected.provider
        || evidence.manifest.provider_fingerprint != expected.provider_fingerprint
        || evidence.manifest.canonical_item_count != expected.canonical_item_count
        || evidence.manifest.canonical_digest != expected.canonical_digest
        || evidence.manifest.delivery_item_count != expected.delivery_item_count
        || evidence.manifest.delivery_digest != expected.delivery_digest
    {
        return Err(KanbanError::Conflict(format!(
            "projection artifact evidence does not match generation {}",
            expected.generation
        )));
    }
    Ok(())
}

fn validate_batch_receipt(batch: &ProjectionBatch, receipt: &ProjectionBatchReceipt) -> Result<()> {
    if receipt.store_name != batch.store_name
        || receipt.database_instance_id != batch.database_instance_id
        || receipt.protocol_version != batch.protocol_version
        || receipt.schema_version != batch.schema_version
        || receipt.provider != batch.provider
        || receipt.provider_fingerprint != batch.provider_fingerprint
        || receipt.target_generation != batch.target_generation
        || receipt.lease_token != batch.lease_token
        || receipt.fence_epoch != batch.fence_epoch
        || receipt.claim_token != batch.claim_token
        || receipt.applied_item_count != batch.items.len()
    {
        return Err(KanbanError::Conflict(format!(
            "projection batch receipt mismatch for store {}",
            batch.store_name
        )));
    }
    Ok(())
}

fn same_artifact(
    expected: &ProjectionArtifactEvidence,
    actual: &ProjectionArtifactEvidence,
) -> bool {
    expected == actual
}

pub(crate) fn validate_physical_active_artifact_with(
    path: &Path,
    store_name: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<Option<ProjectionArtifactEvidence>> {
    let Some(expected) = active_artifact(path, store_name)? else {
        return Ok(None);
    };
    let generation = expected.manifest.generation.as_str();
    let physical_generation = backend.inspect_generation(generation)?.ok_or_else(|| {
        KanbanError::Storage(format!(
            "active projection generation {generation} is missing"
        ))
    })?;
    if !same_artifact(&expected, &physical_generation) {
        return Err(KanbanError::Storage(format!(
            "active projection generation {generation} evidence does not match SQLite"
        )));
    }
    let physical_active = backend.inspect_active()?.ok_or_else(|| {
        KanbanError::Storage("projection store has no physically published generation".to_owned())
    })?;
    if !same_artifact(&expected, &physical_active) {
        return Err(KanbanError::Storage(format!(
            "physically published projection generation does not match SQLite active generation {generation}"
        )));
    }
    Ok(Some(expected))
}

fn advance_checkpoint(conn: &Connection, store_name: &str, now: i64) -> Result<i64> {
    let checkpoint = continuous_checkpoint(conn, store_name)?;
    conn.execute(
        "UPDATE projection_store_state \
         SET checkpoint_cursor=MAX(checkpoint_cursor,?1),last_success_at=?2,updated_at=?2 \
         WHERE store_name=?3",
        params![checkpoint, now, store_name],
    )
    .map_err(storage)?;
    Ok(checkpoint)
}

fn recompute_checkpoint(conn: &Connection, store_name: &str, now: i64) -> Result<i64> {
    let checkpoint = continuous_checkpoint(conn, store_name)?;
    conn.execute(
        "UPDATE projection_store_state \
         SET checkpoint_cursor=?1,updated_at=?2 WHERE store_name=?3",
        params![checkpoint, now, store_name],
    )
    .map_err(storage)?;
    Ok(checkpoint)
}

fn continuous_checkpoint(conn: &Connection, store_name: &str) -> Result<i64> {
    let first_unfinished: Option<i64> = conn
        .query_row(
            "SELECT MIN(cursor) FROM projection_deliveries \
             WHERE store_name=?1 AND status!='done'",
            [store_name],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let checkpoint: i64 = match first_unfinished {
        Some(cursor) => conn
            .query_row(
                "SELECT COALESCE(MAX(cursor),0) FROM projection_deliveries \
                 WHERE store_name=?1 AND status='done' AND cursor<?2",
                params![store_name, cursor],
                |row| row.get(0),
            )
            .map_err(storage)?,
        None => conn
            .query_row(
                "SELECT COALESCE(MAX(cursor),0) FROM projection_deliveries \
                 WHERE store_name=?1 AND status='done'",
                [store_name],
                |row| row.get(0),
            )
            .map_err(storage)?,
    };
    Ok(checkpoint)
}

fn reconcile_legacy_outbox(conn: &Connection, store_name: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE index_outbox SET status='done',last_error=NULL,updated_at=?1 \
         WHERE id IN (\
           SELECT d.outbox_id FROM projection_deliveries d \
           WHERE d.store_name=?2 AND d.status='done' \
             AND NOT EXISTS (\
               SELECT 1 FROM projection_deliveries remaining \
               WHERE remaining.outbox_id=d.outbox_id AND remaining.status!='done'\
             )\
         ) AND status!='done'",
        params![now, store_name],
    )
    .map_err(storage)?;
    Ok(())
}

fn reconcile_legacy_store_state(conn: &Connection, store_name: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE derived_store_state \
         SET last_event_id=COALESCE((\
               SELECT MAX(source_event_id) FROM projection_deliveries \
               WHERE store_name=?1 AND status='done'\
             ),last_event_id),\
             dirty=EXISTS(\
               SELECT 1 FROM projection_deliveries \
               WHERE store_name=?1 AND status!='done'\
             ),\
             last_sync_at=?2,\
             last_error=(\
               SELECT last_error FROM projection_deliveries \
               WHERE store_name=?1 AND status='failed' \
               ORDER BY updated_at DESC,cursor LIMIT 1\
             ),\
             updated_at=?2 \
         WHERE store_name=?1",
        params![store_name, now],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn ensure_legacy_projection_control(conn: &Connection, store_name: &str) -> Result<()> {
    let has_projection_state: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master \
             WHERE type='table' AND name='projection_store_state')",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !has_projection_state {
        return Ok(());
    }
    let state: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT control_plane,building_generation
             FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    if state
        .as_ref()
        .is_some_and(|(control, building)| control == "v2" || building.is_some())
    {
        return Err(KanbanError::Conflict(format!(
            "derived store {store_name} is managed by projection maintenance v2"
        )));
    }
    Ok(())
}

fn projection_status_from_row(
    row: &rusqlite::Row<'_>,
    now: i64,
) -> rusqlite::Result<ProjectionStoreStatus> {
    let active_generation: Option<String> = row.get(5)?;
    let building_generation: Option<String> = row.get(11)?;
    let pending: i64 = row.get(22)?;
    let running: i64 = row.get(23)?;
    let failed: i64 = row.get(24)?;
    let legacy_done: i64 = row.get(25)?;
    let oldest_pending_at: Option<i64> = row.get(26)?;
    let last_error: Option<String> = row.get(28)?;
    let lifecycle_status = if last_error.is_some() || failed > 0 {
        "error"
    } else if building_generation.is_some() {
        "rebuilding"
    } else if active_generation.is_none() {
        "bootstrap_required"
    } else {
        "ready"
    };
    let fallback_reason = if last_error.is_some() || failed > 0 {
        Some("derived_store_error".to_owned())
    } else if active_generation.is_none() {
        Some("generation_rebuild_required".to_owned())
    } else if building_generation.is_some() {
        Some("generation_rebuild".to_owned())
    } else if pending > 0 || running > 0 || legacy_done > 0 {
        Some("projection_lag".to_owned())
    } else {
        None
    };
    Ok(ProjectionStoreStatus {
        store_name: row.get(0)?,
        database_instance_id: row.get(1)?,
        protocol_version: row.get(2)?,
        schema_version: row.get(3)?,
        control_plane: row.get(4)?,
        active_generation,
        active_fingerprint: row.get(6)?,
        active_fence_epoch: row.get(7)?,
        active_provider: row.get(29)?,
        active_provider_fingerprint: row.get(30)?,
        previous_generation: row.get(8)?,
        previous_fingerprint: row.get(9)?,
        previous_fence_epoch: row.get(10)?,
        building_generation,
        building_fingerprint: row.get(12)?,
        building_fence_epoch: row.get(13)?,
        building_provider: row.get(31)?,
        building_provider_fingerprint: row.get(32)?,
        building_phase: row.get(14)?,
        snapshot_cursor: row.get(15)?,
        checkpoint_cursor: row.get(16)?,
        legacy_checkpoint_cursor: row.get(17)?,
        lifecycle_status: lifecycle_status.to_owned(),
        owner: row.get(19)?,
        fence_epoch: row.get(20)?,
        lease_expires_at: row.get(21)?,
        pending,
        running,
        failed,
        legacy_done,
        pending_age_ms: oldest_pending_at.map(|created_at| now.saturating_sub(created_at)),
        last_success_at: row.get(27)?,
        last_error,
        fallback_reason,
        updated_at: row.get(33)?,
    })
}

fn projection_delivery_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectionDelivery> {
    Ok(ProjectionDelivery {
        id: row.get(0)?,
        outbox_id: row.get(1)?,
        store_name: row.get(2)?,
        board_id: row.get(3)?,
        source_event_id: row.get(4)?,
        cursor: row.get(5)?,
        action: row.get(6)?,
        entity_uri: row.get(7)?,
        payload_json: row.get(8)?,
        attempts: row.get(9)?,
    })
}

fn validate_owner_and_ttl(owner: &str, ttl_ms: i64) -> Result<()> {
    if owner.trim().is_empty() {
        return Err(KanbanError::InvalidInput(
            "projection owner cannot be empty".to_owned(),
        ));
    }
    if ttl_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "projection TTL must be positive".to_owned(),
        ));
    }
    Ok(())
}

fn checked_expiry(now: i64, ttl_ms: i64, label: &str) -> Result<i64> {
    now.checked_add(ttl_ms)
        .ok_or_else(|| KanbanError::InvalidInput(format!("{label} TTL overflow")))
}

fn projection_lease_conflict(store_name: &str) -> KanbanError {
    KanbanError::Conflict(format!(
        "projection lease is not owned by this worker for store {store_name}"
    ))
}

fn stale_generation(store_name: &str) -> KanbanError {
    KanbanError::Conflict(format!(
        "projection generation is stale for store {store_name}"
    ))
}
