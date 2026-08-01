use std::{fmt, path::Path};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::connect_file;

use super::{storage, with_immediate_tx};

pub const PROJECTION_PROTOCOL_VERSION: i64 = 2;
const MAX_PROJECTION_BATCH: usize = 1_000;
const TANTIVY_TASKS_STORE: &str = "tantivy_tasks";
const OXIGRAPH_RELATIONS_STORE: &str = "oxigraph_relations";
const LANCEDB_CHUNKS_STORE: &str = "lancedb_chunks";
const LANCEDB_LABEL_ATOMS_STORE: &str = "lancedb_label_atoms";

/// Every projection backend and its SQLite lease share one distinct physical
/// authority fence. The service's legacy `${store}` guard remains the outer
/// service-path lock; this suffix lock is acquired by backend operations (or
/// by the LanceDB child helper) before authority validation and physical
/// mutation. Lease rollover must acquire the same suffix before changing the
/// owner/token/fence, so no backend can mutate under a stale lease. Renewal is
/// deliberately expiry-only and uses its owner/token CAS without this lock so
/// long-running physical work can keep the lease alive.
fn acquire_projection_authority_guard(
    path: &Path,
    store_name: &str,
) -> Result<Option<kanban_local::DerivedStoreWriteGuard>> {
    if matches!(
        store_name,
        TANTIVY_TASKS_STORE
            | OXIGRAPH_RELATIONS_STORE
            | LANCEDB_CHUNKS_STORE
            | LANCEDB_LABEL_ATOMS_STORE
    ) {
        Ok(Some(crate::db::acquire_derived_store_write_guard(
            path,
            &format!("{store_name}-projection-helper"),
        )?))
    } else {
        Ok(None)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionLease {
    pub store_name: String,
    pub owner: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub lease_expires_at: i64,
}

impl fmt::Debug for ProjectionLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionLease")
            .field("store_name", &self.store_name)
            .field("owner", &self.owner)
            .field("lease_token", &"[REDACTED]")
            .field("fence_epoch", &self.fence_epoch)
            .field("lease_expires_at", &self.lease_expires_at)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCorpusMetadata {
    pub corpus_schema: String,
    pub corpus_fingerprint: String,
    pub embedding_model: String,
    pub embedding_dimensions: usize,
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
    pub corpus: Option<ProjectionCorpusMetadata>,
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

/// A typed outcome for snapshot preparation that lets the maintenance runtime
/// distinguish a stale canonical baseline from provider or artifact failures.
///
/// `CoverageChanged` is only emitted after an immediate transaction proves that
/// the current lease still owns the exact snapshotting generation and that the
/// canonical or delivery coverage no longer matches its persisted manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectionSnapshotPrepareDisposition {
    Prepared(Box<ProjectionArtifactEvidence>),
    CoverageChanged,
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

/// Validate the only board-scoped rebuild shape that may be applied to a
/// task/relation projection generation.
///
/// A board rebuild has no source event because it is a canonical snapshot
/// request (for example, the replace-import path).  Requiring the parent
/// outbox row to target this store or `all` keeps a forged delivery from
/// borrowing another store's authority, while the exact URI/action/payload
/// checks prevent a legacy board upsert from becoming an implicit rebuild.
#[cfg(any(feature = "oxigraph-backend", feature = "tantivy-backend"))]
pub(crate) fn validate_board_rebuild_delivery(
    conn: &Connection,
    item: &ProjectionDelivery,
    expected_target: &str,
) -> Result<()> {
    let expected_uri = format!("kb://board/{}", item.board_id);
    if item.source_event_id.is_some()
        || item.action != "rebuild"
        || item.entity_uri != expected_uri
        || item.payload_json != "{}"
    {
        return Err(KanbanError::Conflict(format!(
            "projection delivery {} cannot be mapped to an authorized board rebuild",
            item.id
        )));
    }
    let target = conn
        .query_row(
            "SELECT target FROM index_outbox WHERE id=?1 AND entity_uri=?2
             AND action=?3 AND payload_json=?4 AND source_event_id IS NULL",
            params![item.outbox_id, expected_uri, item.action, item.payload_json],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    if !matches!(target.as_deref(), Some("all")) && target.as_deref() != Some(expected_target) {
        return Err(KanbanError::Conflict(format!(
            "projection delivery {} cannot be mapped to an authorized board rebuild target for {}",
            item.id, expected_target
        )));
    }
    let board_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM boards WHERE id=?1)",
            [&item.board_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if !board_exists {
        return Err(KanbanError::Conflict(format!(
            "projection delivery {} board {} does not exist",
            item.id, item.board_id
        )));
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionBatch {
    pub store_name: String,
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub schema_version: i64,
    pub provider: String,
    pub provider_fingerprint: String,
    pub corpus: Option<ProjectionCorpusMetadata>,
    pub owner: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub target_generation: String,
    pub claim_token: String,
    pub claim_expires_at: i64,
    pub items: Vec<ProjectionDelivery>,
}

impl fmt::Debug for ProjectionBatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionBatch")
            .field("store_name", &self.store_name)
            .field("database_instance_id", &self.database_instance_id)
            .field("protocol_version", &self.protocol_version)
            .field("schema_version", &self.schema_version)
            .field("provider", &self.provider)
            .field("provider_fingerprint", &self.provider_fingerprint)
            .field("corpus", &self.corpus)
            .field("owner", &self.owner)
            .field("lease_token", &"[REDACTED]")
            .field("fence_epoch", &self.fence_epoch)
            .field("target_generation", &self.target_generation)
            .field("claim_token", &"[REDACTED]")
            .field("claim_expires_at", &self.claim_expires_at)
            .field("item_count", &self.items.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl fmt::Debug for ProjectionBatchReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionBatchReceipt")
            .field("store_name", &self.store_name)
            .field("database_instance_id", &self.database_instance_id)
            .field("protocol_version", &self.protocol_version)
            .field("schema_version", &self.schema_version)
            .field("provider", &self.provider)
            .field("provider_fingerprint", &self.provider_fingerprint)
            .field("target_generation", &self.target_generation)
            .field("lease_token", &"[REDACTED]")
            .field("fence_epoch", &self.fence_epoch)
            .field("claim_token", &"[REDACTED]")
            .field("applied_item_count", &self.applied_item_count)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionStoreDescriptor {
    pub store_name: String,
    pub provider: String,
    pub provider_fingerprint: String,
    pub corpus: Option<ProjectionCorpusMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionPublishReceipt {
    pub active: ProjectionArtifactEvidence,
    pub retained_previous: Option<ProjectionArtifactEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionGenerationRole {
    Active,
    Previous,
    Building,
    Orphaned,
}

/// The SQLite snapshot that authorizes a destructive physical mutation.
///
/// The lease token is intentionally redacted from `Debug`; it is an opaque
/// capability and must never appear in logs or diagnostics.
#[derive(Clone, PartialEq, Eq)]
pub struct ProjectionDestructiveAuthority {
    pub owner: String,
    pub lease_token: String,
    pub fence_epoch: i64,
    pub lease_expires_at: i64,
    pub role: ProjectionGenerationRole,
    pub generation: String,
    pub expected_manifest: Option<ProjectionArtifactManifest>,
    pub expected_binding: ProjectionGenerationBinding,
    pub building_phase: Option<String>,
}

impl fmt::Debug for ProjectionDestructiveAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionDestructiveAuthority")
            .field("owner", &self.owner)
            .field("lease_token", &"[REDACTED]")
            .field("fence_epoch", &self.fence_epoch)
            .field("role", &self.role)
            .field("generation", &self.generation)
            .field("expected_manifest", &self.expected_manifest)
            .field("expected_binding", &self.expected_binding)
            .field("building_phase", &self.building_phase)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionGenerationBinding {
    pub generation: String,
    pub fingerprint: Option<String>,
    pub fence_epoch: i64,
    pub snapshot_cursor: Option<i64>,
    pub provider: String,
    pub provider_fingerprint: String,
    pub canonical_count: i64,
    pub canonical_digest: String,
    pub delivery_count: i64,
    pub delivery_digest: String,
    pub corpus: Option<ProjectionCorpusMetadata>,
}

pub trait ProjectionStoreBackend {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor>;

    fn prepare_snapshot(&self, snapshot: &ProjectionSnapshot)
    -> Result<ProjectionArtifactEvidence>;

    /// Prepare physical state under the exact SQLite lease capability.
    /// Backends that participate in the maintenance service must explicitly
    /// implement this entry point; silently dropping the capability would let
    /// a legacy mutator run after lease rollover.
    fn prepare_snapshot_with_authority(
        &self,
        snapshot: &ProjectionSnapshot,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionArtifactEvidence> {
        Err(KanbanError::Conflict(format!(
            "projection backend must implement authority-bearing snapshot preparation for generation {}",
            snapshot.manifest.generation
        )))
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt>;

    fn apply_batch_with_authority(
        &self,
        batch: &ProjectionBatch,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionBatchReceipt> {
        Err(KanbanError::Conflict(format!(
            "projection backend must implement authority-bearing batch apply for generation {}",
            batch.target_generation
        )))
    }

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> Result<ProjectionPublishReceipt>;

    fn publish_generation_with_authority(
        &self,
        _expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionPublishReceipt> {
        Err(KanbanError::Conflict(format!(
            "projection backend must implement authority-bearing publication for generation {}",
            prepared.manifest.generation
        )))
    }

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>>;

    fn inspect_generation(&self, generation: &str) -> Result<Option<ProjectionArtifactEvidence>>;

    fn validate_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let actual = self
            .inspect_generation(&expected.manifest.generation)?
            .ok_or_else(|| {
                KanbanError::Storage(format!(
                    "projection generation {} is missing",
                    expected.manifest.generation
                ))
            })?;
        if actual != *expected {
            return Err(KanbanError::Storage(format!(
                "projection generation {} evidence mismatch",
                expected.manifest.generation
            )));
        }
        Ok(())
    }

    fn validate_generation_publication_with_authority(
        &self,
        expected: &ProjectionArtifactEvidence,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        self.validate_generation_publication(expected)
    }

    fn repair_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend cannot repair publication for generation {}",
            expected.manifest.generation
        )))
    }

    fn repair_generation_publication_with_authority(
        &self,
        expected: &ProjectionArtifactEvidence,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend must implement authority-bearing publication repair for generation {}",
            expected.manifest.generation
        )))
    }

    fn validate_active_contents(&self, _active: &ProjectionArtifactEvidence) -> Result<()> {
        Ok(())
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend cannot quarantine generation {generation}"
        )))
    }

    fn abort_generation(&self, generation: &str) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend cannot abort generation {generation}"
        )))
    }

    fn quarantine_generation_fenced(
        &self,
        generation: &str,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend must implement fenced quarantine for generation {generation}"
        )))
    }

    fn abort_generation_fenced(
        &self,
        generation: &str,
        _authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        Err(KanbanError::Conflict(format!(
            "projection backend must implement fenced abort for generation {generation}"
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionRuntimeAvailability {
    Available,
    Unavailable,
    Unverified,
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
    pub active_corpus: Option<ProjectionCorpusMetadata>,
    pub previous_generation: Option<String>,
    pub previous_fingerprint: Option<String>,
    pub previous_fence_epoch: Option<i64>,
    pub previous_corpus: Option<ProjectionCorpusMetadata>,
    pub building_generation: Option<String>,
    pub building_fingerprint: Option<String>,
    pub building_fence_epoch: Option<i64>,
    pub building_provider: Option<String>,
    pub building_provider_fingerprint: Option<String>,
    pub building_corpus: Option<ProjectionCorpusMetadata>,
    pub building_phase: Option<String>,
    pub snapshot_cursor: i64,
    pub checkpoint_cursor: i64,
    pub legacy_checkpoint_cursor: i64,
    pub lifecycle_status: String,
    pub runtime_availability: ProjectionRuntimeAvailability,
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
    pub capabilities: Vec<String>,
    pub build_identity: Option<String>,
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
    let conn = super::maintenance::connect_existing_database_read_only(path.as_ref())?;
    projection_status_conn(conn)
}

pub(crate) fn projection_status_quiescent(path: &Path) -> Result<ProjectionStatus> {
    let conn = super::maintenance::connect_existing_database_quiescent_read_only(path)?;
    projection_status_conn(conn)
}

fn projection_status_conn(conn: crate::db::DatabaseConnection) -> Result<ProjectionStatus> {
    let now = SystemClock.now_ms();
    let (database_instance_id, protocol_version) = conn
        .query_row(
            "SELECT database_instance_id,protocol_version \
             FROM projection_database WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(storage)?;
    let (owner, mode, lease_expires_at, last_heartbeat_at, capabilities_json, build_identity) =
        conn.query_row(
            "SELECT owner,mode,lease_expires_at,last_heartbeat_at,
                    capabilities_json,build_identity
             FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .map_err(storage)?;
    let capabilities: Vec<String> = serde_json::from_str(&capabilities_json).map_err(|error| {
        KanbanError::Storage(format!(
            "projection maintenance owner capabilities are invalid: {error}"
        ))
    })?;
    if capabilities
        .iter()
        .any(|capability| capability.trim().is_empty())
        || capabilities.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(KanbanError::Storage(
            "projection maintenance owner capabilities are not a canonical set".to_owned(),
        ));
    }
    let active = lease_expires_at.is_some_and(|expires_at| expires_at > now) && owner.is_some();
    if owner.is_some() != build_identity.is_some() {
        return Err(KanbanError::Storage(
            "projection maintenance owner build identity is inconsistent".to_owned(),
        ));
    }
    let maintenance_owner = ProjectionMaintenanceOwnerStatus {
        owner,
        mode,
        capabilities,
        build_identity,
        lease_expires_at,
        last_heartbeat_at,
        active,
    };
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
                 s.active_corpus_schema,s.active_corpus_fingerprint,\
                 s.active_embedding_model,s.active_embedding_dimensions,\
                 s.previous_corpus_schema,s.previous_corpus_fingerprint,\
                 s.previous_embedding_model,s.previous_embedding_dimensions,\
                 s.building_corpus_schema,s.building_corpus_fingerprint,\
                 s.building_embedding_model,s.building_embedding_dimensions,\
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
    acquire_projection_lease_with_before_transaction(
        path.as_ref(),
        store_name,
        owner,
        ttl_ms,
        || {},
    )
}

fn acquire_projection_lease_with_before_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    ttl_ms: i64,
    before_transaction: impl FnOnce(),
) -> Result<ProjectionLease> {
    validate_owner_and_ttl(owner, ttl_ms)?;
    let _authority_guard = acquire_projection_authority_guard(path, store_name)?;
    before_transaction();
    let lease_token = new_typed_id("please");
    let conn = connect_file(path)?;
    let (fence_epoch, lease_expires_at) = with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        let lease_expires_at = checked_expiry(now, ttl_ms, "projection lease")?;
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
        .map(|fence_epoch| (fence_epoch, lease_expires_at))
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
    renew_projection_lease_with_before_transaction(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        ttl_ms,
        || {},
    )
}

fn renew_projection_lease_with_before_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    ttl_ms: i64,
    before_transaction: impl FnOnce(),
) -> Result<ProjectionLease> {
    // Renewal only extends the current owner's expiry; it never changes the
    // owner, opaque token, or fence. It therefore remains safe while a
    // physical backend holds the suffix authority lock, and is required for
    // the maintenance heartbeat to keep long-running helper work alive.
    before_transaction();
    let conn = connect_file(path)?;
    let (fence_epoch, lease_expires_at) = with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        let lease_expires_at = checked_expiry(now, ttl_ms, "projection lease")?;
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
        let fence_epoch = conn
            .query_row(
                "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
                [store_name],
                |row| row.get(0),
            )
            .map_err(storage)?;
        Ok((fence_epoch, lease_expires_at))
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
    release_projection_lease_with_before_transaction(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        || {},
    )
}

fn release_projection_lease_with_before_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    before_transaction: impl FnOnce(),
) -> Result<()> {
    let _authority_guard = acquire_projection_authority_guard(path, store_name)?;
    before_transaction();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
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
    begin_projection_generation_with_before_transaction(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        backend,
        || {},
    )
}

fn begin_projection_generation_with_before_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    before_transaction: impl FnOnce(),
) -> Result<ProjectionArtifactManifest> {
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let descriptor = backend.descriptor()?;
    validate_store_descriptor(store_name, &descriptor)?;
    let descriptor_corpus_dimensions = descriptor
        .corpus
        .as_ref()
        .map(|corpus| {
            i64::try_from(corpus.embedding_dimensions).map_err(|_| {
                KanbanError::InvalidInput(format!(
                    "projection backend corpus dimensions exceed SQLite range for store {store_name}"
                ))
            })
        })
        .transpose()?;
    before_transaction();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
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
                 building_corpus_schema=?5,building_corpus_fingerprint=?6,\
                 building_embedding_model=?7,building_embedding_dimensions=?8,\
                 building_canonical_count=?9,building_canonical_digest=?10,\
                 building_delivery_count=?11,building_delivery_digest=?12,\
                 building_phase='snapshotting',snapshot_cursor=?13,\
                 control_plane='v2',lifecycle_status='rebuilding',last_error=NULL,updated_at=?14 \
             WHERE store_name=?15",
            params![
                generation,
                lease.fence_epoch,
                descriptor.provider,
                descriptor.provider_fingerprint,
                descriptor
                    .corpus
                    .as_ref()
                    .map(|corpus| corpus.corpus_schema.as_str()),
                descriptor
                    .corpus
                    .as_ref()
                    .map(|corpus| corpus.corpus_fingerprint.as_str()),
                descriptor
                    .corpus
                    .as_ref()
                    .map(|corpus| corpus.embedding_model.as_str()),
                descriptor_corpus_dimensions,
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
            corpus: descriptor.corpus,
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
    match prepare_projection_snapshot_with_disposition(
        path,
        store_name,
        owner,
        lease_token,
        backend,
    )? {
        ProjectionSnapshotPrepareDisposition::Prepared(evidence) => Ok(*evidence),
        ProjectionSnapshotPrepareDisposition::CoverageChanged => {
            Err(KanbanError::Conflict(format!(
                "projection snapshot coverage changed for store {store_name} (canonical snapshot coverage changed); automatic maintenance may rebase it"
            )))
        }
    }
}

pub(crate) fn prepare_projection_snapshot_with_disposition(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<ProjectionSnapshotPrepareDisposition> {
    prepare_projection_snapshot_with_disposition_with_before_final_transaction(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        backend,
        || {},
    )
}

fn prepare_projection_snapshot_with_disposition_with_before_final_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    before_final_transaction: impl FnOnce(),
) -> Result<ProjectionSnapshotPrepareDisposition> {
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let manifest = building_manifest(path, store_name, owner, lease_token)?;
    // Keep the lease capability that authorized this snapshot attempt.  Error
    // persistence must use this exact fence rather than refreshing the lease
    // after a backend failure: a same-token fence rollover must not let the
    // stale operation mark the successor's control plane dirty.
    let snapshot_lease = current_lease_authority(path, store_name, owner, lease_token)?;
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
            return classify_snapshot_prepare_error(
                path,
                store_name,
                owner,
                lease_token,
                &manifest,
                &snapshot_lease,
                error,
            );
        }
    };
    let prepare_authority = current_building_authority(path, store_name, owner, lease_token)?;
    let evidence = match backend
        .prepare_snapshot_with_authority(&snapshot, &prepare_authority)
        .and_then(|evidence| {
            validate_artifact_evidence(&manifest, &evidence)?;
            Ok(evidence)
        }) {
        Ok(evidence) => evidence,
        Err(error) => {
            // A provider may have left a partial generation (or a staged
            // directory) behind before reporting failure.  The prepare path
            // has no authority to delete it itself, so immediately hand the
            // exact capability to the fenced abort operation.  If the lease
            // rolled over before that operation acquired the helper suffix,
            // the backend must reject the stale capability and leave the
            // physical evidence for the successor to recover.
            let result_error = match abort_projection_generation_with_authority(
                path,
                store_name,
                owner,
                lease_token,
                backend,
                &prepare_authority,
            ) {
                Ok(()) => error,
                Err(abort_error) => KanbanError::Conflict(format!(
                    "projection snapshot prepare failed and fenced recovery could not clean generation {}: {abort_error}; original error: {error}",
                    prepare_authority.generation
                )),
            };
            let error_lease = lease_from_destructive_authority(store_name, &prepare_authority);
            if let Err(record_error) =
                record_projection_error(path, store_name, &error_lease, &result_error.to_string())
                && !matches!(&record_error, KanbanError::Conflict(_))
            {
                return Err(record_error);
            }
            return Err(result_error);
        }
    };
    before_final_transaction();
    let conn = connect_file(path)?;
    if let Err(error) = with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
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
        return classify_snapshot_prepare_error(
            path,
            store_name,
            owner,
            lease_token,
            &manifest,
            &lease_from_destructive_authority(store_name, &prepare_authority),
            error,
        );
    }
    Ok(ProjectionSnapshotPrepareDisposition::Prepared(Box::new(
        evidence,
    )))
}

fn classify_snapshot_prepare_error(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    manifest: &ProjectionArtifactManifest,
    authority: &ProjectionLease,
    error: KanbanError,
) -> Result<ProjectionSnapshotPrepareDisposition> {
    if let Err(record_error) =
        record_projection_error(path, store_name, authority, &error.to_string())
        && !matches!(&record_error, KanbanError::Conflict(_))
    {
        return Err(record_error);
    }
    match snapshot_prepare_disposition(path, store_name, owner, lease_token, manifest)? {
        SnapshotPrepareDisposition::CoverageChanged => {
            Ok(ProjectionSnapshotPrepareDisposition::CoverageChanged)
        }
        SnapshotPrepareDisposition::Preserve => Err(error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotPrepareDisposition {
    CoverageChanged,
    Preserve,
}

fn snapshot_prepare_disposition(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    manifest: &ProjectionArtifactManifest,
) -> Result<SnapshotPrepareDisposition> {
    snapshot_prepare_disposition_with_before_transaction(
        path,
        store_name,
        owner,
        lease_token,
        manifest,
        || {},
    )
}

fn snapshot_prepare_disposition_with_before_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    manifest: &ProjectionArtifactManifest,
    before_transaction: impl FnOnce(),
) -> Result<SnapshotPrepareDisposition> {
    before_transaction();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        require_current_lease(&conn, store_name, owner, lease_token, now)?;
        let (building_generation, building_fence_epoch, building_phase, building_fingerprint): (
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT building_generation,building_fence_epoch,building_phase,building_fingerprint \
                 FROM projection_store_state WHERE store_name=?1",
                [store_name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(storage)?;
        if building_generation.as_deref() != Some(manifest.generation.as_str())
            || building_fence_epoch != Some(manifest.fence_epoch)
            || building_phase.as_deref() != Some("snapshotting")
            || building_fingerprint.is_some()
        {
            return Ok(SnapshotPrepareDisposition::Preserve);
        }
        let canonical_coverage =
            snapshot_record_coverage(&canonical_snapshot_records(&conn, store_name)?);
        let delivery_coverage =
            delivery_snapshot_coverage(&conn, store_name, manifest.snapshot_cursor)?;
        if canonical_coverage
            != (
                manifest.canonical_item_count,
                manifest.canonical_digest.clone(),
            )
            || delivery_coverage
                != (
                    manifest.delivery_item_count,
                    manifest.delivery_digest.clone(),
                )
        {
            Ok(SnapshotPrepareDisposition::CoverageChanged)
        } else {
            Ok(SnapshotPrepareDisposition::Preserve)
        }
    })
}

pub fn abort_projection_generation(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<()> {
    abort_projection_generation_with_binding(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        backend,
        AbortBinding::Exact,
    )
}

pub(crate) fn abort_incompatible_projection_generation(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<()> {
    if recover_incompatible_projection_bindings(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        backend,
    )? {
        Ok(())
    } else {
        abort_projection_generation_with_binding(
            path.as_ref(),
            store_name,
            owner,
            lease_token,
            backend,
            AbortBinding::Incompatible,
        )
    }
}

pub(crate) fn recover_incompatible_projection_bindings(
    path: impl AsRef<Path>,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<bool> {
    recover_incompatible_projection_bindings_with_before_final_transaction(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        backend,
        || {},
    )
}

fn recover_incompatible_projection_bindings_with_before_final_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    before_final_transaction: impl FnOnce(),
) -> Result<bool> {
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let descriptor = backend.descriptor()?;
    validate_store_descriptor(store_name, &descriptor)?;
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let snapshot =
        projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, now)?;
    snapshot.validate_shape(store_name)?;

    let active_incompatible = snapshot.active.binding_is_incompatible(&descriptor);
    let previous_incompatible = snapshot.previous.binding_is_incompatible(&descriptor);
    let building_incompatible = snapshot.building.binding_is_incompatible(&descriptor);
    if !active_incompatible && !previous_incompatible && !building_incompatible {
        return Ok(false);
    }

    // Advance the store fence before touching any physical generation. The
    // returned snapshot is the authority baseline for the whole recovery;
    // requests queued with the pre-bump fence now fail closed even when they
    // retain the same owner and lease token.
    let snapshot = bump_recovery_fence(path, &conn, store_name, owner, lease_token, &snapshot)?;

    // A building generation created before legacy history was attributed cannot
    // be carried across the recovery boundary, even if its own descriptor is
    // current. If active is discarded, previous must also be discarded so a
    // physical backend cannot silently promote it as a new active generation.
    let reset_active = active_incompatible;
    let reset_previous = reset_active || previous_incompatible;
    let reset_building = snapshot.building.generation.is_some();
    let mut generations_to_quarantine = Vec::new();
    if reset_building {
        push_unique_generation(
            &mut generations_to_quarantine,
            snapshot.building.generation.as_deref(),
        );
    }
    if reset_active {
        push_unique_generation(
            &mut generations_to_quarantine,
            snapshot.active.generation.as_deref(),
        );
    }
    if reset_previous {
        push_unique_generation(
            &mut generations_to_quarantine,
            snapshot.previous.generation.as_deref(),
        );
    }

    for generation in &generations_to_quarantine {
        let (role, binding) = if snapshot.building.generation.as_deref() == Some(generation) {
            (ProjectionGenerationRole::Building, &snapshot.building)
        } else if snapshot.active.generation.as_deref() == Some(generation) {
            (ProjectionGenerationRole::Active, &snapshot.active)
        } else {
            (ProjectionGenerationRole::Previous, &snapshot.previous)
        };
        let mut authority = destructive_authority_from_snapshot(
            &snapshot,
            store_name,
            owner,
            lease_token,
            role,
            binding,
        )?;
        let lease = current_lease_snapshot(path, store_name, owner, lease_token)?;
        if lease.fence_epoch != snapshot.lease.fence_epoch {
            return Err(stale_generation(store_name));
        }
        authority.fence_epoch = lease.fence_epoch;
        authority.lease_expires_at = lease.lease_expires_at;
        backend.quarantine_generation_fenced(generation, &authority)?;
    }

    // Each backend operation owns the distinct projection-helper authority
    // lock while it validates SQLite and mutates or reads physical state. Do
    // not hold that suffix lock across child-helper calls here: LanceDB
    // helpers acquire it in the child process, while the SQLite CAS below is
    // already serialized by the service's immediate transaction and checks
    // the bumped lease owner/token/fence snapshot before committing.
    for generation in &generations_to_quarantine {
        if backend.inspect_generation(generation)?.is_some() {
            return Err(KanbanError::Storage(format!(
                "incompatible projection generation {generation} remained addressable after quarantine"
            )));
        }
    }

    let retained_active = if reset_active {
        None
    } else {
        snapshot.active.evidence(
            store_name,
            "active",
            &snapshot.lease.database_instance_id,
            snapshot.lease.protocol_version,
            snapshot.lease.schema_version,
        )?
    };
    let retained_previous = if reset_previous {
        None
    } else {
        snapshot.previous.evidence(
            store_name,
            "previous",
            &snapshot.lease.database_instance_id,
            snapshot.lease.protocol_version,
            snapshot.lease.schema_version,
        )?
    };
    validate_retained_recovery_generation(backend, retained_active.as_ref(), "active")?;
    validate_retained_recovery_generation(backend, retained_previous.as_ref(), "previous")?;
    match backend.inspect_active() {
        Ok(actual) if actual.as_ref() == retained_active.as_ref() => {}
        Ok(Some(actual)) => {
            return Err(KanbanError::Conflict(format!(
                "projection backend exposes unattributed active generation {} during incompatible binding recovery",
                actual.manifest.generation
            )));
        }
        Ok(None) => {
            return Err(KanbanError::Conflict(format!(
                "projection backend lost the retained compatible active generation during incompatible binding recovery for {store_name}"
            )));
        }
        Err(error) => {
            return Err(KanbanError::Conflict(format!(
                "projection backend active generation is unattributed during incompatible binding recovery for {store_name}: {error}"
            )));
        }
    }

    before_final_transaction();
    with_immediate_tx(&conn, || {
        let tx_now = SystemClock.now_ms();
        let current =
            projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, tx_now)?;
        if !snapshot.matches_after_lease_heartbeat(&current) {
            return Err(stale_generation(store_name));
        }
        conn.execute(
            "UPDATE projection_deliveries
             SET status='pending',published_generation=NULL,
                 claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                 claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,
                 updated_at=?1
             WHERE store_name=?2",
            params![tx_now, store_name],
        )
        .map_err(storage)?;
        recompute_checkpoint(&conn, store_name, tx_now)?;
        let changed = conn
            .execute(
                "UPDATE projection_store_state
                 SET building_generation=CASE WHEN ?3 THEN NULL ELSE building_generation END,
                     building_fingerprint=CASE WHEN ?3 THEN NULL ELSE building_fingerprint END,
                     building_fence_epoch=CASE WHEN ?3 THEN NULL ELSE building_fence_epoch END,
                     building_provider=CASE WHEN ?3 THEN NULL ELSE building_provider END,
                     building_provider_fingerprint=CASE WHEN ?3 THEN NULL ELSE building_provider_fingerprint END,
                     building_corpus_schema=CASE WHEN ?3 THEN NULL ELSE building_corpus_schema END,
                     building_corpus_fingerprint=CASE WHEN ?3 THEN NULL ELSE building_corpus_fingerprint END,
                     building_embedding_model=CASE WHEN ?3 THEN NULL ELSE building_embedding_model END,
                     building_embedding_dimensions=CASE WHEN ?3 THEN NULL ELSE building_embedding_dimensions END,
                     building_canonical_count=CASE WHEN ?3 THEN NULL ELSE building_canonical_count END,
                     building_canonical_digest=CASE WHEN ?3 THEN NULL ELSE building_canonical_digest END,
                     building_delivery_count=CASE WHEN ?3 THEN NULL ELSE building_delivery_count END,
                     building_delivery_digest=CASE WHEN ?3 THEN NULL ELSE building_delivery_digest END,
                     building_phase=CASE WHEN ?3 THEN NULL ELSE building_phase END,
                     active_generation=CASE WHEN ?4 THEN NULL ELSE active_generation END,
                     active_fingerprint=CASE WHEN ?4 THEN NULL ELSE active_fingerprint END,
                     active_fence_epoch=CASE WHEN ?4 THEN NULL ELSE active_fence_epoch END,
                     active_snapshot_cursor=CASE WHEN ?4 THEN NULL ELSE active_snapshot_cursor END,
                     active_provider=CASE WHEN ?4 THEN NULL ELSE active_provider END,
                     active_provider_fingerprint=CASE WHEN ?4 THEN NULL ELSE active_provider_fingerprint END,
                     active_corpus_schema=CASE WHEN ?4 THEN NULL ELSE active_corpus_schema END,
                     active_corpus_fingerprint=CASE WHEN ?4 THEN NULL ELSE active_corpus_fingerprint END,
                     active_embedding_model=CASE WHEN ?4 THEN NULL ELSE active_embedding_model END,
                     active_embedding_dimensions=CASE WHEN ?4 THEN NULL ELSE active_embedding_dimensions END,
                     active_canonical_count=CASE WHEN ?4 THEN NULL ELSE active_canonical_count END,
                     active_canonical_digest=CASE WHEN ?4 THEN NULL ELSE active_canonical_digest END,
                     active_delivery_count=CASE WHEN ?4 THEN NULL ELSE active_delivery_count END,
                     active_delivery_digest=CASE WHEN ?4 THEN NULL ELSE active_delivery_digest END,
                     previous_generation=CASE WHEN ?5 THEN NULL ELSE previous_generation END,
                     previous_fingerprint=CASE WHEN ?5 THEN NULL ELSE previous_fingerprint END,
                     previous_fence_epoch=CASE WHEN ?5 THEN NULL ELSE previous_fence_epoch END,
                     previous_snapshot_cursor=CASE WHEN ?5 THEN NULL ELSE previous_snapshot_cursor END,
                     previous_provider=CASE WHEN ?5 THEN NULL ELSE previous_provider END,
                     previous_provider_fingerprint=CASE WHEN ?5 THEN NULL ELSE previous_provider_fingerprint END,
                     previous_corpus_schema=CASE WHEN ?5 THEN NULL ELSE previous_corpus_schema END,
                     previous_corpus_fingerprint=CASE WHEN ?5 THEN NULL ELSE previous_corpus_fingerprint END,
                     previous_embedding_model=CASE WHEN ?5 THEN NULL ELSE previous_embedding_model END,
                     previous_embedding_dimensions=CASE WHEN ?5 THEN NULL ELSE previous_embedding_dimensions END,
                     previous_canonical_count=CASE WHEN ?5 THEN NULL ELSE previous_canonical_count END,
                     previous_canonical_digest=CASE WHEN ?5 THEN NULL ELSE previous_canonical_digest END,
                     previous_delivery_count=CASE WHEN ?5 THEN NULL ELSE previous_delivery_count END,
                     previous_delivery_digest=CASE WHEN ?5 THEN NULL ELSE previous_delivery_digest END,
                     lifecycle_status=CASE
                       WHEN ?4 OR active_generation IS NULL THEN 'bootstrap_required'
                       ELSE 'ready'
                     END,
                     last_success_at=CASE WHEN ?4 THEN NULL ELSE last_success_at END,
                     last_error=NULL,updated_at=?1
                 WHERE store_name=?2",
                params![
                    tx_now,
                    store_name,
                    reset_building,
                    reset_active,
                    reset_previous
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        Ok(())
    })?;
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbortBinding {
    Exact,
    Incompatible,
}

/// Abort the exact building generation that a failed snapshot prepare was
/// authorized to materialize.  The caller already holds the service's generic
/// store write guard; the backend owns the distinct projection-helper suffix
/// for the physical operation and validates the capability after acquiring it.
/// SQLite cleanup is a second fenced CAS, so a lease rollover between the
/// physical abort and this transaction cannot clear a successor's generation.
fn abort_projection_generation_with_authority(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    authority: &ProjectionDestructiveAuthority,
) -> Result<()> {
    abort_projection_generation_with_authority_before_final_transaction(
        path,
        store_name,
        owner,
        lease_token,
        backend,
        authority,
        || {},
    )
}

fn abort_projection_generation_with_authority_before_final_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    authority: &ProjectionDestructiveAuthority,
    before_final_transaction: impl FnOnce(),
) -> Result<()> {
    if authority.generation.trim().is_empty()
        || authority.owner != owner
        || authority.lease_token != lease_token
        || authority.role != ProjectionGenerationRole::Building
    {
        return Err(stale_generation(store_name));
    }

    // Snapshot the exact binding before asking the provider to mutate.  This
    // is a cheap fail-closed check for fakes and legacy adapters; real
    // providers repeat the same check while holding their helper suffix.
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let snapshot =
        projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, now)?;
    snapshot.validate_shape(store_name)?;
    let current = destructive_authority_from_snapshot(
        &snapshot,
        store_name,
        owner,
        lease_token,
        ProjectionGenerationRole::Building,
        &snapshot.building,
    )?;
    if current.fence_epoch != authority.fence_epoch
        || current.generation != authority.generation
        || current.expected_binding != authority.expected_binding
        || current.expected_manifest != authority.expected_manifest
        || current.building_phase != authority.building_phase
    {
        return Err(stale_generation(store_name));
    }

    // Never remove a physical generation that is currently discoverable as an
    // active publication.  A prepare failure must be recovered through the
    // publish/reconcile path if it reached that state.
    match backend.inspect_active()? {
        Some(active) if active.manifest.generation == authority.generation => {
            return Err(KanbanError::Conflict(format!(
                "projection generation {} is physically active and must be reconciled instead of aborted",
                authority.generation
            )));
        }
        Some(_) | None => {}
    }
    backend.abort_generation_fenced(&authority.generation, authority)?;
    if backend.inspect_generation(&authority.generation)?.is_some() {
        return Err(KanbanError::Storage(format!(
            "aborted projection generation {} remained addressable",
            authority.generation
        )));
    }

    before_final_transaction();
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        require_current_lease(&conn, store_name, owner, lease_token, now)?;
        let current: (Option<String>, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT building_generation,building_fence_epoch,building_phase
                 FROM projection_store_state WHERE store_name=?1",
                [store_name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(storage)?;
        if current.0.as_deref() != Some(authority.generation.as_str())
            || current.1 != Some(authority.expected_binding.fence_epoch)
            || current.2.as_deref() != authority.building_phase.as_deref()
        {
            return Err(stale_generation(store_name));
        }
        conn.execute(
            "UPDATE projection_deliveries
             SET status='pending',published_generation=NULL,
                 claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                 claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,
                 last_error='generation aborted after snapshot prepare failure',updated_at=?1
             WHERE store_name=?2
               AND (published_generation=?3 OR claim_generation=?3)",
            params![now, store_name, authority.generation],
        )
        .map_err(storage)?;
        recompute_checkpoint(&conn, store_name, now)?;
        let changed = conn
            .execute(
                "UPDATE projection_store_state
                 SET building_generation=NULL,building_fingerprint=NULL,
                     building_fence_epoch=NULL,building_provider=NULL,
                     building_provider_fingerprint=NULL,building_corpus_schema=NULL,
                     building_corpus_fingerprint=NULL,building_embedding_model=NULL,
                     building_embedding_dimensions=NULL,building_canonical_count=NULL,
                     building_canonical_digest=NULL,building_delivery_count=NULL,
                     building_delivery_digest=NULL,building_phase=NULL,
                     lifecycle_status=CASE WHEN active_generation IS NULL
                                           THEN 'bootstrap_required' ELSE 'ready' END,
                     last_error=NULL,updated_at=?1
                 WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4
                   AND lease_expires_at>?1 AND building_generation=?5
                   AND building_fence_epoch=?6",
                params![
                    now,
                    store_name,
                    owner,
                    lease_token,
                    authority.generation,
                    authority.expected_binding.fence_epoch,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        Ok(())
    })
}

fn abort_projection_generation_with_binding(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    binding: AbortBinding,
) -> Result<()> {
    abort_projection_generation_with_binding_before_final_transaction(
        path,
        store_name,
        owner,
        lease_token,
        backend,
        binding,
        || {},
    )
}

fn abort_projection_generation_with_binding_before_final_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
    binding: AbortBinding,
    before_final_transaction: impl FnOnce(),
) -> Result<()> {
    let _write_guard = crate::db::acquire_derived_store_write_guard(path, store_name)?;
    let manifest = building_manifest(path, store_name, owner, lease_token)?;
    // Capture the complete logical binding before any provider mutation.  In
    // particular, the lease fence and every active/previous/building binding
    // field belong to this abort capability; a same-owner/token fence rollover
    // must not be able to reuse it at the final SQLite CAS.
    let snapshot_connection = connect_file(path)?;
    let expected_snapshot = projection_binding_recovery_snapshot(
        &snapshot_connection,
        store_name,
        owner,
        lease_token,
        SystemClock.now_ms(),
    )?;
    expected_snapshot.validate_shape(store_name)?;
    let active = expected_snapshot.active.evidence(
        store_name,
        "active",
        &expected_snapshot.lease.database_instance_id,
        expected_snapshot.lease.protocol_version,
        expected_snapshot.lease.schema_version,
    )?;
    let previous = expected_snapshot.previous.evidence(
        store_name,
        "previous",
        &expected_snapshot.lease.database_instance_id,
        expected_snapshot.lease.protocol_version,
        expected_snapshot.lease.schema_version,
    )?;
    let (descriptor, active, previous, reset_active, reset_previous) = match binding {
        AbortBinding::Exact => {
            validate_backend_binding(backend, &manifest)?;
            (None, active, previous, false, false)
        }
        AbortBinding::Incompatible => {
            let descriptor = backend.descriptor()?;
            validate_store_descriptor(store_name, &descriptor)?;
            if validate_descriptor_binding(
                store_name,
                &manifest.provider,
                &manifest.provider_fingerprint,
                manifest.corpus.as_ref(),
                &descriptor,
            )
            .is_ok()
            {
                return Err(KanbanError::Conflict(format!(
                    "projection generation {} is compatible with the current backend and cannot use incompatible-generation recovery",
                    manifest.generation
                )));
            }
            let reset_active = active.as_ref().is_some_and(|evidence| {
                validate_descriptor_binding(
                    store_name,
                    &evidence.manifest.provider,
                    &evidence.manifest.provider_fingerprint,
                    evidence.manifest.corpus.as_ref(),
                    &descriptor,
                )
                .is_err()
            });
            let reset_previous = previous.as_ref().is_some_and(|evidence| {
                reset_active
                    || validate_descriptor_binding(
                        store_name,
                        &evidence.manifest.provider,
                        &evidence.manifest.provider_fingerprint,
                        evidence.manifest.corpus.as_ref(),
                        &descriptor,
                    )
                    .is_err()
            });
            (
                Some(descriptor),
                active,
                previous,
                reset_active,
                reset_previous,
            )
        }
    };
    if binding == AbortBinding::Exact {
        match backend.inspect_active() {
            Ok(Some(active)) if active.manifest.generation == manifest.generation => {
                let matches_sqlite = manifest.fingerprint.as_ref().is_some_and(|fingerprint| {
                    same_artifact(
                        &ProjectionArtifactEvidence {
                            manifest: manifest.clone(),
                            fingerprint: fingerprint.clone(),
                        },
                        &active,
                    )
                });
                if matches_sqlite {
                    return Err(KanbanError::Conflict(format!(
                        "projection generation {} is physically active and must be reconciled instead of aborted",
                        manifest.generation
                    )));
                }
            }
            Ok(Some(_) | None) | Err(KanbanError::Conflict(_)) => {}
            Err(error) => return Err(error),
        }
    }
    let mut generations_to_quarantine = vec![manifest.generation.clone()];
    if reset_active
        && let Some(active) = &active
        && !generations_to_quarantine.contains(&active.manifest.generation)
    {
        generations_to_quarantine.push(active.manifest.generation.clone());
    }
    if reset_previous
        && let Some(previous) = &previous
        && !generations_to_quarantine.contains(&previous.manifest.generation)
    {
        generations_to_quarantine.push(previous.manifest.generation.clone());
    }
    for generation in &generations_to_quarantine {
        let authority = if generation == &manifest.generation {
            let mut authority = destructive_authority_from_snapshot(
                &expected_snapshot,
                store_name,
                owner,
                lease_token,
                ProjectionGenerationRole::Building,
                &expected_snapshot.building,
            )?;
            if authority.generation != *generation
                || authority.generation != manifest.generation
                || authority.expected_binding.fence_epoch != manifest.fence_epoch
            {
                return Err(stale_generation(store_name));
            }
            let lease = current_lease_snapshot(path, store_name, owner, lease_token)?;
            if lease.fence_epoch != expected_snapshot.lease.fence_epoch {
                return Err(stale_generation(store_name));
            }
            authority.lease_expires_at = lease.lease_expires_at;
            authority
        } else if let Some(active) = active
            .as_ref()
            .filter(|e| e.manifest.generation == *generation)
        {
            let lease = current_lease_snapshot(path, store_name, owner, lease_token)?;
            if lease.fence_epoch != expected_snapshot.lease.fence_epoch {
                return Err(stale_generation(store_name));
            }
            destructive_authority_from_evidence(
                owner,
                lease_token,
                ProjectionGenerationRole::Active,
                lease.fence_epoch,
                lease.lease_expires_at,
                active,
            )
        } else if let Some(previous) = previous
            .as_ref()
            .filter(|e| e.manifest.generation == *generation)
        {
            let lease = current_lease_snapshot(path, store_name, owner, lease_token)?;
            if lease.fence_epoch != expected_snapshot.lease.fence_epoch {
                return Err(stale_generation(store_name));
            }
            destructive_authority_from_evidence(
                owner,
                lease_token,
                ProjectionGenerationRole::Previous,
                lease.fence_epoch,
                lease.lease_expires_at,
                previous,
            )
        } else {
            return Err(KanbanError::Storage(format!(
                "projection generation {generation} has no destructive authority binding"
            )));
        };
        backend.quarantine_generation_fenced(generation, &authority)?;
        if backend.inspect_generation(generation)?.is_some() {
            return Err(KanbanError::Storage(format!(
                "abandoned projection generation {generation} remained addressable after quarantine"
            )));
        }
    }
    let expected_active_after_quarantine = if reset_active { None } else { active.as_ref() };
    match backend.inspect_active() {
        Ok(actual) if actual.as_ref() == expected_active_after_quarantine => {}
        Ok(Some(actual)) => {
            return Err(KanbanError::Storage(format!(
                "projection generation {} remained unexpectedly active after quarantine",
                actual.manifest.generation
            )));
        }
        Ok(None) => {
            return Err(KanbanError::Storage(
                "projection backend lost the compatible active generation during quarantine"
                    .to_owned(),
            ));
        }
        Err(error) => {
            if descriptor.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "projection backend still exposes an unattributed incompatible active generation after quarantine: {error}"
                )));
            }
            return Err(error);
        }
    }
    before_final_transaction();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        let current =
            projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, now)?;
        current.validate_shape(store_name)?;
        if !expected_snapshot.matches_after_lease_heartbeat(&current) {
            return Err(stale_generation(store_name));
        }
        let Some(building) = current.building.generation.as_deref() else {
            return Err(KanbanError::Conflict(format!(
                "projection store {store_name} has no building generation to abort"
            )));
        };
        if building != manifest.generation {
            return Err(stale_generation(store_name));
        }
        if reset_active {
            conn.execute(
                "UPDATE projection_deliveries
                 SET status='pending',published_generation=NULL,
                     claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                     claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL,
                     last_error='backend binding generation reset before rebuild',updated_at=?1
                 WHERE store_name=?2",
                params![now, store_name],
            )
            .map_err(storage)?;
        } else {
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
        }
        recompute_checkpoint(&conn, store_name, now)?;
        let changed = conn
            .execute(
            "UPDATE projection_store_state
             SET building_generation=NULL,building_fingerprint=NULL,building_fence_epoch=NULL,
                 building_provider=NULL,building_provider_fingerprint=NULL,
                 building_corpus_schema=NULL,building_corpus_fingerprint=NULL,
                 building_embedding_model=NULL,building_embedding_dimensions=NULL,
                 building_canonical_count=NULL,building_canonical_digest=NULL,
                 building_delivery_count=NULL,building_delivery_digest=NULL,
                 building_phase=NULL,
                 active_generation=CASE WHEN ?10 THEN NULL ELSE active_generation END,
                 active_fingerprint=CASE WHEN ?10 THEN NULL ELSE active_fingerprint END,
                 active_fence_epoch=CASE WHEN ?10 THEN NULL ELSE active_fence_epoch END,
                 active_snapshot_cursor=CASE WHEN ?10 THEN NULL ELSE active_snapshot_cursor END,
                 active_provider=CASE WHEN ?10 THEN NULL ELSE active_provider END,
                 active_provider_fingerprint=CASE WHEN ?10 THEN NULL ELSE active_provider_fingerprint END,
                 active_corpus_schema=CASE WHEN ?10 THEN NULL ELSE active_corpus_schema END,
                 active_corpus_fingerprint=CASE WHEN ?10 THEN NULL ELSE active_corpus_fingerprint END,
                 active_embedding_model=CASE WHEN ?10 THEN NULL ELSE active_embedding_model END,
                 active_embedding_dimensions=CASE WHEN ?10 THEN NULL ELSE active_embedding_dimensions END,
                 active_canonical_count=CASE WHEN ?10 THEN NULL ELSE active_canonical_count END,
                 active_canonical_digest=CASE WHEN ?10 THEN NULL ELSE active_canonical_digest END,
                 active_delivery_count=CASE WHEN ?10 THEN NULL ELSE active_delivery_count END,
                 active_delivery_digest=CASE WHEN ?10 THEN NULL ELSE active_delivery_digest END,
                 previous_generation=CASE WHEN ?11 THEN NULL ELSE previous_generation END,
                 previous_fingerprint=CASE WHEN ?11 THEN NULL ELSE previous_fingerprint END,
                 previous_fence_epoch=CASE WHEN ?11 THEN NULL ELSE previous_fence_epoch END,
                 previous_snapshot_cursor=CASE WHEN ?11 THEN NULL ELSE previous_snapshot_cursor END,
                 previous_provider=CASE WHEN ?11 THEN NULL ELSE previous_provider END,
                 previous_provider_fingerprint=CASE WHEN ?11 THEN NULL ELSE previous_provider_fingerprint END,
                 previous_corpus_schema=CASE WHEN ?11 THEN NULL ELSE previous_corpus_schema END,
                 previous_corpus_fingerprint=CASE WHEN ?11 THEN NULL ELSE previous_corpus_fingerprint END,
                 previous_embedding_model=CASE WHEN ?11 THEN NULL ELSE previous_embedding_model END,
                 previous_embedding_dimensions=CASE WHEN ?11 THEN NULL ELSE previous_embedding_dimensions END,
                 previous_canonical_count=CASE WHEN ?11 THEN NULL ELSE previous_canonical_count END,
                 previous_canonical_digest=CASE WHEN ?11 THEN NULL ELSE previous_canonical_digest END,
                 previous_delivery_count=CASE WHEN ?11 THEN NULL ELSE previous_delivery_count END,
                 previous_delivery_digest=CASE WHEN ?11 THEN NULL ELSE previous_delivery_digest END,
                 lifecycle_status=CASE
                   WHEN ?10 OR active_generation IS NULL THEN 'bootstrap_required'
                   ELSE 'ready'
                 END,
                 last_success_at=CASE WHEN ?10 THEN NULL ELSE last_success_at END,
                 last_error=NULL,updated_at=?1
             WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4
               AND lease_expires_at>?1 AND fence_epoch=?5
               AND building_generation=?6 AND building_fence_epoch IS ?7
               AND active_generation IS ?8 AND previous_generation IS ?9
               AND building_fingerprint IS ?12
               AND building_provider IS ?13
               AND building_provider_fingerprint IS ?14
               AND building_canonical_count IS ?15
               AND building_canonical_digest IS ?16
               AND building_delivery_count IS ?17
               AND building_delivery_digest IS ?18
               AND building_corpus_schema IS ?19
               AND building_corpus_fingerprint IS ?20
               AND building_embedding_model IS ?21
               AND building_embedding_dimensions IS ?22
               AND building_phase IS ?23
               AND active_fingerprint IS ?24
               AND active_fence_epoch IS ?25
               AND active_snapshot_cursor IS ?26
               AND active_provider IS ?27
               AND active_provider_fingerprint IS ?28
               AND active_canonical_count IS ?29
               AND active_canonical_digest IS ?30
               AND active_delivery_count IS ?31
               AND active_delivery_digest IS ?32
               AND active_corpus_schema IS ?33
               AND active_corpus_fingerprint IS ?34
               AND active_embedding_model IS ?35
               AND active_embedding_dimensions IS ?36
               AND previous_fingerprint IS ?37
               AND previous_fence_epoch IS ?38
               AND previous_snapshot_cursor IS ?39
               AND previous_provider IS ?40
               AND previous_provider_fingerprint IS ?41
               AND previous_canonical_count IS ?42
               AND previous_canonical_digest IS ?43
               AND previous_delivery_count IS ?44
               AND previous_delivery_digest IS ?45
               AND previous_corpus_schema IS ?46
               AND previous_corpus_fingerprint IS ?47
               AND previous_embedding_model IS ?48
               AND previous_embedding_dimensions IS ?49",
                params![
                    now,
                    store_name,
                    owner,
                    lease_token,
                    expected_snapshot.lease.fence_epoch,
                    building,
                    expected_snapshot.building.fence_epoch,
                    expected_snapshot.active.generation.as_deref(),
                    expected_snapshot.previous.generation.as_deref(),
                    reset_active,
                    reset_previous,
                    expected_snapshot.building.fingerprint.as_deref(),
                    expected_snapshot.building.provider.as_deref(),
                    expected_snapshot
                        .building
                        .provider_fingerprint
                        .as_deref(),
                    expected_snapshot.building.canonical_count,
                    expected_snapshot.building.canonical_digest.as_deref(),
                    expected_snapshot.building.delivery_count,
                    expected_snapshot.building.delivery_digest.as_deref(),
                    expected_snapshot.building.corpus_schema.as_deref(),
                    expected_snapshot.building.corpus_fingerprint.as_deref(),
                    expected_snapshot.building.embedding_model.as_deref(),
                    expected_snapshot.building.embedding_dimensions,
                    expected_snapshot.building.phase.as_deref(),
                    expected_snapshot.active.fingerprint.as_deref(),
                    expected_snapshot.active.fence_epoch,
                    expected_snapshot.active.snapshot_cursor,
                    expected_snapshot.active.provider.as_deref(),
                    expected_snapshot.active.provider_fingerprint.as_deref(),
                    expected_snapshot.active.canonical_count,
                    expected_snapshot.active.canonical_digest.as_deref(),
                    expected_snapshot.active.delivery_count,
                    expected_snapshot.active.delivery_digest.as_deref(),
                    expected_snapshot.active.corpus_schema.as_deref(),
                    expected_snapshot.active.corpus_fingerprint.as_deref(),
                    expected_snapshot.active.embedding_model.as_deref(),
                    expected_snapshot.active.embedding_dimensions,
                    expected_snapshot.previous.fingerprint.as_deref(),
                    expected_snapshot.previous.fence_epoch,
                    expected_snapshot.previous.snapshot_cursor,
                    expected_snapshot.previous.provider.as_deref(),
                    expected_snapshot
                        .previous
                        .provider_fingerprint
                        .as_deref(),
                    expected_snapshot.previous.canonical_count,
                    expected_snapshot.previous.canonical_digest.as_deref(),
                    expected_snapshot.previous.delivery_count,
                    expected_snapshot.previous.delivery_digest.as_deref(),
                    expected_snapshot.previous.corpus_schema.as_deref(),
                    expected_snapshot.previous.corpus_fingerprint.as_deref(),
                    expected_snapshot.previous.embedding_model.as_deref(),
                    expected_snapshot.previous.embedding_dimensions,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        reconcile_legacy_store_state(&conn, store_name, now)?;
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
    validate_backend_for_target(path, store_name, owner, lease_token, backend)?;
    let batch = claim_projection_batch(path, store_name, owner, lease_token, claim_ttl_ms, limit)?;
    if batch.items.is_empty() {
        return Ok(batch);
    }
    let batch_authority = authority_for_generation(
        path,
        store_name,
        owner,
        lease_token,
        &batch.target_generation,
    )?;
    match backend.apply_batch_with_authority(&batch, &batch_authority) {
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
    let publish_authority = authority_for_generation(
        path,
        store_name,
        owner,
        lease_token,
        &prepared.manifest.generation,
    )?;
    let expected_active = active_artifact(path, store_name)?;
    let operation: Result<ProjectionArtifactEvidence> = (|| {
        let receipt = match backend.inspect_active() {
            Ok(Some(active)) if same_artifact(&prepared, &active) => ProjectionPublishReceipt {
                active,
                retained_previous: inspect_expected_previous(backend, expected_active.as_ref())?,
            },
            Ok(_) | Err(_) => backend.publish_generation_with_authority(
                expected_active.as_ref(),
                &prepared,
                &publish_authority,
            )?,
        };
        let active = validate_publish_receipt(
            backend,
            store_name,
            &prepared,
            expected_active.as_ref(),
            &publish_authority,
            receipt,
        )?;
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
        let error_lease = lease_from_destructive_authority(store_name, &publish_authority);
        if let Err(record_error) =
            record_projection_error(path, store_name, &error_lease, &error.to_string())
            && !matches!(&record_error, KanbanError::Conflict(_))
        {
            return Err(record_error);
        }
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
    let publish_authority = authority_for_generation(
        path,
        store_name,
        owner,
        lease_token,
        &prepared.manifest.generation,
    )?;
    let missing_active = active_artifact(path, store_name)?.ok_or_else(|| {
        KanbanError::Conflict(format!(
            "projection recovery requires a logical active generation for {store_name}"
        ))
    })?;
    let expected_previous = previous_artifact(path, store_name)?;
    let operation = (|| {
        let expected_retained =
            match backend.inspect_generation(&missing_active.manifest.generation) {
                Ok(Some(actual)) if same_artifact(&actual, &missing_active) => {
                    let repair_authority = authority_for_generation(
                        path,
                        store_name,
                        owner,
                        lease_token,
                        &missing_active.manifest.generation,
                    )?;
                    if backend
                        .validate_generation_publication(&missing_active)
                        .is_err()
                    {
                        backend.repair_generation_publication_with_authority(
                            &missing_active,
                            &repair_authority,
                        )?;
                    }
                    backend.validate_generation_publication_with_authority(
                        &missing_active,
                        &repair_authority,
                    )?;
                    Some(missing_active.clone())
                }
                Ok(Some(_)) | Err(KanbanError::Conflict(_)) => {
                    let lease = current_lease_snapshot(path, store_name, owner, lease_token)?;
                    let authority = destructive_authority_from_evidence(
                        owner,
                        lease_token,
                        ProjectionGenerationRole::Active,
                        lease.fence_epoch,
                        lease.lease_expires_at,
                        &missing_active,
                    );
                    quarantine_unreadable_generation(
                        backend,
                        &missing_active.manifest.generation,
                        &authority,
                    )?;
                    expected_previous
                }
                Ok(None) => expected_previous,
                Err(error) => return Err(error),
            };
        inspect_expected_previous(backend, expected_retained.as_ref())?;
        let physical_active = match backend.inspect_active() {
            Ok(Some(active)) if same_artifact(&prepared, &active) => {
                let receipt = ProjectionPublishReceipt {
                    active,
                    retained_previous: expected_retained.clone(),
                };
                let active = validate_publish_receipt(
                    backend,
                    store_name,
                    &prepared,
                    expected_retained.as_ref(),
                    &publish_authority,
                    receipt,
                )?;
                confirm_published_generation(
                    path,
                    store_name,
                    owner,
                    lease_token,
                    &active,
                    expected_retained.as_ref(),
                )?;
                return Ok(active);
            }
            Ok(active) => active,
            Err(_) => {
                let receipt = backend.publish_generation_with_authority(
                    expected_retained.as_ref(),
                    &prepared,
                    &publish_authority,
                )?;
                let active = validate_publish_receipt(
                    backend,
                    store_name,
                    &prepared,
                    expected_retained.as_ref(),
                    &publish_authority,
                    receipt,
                )?;
                confirm_published_generation(
                    path,
                    store_name,
                    owner,
                    lease_token,
                    &active,
                    expected_retained.as_ref(),
                )?;
                return Ok(active);
            }
        };
        if physical_active != expected_retained {
            return Err(KanbanError::Conflict(format!(
                "projection recovery found an unexpected physical predecessor for {store_name}"
            )));
        }
        let receipt = backend.publish_generation_with_authority(
            expected_retained.as_ref(),
            &prepared,
            &publish_authority,
        )?;
        let active = validate_publish_receipt(
            backend,
            store_name,
            &prepared,
            expected_retained.as_ref(),
            &publish_authority,
            receipt,
        )?;
        confirm_published_generation(
            path,
            store_name,
            owner,
            lease_token,
            &active,
            expected_retained.as_ref(),
        )?;
        Ok(active)
    })();
    if let Err(error) = &operation {
        let error_lease = lease_from_destructive_authority(store_name, &publish_authority);
        if let Err(record_error) =
            record_projection_error(path, store_name, &error_lease, &error.to_string())
            && !matches!(&record_error, KanbanError::Conflict(_))
        {
            return Err(record_error);
        }
    }
    operation
}

fn quarantine_unreadable_generation(
    backend: &(impl ProjectionStoreBackend + ?Sized),
    generation: &str,
    authority: &ProjectionDestructiveAuthority,
) -> Result<()> {
    backend
        .quarantine_generation_fenced(generation, authority)
        .map_err(|error| {
            KanbanError::Storage(format!(
                "projection backend could not non-destructively quarantine unreadable generation \
             {generation}: {error}"
            ))
        })
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
    let publish_authority = authority_for_generation(
        path,
        store_name,
        owner,
        lease_token,
        &prepared.manifest.generation,
    )?;
    let operation = (|| {
        let expected_previous = active_artifact(path, store_name)?;
        let receipt = match backend.inspect_active() {
            Ok(Some(active)) if same_artifact(&prepared, &active) => ProjectionPublishReceipt {
                active,
                retained_previous: expected_previous.clone(),
            },
            Ok(Some(_)) => {
                return Err(KanbanError::Conflict(format!(
                    "projection store generation does not match SQLite building state for {store_name}"
                )));
            }
            Ok(None) => {
                return Err(KanbanError::Conflict(format!(
                    "projection store has no published generation to reconcile for {store_name}"
                )));
            }
            Err(_) => backend.publish_generation_with_authority(
                expected_previous.as_ref(),
                &prepared,
                &publish_authority,
            )?,
        };
        let active = validate_publish_receipt(
            backend,
            store_name,
            &prepared,
            expected_previous.as_ref(),
            &publish_authority,
            receipt,
        )?;
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
        let error_lease = lease_from_destructive_authority(store_name, &publish_authority);
        if let Err(record_error) =
            record_projection_error(path, store_name, &error_lease, &error.to_string())
            && !matches!(&record_error, KanbanError::Conflict(_))
        {
            return Err(record_error);
        }
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
    claim_projection_batch_with_before_transaction(
        path.as_ref(),
        store_name,
        owner,
        lease_token,
        claim_ttl_ms,
        limit,
        || {},
    )
}

fn claim_projection_batch_with_before_transaction(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    claim_ttl_ms: i64,
    limit: usize,
    before_transaction: impl FnOnce(),
) -> Result<ProjectionBatch> {
    validate_owner_and_ttl(owner, claim_ttl_ms)?;
    if limit == 0 || limit > MAX_PROJECTION_BATCH {
        return Err(KanbanError::InvalidInput(format!(
            "projection claim limit must be between 1 and {MAX_PROJECTION_BATCH}"
        )));
    }
    let claim_token = new_typed_id("pclaim");
    let conn = connect_file(path)?;
    // Keep the fence observed before waiting for SQLite's write lock as the
    // capability this queued operation was created under. A same-owner
    // rollover keeps owner/token stable, so the final transaction must CAS
    // this fence explicitly rather than silently adopting the successor.
    let expected_fence_epoch =
        current_lease(&conn, store_name, owner, lease_token, SystemClock.now_ms())?.fence_epoch;
    before_transaction();
    let (lease, target_generation, provider, provider_fingerprint, corpus, claim_expires_at, items) =
        with_immediate_tx(&conn, || {
            // BEGIN IMMEDIATE may wait behind another writer. Refresh the
            // clock and authority only after that lock is held so an expired
            // lease cannot clear or claim any delivery rows.
            let now = SystemClock.now_ms();
            let lease = current_lease(&conn, store_name, owner, lease_token, now)?;
            if lease.fence_epoch != expected_fence_epoch {
                return Err(projection_lease_conflict(store_name));
            }
            let claim_expires_at = checked_expiry(now, claim_ttl_ms, "projection claim")?;
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
            let (target_generation, provider, provider_fingerprint, corpus) =
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
                corpus,
                claim_expires_at,
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
        corpus,
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
            reconcile_label_atom_board_compatibility(
                &conn,
                &batch.store_name,
                &batch.target_generation,
                batch.items.iter().map(|item| item.board_id.as_str()),
                now,
            )?;
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

fn record_projection_error(
    path: &Path,
    store_name: &str,
    authority: &ProjectionLease,
    error: &str,
) -> Result<()> {
    if authority.store_name != store_name {
        return Err(KanbanError::Conflict(format!(
            "projection error authority targets {}",
            authority.store_name
        )));
    }
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let changed = conn
            .execute(
                "UPDATE projection_store_state
                 SET lifecycle_status='error',last_error=?1,updated_at=?2
                 WHERE store_name=?3 AND lease_owner=?4 AND lease_token=?5
                   AND fence_epoch=?6 AND lease_expires_at>?2",
                params![
                    error,
                    now,
                    store_name,
                    authority.owner,
                    authority.lease_token,
                    authority.fence_epoch,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(projection_lease_conflict(store_name));
        }
        Ok(())
    })
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
        let previous_corpus = previous.and_then(|manifest| manifest.corpus.as_ref());
        let previous_embedding_dimensions = previous_corpus
            .map(|corpus| i64::try_from(corpus.embedding_dimensions))
            .transpose()
            .map_err(|_| {
                KanbanError::Storage(format!(
                    "projection previous corpus dimensions exceed SQLite range for store {store_name}"
                ))
            })?;
        let changed = conn
            .execute(
                "UPDATE projection_store_state \
                 SET previous_generation=?6,previous_fingerprint=?7,\
                     previous_fence_epoch=?8,previous_snapshot_cursor=?9,\
                     previous_provider=?10,previous_provider_fingerprint=?11,\
                     previous_canonical_count=?12,previous_canonical_digest=?13,\
                     previous_delivery_count=?14,previous_delivery_digest=?15,\
                     previous_corpus_schema=?16,previous_corpus_fingerprint=?17,\
                     previous_embedding_model=?18,previous_embedding_dimensions=?19,\
                     active_generation=building_generation,\
                     active_fingerprint=building_fingerprint,\
                     active_fence_epoch=building_fence_epoch,\
                     active_snapshot_cursor=snapshot_cursor,\
                     active_provider=building_provider,\
                     active_provider_fingerprint=building_provider_fingerprint,\
                     active_corpus_schema=building_corpus_schema,\
                     active_corpus_fingerprint=building_corpus_fingerprint,\
                     active_embedding_model=building_embedding_model,\
                     active_embedding_dimensions=building_embedding_dimensions,\
                     active_canonical_count=building_canonical_count,\
                     active_canonical_digest=building_canonical_digest,\
                     active_delivery_count=building_delivery_count,\
                     active_delivery_digest=building_delivery_digest,\
                     building_generation=NULL,building_fingerprint=NULL,\
                     building_fence_epoch=NULL,building_provider=NULL,\
                     building_provider_fingerprint=NULL,\
                     building_corpus_schema=NULL,building_corpus_fingerprint=NULL,\
                     building_embedding_model=NULL,building_embedding_dimensions=NULL,\
                     building_canonical_count=NULL,\
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
                    previous_corpus.map(|corpus| corpus.corpus_schema.as_str()),
                    previous_corpus.map(|corpus| corpus.corpus_fingerprint.as_str()),
                    previous_corpus.map(|corpus| corpus.embedding_model.as_str()),
                    previous_embedding_dimensions,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        let covered_label_boards =
            label_atom_generation_board_ids(&conn, store_name, &active.manifest.generation)?;
        reconcile_label_atom_board_compatibility(
            &conn,
            store_name,
            &active.manifest.generation,
            covered_label_boards.iter().map(String::as_str),
            now,
        )?;
        reconcile_legacy_outbox(&conn, store_name, now)?;
        reconcile_legacy_store_state(&conn, store_name, now)?;
        Ok(())
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectionBindingRecoverySnapshot {
    lease: CurrentLease,
    active: ProjectionGenerationBindingSnapshot,
    previous: ProjectionGenerationBindingSnapshot,
    building: ProjectionGenerationBindingSnapshot,
    control_plane: String,
    snapshot_cursor: i64,
    checkpoint_cursor: i64,
    legacy_checkpoint_cursor: i64,
    lifecycle_status: String,
    last_success_at: Option<i64>,
    last_error: Option<String>,
    updated_at: i64,
}

impl ProjectionBindingRecoverySnapshot {
    fn validate_shape(&self, store_name: &str) -> Result<()> {
        self.active.validate_shape(store_name, "active", true)?;
        self.previous.validate_shape(store_name, "previous", true)?;
        self.building
            .validate_shape(store_name, "building", false)?;
        let generations = [
            ("active", self.active.generation.as_deref()),
            ("previous", self.previous.generation.as_deref()),
            ("building", self.building.generation.as_deref()),
        ];
        for (index, (left_phase, left_generation)) in generations.iter().enumerate() {
            for (right_phase, right_generation) in generations.iter().skip(index + 1) {
                if let (Some(left_generation), Some(right_generation)) =
                    (*left_generation, *right_generation)
                    && left_generation == right_generation
                {
                    return Err(KanbanError::Storage(format!(
                        "projection store {store_name} aliases {left_phase} and {right_phase} to generation {left_generation}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn matches_after_lease_heartbeat(&self, current: &Self) -> bool {
        let mut expected = self.clone();
        let mut current = current.clone();
        expected.lease.lease_expires_at = 0;
        current.lease.lease_expires_at = 0;
        expected.updated_at = 0;
        current.updated_at = 0;
        expected == current
    }
}

fn destructive_authority_from_snapshot(
    snapshot: &ProjectionBindingRecoverySnapshot,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    role: ProjectionGenerationRole,
    binding: &ProjectionGenerationBindingSnapshot,
) -> Result<ProjectionDestructiveAuthority> {
    let expected_manifest = if binding.fingerprint.is_some() {
        binding
            .evidence(
                store_name,
                match role {
                    ProjectionGenerationRole::Active => "active",
                    ProjectionGenerationRole::Previous => "previous",
                    ProjectionGenerationRole::Building | ProjectionGenerationRole::Orphaned => {
                        "building"
                    }
                },
                &snapshot.lease.database_instance_id,
                snapshot.lease.protocol_version,
                snapshot.lease.schema_version,
            )?
            .map(|evidence| evidence.manifest)
    } else {
        None
    };
    let expected_binding = binding.destructive_binding(store_name)?;
    Ok(ProjectionDestructiveAuthority {
        owner: owner.to_owned(),
        lease_token: lease_token.to_owned(),
        fence_epoch: snapshot.lease.fence_epoch,
        lease_expires_at: snapshot.lease.lease_expires_at,
        role,
        generation: expected_binding.generation.clone(),
        expected_manifest,
        expected_binding,
        building_phase: binding.phase.clone(),
    })
}

fn destructive_authority_from_evidence(
    owner: &str,
    lease_token: &str,
    role: ProjectionGenerationRole,
    current_lease_fence_epoch: i64,
    current_lease_expires_at: i64,
    evidence: &ProjectionArtifactEvidence,
) -> ProjectionDestructiveAuthority {
    let manifest = &evidence.manifest;
    ProjectionDestructiveAuthority {
        owner: owner.to_owned(),
        lease_token: lease_token.to_owned(),
        fence_epoch: current_lease_fence_epoch,
        lease_expires_at: current_lease_expires_at,
        role,
        generation: manifest.generation.clone(),
        expected_manifest: Some(manifest.clone()),
        expected_binding: ProjectionGenerationBinding {
            generation: manifest.generation.clone(),
            fingerprint: manifest.fingerprint.clone(),
            fence_epoch: manifest.fence_epoch,
            snapshot_cursor: Some(manifest.snapshot_cursor),
            provider: manifest.provider.clone(),
            provider_fingerprint: manifest.provider_fingerprint.clone(),
            canonical_count: manifest.canonical_item_count,
            canonical_digest: manifest.canonical_digest.clone(),
            delivery_count: manifest.delivery_item_count,
            delivery_digest: manifest.delivery_digest.clone(),
            corpus: manifest.corpus.clone(),
        },
        building_phase: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectionGenerationBindingSnapshot {
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
    phase: Option<String>,
}

impl ProjectionGenerationBindingSnapshot {
    fn destructive_binding(&self, store_name: &str) -> Result<ProjectionGenerationBinding> {
        let generation = self.generation.clone().ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no generation binding"
            ))
        })?;
        let fence_epoch = self.fence_epoch.ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no generation fence"
            ))
        })?;
        let provider = self.provider.clone().ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no generation provider"
            ))
        })?;
        let provider_fingerprint = self.provider_fingerprint.clone().ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no provider fingerprint"
            ))
        })?;
        let canonical_count = self.canonical_count.ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no canonical count"
            ))
        })?;
        let canonical_digest = self.canonical_digest.clone().ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no canonical digest"
            ))
        })?;
        let delivery_count = self.delivery_count.ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no delivery count"
            ))
        })?;
        let delivery_digest = self.delivery_digest.clone().ok_or_else(|| {
            KanbanError::Storage(format!(
                "projection store {store_name} has no delivery digest"
            ))
        })?;
        let corpus = projection_corpus_from_values(
            self.corpus_schema.clone(),
            self.corpus_fingerprint.clone(),
            self.embedding_model.clone(),
            self.embedding_dimensions,
            store_name,
            "destructive authority",
        )?;
        Ok(ProjectionGenerationBinding {
            generation,
            fingerprint: self.fingerprint.clone(),
            fence_epoch,
            snapshot_cursor: self.snapshot_cursor,
            provider,
            provider_fingerprint,
            canonical_count,
            canonical_digest,
            delivery_count,
            delivery_digest,
            corpus,
        })
    }
}

impl ProjectionGenerationBindingSnapshot {
    fn binding_is_incompatible(&self, descriptor: &ProjectionStoreDescriptor) -> bool {
        self.generation.is_some()
            && (self.provider.as_deref() != Some(descriptor.provider.as_str())
                || self.provider_fingerprint.as_deref()
                    != Some(descriptor.provider_fingerprint.as_str())
                || !self.corpus_matches_descriptor(descriptor.corpus.as_ref()))
    }

    fn corpus_matches_descriptor(&self, expected: Option<&ProjectionCorpusMetadata>) -> bool {
        match expected {
            Some(expected) => {
                self.corpus_schema.as_deref() == Some(expected.corpus_schema.as_str())
                    && self.corpus_fingerprint.as_deref()
                        == Some(expected.corpus_fingerprint.as_str())
                    && self.embedding_model.as_deref() == Some(expected.embedding_model.as_str())
                    && self.embedding_dimensions
                        == i64::try_from(expected.embedding_dimensions).ok()
            }
            None => {
                self.corpus_schema.is_none()
                    && self.corpus_fingerprint.is_none()
                    && self.embedding_model.is_none()
                    && self.embedding_dimensions.is_none()
            }
        }
    }

    fn validate_shape(
        &self,
        store_name: &str,
        phase: &str,
        requires_snapshot_cursor: bool,
    ) -> Result<()> {
        if self.generation.is_none() {
            if self.fingerprint.is_some()
                || self.fence_epoch.is_some()
                || self.snapshot_cursor.is_some()
                || self.provider.is_some()
                || self.provider_fingerprint.is_some()
                || self.canonical_count.is_some()
                || self.canonical_digest.is_some()
                || self.delivery_count.is_some()
                || self.delivery_digest.is_some()
                || self.corpus_schema.is_some()
                || self.corpus_fingerprint.is_some()
                || self.embedding_model.is_some()
                || self.embedding_dimensions.is_some()
                || self.phase.is_some()
            {
                return Err(KanbanError::Storage(format!(
                    "projection store {store_name} has orphan {phase} generation metadata"
                )));
            }
            return Ok(());
        }
        if phase == "building"
            && self.phase.as_deref() == Some("snapshotting")
            && (self.fingerprint.is_some() || self.snapshot_cursor.is_some())
        {
            return Err(KanbanError::Storage(format!(
                "projection store {store_name} snapshotting generation has prepared evidence"
            )));
        }
        let prepared_or_published =
            phase != "building" || self.phase.as_deref() != Some("snapshotting");
        if (prepared_or_published && self.fingerprint.is_none())
            || self.fence_epoch.is_none()
            || (requires_snapshot_cursor && self.snapshot_cursor.is_none())
            || self.provider.is_none()
            || self.provider_fingerprint.is_none()
            || self.canonical_count.is_none()
            || self.canonical_digest.is_none()
            || self.delivery_count.is_none()
            || self.delivery_digest.is_none()
            || (phase == "building" && self.phase.is_none())
        {
            return Err(KanbanError::Storage(format!(
                "projection store {store_name} has incomplete {phase} generation evidence"
            )));
        }
        let corpus_fields = [
            self.corpus_schema.is_some(),
            self.corpus_fingerprint.is_some(),
            self.embedding_model.is_some(),
            self.embedding_dimensions.is_some(),
        ];
        if corpus_fields.iter().any(|present| *present)
            && !corpus_fields.iter().all(|present| *present)
        {
            return Err(KanbanError::Storage(format!(
                "projection store {store_name} has incomplete {phase} corpus binding"
            )));
        }
        Ok(())
    }

    fn evidence(
        &self,
        store_name: &str,
        phase: &str,
        database_instance_id: &str,
        protocol_version: i64,
        schema_version: i64,
    ) -> Result<Option<ProjectionArtifactEvidence>> {
        let Some(generation) = &self.generation else {
            return Ok(None);
        };
        let fingerprint = required_artifact_field(
            self.fingerprint.clone(),
            store_name,
            &format!("recovery {phase}_fingerprint"),
        )?;
        let embedding_dimensions = self
            .embedding_dimensions
            .map(|dimensions| {
                usize::try_from(dimensions).map_err(|_| {
                    KanbanError::Storage(format!(
                        "projection store {store_name} has invalid recovery {phase} embedding dimensions"
                    ))
                })
            })
            .transpose()?;
        let corpus = projection_corpus_from_values(
            self.corpus_schema.clone(),
            self.corpus_fingerprint.clone(),
            self.embedding_model.clone(),
            self.embedding_dimensions,
            store_name,
            &format!("recovery {phase}"),
        )?;
        if corpus.as_ref().map(|corpus| corpus.embedding_dimensions) != embedding_dimensions {
            return Err(KanbanError::Storage(format!(
                "projection store {store_name} has inconsistent recovery {phase} corpus binding"
            )));
        }
        Ok(Some(ProjectionArtifactEvidence {
            manifest: ProjectionArtifactManifest {
                store_name: store_name.to_owned(),
                database_instance_id: database_instance_id.to_owned(),
                protocol_version,
                schema_version,
                generation: generation.clone(),
                fence_epoch: required_artifact_field(
                    self.fence_epoch,
                    store_name,
                    &format!("recovery {phase}_fence_epoch"),
                )?,
                snapshot_cursor: required_artifact_field(
                    self.snapshot_cursor,
                    store_name,
                    &format!("recovery {phase}_snapshot_cursor"),
                )?,
                provider: required_artifact_field(
                    self.provider.clone(),
                    store_name,
                    &format!("recovery {phase}_provider"),
                )?,
                provider_fingerprint: required_artifact_field(
                    self.provider_fingerprint.clone(),
                    store_name,
                    &format!("recovery {phase}_provider_fingerprint"),
                )?,
                corpus,
                canonical_item_count: required_artifact_field(
                    self.canonical_count,
                    store_name,
                    &format!("recovery {phase}_canonical_count"),
                )?,
                canonical_digest: required_artifact_field(
                    self.canonical_digest.clone(),
                    store_name,
                    &format!("recovery {phase}_canonical_digest"),
                )?,
                delivery_item_count: required_artifact_field(
                    self.delivery_count,
                    store_name,
                    &format!("recovery {phase}_delivery_count"),
                )?,
                delivery_digest: required_artifact_field(
                    self.delivery_digest.clone(),
                    store_name,
                    &format!("recovery {phase}_delivery_digest"),
                )?,
                fingerprint: Some(fingerprint.clone()),
            },
            fingerprint,
        }))
    }
}

fn projection_binding_recovery_snapshot(
    conn: &Connection,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    now: i64,
) -> Result<ProjectionBindingRecoverySnapshot> {
    conn.query_row(
        "SELECT database_instance_id,protocol_version,schema_version,fence_epoch,lease_expires_at,
                active_generation,active_fingerprint,active_fence_epoch,active_snapshot_cursor,
                active_provider,active_provider_fingerprint,
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
                building_provider,building_provider_fingerprint,
                building_canonical_count,building_canonical_digest,
                building_delivery_count,building_delivery_digest,
                building_corpus_schema,building_corpus_fingerprint,
                building_embedding_model,building_embedding_dimensions,building_phase,
                control_plane,snapshot_cursor,checkpoint_cursor,legacy_checkpoint_cursor,
                lifecycle_status,last_success_at,last_error,updated_at
         FROM projection_store_state
         WHERE store_name=?1 AND lease_owner=?2 AND lease_token=?3
           AND lease_expires_at>?4",
        params![store_name, owner, lease_token, now],
        |row| {
            Ok(ProjectionBindingRecoverySnapshot {
                lease: CurrentLease {
                    database_instance_id: row.get(0)?,
                    protocol_version: row.get(1)?,
                    schema_version: row.get(2)?,
                    fence_epoch: row.get(3)?,
                    lease_expires_at: row.get(4)?,
                },
                active: ProjectionGenerationBindingSnapshot {
                    generation: row.get(5)?,
                    fingerprint: row.get(6)?,
                    fence_epoch: row.get(7)?,
                    snapshot_cursor: row.get(8)?,
                    provider: row.get(9)?,
                    provider_fingerprint: row.get(10)?,
                    canonical_count: row.get(11)?,
                    canonical_digest: row.get(12)?,
                    delivery_count: row.get(13)?,
                    delivery_digest: row.get(14)?,
                    corpus_schema: row.get(15)?,
                    corpus_fingerprint: row.get(16)?,
                    embedding_model: row.get(17)?,
                    embedding_dimensions: row.get(18)?,
                    phase: None,
                },
                previous: ProjectionGenerationBindingSnapshot {
                    generation: row.get(19)?,
                    fingerprint: row.get(20)?,
                    fence_epoch: row.get(21)?,
                    snapshot_cursor: row.get(22)?,
                    provider: row.get(23)?,
                    provider_fingerprint: row.get(24)?,
                    canonical_count: row.get(25)?,
                    canonical_digest: row.get(26)?,
                    delivery_count: row.get(27)?,
                    delivery_digest: row.get(28)?,
                    corpus_schema: row.get(29)?,
                    corpus_fingerprint: row.get(30)?,
                    embedding_model: row.get(31)?,
                    embedding_dimensions: row.get(32)?,
                    phase: None,
                },
                building: ProjectionGenerationBindingSnapshot {
                    generation: row.get(33)?,
                    fingerprint: row.get(34)?,
                    fence_epoch: row.get(35)?,
                    snapshot_cursor: match row.get::<_, Option<String>>(46)?.as_deref() {
                        Some("prepared" | "store_published") => Some(row.get(48)?),
                        _ => None,
                    },
                    provider: row.get(36)?,
                    provider_fingerprint: row.get(37)?,
                    canonical_count: row.get(38)?,
                    canonical_digest: row.get(39)?,
                    delivery_count: row.get(40)?,
                    delivery_digest: row.get(41)?,
                    corpus_schema: row.get(42)?,
                    corpus_fingerprint: row.get(43)?,
                    embedding_model: row.get(44)?,
                    embedding_dimensions: row.get(45)?,
                    phase: row.get(46)?,
                },
                control_plane: row.get(47)?,
                snapshot_cursor: row.get(48)?,
                checkpoint_cursor: row.get(49)?,
                legacy_checkpoint_cursor: row.get(50)?,
                lifecycle_status: row.get(51)?,
                last_success_at: row.get(52)?,
                last_error: row.get(53)?,
                updated_at: row.get(54)?,
            })
        },
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| projection_lease_conflict(store_name))
}

/// Reserve a fresh store fence for incompatible-generation recovery.
///
/// The suffix authority lock is held only while this SQLite CAS runs. Child
/// helper calls must acquire the suffix themselves, so retaining it across a
/// LanceDB subprocess would be non-reentrant. Bumping the fence before any
/// physical recovery work invalidates queued requests carrying the previous
/// same-owner lease fence; the bump is intentionally not rolled back if a
/// later physical step fails, allowing restart recovery to advance again.
fn bump_recovery_fence(
    path: &Path,
    conn: &Connection,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    expected: &ProjectionBindingRecoverySnapshot,
) -> Result<ProjectionBindingRecoverySnapshot> {
    bump_recovery_fence_with_before_transaction(
        path,
        conn,
        store_name,
        owner,
        lease_token,
        expected,
        || {},
    )
}

fn bump_recovery_fence_with_before_transaction(
    path: &Path,
    conn: &Connection,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    expected: &ProjectionBindingRecoverySnapshot,
    before_transaction: impl FnOnce(),
) -> Result<ProjectionBindingRecoverySnapshot> {
    let _authority_guard = acquire_projection_authority_guard(path, store_name)?;
    before_transaction();
    with_immediate_tx(conn, || {
        let now = SystemClock.now_ms();
        let current =
            projection_binding_recovery_snapshot(conn, store_name, owner, lease_token, now)?;
        if !expected.matches_after_lease_heartbeat(&current) {
            return Err(stale_generation(store_name));
        }
        let changed = conn
            .execute(
                "UPDATE projection_store_state
                 SET fence_epoch=fence_epoch+1,updated_at=?1
                 WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4
                   AND lease_expires_at>?1 AND fence_epoch=?5
                   AND control_plane=?6",
                params![
                    now,
                    store_name,
                    owner,
                    lease_token,
                    expected.lease.fence_epoch,
                    expected.control_plane,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(stale_generation(store_name));
        }
        projection_binding_recovery_snapshot(conn, store_name, owner, lease_token, now)
    })
}

fn push_unique_generation(generations: &mut Vec<String>, generation: Option<&str>) {
    if let Some(generation) = generation
        && !generations.iter().any(|existing| existing == generation)
    {
        generations.push(generation.to_owned());
    }
}

fn validate_retained_recovery_generation(
    backend: &(impl ProjectionStoreBackend + ?Sized),
    expected: Option<&ProjectionArtifactEvidence>,
    phase: &str,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let generation = &expected.manifest.generation;
    match backend.inspect_generation(generation)? {
        Some(actual) if actual == *expected => {}
        Some(_) => {
            return Err(KanbanError::Conflict(format!(
                "projection backend {phase} generation {generation} changed during incompatible binding recovery"
            )));
        }
        None => {
            return Err(KanbanError::Conflict(format!(
                "projection backend lost retained {phase} generation {generation} during incompatible binding recovery"
            )));
        }
    }
    backend.validate_generation_publication(expected)
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

fn current_lease_snapshot(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
) -> Result<CurrentLease> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    current_lease(&conn, store_name, owner, lease_token, now)
}

fn current_lease_authority(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
) -> Result<ProjectionLease> {
    let lease = current_lease_snapshot(path, store_name, owner, lease_token)?;
    Ok(ProjectionLease {
        store_name: store_name.to_owned(),
        owner: owner.to_owned(),
        lease_token: lease_token.to_owned(),
        fence_epoch: lease.fence_epoch,
        lease_expires_at: lease.lease_expires_at,
    })
}

fn lease_from_destructive_authority(
    store_name: &str,
    authority: &ProjectionDestructiveAuthority,
) -> ProjectionLease {
    ProjectionLease {
        store_name: store_name.to_owned(),
        owner: authority.owner.clone(),
        lease_token: authority.lease_token.clone(),
        fence_epoch: authority.fence_epoch,
        lease_expires_at: authority.lease_expires_at,
    }
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
                building_delivery_count,building_delivery_digest,\
                building_corpus_schema,building_corpus_fingerprint,\
                building_embedding_model,building_embedding_dimensions,\
                building_fingerprint \
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
                corpus: projection_corpus_from_row(row, 9, store_name, "building")?,
                fingerprint: row.get(13)?,
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

/// Build an exact physical-operation capability from the live SQLite binding.
/// This is intentionally read immediately before the backend call; the
/// backend then re-reads and validates the same owner/token/fence/binding while
/// holding its projection-helper suffix lock.
fn authority_for_generation(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    generation: &str,
) -> Result<ProjectionDestructiveAuthority> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let snapshot =
        projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, now)?;
    snapshot.validate_shape(store_name)?;
    let candidates = [
        (ProjectionGenerationRole::Building, &snapshot.building),
        (ProjectionGenerationRole::Active, &snapshot.active),
        (ProjectionGenerationRole::Previous, &snapshot.previous),
    ];
    let (role, binding) = candidates
        .into_iter()
        .find(|(_, binding)| binding.generation.as_deref() == Some(generation))
        .ok_or_else(|| {
            KanbanError::Conflict(format!(
                "projection generation {generation} has no current SQLite authority for store {store_name}"
            ))
        })?;
    destructive_authority_from_snapshot(&snapshot, store_name, owner, lease_token, role, binding)
}

fn current_building_authority(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
) -> Result<ProjectionDestructiveAuthority> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let snapshot =
        projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, now)?;
    snapshot.validate_shape(store_name)?;
    destructive_authority_from_snapshot(
        &snapshot,
        store_name,
        owner,
        lease_token,
        ProjectionGenerationRole::Building,
        &snapshot.building,
    )
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
                active_delivery_count,active_delivery_digest,
                active_corpus_schema,active_corpus_fingerprint,
                active_embedding_model,active_embedding_dimensions,active_fingerprint
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
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<i64>>(15)?,
                    row.get::<_, Option<String>>(16)?,
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
        corpus_schema,
        corpus_fingerprint,
        embedding_model,
        embedding_dimensions,
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
        corpus: projection_corpus_from_values(
            corpus_schema,
            corpus_fingerprint,
            embedding_model,
            embedding_dimensions,
            store_name,
            "active",
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
                    previous_delivery_count,previous_delivery_digest,
                    previous_corpus_schema,previous_corpus_fingerprint,
                    previous_embedding_model,previous_embedding_dimensions,previous_fingerprint
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
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<i64>>(15)?,
                    row.get::<_, Option<String>>(16)?,
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
        corpus_schema,
        corpus_fingerprint,
        embedding_model,
        embedding_dimensions,
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
        corpus: projection_corpus_from_values(
            corpus_schema,
            corpus_fingerprint,
            embedding_model,
            embedding_dimensions,
            store_name,
            "previous",
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

fn projection_corpus_from_row(
    row: &rusqlite::Row<'_>,
    start: usize,
    store_name: &str,
    phase: &str,
) -> rusqlite::Result<Option<ProjectionCorpusMetadata>> {
    projection_corpus_from_values(
        row.get(start)?,
        row.get(start + 1)?,
        row.get(start + 2)?,
        row.get(start + 3)?,
        store_name,
        phase,
    )
    .map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            start,
            rusqlite::types::Type::Null,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })
}

pub(super) fn projection_corpus_from_values(
    corpus_schema: Option<String>,
    corpus_fingerprint: Option<String>,
    embedding_model: Option<String>,
    embedding_dimensions: Option<i64>,
    store_name: &str,
    phase: &str,
) -> Result<Option<ProjectionCorpusMetadata>> {
    match (
        corpus_schema,
        corpus_fingerprint,
        embedding_model,
        embedding_dimensions,
    ) {
        (None, None, None, None) => Ok(None),
        (
            Some(corpus_schema),
            Some(corpus_fingerprint),
            Some(embedding_model),
            Some(dimensions),
        ) if !corpus_schema.trim().is_empty()
            && !corpus_fingerprint.trim().is_empty()
            && !embedding_model.trim().is_empty()
            && dimensions > 0 =>
        {
            let embedding_dimensions = usize::try_from(dimensions).map_err(|_| {
                KanbanError::Storage(format!(
                    "projection store {store_name} has invalid {phase} embedding dimensions"
                ))
            })?;
            Ok(Some(ProjectionCorpusMetadata {
                corpus_schema,
                corpus_fingerprint,
                embedding_model,
                embedding_dimensions,
            }))
        }
        _ => Err(KanbanError::Storage(format!(
            "projection store {store_name} has incomplete {phase} corpus binding"
        ))),
    }
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
    backend.validate_generation_publication(expected)?;
    Ok(Some(actual))
}

fn target_generation_for_claim(
    conn: &Connection,
    store_name: &str,
) -> Result<(String, String, String, Option<ProjectionCorpusMetadata>)> {
    struct GenerationCandidates {
        active: Option<String>,
        active_provider: Option<String>,
        active_provider_fingerprint: Option<String>,
        active_corpus_schema: Option<String>,
        active_corpus_fingerprint: Option<String>,
        active_embedding_model: Option<String>,
        active_embedding_dimensions: Option<i64>,
        building: Option<String>,
        building_provider: Option<String>,
        building_provider_fingerprint: Option<String>,
        building_corpus_schema: Option<String>,
        building_corpus_fingerprint: Option<String>,
        building_embedding_model: Option<String>,
        building_embedding_dimensions: Option<i64>,
        phase: Option<String>,
    }

    let candidates = conn
        .query_row(
            "SELECT active_generation,active_provider,active_provider_fingerprint,
                    active_corpus_schema,active_corpus_fingerprint,
                    active_embedding_model,active_embedding_dimensions,
                    building_generation,building_provider,building_provider_fingerprint,
                    building_corpus_schema,building_corpus_fingerprint,
                    building_embedding_model,building_embedding_dimensions,building_phase \
             FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| {
                Ok(GenerationCandidates {
                    active: row.get(0)?,
                    active_provider: row.get(1)?,
                    active_provider_fingerprint: row.get(2)?,
                    active_corpus_schema: row.get(3)?,
                    active_corpus_fingerprint: row.get(4)?,
                    active_embedding_model: row.get(5)?,
                    active_embedding_dimensions: row.get(6)?,
                    building: row.get(7)?,
                    building_provider: row.get(8)?,
                    building_provider_fingerprint: row.get(9)?,
                    building_corpus_schema: row.get(10)?,
                    building_corpus_fingerprint: row.get(11)?,
                    building_embedding_model: row.get(12)?,
                    building_embedding_dimensions: row.get(13)?,
                    phase: row.get(14)?,
                })
            },
        )
        .map_err(storage)?;
    let active_corpus = projection_corpus_from_values(
        candidates.active_corpus_schema,
        candidates.active_corpus_fingerprint,
        candidates.active_embedding_model,
        candidates.active_embedding_dimensions,
        store_name,
        "active",
    )?;
    let building_corpus = projection_corpus_from_values(
        candidates.building_corpus_schema,
        candidates.building_corpus_fingerprint,
        candidates.building_embedding_model,
        candidates.building_embedding_dimensions,
        store_name,
        "building",
    )?;
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
            building_corpus,
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
            active_corpus,
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
    let expected_corpus_schema = match store_name {
        LANCEDB_CHUNKS_STORE => Some("task-chunks-v2"),
        LANCEDB_LABEL_ATOMS_STORE => Some("label-atoms-v2"),
        _ => None,
    };
    match (expected_corpus_schema, descriptor.corpus.as_ref()) {
        (Some(_), None) => {
            return Err(KanbanError::InvalidInput(format!(
                "projection backend is missing the required corpus binding for store {store_name}"
            )));
        }
        (Some(expected_schema), Some(corpus))
            if corpus.corpus_schema != expected_schema
                || corpus.corpus_fingerprint.trim().is_empty()
                || corpus.embedding_model.trim().is_empty()
                || corpus.embedding_dimensions == 0
                || i64::try_from(corpus.embedding_dimensions).is_err() =>
        {
            return Err(KanbanError::InvalidInput(format!(
                "projection backend corpus binding is invalid for store {store_name}"
            )));
        }
        (None, Some(_)) => {
            return Err(KanbanError::InvalidInput(format!(
                "projection backend has an unexpected corpus binding for store {store_name}"
            )));
        }
        _ => {}
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
        manifest.corpus.as_ref(),
        &descriptor,
    )
}

pub(crate) fn validate_backend_for_target(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<()> {
    let descriptor = backend.descriptor()?;
    validate_store_descriptor(store_name, &descriptor)?;
    let conn = connect_file(path)?;
    let (target_generation, provider, provider_fingerprint, corpus) =
        target_generation_for_claim(&conn, store_name)?;
    validate_descriptor_binding(
        store_name,
        &provider,
        &provider_fingerprint,
        corpus.as_ref(),
        &descriptor,
    )?;
    let expected = match active_artifact(path, store_name)? {
        Some(active) if active.manifest.generation == target_generation => active,
        _ => prepared_manifest(path, store_name, owner, lease_token)?,
    };
    if expected.manifest.generation != target_generation {
        return Err(KanbanError::Conflict(format!(
            "projection target generation evidence does not match SQLite for store {store_name}"
        )));
    }
    let actual = backend
        .inspect_generation(&target_generation)?
        .ok_or_else(|| {
            KanbanError::Conflict(format!(
                "projection target generation {target_generation} is missing for store {store_name}"
            ))
        })?;
    if !same_artifact(&expected, &actual) {
        return Err(KanbanError::Conflict(format!(
            "projection target generation {target_generation} evidence does not match SQLite for store {store_name}"
        )));
    }
    Ok(())
}

fn validate_descriptor_binding(
    store_name: &str,
    expected_provider: &str,
    expected_provider_fingerprint: &str,
    expected_corpus: Option<&ProjectionCorpusMetadata>,
    descriptor: &ProjectionStoreDescriptor,
) -> Result<()> {
    if descriptor.provider != expected_provider
        || descriptor.provider_fingerprint != expected_provider_fingerprint
    {
        return Err(KanbanError::Conflict(format!(
            "projection backend provider binding does not match generation for store {store_name}"
        )));
    }
    if descriptor.corpus.as_ref() != expected_corpus {
        return Err(KanbanError::Conflict(format!(
            "projection backend corpus binding does not match generation for store {store_name}"
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
                    COALESCE((
                      SELECT group_concat(ordered.body, char(10))
                      FROM (
                        SELECT c.body
                        FROM task_comments c
                        WHERE c.board_id=t.board_id AND c.task_id=t.id
                        ORDER BY c.created_at,c.id
                      ) ordered
                    ),''),
                    COALESCE((
                      SELECT group_concat(ordered.text, char(10))
                      FROM (
                        SELECT COALESCE(r.summary,'') || ' ' ||
                               COALESCE(r.error,'') AS text
                        FROM task_runs r
                        WHERE r.board_id=t.board_id AND r.task_id=t.id
                        ORDER BY r.started_at,r.id
                      ) ordered
                    ),''),
                    COALESCE((
                      SELECT group_concat(ordered.text, char(10))
                      FROM (
                        SELECT e.kind || ' ' || e.payload_json AS text
                        FROM task_events e
                        WHERE e.board_id=t.board_id AND e.task_id=t.id
                        ORDER BY e.id
                      ) ordered
                    ),'')
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
        || evidence.manifest.corpus != expected.corpus
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

fn validate_publish_receipt(
    backend: &(impl ProjectionStoreBackend + ?Sized),
    store_name: &str,
    prepared: &ProjectionArtifactEvidence,
    expected_previous: Option<&ProjectionArtifactEvidence>,
    authority: &ProjectionDestructiveAuthority,
    receipt: ProjectionPublishReceipt,
) -> Result<ProjectionArtifactEvidence> {
    validate_artifact_evidence(&prepared.manifest, &receipt.active)?;
    if receipt.retained_previous.as_ref() != expected_previous {
        return Err(KanbanError::Storage(format!(
            "projection store did not retain the expected previous physical generation for {store_name}"
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
    backend.validate_generation_publication_with_authority(&active, authority)?;
    inspect_expected_previous(backend, expected_previous)?;
    Ok(active)
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
    let conn = connect_file(path)?;
    let contents_are_authoritative = conn
        .query_row(
            "SELECT s.building_generation IS NULL
                    AND NOT EXISTS(
                      SELECT 1 FROM projection_deliveries d
                      WHERE d.store_name=s.store_name
                        AND d.status IN ('pending','running','failed','legacy_done')
                    )
             FROM projection_store_state s WHERE s.store_name=?1",
            [store_name],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage)?;
    if contents_are_authoritative {
        backend.validate_active_contents(&expected)?;
    }
    Ok(Some(expected))
}

pub(crate) fn validate_physical_previous_artifact_with(
    path: &Path,
    store_name: &str,
    backend: &(impl ProjectionStoreBackend + ?Sized),
) -> Result<Option<ProjectionArtifactEvidence>> {
    let Some(expected) = previous_artifact(path, store_name)? else {
        return Ok(None);
    };
    backend.validate_generation_publication(&expected)?;
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

fn label_atom_generation_board_ids(
    conn: &Connection,
    store_name: &str,
    generation: &str,
) -> Result<Vec<String>> {
    if store_name != LANCEDB_LABEL_ATOMS_STORE {
        return Ok(Vec::new());
    }
    let mut statement = conn
        .prepare(
            "SELECT DISTINCT board_id
             FROM projection_deliveries
             WHERE store_name=?1 AND published_generation=?2
             ORDER BY board_id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map(params![store_name, generation], |row| row.get(0))
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn reconcile_label_atom_board_compatibility<'a>(
    conn: &Connection,
    store_name: &str,
    generation: &str,
    board_ids: impl IntoIterator<Item = &'a str>,
    now: i64,
) -> Result<()> {
    if store_name != LANCEDB_LABEL_ATOMS_STORE {
        return Ok(());
    }
    for board_id in board_ids {
        conn.execute(
            "UPDATE label_atom_index_boards
             SET dirty=0,last_rebuild_at=?1,last_error=NULL,updated_at=?1
             WHERE store_name=?2 AND board_id=?3
               AND EXISTS (
                 SELECT 1 FROM projection_store_state
                 WHERE store_name=?2 AND active_generation=?4
               )
               AND NOT EXISTS (
                 SELECT 1 FROM projection_deliveries
                 WHERE store_name=?2 AND board_id=?3 AND status!='done'
               )",
            params![now, store_name, board_id, generation],
        )
        .map_err(storage)?;
    }
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
    let store_name: String = row.get(0)?;
    let active_generation: Option<String> = row.get(5)?;
    let previous_generation: Option<String> = row.get(8)?;
    let building_generation: Option<String> = row.get(11)?;
    let active_corpus = projection_corpus_from_row(row, 33, &store_name, "active")?;
    let previous_corpus = projection_corpus_from_row(row, 37, &store_name, "previous")?;
    let building_corpus = projection_corpus_from_row(row, 41, &store_name, "building")?;
    let pending: i64 = row.get(22)?;
    let running: i64 = row.get(23)?;
    let failed: i64 = row.get(24)?;
    let legacy_done: i64 = row.get(25)?;
    let oldest_pending_at: Option<i64> = row.get(26)?;
    let last_error: Option<String> = row.get(28)?;
    let is_lance_store = matches!(
        store_name.as_str(),
        LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE
    );
    let corpus_binding_invalid = if is_lance_store {
        !generation_corpus_binding_is_valid(
            &store_name,
            active_generation.as_deref(),
            active_corpus.as_ref(),
        ) || !generation_corpus_binding_is_valid(
            &store_name,
            previous_generation.as_deref(),
            previous_corpus.as_ref(),
        ) || !generation_corpus_binding_is_valid(
            &store_name,
            building_generation.as_deref(),
            building_corpus.as_ref(),
        )
    } else {
        active_corpus.is_some() || previous_corpus.is_some() || building_corpus.is_some()
    };
    let lifecycle_status = if corpus_binding_invalid || last_error.is_some() || failed > 0 {
        "error"
    } else if building_generation.is_some() {
        "rebuilding"
    } else if active_generation.is_none() {
        "bootstrap_required"
    } else {
        "ready"
    };
    let fallback_reason = if corpus_binding_invalid && is_lance_store {
        Some("corpus_binding_upgrade_required".to_owned())
    } else if corpus_binding_invalid {
        Some("corpus_binding_invalid".to_owned())
    } else if last_error.is_some() || failed > 0 {
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
        store_name,
        database_instance_id: row.get(1)?,
        protocol_version: row.get(2)?,
        schema_version: row.get(3)?,
        control_plane: row.get(4)?,
        active_generation,
        active_fingerprint: row.get(6)?,
        active_fence_epoch: row.get(7)?,
        active_provider: row.get(29)?,
        active_provider_fingerprint: row.get(30)?,
        active_corpus,
        previous_generation,
        previous_fingerprint: row.get(9)?,
        previous_fence_epoch: row.get(10)?,
        previous_corpus,
        building_generation,
        building_fingerprint: row.get(12)?,
        building_fence_epoch: row.get(13)?,
        building_provider: row.get(31)?,
        building_provider_fingerprint: row.get(32)?,
        building_corpus,
        building_phase: row.get(14)?,
        snapshot_cursor: row.get(15)?,
        checkpoint_cursor: row.get(16)?,
        legacy_checkpoint_cursor: row.get(17)?,
        lifecycle_status: lifecycle_status.to_owned(),
        runtime_availability: ProjectionRuntimeAvailability::Unverified,
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
        updated_at: row.get(45)?,
    })
}

fn generation_corpus_binding_is_valid(
    store_name: &str,
    generation: Option<&str>,
    corpus: Option<&ProjectionCorpusMetadata>,
) -> bool {
    match (generation, corpus) {
        (None, None) => true,
        (Some(_), Some(corpus)) => {
            corpus.corpus_schema
                == match store_name {
                    LANCEDB_CHUNKS_STORE => "task-chunks-v2",
                    LANCEDB_LABEL_ATOMS_STORE => "label-atoms-v2",
                    _ => return false,
                }
                && !corpus.corpus_fingerprint.trim().is_empty()
                && !corpus.embedding_model.trim().is_empty()
                && corpus.embedding_dimensions > 0
        }
        _ => false,
    }
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

#[cfg(test)]
mod read_only_publication_validation_tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;
    use crate::init::init_database;

    struct CorruptMarkerBackend {
        generation: ProjectionArtifactEvidence,
        repair_calls: AtomicUsize,
    }

    impl ProjectionStoreBackend for CorruptMarkerBackend {
        fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
            unreachable!("read-only validation does not inspect the descriptor")
        }

        fn prepare_snapshot(
            &self,
            _snapshot: &ProjectionSnapshot,
        ) -> Result<ProjectionArtifactEvidence> {
            unreachable!("read-only validation does not prepare snapshots")
        }

        fn apply_batch(&self, _batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
            unreachable!("read-only validation does not apply batches")
        }

        fn publish_generation(
            &self,
            _expected_active: Option<&ProjectionArtifactEvidence>,
            _prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            unreachable!("read-only validation does not publish generations")
        }

        fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
            Err(KanbanError::Storage(
                "corrupt publication marker".to_owned(),
            ))
        }

        fn inspect_generation(
            &self,
            generation: &str,
        ) -> Result<Option<ProjectionArtifactEvidence>> {
            Ok((generation == self.generation.manifest.generation)
                .then_some(self.generation.clone()))
        }

        fn repair_generation_publication(
            &self,
            _expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            self.repair_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct PrepareFailureBackend {
        db_path: std::path::PathBuf,
        descriptor: ProjectionStoreDescriptor,
        fail_once: AtomicBool,
        rollover_before_abort: AtomicBool,
        rollover_before_prepare_commit: AtomicBool,
        rollover_before_publish: AtomicBool,
        state: std::sync::Mutex<PrepareFailureBackendState>,
    }

    #[derive(Default)]
    struct PrepareFailureBackendState {
        generations: std::collections::BTreeMap<String, ProjectionArtifactEvidence>,
        active: Option<ProjectionArtifactEvidence>,
        previous: Option<ProjectionArtifactEvidence>,
        quarantined_roles: std::collections::BTreeMap<String, ProjectionGenerationRole>,
        successor: Option<ProjectionLease>,
        abort_authority: Option<ProjectionDestructiveAuthority>,
    }

    impl PrepareFailureBackend {
        fn new(path: &std::path::Path) -> Self {
            Self {
                db_path: path.to_owned(),
                descriptor: ProjectionStoreDescriptor {
                    store_name: "tantivy_tasks".to_owned(),
                    provider: "prepare-failure-fixture".to_owned(),
                    provider_fingerprint: "prepare-failure-fixture-v1".to_owned(),
                    corpus: None,
                },
                fail_once: AtomicBool::new(true),
                rollover_before_abort: AtomicBool::new(false),
                rollover_before_prepare_commit: AtomicBool::new(false),
                rollover_before_publish: AtomicBool::new(false),
                state: std::sync::Mutex::new(PrepareFailureBackendState::default()),
            }
        }

        fn rollover_before_abort(&self) {
            self.rollover_before_abort.store(true, Ordering::SeqCst);
        }

        fn rollover_before_prepare_commit(&self) {
            self.rollover_before_prepare_commit
                .store(true, Ordering::SeqCst);
        }

        fn rollover_before_publish(&self) {
            self.rollover_before_publish.store(true, Ordering::SeqCst);
        }

        fn successor_lease(&self) -> ProjectionLease {
            self.state
                .lock()
                .expect("prepare-failure fixture lock")
                .successor
                .clone()
                .expect("successor lease")
        }

        fn generation_present(&self, generation: &str) -> bool {
            self.state
                .lock()
                .expect("prepare-failure fixture lock")
                .generations
                .contains_key(generation)
        }

        fn abort_authority(&self) -> ProjectionDestructiveAuthority {
            self.state
                .lock()
                .expect("prepare-failure fixture lock")
                .abort_authority
                .clone()
                .expect("fenced abort authority")
        }

        fn validate_mutating_authority(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            if authority.generation != generation
                || authority.expected_binding.generation != generation
                || authority.role == ProjectionGenerationRole::Orphaned
                || authority.owner.trim().is_empty()
                || authority.lease_token.trim().is_empty()
            {
                return Err(KanbanError::Conflict(
                    "prepare-failure fixture authority mismatch".to_owned(),
                ));
            }
            Ok(())
        }
    }

    impl ProjectionStoreBackend for PrepareFailureBackend {
        fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
            Ok(self.descriptor.clone())
        }

        fn prepare_snapshot(
            &self,
            snapshot: &ProjectionSnapshot,
        ) -> Result<ProjectionArtifactEvidence> {
            let generation = snapshot.manifest.generation.clone();
            let fingerprint = format!("prepare-failure:{generation}");
            let mut evidence_manifest = snapshot.manifest.clone();
            evidence_manifest.fingerprint = Some(fingerprint.clone());
            let evidence = ProjectionArtifactEvidence {
                manifest: evidence_manifest,
                fingerprint,
            };
            let mut state = self.state.lock().expect("prepare-failure fixture lock");
            if self.fail_once.swap(false, Ordering::SeqCst) {
                // Model a provider crash after creating a partial generation.
                state.generations.insert(generation, evidence);
                return Err(KanbanError::Storage(
                    "prepare provider failed after partial materialization".to_owned(),
                ));
            }
            state.generations.insert(generation, evidence.clone());
            Ok(evidence)
        }

        fn prepare_snapshot_with_authority(
            &self,
            snapshot: &ProjectionSnapshot,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionArtifactEvidence> {
            self.validate_mutating_authority(&snapshot.manifest.generation, authority)?;
            let evidence = self.prepare_snapshot(snapshot)?;
            if self
                .rollover_before_prepare_commit
                .swap(false, Ordering::SeqCst)
            {
                // Force the service's final SQLite CAS to fail after the
                // provider has prepared physical evidence. The fence bump
                // keeps owner/token stable, so error persistence must not
                // replace this original stale-generation failure.
                let conn = connect_file(&self.db_path)?;
                let before = projection_binding_recovery_snapshot(
                    &conn,
                    &self.descriptor.store_name,
                    &authority.owner,
                    &authority.lease_token,
                    SystemClock.now_ms(),
                )?;
                let after = bump_recovery_fence(
                    &self.db_path,
                    &conn,
                    &self.descriptor.store_name,
                    &authority.owner,
                    &authority.lease_token,
                    &before,
                )?;
                conn.execute(
                    "UPDATE projection_store_state
                     SET building_phase='prepared',updated_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4
                       AND fence_epoch=?5",
                    params![
                        SystemClock.now_ms(),
                        self.descriptor.store_name,
                        authority.owner,
                        authority.lease_token,
                        after.lease.fence_epoch,
                    ],
                )
                .map_err(storage)?;
                self.state
                    .lock()
                    .expect("prepare-failure fixture lock")
                    .successor = Some(ProjectionLease {
                    store_name: self.descriptor.store_name.clone(),
                    owner: authority.owner.clone(),
                    lease_token: authority.lease_token.clone(),
                    fence_epoch: after.lease.fence_epoch,
                    lease_expires_at: after.lease.lease_expires_at,
                });
            }
            Ok(evidence)
        }

        fn apply_batch(&self, _batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
            Err(KanbanError::Conflict(
                "prepare-failure fixture does not apply batches".to_owned(),
            ))
        }

        fn apply_batch_with_authority(
            &self,
            batch: &ProjectionBatch,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionBatchReceipt> {
            self.validate_mutating_authority(&batch.target_generation, authority)?;
            self.apply_batch(batch)
        }

        fn publish_generation(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            let mut state = self.state.lock().expect("prepare-failure fixture lock");
            if state.generations.get(&prepared.manifest.generation) != Some(prepared) {
                return Err(KanbanError::Conflict(
                    "prepare-failure fixture generation is missing".to_owned(),
                ));
            }
            if state.active.as_ref() != expected_active {
                return Err(KanbanError::Conflict(
                    "prepare-failure fixture active predecessor mismatch".to_owned(),
                ));
            }
            let retained_previous = state.active.take();
            state.previous = retained_previous.clone();
            if let Some(previous) = &retained_previous {
                state.quarantined_roles.insert(
                    previous.manifest.generation.clone(),
                    ProjectionGenerationRole::Previous,
                );
            }
            state
                .quarantined_roles
                .remove(&prepared.manifest.generation);
            state.active = Some(prepared.clone());
            state.quarantined_roles.insert(
                prepared.manifest.generation.clone(),
                ProjectionGenerationRole::Active,
            );
            Ok(ProjectionPublishReceipt {
                active: prepared.clone(),
                retained_previous,
            })
        }

        fn publish_generation_with_authority(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionPublishReceipt> {
            self.validate_mutating_authority(&prepared.manifest.generation, authority)?;
            if self.rollover_before_publish.swap(false, Ordering::SeqCst) {
                // Model an in-place recovery fence rollover: the same lease
                // owner/token survives, but the successor fence invalidates
                // the authority captured by the failed publish operation.
                let conn = connect_file(&self.db_path)?;
                let before = projection_binding_recovery_snapshot(
                    &conn,
                    &self.descriptor.store_name,
                    &authority.owner,
                    &authority.lease_token,
                    SystemClock.now_ms(),
                )?;
                let after = bump_recovery_fence(
                    &self.db_path,
                    &conn,
                    &self.descriptor.store_name,
                    &authority.owner,
                    &authority.lease_token,
                    &before,
                )?;
                let successor = ProjectionLease {
                    store_name: self.descriptor.store_name.clone(),
                    owner: authority.owner.clone(),
                    lease_token: authority.lease_token.clone(),
                    fence_epoch: after.lease.fence_epoch,
                    lease_expires_at: after.lease.lease_expires_at,
                };
                self.state
                    .lock()
                    .expect("prepare-failure fixture lock")
                    .successor = Some(successor);
                return Err(KanbanError::Storage(
                    "prepare-failure fixture publish failed after lease rollover".to_owned(),
                ));
            }
            self.publish_generation(expected_active, prepared)
        }

        fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
            Ok(self
                .state
                .lock()
                .expect("prepare-failure fixture lock")
                .active
                .clone())
        }

        fn inspect_generation(
            &self,
            generation: &str,
        ) -> Result<Option<ProjectionArtifactEvidence>> {
            Ok(self
                .state
                .lock()
                .expect("prepare-failure fixture lock")
                .generations
                .get(generation)
                .cloned())
        }

        fn abort_generation_fenced(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            self.state
                .lock()
                .expect("prepare-failure fixture lock")
                .abort_authority = Some(authority.clone());
            if self.rollover_before_abort.swap(false, Ordering::SeqCst) {
                release_projection_lease(
                    &self.db_path,
                    &self.descriptor.store_name,
                    &authority.owner,
                    &authority.lease_token,
                )?;
                let successor = acquire_projection_lease(
                    &self.db_path,
                    "tantivy_tasks",
                    "prepare-successor",
                    20_000,
                )?;
                self.state
                    .lock()
                    .expect("prepare-failure fixture lock")
                    .successor = Some(successor);
                return Err(KanbanError::Conflict(
                    "prepare-failure fixture observed a rolled-over lease".to_owned(),
                ));
            }
            self.state
                .lock()
                .expect("prepare-failure fixture lock")
                .generations
                .remove(generation);
            Ok(())
        }

        fn quarantine_generation_fenced(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            let mut state = self.state.lock().expect("prepare-failure fixture lock");
            let actual_role = if state
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.generation == generation)
            {
                ProjectionGenerationRole::Active
            } else if state
                .previous
                .as_ref()
                .is_some_and(|previous| previous.manifest.generation == generation)
            {
                ProjectionGenerationRole::Previous
            } else {
                state
                    .quarantined_roles
                    .get(generation)
                    .copied()
                    .unwrap_or(ProjectionGenerationRole::Building)
            };
            if authority.generation != generation
                || authority.role != actual_role
                || authority.role == ProjectionGenerationRole::Orphaned
            {
                return Err(KanbanError::Conflict(
                    "prepare-failure fixture fenced quarantine authority mismatch".to_owned(),
                ));
            }
            if let Some(evidence) = state.generations.get(generation) {
                let binding = &authority.expected_binding;
                let stable_binding_matches = binding.generation == generation
                    && binding.fence_epoch == evidence.manifest.fence_epoch
                    && binding
                        .snapshot_cursor
                        .is_none_or(|cursor| cursor == evidence.manifest.snapshot_cursor)
                    && binding.canonical_count == evidence.manifest.canonical_item_count
                    && binding.canonical_digest == evidence.manifest.canonical_digest
                    && binding.delivery_count == evidence.manifest.delivery_item_count
                    && binding.delivery_digest == evidence.manifest.delivery_digest
                    && binding
                        .fingerprint
                        .as_deref()
                        .is_none_or(|fingerprint| fingerprint == evidence.fingerprint)
                    && authority.expected_manifest.as_ref().is_none_or(|manifest| {
                        manifest.generation == evidence.manifest.generation
                            && manifest.fingerprint == evidence.manifest.fingerprint
                    });
                if !stable_binding_matches {
                    return Err(KanbanError::Conflict(
                        "prepare-failure fixture binding evidence mismatch".to_owned(),
                    ));
                }
            } else if authority.expected_binding.generation != generation {
                return Err(KanbanError::Conflict(
                    "prepare-failure fixture missing binding evidence".to_owned(),
                ));
            }
            state
                .quarantined_roles
                .insert(generation.to_owned(), actual_role);
            state.generations.remove(generation);
            // A binding reset quarantines a physically published active
            // generation as well as the failed building candidate.  Keep the
            // fixture's publication marker in lockstep with that physical
            // quarantine so the service can verify `active=None`.
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.generation == generation)
            {
                state.active = None;
            }
            if state
                .previous
                .as_ref()
                .is_some_and(|previous| previous.manifest.generation == generation)
            {
                state.previous = None;
            }
            Ok(())
        }
    }

    fn binding_abort_side_state(
        conn: &Connection,
        store_name: &str,
    ) -> anyhow::Result<(String, String)> {
        Ok(conn.query_row(
            "SELECT
               (SELECT COALESCE(json_group_array(json_object(
                  'id',id,'status',status,'published_generation',published_generation,
                  'claim_owner',claim_owner,'claim_token',claim_token,
                  'claim_lease_token',claim_lease_token,'claim_fence_epoch',claim_fence_epoch,
                  'claim_generation',claim_generation,'claim_expires_at',claim_expires_at,
                  'attempts',attempts,'next_attempt_at',next_attempt_at,
                  'last_error',last_error,'updated_at',updated_at)), '[]')
                FROM (SELECT * FROM projection_deliveries
                      WHERE store_name=?1 ORDER BY id)),
               (SELECT COALESCE(json_group_array(json_object(
                  'id',id,'status',status,'attempts',attempts,
                  'last_error',last_error,'updated_at',updated_at)), '[]')
                FROM (SELECT * FROM index_outbox ORDER BY id))",
            [store_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    fn binding_abort_snapshot(
        path: &std::path::Path,
        store_name: &str,
        owner: &str,
        lease_token: &str,
    ) -> anyhow::Result<(ProjectionBindingRecoverySnapshot, (String, String))> {
        let conn = connect_file(path)?;
        let snapshot =
            projection_binding_recovery_snapshot(&conn, store_name, owner, lease_token, 0)?;
        Ok((snapshot, binding_abort_side_state(&conn, store_name)?))
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct BindingAbortDeliveryState {
        id: i64,
        status: String,
        published_generation: Option<String>,
        claim_owner: Option<String>,
        claim_token: Option<String>,
        claim_lease_token: Option<String>,
        claim_fence_epoch: Option<i64>,
        claim_generation: Option<String>,
        claim_expires_at: Option<i64>,
        attempts: i64,
        next_attempt_at: i64,
        last_error: Option<String>,
        updated_at: i64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct BindingAbortStoreState {
        active_generation: Option<String>,
        previous_generation: Option<String>,
        building_generation: Option<String>,
        checkpoint_cursor: i64,
        legacy_checkpoint_cursor: i64,
        lifecycle_status: String,
        last_success_at: Option<i64>,
        last_error: Option<String>,
        updated_at: i64,
    }

    fn binding_abort_delivery_state(
        conn: &Connection,
        store_name: &str,
    ) -> anyhow::Result<Vec<BindingAbortDeliveryState>> {
        let mut statement = conn.prepare(
            "SELECT id,status,published_generation,claim_owner,claim_token,
                    claim_lease_token,claim_fence_epoch,claim_generation,
                    claim_expires_at,attempts,next_attempt_at,last_error,updated_at
             FROM projection_deliveries
             WHERE store_name=?1 ORDER BY id",
        )?;
        Ok(statement
            .query_map([store_name], |row| {
                Ok(BindingAbortDeliveryState {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    published_generation: row.get(2)?,
                    claim_owner: row.get(3)?,
                    claim_token: row.get(4)?,
                    claim_lease_token: row.get(5)?,
                    claim_fence_epoch: row.get(6)?,
                    claim_generation: row.get(7)?,
                    claim_expires_at: row.get(8)?,
                    attempts: row.get(9)?,
                    next_attempt_at: row.get(10)?,
                    last_error: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn assert_reset_delivery_fields(
        before: &[BindingAbortDeliveryState],
        after: &[BindingAbortDeliveryState],
        expected_last_error: &str,
    ) {
        assert_eq!(
            before.len(),
            after.len(),
            "binding reset must preserve the delivery set"
        );
        for expected in before {
            let actual = after
                .iter()
                .find(|delivery| delivery.id == expected.id)
                .unwrap_or_else(|| panic!("binding reset dropped delivery {}", expected.id));
            assert_eq!(actual.status, "pending");
            assert_eq!(actual.published_generation, None);
            assert_eq!(actual.claim_owner, None);
            assert_eq!(actual.claim_token, None);
            assert_eq!(actual.claim_lease_token, None);
            assert_eq!(actual.claim_fence_epoch, None);
            assert_eq!(actual.claim_generation, None);
            assert_eq!(actual.claim_expires_at, None);
            assert_eq!(actual.attempts, expected.attempts);
            assert_eq!(actual.next_attempt_at, expected.next_attempt_at);
            assert_eq!(actual.last_error.as_deref(), Some(expected_last_error));
            assert!(actual.updated_at >= expected.updated_at);
        }
        for actual in after {
            assert!(
                before.iter().any(|delivery| delivery.id == actual.id),
                "binding reset added delivery {}",
                actual.id
            );
        }
    }

    fn binding_abort_store_state(
        conn: &Connection,
        store_name: &str,
    ) -> anyhow::Result<BindingAbortStoreState> {
        Ok(conn.query_row(
            "SELECT active_generation,previous_generation,building_generation,
                    checkpoint_cursor,legacy_checkpoint_cursor,lifecycle_status,
                    last_success_at,last_error,updated_at
             FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| {
                Ok(BindingAbortStoreState {
                    active_generation: row.get(0)?,
                    previous_generation: row.get(1)?,
                    building_generation: row.get(2)?,
                    checkpoint_cursor: row.get(3)?,
                    legacy_checkpoint_cursor: row.get(4)?,
                    lifecycle_status: row.get(5)?,
                    last_success_at: row.get(6)?,
                    last_error: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )?)
    }

    fn binding_abort_delivery_and_store_state(
        path: &std::path::Path,
        store_name: &str,
    ) -> anyhow::Result<(
        Vec<BindingAbortDeliveryState>,
        BindingAbortStoreState,
        String,
    )> {
        let conn = connect_file(path)?;
        let deliveries = binding_abort_delivery_state(&conn, store_name)?;
        let store = binding_abort_store_state(&conn, store_name)?;
        let outbox = conn.query_row(
            "SELECT COALESCE(json_group_array(json_object(
                      'id',id,'status',status,'attempts',attempts,
                      'last_error',last_error,'updated_at',updated_at)), '[]')
             FROM (SELECT * FROM index_outbox ORDER BY id)",
            [],
            |row| row.get(0),
        )?;
        Ok((deliveries, store, outbox))
    }

    fn abort_binding_with_barrier<T>(
        path: &std::path::Path,
        owner: &str,
        lease_token: &str,
        backend: &PrepareFailureBackend,
        binding: AbortBinding,
        after_barrier: impl FnOnce(&std::sync::mpsc::Sender<()>) -> anyhow::Result<T>,
    ) -> anyhow::Result<(T, Result<()>)> {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| -> anyhow::Result<(T, Result<()>)> {
            let abort = scope.spawn(move || {
                abort_projection_generation_with_binding_before_final_transaction(
                    path,
                    "tantivy_tasks",
                    owner,
                    lease_token,
                    backend,
                    binding,
                    move || {
                        entered_tx
                            .send(())
                            .expect("binding abort reached final transaction barrier");
                        resume_rx
                            .recv()
                            .expect("resume binding abort at final transaction barrier");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("binding abort reached final transaction barrier");
            let value = match after_barrier(&resume_tx) {
                Ok(value) => value,
                Err(error) => {
                    let _ = resume_tx.send(());
                    let _ = abort.join();
                    return Err(error);
                }
            };
            let result = abort.join().expect("binding abort thread must not panic");
            Ok((value, result))
        })
    }

    fn bump_binding_fence_direct(
        path: &std::path::Path,
        owner: &str,
        lease_token: &str,
    ) -> anyhow::Result<()> {
        // The normal recovery-fence API acquires the outer service guard. A
        // paused abort already owns that guard, so this test-only actor uses
        // a direct IMMEDIATE SQLite transaction to model the rollover.
        let actor = connect_file(path)?;
        with_immediate_tx(&actor, || {
            let changed = actor
                .execute(
                    "UPDATE projection_store_state
                     SET fence_epoch=fence_epoch+1,updated_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                    params![SystemClock.now_ms(), "tantivy_tasks", owner, lease_token],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::Storage(
                    "test fence rollover actor did not update the lease".to_owned(),
                ));
            }
            Ok(())
        })?;
        Ok(())
    }

    fn prepare_fixture_generation(
        path: &std::path::Path,
        backend: &PrepareFailureBackend,
        owner: &str,
        lease_token: &str,
    ) -> anyhow::Result<ProjectionArtifactManifest> {
        let manifest =
            begin_projection_generation(path, "tantivy_tasks", owner, lease_token, backend)?;
        prepare_projection_snapshot_with(path, "tantivy_tasks", owner, lease_token, backend)?;
        Ok(manifest)
    }

    fn prepare_and_publish_fixture_generation(
        path: &std::path::Path,
        backend: &PrepareFailureBackend,
        owner: &str,
        lease_token: &str,
    ) -> anyhow::Result<ProjectionArtifactEvidence> {
        prepare_fixture_generation(path, backend, owner, lease_token)?;
        Ok(publish_projection_generation_with(
            path,
            "tantivy_tasks",
            owner,
            lease_token,
            backend,
        )?)
    }

    #[test]
    fn failed_prepare_fenced_abort_cleans_partial_state_and_retry_converges() -> anyhow::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let backend = PrepareFailureBackend::new(&path);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "prepare-owner", 20_000)?;
        let first = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )?;

        let error = prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("the fixture fails after creating partial physical state");
        assert!(error.to_string().contains("partial materialization"));
        assert!(!backend.generation_present(&first.generation));
        let authority = backend.abort_authority();
        assert_eq!(authority.owner, "prepare-owner");
        assert_eq!(authority.lease_token, lease.lease_token);
        assert_eq!(authority.generation, first.generation);

        let status = projection_status(&path)?;
        let store = status
            .stores
            .iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy projection status");
        assert!(store.building_generation.is_none());
        assert_eq!(store.lifecycle_status, "error");
        assert!(
            store
                .last_error
                .as_deref()
                .is_some_and(|last_error| last_error.contains("partial materialization"))
        );
        let referenced: i64 = connect_file(&path)?.query_row(
            "SELECT COUNT(*) FROM projection_deliveries
             WHERE store_name='tantivy_tasks'
               AND (published_generation=?1 OR claim_generation=?1)",
            [&first.generation],
            |row| row.get(0),
        )?;
        assert_eq!(referenced, 0, "fenced abort must clear delivery references");

        let retry = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )?;
        let evidence = prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )?;
        assert_eq!(evidence.manifest.generation, retry.generation);
        assert!(backend.generation_present(&retry.generation));
        release_projection_lease(&path, "tantivy_tasks", "prepare-owner", &lease.lease_token)?;
        Ok(())
    }

    #[test]
    fn prepare_abort_rollover_leaves_stale_partial_state_for_successor_recovery()
    -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let backend = PrepareFailureBackend::new(&path);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "prepare-owner", 20_000)?;
        let first = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )?;
        backend.rollover_before_abort();

        let error = prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("rollover must make the captured abort authority stale");
        assert!(
            error
                .to_string()
                .contains("fenced recovery could not clean")
        );
        assert!(backend.generation_present(&first.generation));

        let successor = backend.successor_lease();
        let status = projection_status(&path)?;
        let store = status
            .stores
            .iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy projection status");
        assert_eq!(store.owner.as_deref(), Some("prepare-successor"));
        assert_eq!(
            store.building_generation.as_deref(),
            Some(first.generation.as_str())
        );

        abort_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-successor",
            &successor.lease_token,
            &backend,
        )?;
        assert!(!backend.generation_present(&first.generation));
        let retry = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-successor",
            &successor.lease_token,
            &backend,
        )?;
        let evidence = prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "prepare-successor",
            &successor.lease_token,
            &backend,
        )?;
        assert_eq!(evidence.manifest.generation, retry.generation);
        release_projection_lease(
            &path,
            "tantivy_tasks",
            "prepare-successor",
            &successor.lease_token,
        )?;
        Ok(())
    }

    #[test]
    fn publish_failure_does_not_persist_error_under_a_rolled_over_fence() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "publish-owner", 20_000)?;
        begin_projection_generation(
            &path,
            "tantivy_tasks",
            "publish-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "publish-owner",
            &lease.lease_token,
            &backend,
        )?;
        backend.rollover_before_publish();

        let error = publish_projection_generation_with(
            &path,
            "tantivy_tasks",
            "publish-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("publish failure must surface after the lease rolls over");
        assert!(
            error
                .to_string()
                .contains("publish failed after lease rollover")
        );

        let successor = backend.successor_lease();
        let store = projection_status(&path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy projection status");
        assert_eq!(store.owner.as_deref(), Some("publish-owner"));
        assert_eq!(store.fence_epoch, successor.fence_epoch);
        assert_eq!(store.lifecycle_status, "rebuilding");
        assert_eq!(store.last_error, None);
        assert!(store.building_generation.is_some());

        release_projection_lease(
            &path,
            "tantivy_tasks",
            "publish-owner",
            &successor.lease_token,
        )?;
        Ok(())
    }

    #[test]
    fn prepare_commit_failure_preserves_original_error_after_fence_rollover() -> anyhow::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "prepare-owner", 20_000)?;
        begin_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )?;
        backend.rollover_before_prepare_commit();

        let error = prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("the final snapshot CAS must fail after the fence rollover");
        assert!(error.to_string().contains("projection generation is stale"));

        let successor = backend.successor_lease();
        let store = projection_status(&path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy projection status");
        assert_eq!(store.owner.as_deref(), Some("prepare-owner"));
        assert_eq!(store.fence_epoch, successor.fence_epoch);
        assert_eq!(store.lifecycle_status, "rebuilding");
        assert_eq!(store.last_error, None);

        release_projection_lease(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &successor.lease_token,
        )?;
        Ok(())
    }

    #[test]
    fn read_only_active_validation_never_repairs_a_corrupt_publication_marker() -> anyhow::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state
             SET active_generation='gen_read_only',
                 active_fingerprint='fingerprint-read-only',
                 active_fence_epoch=1,
                 active_snapshot_cursor=0,
                 active_provider='fake',
                 active_provider_fingerprint='fake-v1',
                 active_canonical_count=0,
                 active_canonical_digest='fnv64:0000000000000000',
                 active_delivery_count=0,
                 active_delivery_digest='fnv64:0000000000000000',
                 control_plane='v2',
                 lifecycle_status='ready'
             WHERE store_name='tantivy_tasks'",
            [],
        )?;
        let generation =
            active_artifact(&path, "tantivy_tasks")?.expect("active SQLite generation");
        let backend = CorruptMarkerBackend {
            generation,
            repair_calls: AtomicUsize::new(0),
        };

        let error =
            validate_physical_active_artifact_with(&path, "tantivy_tasks", &backend).unwrap_err();

        assert!(error.to_string().contains("corrupt publication marker"));
        assert_eq!(backend.repair_calls.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[test]
    fn destructive_authority_debug_redacts_lease_token() {
        let authority = ProjectionDestructiveAuthority {
            owner: "owner-a".to_owned(),
            lease_token: "secret-token".to_owned(),
            fence_epoch: 9,
            lease_expires_at: 100,
            role: ProjectionGenerationRole::Building,
            generation: "gen-a".to_owned(),
            expected_manifest: None,
            expected_binding: ProjectionGenerationBinding {
                generation: "gen-a".to_owned(),
                fingerprint: None,
                fence_epoch: 9,
                snapshot_cursor: None,
                provider: "provider".to_owned(),
                provider_fingerprint: "provider-v1".to_owned(),
                canonical_count: 0,
                canonical_digest: "digest".to_owned(),
                delivery_count: 0,
                delivery_digest: "digest".to_owned(),
                corpus: None,
            },
            building_phase: Some("snapshotting".to_owned()),
        };
        let debug = format!("{authority:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret-token"));
    }

    #[test]
    fn recovery_fence_bump_rejects_expired_lease_after_writer_delay() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("recovery fence delay target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "recovery-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "recovery-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "recovery-owner",
            &lease.lease_token,
            &backend,
        )?;
        let conn = connect_file(&path)?;
        let expected = projection_binding_recovery_snapshot(
            &conn,
            "tantivy_tasks",
            "recovery-owner",
            &lease.lease_token,
            SystemClock.now_ms(),
        )?;
        let snapshots = |conn: &Connection| -> anyhow::Result<(String, String)> {
            let deliveries = conn.query_row(
                "SELECT COALESCE(json_group_array(json_object(
                    'id',id,'status',status,'claim_owner',claim_owner,
                    'claim_token',claim_token,'claim_lease_token',claim_lease_token,
                    'claim_fence_epoch',claim_fence_epoch,
                    'claim_generation',claim_generation,
                    'claim_expires_at',claim_expires_at,
                    'published_generation',published_generation,
                    'last_error',last_error,'updated_at',updated_at)), '[]')
                 FROM (SELECT * FROM projection_deliveries
                       WHERE store_name='tantivy_tasks' ORDER BY id)",
                [],
                |row| row.get(0),
            )?;
            let outbox = conn.query_row(
                "SELECT COALESCE(json_group_array(json_object(
                    'id',id,'status',status,'attempts',attempts,
                    'last_error',last_error,'updated_at',updated_at)), '[]')
                 FROM (SELECT * FROM index_outbox ORDER BY id)",
                [],
                |row| row.get(0),
            )?;
            Ok((deliveries, outbox))
        };
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();

        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let lease_token = &lease.lease_token;
            let expected_ref = &expected;
            let bump = scope.spawn(move || {
                let recovery_connection = connect_file(path_ref)?;
                bump_recovery_fence_with_before_transaction(
                    path_ref,
                    &recovery_connection,
                    "tantivy_tasks",
                    "recovery-owner",
                    lease_token,
                    expected_ref,
                    move || {
                        entered_tx
                            .send(())
                            .expect("test observes recovery fence at pre-transaction barrier");
                        resume_rx
                            .recv()
                            .expect("test resumes recovery fence against writer lock");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("recovery fence reached pre-transaction barrier");

            let expires_at = SystemClock.now_ms() + 75;
            let conn = connect_file(&path)?;
            with_immediate_tx(&conn, || {
                let changed = conn
                    .execute(
                        "UPDATE projection_store_state SET lease_expires_at=?1
                         WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                        params![
                            expires_at,
                            "tantivy_tasks",
                            "recovery-owner",
                            lease.lease_token
                        ],
                    )
                    .map_err(storage)?;
                if changed != 1 {
                    return Err(KanbanError::Storage(
                        "test failed to shorten recovery owner lease".to_owned(),
                    ));
                }
                Ok(())
            })?;
            let snapshot_connection = connect_file(&path)?;
            let before_side_state = snapshots(&snapshot_connection)?;
            let mut expected_after_shortened_lease = expected.clone();
            expected_after_shortened_lease.lease.lease_expires_at = expires_at;

            let writer = connect_file(&path)?;
            writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
            resume_tx
                .send(())
                .expect("resume recovery fence against held writer lock");
            while SystemClock.now_ms() <= expires_at {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            writer.execute_batch("COMMIT").map_err(storage)?;

            let error = bump
                .join()
                .expect("recovery fence thread must not panic")
                .expect_err("recovery fence delayed beyond expiry must reject stale owner");
            assert!(matches!(error, KanbanError::Conflict(_)));
            let snapshot_connection = connect_file(&path)?;
            let after = projection_binding_recovery_snapshot(
                &snapshot_connection,
                "tantivy_tasks",
                "recovery-owner",
                &lease.lease_token,
                0,
            )?;
            assert_eq!(after, expected_after_shortened_lease);
            assert_eq!(snapshots(&snapshot_connection)?, before_side_state);
            assert!(
                backend.generation_present(&manifest.generation),
                "fence rejection must retain physical prepared evidence"
            );
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn incompatible_recovery_rejects_expired_lease_before_final_commit() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("incompatible recovery delay target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "recovery-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "recovery-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "recovery-owner",
            &lease.lease_token,
            &backend,
        )?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state SET building_provider_fingerprint='incompatible-v0'
             WHERE store_name='tantivy_tasks'",
            [],
        )?;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();

        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let lease_token = &lease.lease_token;
            let backend_ref = &backend;
            let recovery = scope.spawn(move || {
                recover_incompatible_projection_bindings_with_before_final_transaction(
                    path_ref,
                    "tantivy_tasks",
                    "recovery-owner",
                    lease_token,
                    backend_ref,
                    move || {
                        entered_tx
                            .send(())
                            .expect("physical recovery reached final SQLite barrier");
                        resume_rx
                            .recv()
                            .expect("test resumes incompatible recovery against writer lock");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("physical recovery completed before final SQLite barrier");

            let expires_at = SystemClock.now_ms() + 75;
            let conn = connect_file(&path)?;
            with_immediate_tx(&conn, || {
                conn.execute(
                    "UPDATE projection_store_state SET lease_expires_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                    params![
                        expires_at,
                        "tantivy_tasks",
                        "recovery-owner",
                        lease.lease_token
                    ],
                )
                .map_err(storage)?;
                Ok(())
            })?;
            let (before, delivery_outbox_before) = binding_abort_snapshot(
                &path,
                "tantivy_tasks",
                "recovery-owner",
                &lease.lease_token,
            )?;
            let writer = connect_file(&path)?;
            writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
            resume_tx.send(()).expect("resume final recovery commit");
            while SystemClock.now_ms() <= expires_at {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            writer.execute_batch("COMMIT").map_err(storage)?;

            let error = recovery
                .join()
                .expect("recovery thread must not panic")
                .expect_err("expired recovery lease must reject final SQLite commit");
            assert!(matches!(error, KanbanError::Conflict(_)));
            let (after, delivery_outbox_after) = binding_abort_snapshot(
                &path,
                "tantivy_tasks",
                "recovery-owner",
                &lease.lease_token,
            )?;
            assert_eq!(after, before);
            assert_eq!(delivery_outbox_after, delivery_outbox_before);
            assert!(
                !backend.generation_present(&manifest.generation),
                "physical quarantine is not rolled back after a stale final SQLite commit"
            );
            let successor = acquire_projection_lease(&path, "tantivy_tasks", "successor", 10_000)?;
            assert_eq!(successor.owner, "successor");
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn abort_binding_rejects_expired_lease_before_final_commit() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("binding abort delay"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "binding-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state SET building_provider_fingerprint='incompatible-v0'
             WHERE store_name='tantivy_tasks'",
            [],
        )?;
        let ((before, delivery_outbox_before), error) = abort_binding_with_barrier(
            &path,
            "binding-owner",
            &lease.lease_token,
            &backend,
            AbortBinding::Incompatible,
            |resume| {
                let expires_at = SystemClock.now_ms() + 75;
                let conn = connect_file(&path)?;
                with_immediate_tx(&conn, || {
                    conn.execute("UPDATE projection_store_state SET lease_expires_at=?1 WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4", params![expires_at, "tantivy_tasks", "binding-owner", lease.lease_token]).map_err(storage)?;
                    Ok(())
                })?;
                let before = binding_abort_snapshot(
                    &path,
                    "tantivy_tasks",
                    "binding-owner",
                    &lease.lease_token,
                )?;
                let writer = connect_file(&path)?;
                writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
                resume.send(()).expect("resume binding abort final commit");
                while SystemClock.now_ms() <= expires_at {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                writer.execute_batch("COMMIT").map_err(storage)?;
                Ok(before)
            },
        )?;
        assert!(matches!(error, Err(KanbanError::Conflict(_))));
        let (after, delivery_outbox_after) =
            binding_abort_snapshot(&path, "tantivy_tasks", "binding-owner", &lease.lease_token)?;
        assert_eq!(after, before);
        assert_eq!(delivery_outbox_after, delivery_outbox_before);
        assert!(!backend.generation_present(&manifest.generation));
        assert_eq!(
            acquire_projection_lease(&path, "tantivy_tasks", "successor", 10_000)?.owner,
            "successor"
        );
        Ok(())
    }

    #[test]
    fn abort_binding_rejects_same_owner_fence_rollover_before_final_commit() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("binding abort fence rollover target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "binding-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state SET building_provider_fingerprint='incompatible-v0'
             WHERE store_name='tantivy_tasks'",
            [],
        )?;
        let ((before, delivery_outbox_before), error) = abort_binding_with_barrier(
            &path,
            "binding-owner",
            &lease.lease_token,
            &backend,
            AbortBinding::Incompatible,
            |resume| {
                bump_binding_fence_direct(&path, "binding-owner", &lease.lease_token)?;
                let before = binding_abort_snapshot(
                    &path,
                    "tantivy_tasks",
                    "binding-owner",
                    &lease.lease_token,
                )?;
                resume
                    .send(())
                    .expect("resume binding abort after direct fence rollover");
                Ok(before)
            },
        )?;
        assert!(matches!(error, Err(KanbanError::Conflict(_))), "{error:?}");
        let (after, delivery_outbox_after) =
            binding_abort_snapshot(&path, "tantivy_tasks", "binding-owner", &lease.lease_token)?;
        assert_eq!(after, before);
        assert_eq!(delivery_outbox_after, delivery_outbox_before);
        assert!(
            !backend.generation_present(&manifest.generation),
            "physical quarantine must survive stale final binding CAS"
        );

        // The same owner/token can retry with the bumped fence. This is
        // the successor evidence for the direct in-place rollover.
        abort_incompatible_projection_generation(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        let retry =
            binding_abort_snapshot(&path, "tantivy_tasks", "binding-owner", &lease.lease_token)?.0;
        assert!(retry.building.generation.is_none());
        Ok(())
    }

    #[test]
    fn abort_binding_reset_active_rejects_same_owner_fence_rollover_before_final_commit()
    -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("binding abort active reset target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "binding-owner", 20_000)?;

        let active_manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        publish_projection_generation_with(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        let active_snapshot_connection = connect_file(&path)?;
        assert_eq!(
            projection_binding_recovery_snapshot(
                &active_snapshot_connection,
                "tantivy_tasks",
                "binding-owner",
                &lease.lease_token,
                SystemClock.now_ms(),
            )?
            .active
            .generation
            .as_deref(),
            Some(active_manifest.generation.as_str())
        );

        let building_manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
        )?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state
             SET active_provider_fingerprint='incompatible-v0',
                 building_provider_fingerprint='incompatible-v0'
             WHERE store_name='tantivy_tasks'",
            [],
        )?;

        let ((before, delivery_outbox_before), error) = abort_binding_with_barrier(
            &path,
            "binding-owner",
            &lease.lease_token,
            &backend,
            AbortBinding::Incompatible,
            |resume| {
                bump_binding_fence_direct(&path, "binding-owner", &lease.lease_token)?;
                let before = binding_abort_snapshot(
                    &path,
                    "tantivy_tasks",
                    "binding-owner",
                    &lease.lease_token,
                )?;
                resume
                    .send(())
                    .expect("resume active reset abort after direct fence rollover");
                Ok(before)
            },
        )?;
        assert!(matches!(error, Err(KanbanError::Conflict(_))), "{error:?}");
        let (after, delivery_outbox_after) =
            binding_abort_snapshot(&path, "tantivy_tasks", "binding-owner", &lease.lease_token)?;
        assert_eq!(after, before);
        assert_eq!(delivery_outbox_after, delivery_outbox_before);
        assert!(!backend.generation_present(&building_manifest.generation));
        assert!(!backend.generation_present(&active_manifest.generation));
        assert!(backend.inspect_active()?.is_none());

        // The bumped fence is a valid retry capability for the same
        // owner/token. Both the active and building bindings now reset.
        let (deliveries_before_success, store_before_success, outbox_before_success) =
            binding_abort_delivery_and_store_state(&path, "tantivy_tasks")?;
        abort_projection_generation_with_binding(
            &path,
            "tantivy_tasks",
            "binding-owner",
            &lease.lease_token,
            &backend,
            AbortBinding::Incompatible,
        )?;
        let retry =
            binding_abort_snapshot(&path, "tantivy_tasks", "binding-owner", &lease.lease_token)?.0;
        assert!(retry.building.generation.is_none());
        assert!(retry.active.generation.is_none());
        assert_eq!(retry.lifecycle_status, "bootstrap_required");
        let (deliveries_after_success, store_after_success, outbox_after_success) =
            binding_abort_delivery_and_store_state(&path, "tantivy_tasks")?;
        assert_eq!(outbox_after_success, outbox_before_success);
        assert_eq!(store_after_success.active_generation, None);
        assert_eq!(store_after_success.previous_generation, None);
        assert_eq!(store_after_success.building_generation, None);
        assert_eq!(store_after_success.checkpoint_cursor, 0);
        assert_eq!(
            store_after_success.legacy_checkpoint_cursor,
            store_before_success.legacy_checkpoint_cursor
        );
        assert_eq!(store_after_success.lifecycle_status, "bootstrap_required");
        assert_eq!(store_after_success.last_success_at, None);
        assert_eq!(store_after_success.last_error, None);
        assert!(store_after_success.updated_at >= store_before_success.updated_at);
        assert_reset_delivery_fields(
            &deliveries_before_success,
            &deliveries_after_success,
            "backend binding generation reset before rebuild",
        );
        Ok(())
    }

    #[test]
    fn abort_binding_reset_previous_active_and_building_rejects_fence_rollover()
    -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("binding abort previous reset target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "binding-owner", 20_000)?;

        // Publish A, then publish B so SQLite and the fixture both have a
        // real previous A + active B chain before preparing C.
        let previous_manifest = prepare_and_publish_fixture_generation(
            &path,
            &backend,
            "binding-owner",
            &lease.lease_token,
        )?;
        let active_manifest = prepare_and_publish_fixture_generation(
            &path,
            &backend,
            "binding-owner",
            &lease.lease_token,
        )?;
        let building_manifest =
            prepare_fixture_generation(&path, &backend, "binding-owner", &lease.lease_token)?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state
             SET previous_provider_fingerprint='incompatible-v0',
                 building_provider_fingerprint='incompatible-v0'
             WHERE store_name='tantivy_tasks'",
            [],
        )?;
        let (before, side_before) =
            binding_abort_snapshot(&path, "tantivy_tasks", "binding-owner", &lease.lease_token)?;
        assert_eq!(
            before.previous.generation.as_deref(),
            Some(previous_manifest.manifest.generation.as_str())
        );
        assert_eq!(
            before.active.generation.as_deref(),
            Some(active_manifest.manifest.generation.as_str())
        );
        assert_eq!(
            before.building.generation.as_deref(),
            Some(building_manifest.generation.as_str())
        );
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let token_ref = &lease.lease_token;
            let backend_ref = &backend;
            let abort = scope.spawn(move || {
                abort_projection_generation_with_binding_before_final_transaction(
                    path_ref,
                    "tantivy_tasks",
                    "binding-owner",
                    token_ref,
                    backend_ref,
                    AbortBinding::Incompatible,
                    move || {
                        entered_tx.send(()).expect("previous reset fence barrier");
                        resume_rx
                            .recv()
                            .expect("resume previous reset after fence rollover");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("previous reset abort reached final barrier");
            bump_binding_fence_direct(&path, "binding-owner", &lease.lease_token)?;
            resume_tx
                .send(())
                .expect("resume previous reset after direct fence rollover");
            let error = abort
                .join()
                .expect("previous reset abort thread must not panic")
                .expect_err("same-owner fence rollover must reject previous reset final CAS");
            assert!(matches!(error, KanbanError::Conflict(_)), "{error}");

            let (after, side_after) = binding_abort_snapshot(
                &path,
                "tantivy_tasks",
                "binding-owner",
                &lease.lease_token,
            )?;
            let mut expected_after_rollover = before.clone();
            expected_after_rollover.lease.fence_epoch += 1;
            expected_after_rollover.updated_at = after.updated_at;
            assert_eq!(after, expected_after_rollover);
            assert_eq!(side_after, side_before);
            assert!(!backend.generation_present(&previous_manifest.manifest.generation));
            assert!(backend.generation_present(&active_manifest.manifest.generation));
            assert!(!backend.generation_present(&building_manifest.generation));
            assert_eq!(
                backend
                    .inspect_active()?
                    .expect("compatible active generation remains after previous-only reset")
                    .manifest
                    .generation,
                active_manifest.manifest.generation
            );

            // The bumped same-owner/token fence is the retry capability;
            // previous/building clear while compatible active B remains.
            let (deliveries_before_success, store_before_success, outbox_before_success) =
                binding_abort_delivery_and_store_state(&path, "tantivy_tasks")?;
            abort_projection_generation_with_binding(
                &path,
                "tantivy_tasks",
                "binding-owner",
                &lease.lease_token,
                &backend,
                AbortBinding::Incompatible,
            )?;
            let retry = binding_abort_snapshot(
                &path,
                "tantivy_tasks",
                "binding-owner",
                &lease.lease_token,
            )?
            .0;
            assert!(retry.previous.generation.is_none());
            assert_eq!(
                retry.active.generation.as_deref(),
                Some(active_manifest.manifest.generation.as_str())
            );
            assert!(retry.building.generation.is_none());
            assert_eq!(retry.lifecycle_status, "ready");
            let (deliveries_after_success, store_after_success, outbox_after_success) =
                binding_abort_delivery_and_store_state(&path, "tantivy_tasks")?;
            assert_eq!(outbox_after_success, outbox_before_success);
            assert_eq!(
                store_after_success.active_generation,
                Some(active_manifest.manifest.generation.clone())
            );
            assert_eq!(store_after_success.previous_generation, None);
            assert_eq!(store_after_success.building_generation, None);
            assert_eq!(store_after_success.checkpoint_cursor, 0);
            assert_eq!(
                store_after_success.legacy_checkpoint_cursor,
                store_before_success.legacy_checkpoint_cursor
            );
            assert_eq!(store_after_success.lifecycle_status, "ready");
            assert_eq!(
                store_after_success.last_success_at,
                store_before_success.last_success_at
            );
            assert_eq!(store_after_success.last_error, None);
            assert!(store_after_success.updated_at >= store_before_success.updated_at);
            assert_reset_delivery_fields(
                &deliveries_before_success,
                &deliveries_after_success,
                "generation aborted before publish",
            );
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn abort_generation_rejects_expired_lease_before_final_commit() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("abort final commit delay target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "abort-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "abort-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "abort-owner",
            &lease.lease_token,
            &backend,
        )?;
        let authority =
            current_building_authority(&path, "tantivy_tasks", "abort-owner", &lease.lease_token)?;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let token_ref = &lease.lease_token;
            let backend_ref = &backend;
            let authority_ref = &authority;
            let abort = scope.spawn(move || {
                abort_projection_generation_with_authority_before_final_transaction(
                    path_ref,
                    "tantivy_tasks",
                    "abort-owner",
                    token_ref,
                    backend_ref,
                    authority_ref,
                    move || {
                        entered_tx
                            .send(())
                            .expect("physical abort reached final SQLite barrier");
                        resume_rx
                            .recv()
                            .expect("test resumes abort final commit against writer lock");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("physical abort completed before final SQLite barrier");
            let expires_at = SystemClock.now_ms() + 75;
            let conn = connect_file(&path)?;
            with_immediate_tx(&conn, || {
                conn.execute(
                    "UPDATE projection_store_state SET lease_expires_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                    params![
                        expires_at,
                        "tantivy_tasks",
                        "abort-owner",
                        lease.lease_token
                    ],
                )
                .map_err(storage)?;
                Ok(())
            })?;
            let (before, delivery_outbox_before) =
                binding_abort_snapshot(&path, "tantivy_tasks", "abort-owner", &lease.lease_token)?;
            let writer = connect_file(&path)?;
            writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
            resume_tx.send(()).expect("resume final abort commit");
            while SystemClock.now_ms() <= expires_at {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            writer.execute_batch("COMMIT").map_err(storage)?;
            let error = abort
                .join()
                .expect("abort thread must not panic")
                .expect_err("expired abort lease must reject final SQLite commit");
            assert!(matches!(error, KanbanError::Conflict(_)));
            let (after, delivery_outbox_after) =
                binding_abort_snapshot(&path, "tantivy_tasks", "abort-owner", &lease.lease_token)?;
            assert_eq!(after, before);
            assert_eq!(delivery_outbox_after, delivery_outbox_before);
            assert!(
                !backend.generation_present(&manifest.generation),
                "physical abort is retained when stale SQLite cleanup is rejected"
            );
            let successor = acquire_projection_lease(&path, "tantivy_tasks", "successor", 10_000)?;
            assert_eq!(successor.owner, "successor");
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn recovery_fence_bump_invalidates_the_pre_bump_snapshot() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "owner", 10_000)?;
        let conn = connect_file(&path)?;
        let before = projection_binding_recovery_snapshot(
            &conn,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            SystemClock.now_ms(),
        )?;

        let after = bump_recovery_fence(
            &path,
            &conn,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            &before,
        )?;
        assert_eq!(after.lease.fence_epoch, before.lease.fence_epoch + 1);
        assert!(
            bump_recovery_fence(
                &path,
                &conn,
                "tantivy_tasks",
                "owner",
                &lease.lease_token,
                &before,
            )
            .is_err()
        );
        conn.execute(
            "UPDATE projection_store_state SET lease_expires_at=0 WHERE store_name=?1",
            ["tantivy_tasks"],
        )?;
        let takeover = acquire_projection_lease(&path, "tantivy_tasks", "new-owner", 10_000)?;
        assert!(takeover.fence_epoch > after.lease.fence_epoch);
        assert!(
            bump_recovery_fence(
                &path,
                &conn,
                "tantivy_tasks",
                "owner",
                &lease.lease_token,
                &after,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn prepared_building_recovery_snapshot_retains_the_global_cursor() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "owner", 10_000)?;
        let building = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            &backend,
        )?;
        let conn = connect_file(&path)?;
        let snapshotting = projection_binding_recovery_snapshot(
            &conn,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            SystemClock.now_ms(),
        )?;
        assert_eq!(
            snapshotting.building.generation.as_deref(),
            Some(building.generation.as_str())
        );
        assert_eq!(snapshotting.building.phase.as_deref(), Some("snapshotting"));
        assert!(snapshotting.building.fingerprint.is_none());
        assert!(snapshotting.building.snapshot_cursor.is_none());

        let mut cursor_corruption = snapshotting.clone();
        cursor_corruption.building.snapshot_cursor = Some(snapshotting.snapshot_cursor);
        let error = cursor_corruption
            .validate_shape("tantivy_tasks")
            .expect_err("snapshotting cursor evidence must fail closed");
        assert!(error.to_string().contains("snapshotting generation"));

        conn.execute(
            "UPDATE projection_store_state
             SET building_fingerprint='corrupt'
             WHERE store_name=?1",
            ["tantivy_tasks"],
        )?;
        let fingerprint_corruption = projection_binding_recovery_snapshot(
            &conn,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            SystemClock.now_ms(),
        )?;
        assert!(fingerprint_corruption.building.snapshot_cursor.is_none());
        let error = fingerprint_corruption
            .validate_shape("tantivy_tasks")
            .expect_err("snapshotting fingerprint evidence must fail closed");
        assert!(error.to_string().contains("snapshotting generation"));
        conn.execute(
            "UPDATE projection_store_state
             SET building_fingerprint=NULL
             WHERE store_name=?1",
            ["tantivy_tasks"],
        )?;

        let prepared = prepare_projection_snapshot_with(
            &path,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            &backend,
        )?;
        let prepared_snapshot = projection_binding_recovery_snapshot(
            &conn,
            "tantivy_tasks",
            "owner",
            &lease.lease_token,
            SystemClock.now_ms(),
        )?;
        assert_eq!(
            prepared_snapshot.building.phase.as_deref(),
            Some("prepared")
        );
        assert_eq!(
            prepared_snapshot.building.snapshot_cursor,
            Some(prepared.manifest.snapshot_cursor)
        );
        release_projection_lease(&path, "tantivy_tasks", "owner", &lease.lease_token)?;
        Ok(())
    }

    #[test]
    fn projection_error_persistence_rejects_a_handed_off_authority() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let old = acquire_projection_lease(&path, "tantivy_tasks", "old-owner", 10_000)?;
        let authority =
            current_lease_authority(&path, "tantivy_tasks", "old-owner", &old.lease_token)?;
        release_projection_lease(&path, "tantivy_tasks", "old-owner", &old.lease_token)?;
        let successor = acquire_projection_lease(&path, "tantivy_tasks", "new-owner", 10_000)?;
        let before = projection_status(&path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy status");

        let error = record_projection_error(
            &path,
            "tantivy_tasks",
            &authority,
            "failure from the previous owner",
        )
        .expect_err("a handed-off authority must not update the successor");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let after = projection_status(&path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy status");
        assert_eq!(after.lifecycle_status, before.lifecycle_status);
        assert_eq!(after.last_error, before.last_error);
        assert_eq!(after.owner.as_deref(), Some("new-owner"));
        assert_eq!(after.fence_epoch, successor.fence_epoch);

        release_projection_lease(&path, "tantivy_tasks", "new-owner", &successor.lease_token)?;
        Ok(())
    }

    #[test]
    fn fenced_destructive_defaults_fail_closed() -> anyhow::Result<()> {
        let backend = CorruptMarkerBackend {
            generation: ProjectionArtifactEvidence {
                manifest: ProjectionArtifactManifest {
                    store_name: "tantivy_tasks".to_owned(),
                    database_instance_id: "db".to_owned(),
                    protocol_version: 2,
                    schema_version: 1,
                    generation: "gen".to_owned(),
                    fence_epoch: 7,
                    snapshot_cursor: 0,
                    provider: "fake".to_owned(),
                    provider_fingerprint: "fake-v1".to_owned(),
                    corpus: None,
                    canonical_item_count: 0,
                    canonical_digest: "d".to_owned(),
                    delivery_item_count: 0,
                    delivery_digest: "d".to_owned(),
                    fingerprint: Some("f".to_owned()),
                },
                fingerprint: "f".to_owned(),
            },
            repair_calls: AtomicUsize::new(0),
        };
        let authority = ProjectionDestructiveAuthority {
            owner: "owner".to_owned(),
            lease_token: "token".to_owned(),
            fence_epoch: 9,
            lease_expires_at: 100,
            role: ProjectionGenerationRole::Building,
            generation: "gen".to_owned(),
            expected_manifest: None,
            expected_binding: ProjectionGenerationBinding {
                generation: "gen".to_owned(),
                fingerprint: None,
                fence_epoch: 7,
                snapshot_cursor: None,
                provider: "fake".to_owned(),
                provider_fingerprint: "fake-v1".to_owned(),
                canonical_count: 0,
                canonical_digest: "d".to_owned(),
                delivery_count: 0,
                delivery_digest: "d".to_owned(),
                corpus: None,
            },
            building_phase: None,
        };
        let error = backend
            .quarantine_generation_fenced("gen", &authority)
            .expect_err("default fenced destructive operation must fail closed");
        assert!(
            error
                .to_string()
                .contains("must implement fenced quarantine")
        );
        Ok(())
    }

    #[test]
    fn destructive_authority_uses_current_lease_fence_separately_from_artifact_fence() {
        let evidence = ProjectionArtifactEvidence {
            manifest: ProjectionArtifactManifest {
                store_name: "tantivy_tasks".to_owned(),
                database_instance_id: "db".to_owned(),
                protocol_version: 2,
                schema_version: 1,
                generation: "gen".to_owned(),
                fence_epoch: 7,
                snapshot_cursor: 0,
                provider: "fake".to_owned(),
                provider_fingerprint: "fake-v1".to_owned(),
                corpus: None,
                canonical_item_count: 0,
                canonical_digest: "d".to_owned(),
                delivery_item_count: 0,
                delivery_digest: "d".to_owned(),
                fingerprint: Some("f".to_owned()),
            },
            fingerprint: "f".to_owned(),
        };
        let authority = destructive_authority_from_evidence(
            "owner",
            "token",
            ProjectionGenerationRole::Active,
            9,
            100,
            &evidence,
        );
        assert_eq!(authority.fence_epoch, 9);
        assert_eq!(authority.lease_expires_at, 100);
        assert_eq!(authority.expected_binding.fence_epoch, 7);
    }

    #[test]
    fn begin_generation_rejects_expired_lease_after_writer_delay() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("begin generation delay target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "begin-owner", 10_000)?;
        let (delivery_id, outbox_id): (i64, i64) = connect_file(&path)?.query_row(
            "SELECT id,outbox_id FROM projection_deliveries
             WHERE store_name=?1 ORDER BY id LIMIT 1",
            ["tantivy_tasks"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let conn = connect_file(&path)?;
        with_immediate_tx(&conn, || {
            let changed = conn
                .execute(
                    "UPDATE projection_deliveries
                     SET status='running',claim_owner=?1,claim_token='begin-claim',
                         claim_lease_token=?2,claim_fence_epoch=?3,
                         claim_generation='begin-generation',claim_expires_at=0
                     WHERE id=?4 AND status='pending'",
                    params![
                        "begin-owner",
                        lease.lease_token,
                        lease.fence_epoch,
                        delivery_id
                    ],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::Storage(
                    "test failed to seed expired running projection delivery".to_owned(),
                ));
            }
            Ok(())
        })?;
        let snapshots = |conn: &Connection| -> anyhow::Result<(String, String, String, String)> {
            let store = conn.query_row(
                "SELECT json_object(
                    'lease_owner',lease_owner,'lease_token',lease_token,
                    'lease_expires_at',lease_expires_at,'fence_epoch',fence_epoch,
                    'building_generation',building_generation,
                    'building_fingerprint',building_fingerprint,
                    'building_fence_epoch',building_fence_epoch,
                    'building_provider',building_provider,
                    'building_provider_fingerprint',building_provider_fingerprint,
                    'building_corpus_schema',building_corpus_schema,
                    'building_corpus_fingerprint',building_corpus_fingerprint,
                    'building_embedding_model',building_embedding_model,
                    'building_embedding_dimensions',building_embedding_dimensions,
                    'building_canonical_count',building_canonical_count,
                    'building_canonical_digest',building_canonical_digest,
                    'building_delivery_count',building_delivery_count,
                    'building_delivery_digest',building_delivery_digest,
                    'building_phase',building_phase,'snapshot_cursor',snapshot_cursor,
                    'control_plane',control_plane,'lifecycle_status',lifecycle_status,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM projection_store_state WHERE store_name='tantivy_tasks'",
                [],
                |row| row.get(0),
            )?;
            let delivery = conn.query_row(
                "SELECT json_object(
                    'status',status,'claim_owner',claim_owner,'claim_token',claim_token,
                    'claim_lease_token',claim_lease_token,
                    'claim_fence_epoch',claim_fence_epoch,
                    'claim_generation',claim_generation,
                    'claim_expires_at',claim_expires_at,
                    'published_generation',published_generation,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM projection_deliveries WHERE id=?1",
                [delivery_id],
                |row| row.get(0),
            )?;
            let outbox = conn.query_row(
                "SELECT json_object(
                    'source_event_id',source_event_id,'target',target,
                    'projection_store',projection_store,'entity_uri',entity_uri,
                    'action',action,'payload_json',payload_json,'status',status,
                    'attempts',attempts,'last_error',last_error,
                    'created_at',created_at,'updated_at',updated_at)
                 FROM index_outbox WHERE id=?1",
                [outbox_id],
                |row| row.get(0),
            )?;
            let checkpoint = conn.query_row(
                "SELECT json_object(
                    'schema_version',schema_version,'last_event_id',last_event_id,
                    'dirty',dirty,'last_rebuild_at',last_rebuild_at,
                    'last_sync_at',last_sync_at,'last_error',last_error,
                    'updated_at',updated_at)
                 FROM derived_store_state WHERE store_name='tantivy_tasks'",
                [],
                |row| row.get(0),
            )?;
            Ok((store, delivery, outbox, checkpoint))
        };
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();

        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let lease_token = &lease.lease_token;
            let backend_ref = &backend;
            let begin = scope.spawn(move || {
                begin_projection_generation_with_before_transaction(
                    path_ref,
                    "tantivy_tasks",
                    "begin-owner",
                    lease_token,
                    backend_ref,
                    move || {
                        entered_tx
                            .send(())
                            .expect("test observes begin generation at pre-transaction barrier");
                        resume_rx
                            .recv()
                            .expect("test resumes begin generation against writer lock");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("begin generation reached pre-transaction barrier");

            let expires_at = SystemClock.now_ms() + 75;
            let conn = connect_file(&path)?;
            with_immediate_tx(&conn, || {
                let changed = conn
                    .execute(
                        "UPDATE projection_store_state SET lease_expires_at=?1
                         WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                        params![
                            expires_at,
                            "tantivy_tasks",
                            "begin-owner",
                            lease.lease_token
                        ],
                    )
                    .map_err(storage)?;
                if changed != 1 {
                    return Err(KanbanError::Storage(
                        "test failed to shorten projection lease before generation begin"
                            .to_owned(),
                    ));
                }
                Ok(())
            })?;
            let snapshot_connection = connect_file(&path)?;
            let before = snapshots(&snapshot_connection)?;

            let writer = connect_file(&path)?;
            writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
            resume_tx
                .send(())
                .expect("resume begin generation against held writer lock");
            while SystemClock.now_ms() <= expires_at {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            writer.execute_batch("COMMIT").map_err(storage)?;

            let error = begin
                .join()
                .expect("begin generation thread must not panic")
                .expect_err("begin generation delayed beyond expiry must reject stale owner");
            assert!(matches!(error, KanbanError::Conflict(_)));
            let snapshot_connection = connect_file(&path)?;
            let after = snapshots(&snapshot_connection)?;
            assert_eq!(after, before);
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn prepare_snapshot_rejects_expired_lease_before_final_commit() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("prepare final commit delay target"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        backend.fail_once.store(false, Ordering::SeqCst);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "prepare-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "prepare-owner",
            &lease.lease_token,
            &backend,
        )?;
        let (delivery_id, outbox_id): (i64, i64) = connect_file(&path)?.query_row(
            "SELECT id,outbox_id FROM projection_deliveries
             WHERE store_name=?1 ORDER BY id LIMIT 1",
            ["tantivy_tasks"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let snapshots = |conn: &Connection| -> anyhow::Result<(String, String, String, String)> {
            let store = conn.query_row(
                "SELECT json_object(
                    'lease_owner',lease_owner,'lease_token',lease_token,
                    'lease_expires_at',lease_expires_at,'fence_epoch',fence_epoch,
                    'building_generation',building_generation,
                    'building_fingerprint',building_fingerprint,
                    'building_fence_epoch',building_fence_epoch,
                    'building_provider',building_provider,
                    'building_provider_fingerprint',building_provider_fingerprint,
                    'building_phase',building_phase,'snapshot_cursor',snapshot_cursor,
                    'checkpoint_cursor',checkpoint_cursor,
                    'legacy_checkpoint_cursor',legacy_checkpoint_cursor,
                    'last_success_at',last_success_at,
                    'control_plane',control_plane,'lifecycle_status',lifecycle_status,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM projection_store_state WHERE store_name='tantivy_tasks'",
                [],
                |row| row.get(0),
            )?;
            let delivery = conn.query_row(
                "SELECT json_object(
                    'status',status,'claim_owner',claim_owner,'claim_token',claim_token,
                    'claim_lease_token',claim_lease_token,
                    'claim_fence_epoch',claim_fence_epoch,
                    'claim_generation',claim_generation,
                    'claim_expires_at',claim_expires_at,
                    'published_generation',published_generation,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM projection_deliveries WHERE id=?1",
                [delivery_id],
                |row| row.get(0),
            )?;
            let outbox = conn.query_row(
                "SELECT json_object(
                    'status',status,'attempts',attempts,'last_error',last_error,
                    'updated_at',updated_at)
                 FROM index_outbox WHERE id=?1",
                [outbox_id],
                |row| row.get(0),
            )?;
            let checkpoint = conn.query_row(
                "SELECT json_object(
                    'last_event_id',last_event_id,'dirty',dirty,
                    'last_rebuild_at',last_rebuild_at,'last_sync_at',last_sync_at,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM derived_store_state WHERE store_name='tantivy_tasks'",
                [],
                |row| row.get(0),
            )?;
            Ok((store, delivery, outbox, checkpoint))
        };
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();

        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let lease_token = &lease.lease_token;
            let backend_ref = &backend;
            let prepare = scope.spawn(move || {
                prepare_projection_snapshot_with_disposition_with_before_final_transaction(
                    path_ref,
                    "tantivy_tasks",
                    "prepare-owner",
                    lease_token,
                    backend_ref,
                    move || {
                        entered_tx
                            .send(())
                            .expect("physical prepare reached final SQLite barrier");
                        resume_rx
                            .recv()
                            .expect("test resumes final prepare commit against writer lock");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("physical prepare completed before final SQLite barrier");

            let expires_at = SystemClock.now_ms() + 75;
            let conn = connect_file(&path)?;
            with_immediate_tx(&conn, || {
                let changed = conn
                    .execute(
                        "UPDATE projection_store_state SET lease_expires_at=?1
                         WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                        params![
                            expires_at,
                            "tantivy_tasks",
                            "prepare-owner",
                            lease.lease_token
                        ],
                    )
                    .map_err(storage)?;
                if changed != 1 {
                    return Err(KanbanError::Storage(
                        "test failed to shorten projection lease before final prepare commit"
                            .to_owned(),
                    ));
                }
                Ok(())
            })?;
            let snapshot_connection = connect_file(&path)?;
            let before = snapshots(&snapshot_connection)?;

            let writer = connect_file(&path)?;
            writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
            resume_tx
                .send(())
                .expect("resume final prepare commit against held writer lock");
            while SystemClock.now_ms() <= expires_at {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            writer.execute_batch("COMMIT").map_err(storage)?;

            let error = prepare
                .join()
                .expect("prepare thread must not panic")
                .expect_err("final prepare commit delayed beyond expiry must reject stale owner");
            assert!(matches!(error, KanbanError::Conflict(_)));
            assert!(
                backend.generation_present(&manifest.generation),
                "physical prepare evidence remains for fenced abort or successor recovery"
            );
            let snapshot_connection = connect_file(&path)?;
            let after = snapshots(&snapshot_connection)?;
            assert_eq!(after, before);
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn snapshot_prepare_disposition_rejects_expired_lease_after_writer_delay() -> anyhow::Result<()>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("snapshot disposition baseline"),
        )?;
        let backend = PrepareFailureBackend::new(&path);
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "snapshot-owner", 10_000)?;
        let manifest = begin_projection_generation(
            &path,
            "tantivy_tasks",
            "snapshot-owner",
            &lease.lease_token,
            &backend,
        )?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("snapshot disposition coverage drift"),
        )?;
        assert!(
            !backend.generation_present(&manifest.generation),
            "the read-only disposition fixture has no physical generation"
        );
        let snapshots = |conn: &Connection| -> anyhow::Result<(String, String, String, String)> {
            let store = conn.query_row(
                "SELECT json_object(
                    'lease_owner',lease_owner,'lease_token',lease_token,
                    'lease_expires_at',lease_expires_at,'fence_epoch',fence_epoch,
                    'building_generation',building_generation,
                    'building_fingerprint',building_fingerprint,
                    'building_fence_epoch',building_fence_epoch,
                    'building_phase',building_phase,'snapshot_cursor',snapshot_cursor,
                    'checkpoint_cursor',checkpoint_cursor,
                    'legacy_checkpoint_cursor',legacy_checkpoint_cursor,
                    'last_success_at',last_success_at,
                    'control_plane',control_plane,'lifecycle_status',lifecycle_status,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM projection_store_state WHERE store_name='tantivy_tasks'",
                [],
                |row| row.get(0),
            )?;
            let deliveries = conn.query_row(
                "SELECT COALESCE(json_group_array(json_object(
                    'id',id,'status',status,'claim_owner',claim_owner,
                    'claim_token',claim_token,'claim_lease_token',claim_lease_token,
                    'claim_fence_epoch',claim_fence_epoch,
                    'claim_generation',claim_generation,
                    'claim_expires_at',claim_expires_at,
                    'published_generation',published_generation,
                    'last_error',last_error,'updated_at',updated_at)), '[]')
                 FROM (SELECT * FROM projection_deliveries
                       WHERE store_name='tantivy_tasks' ORDER BY id)",
                [],
                |row| row.get(0),
            )?;
            let outbox = conn.query_row(
                "SELECT COALESCE(json_group_array(json_object(
                    'id',id,'status',status,'attempts',attempts,
                    'last_error',last_error,'updated_at',updated_at)), '[]')
                 FROM (SELECT * FROM index_outbox ORDER BY id)",
                [],
                |row| row.get(0),
            )?;
            let checkpoint = conn.query_row(
                "SELECT json_object(
                    'last_event_id',last_event_id,'dirty',dirty,
                    'last_rebuild_at',last_rebuild_at,'last_sync_at',last_sync_at,
                    'last_error',last_error,'updated_at',updated_at)
                 FROM derived_store_state WHERE store_name='tantivy_tasks'",
                [],
                |row| row.get(0),
            )?;
            Ok((store, deliveries, outbox, checkpoint))
        };
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();

        std::thread::scope(|scope| -> anyhow::Result<()> {
            let path_ref = &path;
            let lease_token = &lease.lease_token;
            let manifest_ref = &manifest;
            let disposition = scope.spawn(move || {
                snapshot_prepare_disposition_with_before_transaction(
                    path_ref,
                    "tantivy_tasks",
                    "snapshot-owner",
                    lease_token,
                    manifest_ref,
                    move || {
                        entered_tx
                            .send(())
                            .expect("test observes disposition at pre-transaction barrier");
                        resume_rx
                            .recv()
                            .expect("test resumes disposition against writer lock");
                    },
                )
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("snapshot disposition reached pre-transaction barrier");

            let expires_at = SystemClock.now_ms() + 75;
            let conn = connect_file(&path)?;
            with_immediate_tx(&conn, || {
                let changed = conn
                    .execute(
                        "UPDATE projection_store_state SET lease_expires_at=?1
                         WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                        params![
                            expires_at,
                            "tantivy_tasks",
                            "snapshot-owner",
                            lease.lease_token
                        ],
                    )
                    .map_err(storage)?;
                if changed != 1 {
                    return Err(KanbanError::Storage(
                        "test failed to shorten projection lease before disposition".to_owned(),
                    ));
                }
                Ok(())
            })?;
            let snapshot_connection = connect_file(&path)?;
            let before = snapshots(&snapshot_connection)?;

            let writer = connect_file(&path)?;
            writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
            resume_tx
                .send(())
                .expect("resume snapshot disposition against held writer lock");
            while SystemClock.now_ms() <= expires_at {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            writer.execute_batch("COMMIT").map_err(storage)?;

            let error = disposition
                .join()
                .expect("snapshot disposition thread must not panic")
                .expect_err("disposition delayed beyond expiry must reject stale owner");
            assert!(matches!(error, KanbanError::Conflict(_)));
            assert!(
                !backend.generation_present(&manifest.generation),
                "read-only disposition must not create physical evidence"
            );
            let snapshot_connection = connect_file(&path)?;
            let after = snapshots(&snapshot_connection)?;
            assert_eq!(after, before);
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn projection_lease_release_rejects_expired_owner_after_writer_delay() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        crate::service::create_task(
            &path,
            "default",
            "test",
            crate::service::CreateTask::ready("release delay target"),
        )?;
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "release-owner", 10_000)?;
        let delivery_id: i64 = connect_file(&path)?.query_row(
            "SELECT id FROM projection_deliveries WHERE store_name=?1 ORDER BY id LIMIT 1",
            ["tantivy_tasks"],
            |row| row.get(0),
        )?;
        let claim_expires_at = SystemClock.now_ms() + 5_000;
        let conn = connect_file(&path)?;
        with_immediate_tx(&conn, || {
            let changed = conn
                .execute(
                    "UPDATE projection_deliveries
                     SET status='running',claim_owner=?1,claim_token='release-claim',
                         claim_lease_token=?2,claim_fence_epoch=?3,
                         claim_generation='release-generation',claim_expires_at=?4
                     WHERE id=?5 AND status='pending'",
                    params![
                        "release-owner",
                        lease.lease_token,
                        lease.fence_epoch,
                        claim_expires_at,
                        delivery_id
                    ],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::Storage(
                    "test failed to seed running projection delivery".to_owned(),
                ));
            }
            Ok(())
        })?;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let release_path = path.clone();
        let release_token = lease.lease_token.clone();
        let release = std::thread::spawn(move || {
            release_projection_lease_with_before_transaction(
                &release_path,
                "tantivy_tasks",
                "release-owner",
                &release_token,
                || {
                    entered_tx
                        .send(())
                        .expect("test observes lease release at pre-transaction barrier");
                    resume_rx
                        .recv()
                        .expect("test resumes lease release against writer lock");
                },
            )
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("projection lease release reached pre-transaction barrier");

        let expires_at = SystemClock.now_ms() + 75;
        let conn = connect_file(&path)?;
        with_immediate_tx(&conn, || {
            let changed = conn
                .execute(
                    "UPDATE projection_store_state SET lease_expires_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                    params![
                        expires_at,
                        "tantivy_tasks",
                        "release-owner",
                        lease.lease_token
                    ],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::Storage(
                    "test failed to shorten projection lease before release".to_owned(),
                ));
            }
            Ok(())
        })?;
        let conn = connect_file(&path)?;
        let store_before: (Option<String>, Option<String>, i64, Option<i64>, i64) = conn
            .query_row(
                "SELECT lease_owner,lease_token,fence_epoch,lease_expires_at,updated_at
                 FROM projection_store_state WHERE store_name=?1",
                ["tantivy_tasks"],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )?;
        let delivery_before: (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            Option<String>,
            i64,
        ) = conn.query_row(
            "SELECT status,claim_owner,claim_token,claim_lease_token,claim_fence_epoch,
                    claim_generation,claim_expires_at,last_error,updated_at
             FROM projection_deliveries WHERE id=?1",
            [delivery_id],
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
                    row.get(8)?,
                ))
            },
        )?;

        let writer = connect_file(&path)?;
        writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
        resume_tx
            .send(())
            .expect("resume projection lease release against held writer lock");
        while SystemClock.now_ms() <= expires_at {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        writer.execute_batch("COMMIT").map_err(storage)?;

        let error = release
            .join()
            .expect("projection lease release thread must not panic")
            .expect_err("release delayed beyond expiry must reject stale owner");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let conn = connect_file(&path)?;
        let store_after: (Option<String>, Option<String>, i64, Option<i64>, i64) = conn.query_row(
            "SELECT lease_owner,lease_token,fence_epoch,lease_expires_at,updated_at
             FROM projection_store_state WHERE store_name=?1",
            ["tantivy_tasks"],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let delivery_after: (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            Option<String>,
            i64,
        ) = conn.query_row(
            "SELECT status,claim_owner,claim_token,claim_lease_token,claim_fence_epoch,
                    claim_generation,claim_expires_at,last_error,updated_at
             FROM projection_deliveries WHERE id=?1",
            [delivery_id],
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
                    row.get(8)?,
                ))
            },
        )?;
        assert_eq!(store_after, store_before);
        assert_eq!(delivery_after, delivery_before);
        Ok(())
    }

    #[test]
    fn projection_lease_acquisition_uses_fresh_timestamp_after_writer_delay() -> anyhow::Result<()>
    {
        const LEASE_TTL_MS: i64 = 500;
        const MIN_REMAINING_TTL_MS: i64 = 350;

        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let fence_before: i64 = connect_file(&path)?.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            ["tantivy_tasks"],
            |row| row.get(0),
        )?;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let acquisition_path = path.clone();
        let acquisition = std::thread::spawn(move || {
            acquire_projection_lease_with_before_transaction(
                &acquisition_path,
                "tantivy_tasks",
                "delayed-owner",
                LEASE_TTL_MS,
                || {
                    entered_tx
                        .send(())
                        .expect("test observes lease acquisition at pre-transaction barrier");
                    resume_rx
                        .recv()
                        .expect("test resumes lease acquisition against writer lock");
                },
            )
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("projection lease acquisition reached pre-transaction barrier");

        let writer = connect_file(&path)?;
        writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
        resume_tx
            .send(())
            .expect("resume projection lease acquisition against held writer lock");
        std::thread::sleep(std::time::Duration::from_millis(
            (LEASE_TTL_MS + 200) as u64,
        ));
        let acquisition_boundary = SystemClock.now_ms();
        writer.execute_batch("COMMIT").map_err(storage)?;

        let lease = acquisition
            .join()
            .expect("projection lease acquisition thread must not panic")?;
        let conn = connect_file(&path)?;
        let (owner, token, fence_epoch, lease_expires_at, updated_at): (
            Option<String>,
            Option<String>,
            i64,
            Option<i64>,
            i64,
        ) = conn.query_row(
            "SELECT lease_owner,lease_token,fence_epoch,lease_expires_at,updated_at
             FROM projection_store_state WHERE store_name=?1",
            ["tantivy_tasks"],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        assert_eq!(lease.store_name, "tantivy_tasks");
        assert_eq!(lease.owner, "delayed-owner");
        assert_eq!(owner.as_deref(), Some(lease.owner.as_str()));
        assert_eq!(token.as_deref(), Some(lease.lease_token.as_str()));
        assert_eq!(fence_epoch, fence_before + 1);
        assert_eq!(lease.fence_epoch, fence_epoch);
        assert_eq!(
            lease.lease_expires_at,
            lease_expires_at.expect("stored lease expiry")
        );
        assert_eq!(lease.lease_expires_at, updated_at + LEASE_TTL_MS);
        assert!(updated_at >= acquisition_boundary);
        assert!(
            lease.lease_expires_at >= acquisition_boundary + MIN_REMAINING_TTL_MS,
            "lease acquisition must retain at least {MIN_REMAINING_TTL_MS}ms after the writer lock; expiry={}, boundary={acquisition_boundary}",
            lease.lease_expires_at
        );

        release_projection_lease(&path, "tantivy_tasks", "delayed-owner", &lease.lease_token)?;
        Ok(())
    }

    #[test]
    fn projection_lease_renew_does_not_revive_expired_lease_after_writer_delay()
    -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "test")?;
        let lease = acquire_projection_lease(&path, "tantivy_tasks", "owner", 1_000)?;
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let renewal_path = path.clone();
        let renewal_token = lease.lease_token.clone();
        let renewal = std::thread::spawn(move || {
            renew_projection_lease_with_before_transaction(
                &renewal_path,
                "tantivy_tasks",
                "owner",
                &renewal_token,
                1_000,
                || {
                    entered_tx
                        .send(())
                        .expect("test observes timestamp sampling before writer lock");
                    resume_rx
                        .recv()
                        .expect("test resumes store renewal against writer lock");
                },
            )
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("store renewal reached pre-transaction barrier");

        let expires_at = SystemClock.now_ms() + 75;
        let conn = connect_file(&path)?;
        with_immediate_tx(&conn, || {
            let changed = conn
                .execute(
                    "UPDATE projection_store_state SET lease_expires_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                    params![
                        expires_at,
                        "tantivy_tasks",
                        "owner",
                        lease.lease_token.as_str()
                    ],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::Storage(
                    "test failed to shorten projection lease".to_owned(),
                ));
            }
            Ok(())
        })?;

        let writer = connect_file(&path)?;
        writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
        resume_tx
            .send(())
            .expect("resume store renewal against held writer lock");
        while SystemClock.now_ms() <= expires_at {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        writer.execute_batch("COMMIT").map_err(storage)?;

        let error = renewal
            .join()
            .expect("store renewal thread must not panic")
            .expect_err("store renewal delayed beyond expiry must not revive the lease");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let conn = connect_file(&path)?;
        let (owner, token, fence_epoch, actual_expiry): (Option<String>, Option<String>, i64, i64) =
            conn.query_row(
                "SELECT lease_owner,lease_token,fence_epoch,lease_expires_at
                 FROM projection_store_state WHERE store_name=?1",
                ["tantivy_tasks"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        assert_eq!(owner.as_deref(), Some("owner"));
        assert_eq!(token.as_deref(), Some(lease.lease_token.as_str()));
        assert_eq!(fence_epoch, lease.fence_epoch);
        assert_eq!(actual_expiry, expires_at);
        Ok(())
    }
}
