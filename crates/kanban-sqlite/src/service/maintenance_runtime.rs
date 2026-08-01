use std::{
    fmt,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{OnceLock, mpsc},
    thread,
    time::Duration,
};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
#[cfg(test)]
use kanban_indexer::DERIVED_STORE_SCHEMA_VERSION;
use kanban_indexer::{
    LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE, OXIGRAPH_RELATIONS_STORE, TANTIVY_TASKS_STORE,
};
#[cfg(test)]
use kanban_local::DerivedStoreWriteGuard;
use rusqlite::OptionalExtension;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::connect_file;

use super::lancedb_projection::{
    LanceDbProjectionFailureClass, LanceDbProjectionStore, lancedb_failure_class,
};
#[cfg(feature = "oxigraph-backend")]
use super::oxigraph_projection::OxigraphProjectionStore;
#[cfg(feature = "tantivy-backend")]
use super::tantivy_projection::TantivyProjectionStore;
#[cfg(test)]
use super::{
    ProjectionArtifactManifest, ProjectionDestructiveAuthority, ProjectionGenerationBinding,
    ProjectionGenerationRole,
};
use super::{
    ProjectionCorpusMetadata, ProjectionLease, ProjectionRuntimeAvailability, ProjectionStatus,
    ProjectionStoreDescriptor, ProjectionStoreStatus, projection_status,
    projection_status_quiescent, storage, with_immediate_tx,
};
use super::{
    ProjectionSnapshotPrepareDisposition, ProjectionStoreBackend,
    abort_incompatible_projection_generation, abort_projection_generation,
    acquire_projection_lease, begin_projection_generation, prepare_projection_snapshot_with,
    prepare_projection_snapshot_with_disposition, publish_projection_generation_with,
    reconcile_projection_generation_with, recover_incompatible_projection_bindings,
    recover_projection_generation_with, release_projection_lease, renew_projection_lease,
    run_projection_batch_with, validate_backend_for_target, validate_physical_active_artifact_with,
    validate_physical_previous_artifact_with,
};

pub const DEFAULT_MAINTENANCE_LEASE_TTL_MS: i64 = 3_600_000;
pub const DEFAULT_MAINTENANCE_CLAIM_TTL_MS: i64 = 300_000;
pub const DEFAULT_MAINTENANCE_BATCH_SIZE: usize = 250;
const MAX_REBUILD_CATCH_UP_BATCHES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceMode {
    Once,
    Continuous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceRebuildIntent {
    Fresh,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaintenanceStoreRunIntent {
    Automatic,
    Fresh,
    Resume,
}

impl From<MaintenanceRebuildIntent> for MaintenanceStoreRunIntent {
    fn from(intent: MaintenanceRebuildIntent) -> Self {
        match intent {
            MaintenanceRebuildIntent::Fresh => Self::Fresh,
            MaintenanceRebuildIntent::Resume => Self::Resume,
        }
    }
}

impl MaintenanceStoreRunIntent {
    fn force_rebuild(self) -> bool {
        matches!(self, Self::Fresh | Self::Resume)
    }
}

impl MaintenanceMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Continuous => "continuous",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceRunOptions {
    pub lease_ttl_ms: i64,
    pub claim_ttl_ms: i64,
    pub batch_size: usize,
}

impl Default for MaintenanceRunOptions {
    fn default() -> Self {
        Self {
            lease_ttl_ms: DEFAULT_MAINTENANCE_LEASE_TTL_MS,
            claim_ttl_ms: DEFAULT_MAINTENANCE_CLAIM_TTL_MS,
            batch_size: DEFAULT_MAINTENANCE_BATCH_SIZE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceStoreFailureKind {
    Provider,
    Backend,
    Delivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaintenanceStoreResult {
    Succeeded {
        action: String,
        processed: usize,
    },
    Failed {
        kind: MaintenanceStoreFailureKind,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceStoreRun {
    pub store_name: String,
    pub result: MaintenanceStoreResult,
    pub lifecycle_status: String,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceRunReport {
    pub database_instance_id: String,
    pub protocol_version: i64,
    pub owner: String,
    pub mode: MaintenanceMode,
    pub stores: Vec<MaintenanceStoreRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MaintenanceRuntimeIdentity {
    capabilities_json: String,
    build_identity: String,
}

impl MaintenanceRuntimeIdentity {
    fn current() -> Result<Self> {
        Self::new(compiled_capabilities(), runtime_build_identity()?)
    }

    fn new(mut capabilities: Vec<String>, build_identity: impl Into<String>) -> Result<Self> {
        capabilities.sort();
        capabilities.dedup();
        if capabilities
            .iter()
            .any(|capability| capability.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput(
                "maintenance capability cannot be empty".to_owned(),
            ));
        }
        let build_identity = build_identity.into();
        if build_identity.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "maintenance build identity cannot be empty".to_owned(),
            ));
        }
        let capabilities_json = serde_json::to_string(&capabilities)
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(Self {
            capabilities_json,
            build_identity,
        })
    }

    #[cfg(all(test, feature = "tantivy-backend", feature = "oxigraph-backend"))]
    fn for_test(capabilities: Vec<String>, build_identity: &str) -> Self {
        Self::new(capabilities, build_identity).expect("valid test runtime identity")
    }
}

pub struct MaintenanceSession {
    db_path: PathBuf,
    owner: String,
    lease_token: String,
    mode: MaintenanceMode,
    options: MaintenanceRunOptions,
    identity: MaintenanceRuntimeIdentity,
    released: bool,
}

impl fmt::Debug for MaintenanceSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaintenanceSession")
            .field("db_path", &self.db_path)
            .field("owner", &self.owner)
            .field("lease_token", &"[REDACTED]")
            .field("mode", &self.mode)
            .field("options", &self.options)
            .field("identity", &self.identity)
            .field("released", &self.released)
            .finish()
    }
}

impl MaintenanceSession {
    pub fn start(
        path: impl AsRef<Path>,
        owner: &str,
        mode: MaintenanceMode,
        options: MaintenanceRunOptions,
    ) -> Result<Self> {
        Self::start_with_identity(
            path,
            owner,
            mode,
            options,
            MaintenanceRuntimeIdentity::current()?,
        )
    }

    fn start_with_identity(
        path: impl AsRef<Path>,
        owner: &str,
        mode: MaintenanceMode,
        options: MaintenanceRunOptions,
        identity: MaintenanceRuntimeIdentity,
    ) -> Result<Self> {
        validate_options(owner, &options)?;
        let db_path = path.as_ref().to_path_buf();
        let conn = super::maintenance::connect_existing_database(&db_path)?;
        validate_continuous_capabilities(&conn, mode, &identity)?;
        drop(conn);
        let lease_token =
            acquire_maintenance_owner(&db_path, owner, mode, options.lease_ttl_ms, &identity)?;
        Ok(Self {
            db_path,
            owner: owner.to_owned(),
            lease_token,
            mode,
            options,
            identity,
            released: false,
        })
    }

    pub fn run_once(&mut self) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        let stores = vec![
            #[cfg(feature = "tantivy-backend")]
            run_tantivy_once(self, MaintenanceStoreRunIntent::Automatic)?,
            #[cfg(feature = "oxigraph-backend")]
            run_oxigraph_once(self, MaintenanceStoreRunIntent::Automatic)?,
            run_lancedb_once(
                self,
                LANCEDB_LABEL_ATOMS_STORE,
                "LanceDB label atoms",
                MaintenanceStoreRunIntent::Automatic,
            )?,
            run_lancedb_once(
                self,
                LANCEDB_CHUNKS_STORE,
                "LanceDB task chunks",
                MaintenanceStoreRunIntent::Automatic,
            )?,
        ];
        renew_maintenance_owner(self)?;
        self.report(stores)
    }

    pub fn rebuild(&mut self, store_name: &str) -> Result<MaintenanceRunReport> {
        self.rebuild_with_intent(store_name, MaintenanceRebuildIntent::Fresh)
    }

    pub fn resume_rebuild(&mut self, store_name: &str) -> Result<MaintenanceRunReport> {
        self.rebuild_with_intent(store_name, MaintenanceRebuildIntent::Resume)
    }

    fn rebuild_with_intent(
        &mut self,
        store_name: &str,
        intent: MaintenanceRebuildIntent,
    ) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        validate_rebuild_intent(&self.db_path, store_name, intent)?;
        let run_intent = MaintenanceStoreRunIntent::from(intent);
        let store = match store_name {
            TANTIVY_TASKS_STORE => run_tantivy_once(self, run_intent)?,
            OXIGRAPH_RELATIONS_STORE => run_oxigraph_once(self, run_intent)?,
            LANCEDB_LABEL_ATOMS_STORE => {
                run_lancedb_once(self, store_name, "LanceDB label atoms", run_intent)?
            }
            LANCEDB_CHUNKS_STORE => {
                run_lancedb_once(self, store_name, "LanceDB task chunks", run_intent)?
            }
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "projection store {store_name} is not yet wired to the unified maintenance runtime"
                )));
            }
        };
        renew_maintenance_owner(self)?;
        self.report(vec![store])
    }

    pub fn rebuild_all(&mut self) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        for store_name in compiled_capabilities() {
            validate_rebuild_intent(&self.db_path, &store_name, MaintenanceRebuildIntent::Fresh)?;
        }
        let stores = vec![
            #[cfg(feature = "tantivy-backend")]
            run_tantivy_once(self, MaintenanceStoreRunIntent::Fresh)?,
            #[cfg(feature = "oxigraph-backend")]
            run_oxigraph_once(self, MaintenanceStoreRunIntent::Fresh)?,
            run_lancedb_once(
                self,
                LANCEDB_LABEL_ATOMS_STORE,
                "LanceDB label atoms",
                MaintenanceStoreRunIntent::Fresh,
            )?,
            run_lancedb_once(
                self,
                LANCEDB_CHUNKS_STORE,
                "LanceDB task chunks",
                MaintenanceStoreRunIntent::Fresh,
            )?,
        ];
        renew_maintenance_owner(self)?;
        self.report(stores)
    }

    pub fn heartbeat(&mut self) -> Result<()> {
        renew_maintenance_owner(self)
    }

    pub(super) fn run_with_owner_heartbeat<T>(
        &self,
        operation: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        renew_maintenance_owner(self)?;
        let interval_ms = (self.options.lease_ttl_ms / 3).clamp(1, 60_000) as u64;
        thread::scope(|scope| {
            let (stop_tx, stop_rx) = mpsc::channel();
            let heartbeat = scope.spawn(move || {
                loop {
                    match stop_rx.recv_timeout(Duration::from_millis(interval_ms)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                        Err(mpsc::RecvTimeoutError::Timeout) => renew_maintenance_owner(self)?,
                    }
                }
            });
            let operation_result = operation();
            let _ = stop_tx.send(());
            let heartbeat_result = heartbeat.join().map_err(|_| {
                KanbanError::Storage("projection maintenance heartbeat thread panicked".to_owned())
            })?;
            match (operation_result, heartbeat_result) {
                (Err(error), _) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Ok(value), Ok(())) => {
                    renew_maintenance_owner(self)?;
                    Ok(value)
                }
            }
        })
    }

    pub(super) fn renew_and_validate_database_identity(
        &self,
        expected_database_instance_id: &str,
    ) -> Result<()> {
        let now = SystemClock.now_ms();
        let expires_at = checked_expiry(now, self.options.lease_ttl_ms)?;
        let conn = connect_file(&self.db_path)?;
        with_immediate_tx(&conn, || {
            renew_maintenance_owner_on_connection(self, &conn, now, expires_at)?;
            let actual_database_instance_id = conn
                .query_row(
                    "SELECT database_instance_id
                     FROM projection_database
                     WHERE singleton=1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(storage)?;
            if actual_database_instance_id != expected_database_instance_id {
                return Err(KanbanError::Conflict(format!(
                    "projection database identity changed while maintenance owner was active: expected {expected_database_instance_id}, got {actual_database_instance_id}"
                )));
            }
            Ok(())
        })
    }

    pub fn lease_ttl_ms(&self) -> i64 {
        self.options.lease_ttl_ms
    }

    pub fn finish(mut self) -> Result<()> {
        let result = release_maintenance_owner(
            &self.db_path,
            &self.owner,
            &self.lease_token,
            &self.identity,
        );
        if result.is_ok() {
            self.released = true;
        }
        result
    }

    fn report(&self, stores: Vec<MaintenanceStoreRun>) -> Result<MaintenanceRunReport> {
        let status = maintenance_status(&self.db_path)?;
        Ok(MaintenanceRunReport {
            database_instance_id: status.database_instance_id,
            protocol_version: status.protocol_version,
            owner: self.owner.clone(),
            mode: self.mode,
            stores,
        })
    }
}

impl Drop for MaintenanceSession {
    fn drop(&mut self) {
        if !self.released {
            let _ = release_maintenance_owner(
                &self.db_path,
                &self.owner,
                &self.lease_token,
                &self.identity,
            );
        }
    }
}

pub fn maintenance_status(path: impl AsRef<Path>) -> Result<ProjectionStatus> {
    let path = path.as_ref();
    let mut status = projection_status(path)?;
    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    {
        #[cfg(feature = "tantivy-backend")]
        enrich_tantivy_physical_health(path, &mut status)?;
        #[cfg(feature = "oxigraph-backend")]
        enrich_oxigraph_physical_health(path, &mut status)?;
    }
    apply_runtime_availability(&mut status);
    enrich_lancedb_physical_health(
        path,
        &mut status,
        LANCEDB_LABEL_ATOMS_STORE,
        "LanceDB label atoms",
    )?;
    enrich_lancedb_physical_health(
        path,
        &mut status,
        LANCEDB_CHUNKS_STORE,
        "LanceDB task chunks",
    )?;
    Ok(status)
}

pub fn maintenance_continuous_capability_complete() -> bool {
    [
        TANTIVY_TASKS_STORE,
        OXIGRAPH_RELATIONS_STORE,
        LANCEDB_LABEL_ATOMS_STORE,
        LANCEDB_CHUNKS_STORE,
    ]
    .into_iter()
    .all(compiled_capability)
}

fn apply_runtime_availability(status: &mut ProjectionStatus) {
    for store in &mut status.stores {
        if !compiled_capability(&store.store_name) {
            store.runtime_availability = ProjectionRuntimeAvailability::Unavailable;
            store.fallback_reason = Some("backend_unavailable".to_owned());
            continue;
        }
        if status.maintenance_owner.active
            && !status
                .maintenance_owner
                .capabilities
                .iter()
                .any(|capability| capability == &store.store_name)
        {
            store.runtime_availability = ProjectionRuntimeAvailability::Unverified;
            store.fallback_reason = Some("maintenance_owner_capability_unverified".to_owned());
            continue;
        }
        store.runtime_availability = ProjectionRuntimeAvailability::Available;
    }
}

fn compiled_capabilities() -> Vec<String> {
    [
        (TANTIVY_TASKS_STORE, cfg!(feature = "tantivy-backend")),
        (OXIGRAPH_RELATIONS_STORE, cfg!(feature = "oxigraph-backend")),
        (LANCEDB_LABEL_ATOMS_STORE, true),
        (LANCEDB_CHUNKS_STORE, true),
    ]
    .into_iter()
    .filter(|(_, enabled)| *enabled)
    .map(|(store_name, _)| store_name.to_owned())
    .collect()
}

fn compiled_capability(store_name: &str) -> bool {
    match store_name {
        TANTIVY_TASKS_STORE => cfg!(feature = "tantivy-backend"),
        OXIGRAPH_RELATIONS_STORE => cfg!(feature = "oxigraph-backend"),
        LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE => true,
        _ => false,
    }
}

fn enrich_lancedb_physical_health(
    path: &Path,
    status: &mut ProjectionStatus,
    store_name: &str,
    display_name: &str,
) -> Result<()> {
    let backend = match LanceDbProjectionStore::connect_resolved(path, store_name) {
        Ok(backend) => backend,
        Err(error) => {
            if let Some(store) = status
                .stores
                .iter_mut()
                .find(|store| store.store_name == store_name)
            {
                let failure = lancedb_failure_class(&error);
                store.runtime_availability = ProjectionRuntimeAvailability::Unavailable;
                store.lifecycle_status = "error".to_owned();
                store.last_error = Some(format!(
                    "{display_name} projection helper is unavailable: {error}"
                ));
                if store.fallback_reason.as_deref() != Some("corpus_binding_upgrade_required") {
                    store.fallback_reason = Some(
                        match failure {
                            LanceDbProjectionFailureClass::Provider => "provider_unavailable",
                            LanceDbProjectionFailureClass::Backend
                            | LanceDbProjectionFailureClass::Delivery => "helper_unavailable",
                        }
                        .to_owned(),
                    );
                }
            }
            return Ok(());
        }
    };
    enrich_physical_health(path, status, store_name, display_name, &backend)
}

#[cfg(feature = "oxigraph-backend")]
fn enrich_oxigraph_physical_health(path: &Path, status: &mut ProjectionStatus) -> Result<()> {
    let backend = match OxigraphProjectionStore::new(path) {
        Ok(backend) => backend,
        Err(error) => {
            mark_physical_health_unavailable(status, OXIGRAPH_RELATIONS_STORE, "Oxigraph", error);
            return Ok(());
        }
    };
    enrich_physical_health(path, status, OXIGRAPH_RELATIONS_STORE, "Oxigraph", &backend)
}

#[cfg(feature = "tantivy-backend")]
fn enrich_tantivy_physical_health(path: &Path, status: &mut ProjectionStatus) -> Result<()> {
    let backend = match TantivyProjectionStore::new(path) {
        Ok(backend) => backend,
        Err(error) => {
            mark_physical_health_unavailable(status, TANTIVY_TASKS_STORE, "Tantivy", error);
            return Ok(());
        }
    };
    enrich_physical_health(path, status, TANTIVY_TASKS_STORE, "Tantivy", &backend)
}

#[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
fn mark_physical_health_unavailable(
    status: &mut ProjectionStatus,
    store_name: &str,
    display_name: &str,
    error: KanbanError,
) {
    if let Some(store) = status
        .stores
        .iter_mut()
        .find(|store| store.store_name == store_name)
    {
        store.lifecycle_status = "error".to_owned();
        store.last_error = Some(format!(
            "{display_name} physical backend is unavailable: {error}"
        ));
        store.fallback_reason = Some("physical_generation_unavailable".to_owned());
    }
}

fn enrich_physical_health(
    path: &Path,
    status: &mut ProjectionStatus,
    store_name: &str,
    display_name: &str,
    backend: &impl ProjectionStoreBackend,
) -> Result<()> {
    let Some(store) = status
        .stores
        .iter_mut()
        .find(|store| store.store_name == store_name && store.control_plane == "v2")
    else {
        return Ok(());
    };
    let Some(generation) = store.active_generation.as_deref() else {
        return Ok(());
    };
    let physical = validate_physical_active_artifact_with(path, store_name, backend)
        .and_then(|evidence| {
            evidence.ok_or_else(|| {
                KanbanError::Storage(format!(
                    "active {display_name} generation {generation} is missing from SQLite"
                ))
            })
        })
        .and_then(|active| {
            validate_physical_previous_artifact_with(path, store_name, backend)?;
            Ok(active)
        });
    if let Err(error) = physical {
        store.lifecycle_status = "error".to_owned();
        store.last_error = Some(error.to_string());
        if store.fallback_reason.as_deref() != Some("corpus_binding_upgrade_required") {
            store.fallback_reason = Some("physical_generation_unavailable".to_owned());
        }
    }
    Ok(())
}

pub fn maintenance_run_once(
    path: impl AsRef<Path>,
    owner: &str,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceRunReport> {
    let mut session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let report = session.run_once()?;
    session.finish()?;
    Ok(report)
}

pub fn maintenance_rebuild_store(
    path: impl AsRef<Path>,
    owner: &str,
    store_name: &str,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceRunReport> {
    let mut session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let report = session.rebuild(store_name)?;
    session.finish()?;
    Ok(report)
}

pub fn maintenance_resume_rebuild_store(
    path: impl AsRef<Path>,
    owner: &str,
    store_name: &str,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceRunReport> {
    let mut session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let report = session.resume_rebuild(store_name)?;
    session.finish()?;
    Ok(report)
}

pub fn maintenance_rebuild_all(
    path: impl AsRef<Path>,
    owner: &str,
    options: MaintenanceRunOptions,
) -> Result<MaintenanceRunReport> {
    let mut session = MaintenanceSession::start(path, owner, MaintenanceMode::Once, options)?;
    let report = session.rebuild_all()?;
    session.finish()?;
    Ok(report)
}

pub fn maintenance_plan_rebuild_store(
    path: impl AsRef<Path>,
    owner: &str,
    store_name: &str,
    intent: MaintenanceRebuildIntent,
) -> Result<MaintenanceRunReport> {
    let path = path.as_ref();
    let status = maintenance_plan_status(path)?;
    validate_rebuild_intent_in_status(&status, store_name, intent)?;
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            KanbanError::InvalidInput(format!(
                "projection store {store_name} is not yet wired to the unified maintenance runtime"
            ))
        })?;
    Ok(MaintenanceRunReport {
        database_instance_id: status.database_instance_id,
        protocol_version: status.protocol_version,
        owner: owner.to_owned(),
        mode: MaintenanceMode::Once,
        stores: vec![planned_rebuild_store(store, intent)],
    })
}

pub fn maintenance_plan_rebuild_all(
    path: impl AsRef<Path>,
    owner: &str,
) -> Result<MaintenanceRunReport> {
    let path = path.as_ref();
    let store_names = compiled_capabilities();
    let status = maintenance_plan_status(path)?;
    for store_name in &store_names {
        validate_rebuild_intent_in_status(&status, store_name, MaintenanceRebuildIntent::Fresh)?;
    }
    let stores = store_names
        .iter()
        .map(|store_name| {
            status
                .stores
                .iter()
                .find(|store| store.store_name == *store_name)
                .map(|store| planned_rebuild_store(store, MaintenanceRebuildIntent::Fresh))
                .ok_or_else(|| {
                    KanbanError::Storage(format!("projection store {store_name} state is missing"))
                })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(MaintenanceRunReport {
        database_instance_id: status.database_instance_id,
        protocol_version: status.protocol_version,
        owner: owner.to_owned(),
        mode: MaintenanceMode::Once,
        stores,
    })
}

fn maintenance_plan_status(path: &Path) -> Result<ProjectionStatus> {
    let mut status = projection_status_quiescent(path)?;
    apply_runtime_availability(&mut status);
    Ok(status)
}

fn validate_rebuild_intent(
    path: &Path,
    store_name: &str,
    intent: MaintenanceRebuildIntent,
) -> Result<()> {
    let status = projection_status(path)?;
    validate_rebuild_intent_in_status(&status, store_name, intent)
}

fn validate_rebuild_intent_in_status(
    status: &ProjectionStatus,
    store_name: &str,
    intent: MaintenanceRebuildIntent,
) -> Result<()> {
    if !compiled_capability(store_name) {
        return Err(KanbanError::InvalidInput(format!(
            "projection store {store_name} is not available in this maintenance runtime"
        )));
    }
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            KanbanError::Storage(format!("projection store {store_name} state is missing"))
        })?;
    match (intent, store.building_generation.as_deref()) {
        (MaintenanceRebuildIntent::Fresh, Some(generation)) => {
            Err(KanbanError::InvalidInput(format!(
                "projection store {store_name} has unfinished generation {generation}; use --resume"
            )))
        }
        (MaintenanceRebuildIntent::Resume, None) => Err(KanbanError::InvalidInput(format!(
            "projection store {store_name} has no unfinished generation to resume"
        ))),
        _ => Ok(()),
    }
}

fn planned_rebuild_store(
    store: &ProjectionStoreStatus,
    intent: MaintenanceRebuildIntent,
) -> MaintenanceStoreRun {
    MaintenanceStoreRun {
        store_name: store.store_name.clone(),
        result: MaintenanceStoreResult::Succeeded {
            action: match intent {
                MaintenanceRebuildIntent::Fresh => "dry_run_rebuild",
                MaintenanceRebuildIntent::Resume => "dry_run_resume",
            }
            .to_owned(),
            processed: 0,
        },
        lifecycle_status: store.lifecycle_status.clone(),
        fallback_reason: store.fallback_reason.clone(),
    }
}

fn validate_continuous_capabilities(
    conn: &rusqlite::Connection,
    mode: MaintenanceMode,
    identity: &MaintenanceRuntimeIdentity,
) -> Result<()> {
    if mode != MaintenanceMode::Continuous {
        return Ok(());
    }
    let capabilities: Vec<String> = serde_json::from_str(&identity.capabilities_json)
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    let mut statement = conn
        .prepare("SELECT store_name FROM projection_store_state ORDER BY store_name")
        .map_err(storage)?;
    let required = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    let missing = required
        .into_iter()
        .filter(|store_name| !capabilities.contains(store_name))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(KanbanError::InvalidInput(format!(
            "continuous maintenance requires capabilities for every projection store; missing: {}",
            missing.join(",")
        )));
    }
    Ok(())
}

fn acquire_maintenance_owner(
    path: &Path,
    owner: &str,
    mode: MaintenanceMode,
    ttl_ms: i64,
    identity: &MaintenanceRuntimeIdentity,
) -> Result<String> {
    let now = SystemClock.now_ms();
    let expires_at = checked_expiry(now, ttl_ms)?;
    let lease_token = new_typed_id("pmlease");
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let changed = conn
            .execute(
                "UPDATE projection_maintenance_owner
                 SET owner=?1,lease_token=?2,lease_expires_at=?3,mode=?4,
                     started_at=?5,last_heartbeat_at=?5,
                     capabilities_json=?6,build_identity=?7,updated_at=?5
                 WHERE singleton=1
                   AND (lease_token IS NULL OR lease_expires_at<=?5)",
                params![
                    owner,
                    lease_token,
                    expires_at,
                    mode.as_str(),
                    now,
                    identity.capabilities_json,
                    identity.build_identity
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::Conflict(
                "projection maintenance runtime already has an active owner".to_owned(),
            ));
        }
        Ok(())
    })?;
    Ok(lease_token)
}

fn renew_maintenance_owner(session: &MaintenanceSession) -> Result<()> {
    renew_maintenance_owner_lease(
        &session.db_path,
        &session.owner,
        &session.lease_token,
        session.options.lease_ttl_ms,
        &session.identity,
    )
}

fn renew_maintenance_owner_lease(
    path: &Path,
    owner: &str,
    lease_token: &str,
    ttl_ms: i64,
    identity: &MaintenanceRuntimeIdentity,
) -> Result<()> {
    renew_maintenance_owner_lease_with_before_transaction(
        path,
        owner,
        lease_token,
        ttl_ms,
        identity,
        || {},
    )
}

fn renew_maintenance_owner_lease_with_before_transaction(
    path: &Path,
    owner: &str,
    lease_token: &str,
    ttl_ms: i64,
    identity: &MaintenanceRuntimeIdentity,
    before_transaction: impl FnOnce(),
) -> Result<()> {
    before_transaction();
    let conn = connect_file(path)?;
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        let expires_at = checked_expiry(now, ttl_ms)?;
        renew_maintenance_owner_lease_on_connection(
            &conn,
            owner,
            lease_token,
            identity,
            now,
            expires_at,
        )
    })
}

fn renew_maintenance_owner_on_connection(
    session: &MaintenanceSession,
    conn: &rusqlite::Connection,
    now: i64,
    expires_at: i64,
) -> Result<()> {
    renew_maintenance_owner_lease_on_connection(
        conn,
        &session.owner,
        &session.lease_token,
        &session.identity,
        now,
        expires_at,
    )
}

fn renew_maintenance_owner_lease_on_connection(
    conn: &rusqlite::Connection,
    owner: &str,
    lease_token: &str,
    identity: &MaintenanceRuntimeIdentity,
    now: i64,
    expires_at: i64,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE projection_maintenance_owner
             SET lease_expires_at=?1,last_heartbeat_at=?2,updated_at=?2
             WHERE singleton=1 AND owner=?3 AND lease_token=?4
               AND lease_expires_at>?2
               AND capabilities_json=?5 AND build_identity=?6",
            params![
                expires_at,
                now,
                owner,
                lease_token,
                identity.capabilities_json,
                identity.build_identity
            ],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Conflict(
            "projection maintenance owner lease is stale".to_owned(),
        ));
    }
    Ok(())
}

fn release_maintenance_owner(
    path: &Path,
    owner: &str,
    lease_token: &str,
    identity: &MaintenanceRuntimeIdentity,
) -> Result<()> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let changed = conn
        .execute(
            "UPDATE projection_maintenance_owner
         SET owner=NULL,lease_token=NULL,lease_expires_at=NULL,mode=NULL,
             started_at=NULL,last_heartbeat_at=NULL,
             capabilities_json='[]',build_identity=NULL,updated_at=?1
         WHERE singleton=1 AND owner=?2 AND lease_token=?3
           AND capabilities_json=?4 AND build_identity=?5",
            params![
                now,
                owner,
                lease_token,
                identity.capabilities_json,
                identity.build_identity
            ],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Conflict(
            "projection maintenance owner token is stale".to_owned(),
        ));
    }
    Ok(())
}

fn run_tantivy_once(
    session: &mut MaintenanceSession,
    intent: MaintenanceStoreRunIntent,
) -> Result<MaintenanceStoreRun> {
    #[cfg(feature = "tantivy-backend")]
    {
        let backend = match TantivyProjectionStore::new(&session.db_path) {
            Ok(backend) => backend,
            Err(error) => {
                return failed_store_run_without_store_lease(
                    session,
                    TANTIVY_TASKS_STORE,
                    "Tantivy",
                    MaintenanceStoreFailureKind::Backend,
                    error,
                );
            }
        };
        run_projection_store_once(session, TANTIVY_TASKS_STORE, "Tantivy", &backend, intent)
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = (session, intent);
        Err(KanbanError::InvalidInput(
            "unified Tantivy maintenance requires the tantivy-backend feature".to_owned(),
        ))
    }
}

fn run_oxigraph_once(
    session: &mut MaintenanceSession,
    intent: MaintenanceStoreRunIntent,
) -> Result<MaintenanceStoreRun> {
    #[cfg(feature = "oxigraph-backend")]
    {
        let backend = match OxigraphProjectionStore::new(&session.db_path) {
            Ok(backend) => backend,
            Err(error) => {
                return failed_store_run_without_store_lease(
                    session,
                    OXIGRAPH_RELATIONS_STORE,
                    "Oxigraph",
                    MaintenanceStoreFailureKind::Backend,
                    error,
                );
            }
        };
        run_projection_store_once(
            session,
            OXIGRAPH_RELATIONS_STORE,
            "Oxigraph",
            &backend,
            intent,
        )
    }
    #[cfg(not(feature = "oxigraph-backend"))]
    {
        let _ = (session, intent);
        Err(KanbanError::InvalidInput(
            "unified Oxigraph maintenance requires the oxigraph-backend feature".to_owned(),
        ))
    }
}

fn run_lancedb_once(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    intent: MaintenanceStoreRunIntent,
) -> Result<MaintenanceStoreRun> {
    let backend = match LanceDbProjectionStore::connect_resolved(&session.db_path, store_name) {
        Ok(backend) => backend,
        Err(error) => {
            let kind = match lancedb_failure_class(&error) {
                LanceDbProjectionFailureClass::Provider => MaintenanceStoreFailureKind::Provider,
                LanceDbProjectionFailureClass::Backend => MaintenanceStoreFailureKind::Backend,
                LanceDbProjectionFailureClass::Delivery => MaintenanceStoreFailureKind::Delivery,
            };
            return failed_store_run_without_store_lease(
                session,
                store_name,
                display_name,
                kind,
                error,
            );
        }
    };
    run_projection_store_once(session, store_name, display_name, &backend, intent)
}

#[derive(Debug)]
enum MaintenanceStoreAttemptError {
    Fatal(KanbanError),
    Store {
        kind: MaintenanceStoreFailureKind,
        error: KanbanError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetValidationDisposition {
    Retry,
    Rebuild,
}

fn target_validation_disposition(error: &KanbanError) -> TargetValidationDisposition {
    match error {
        KanbanError::Conflict(_) | KanbanError::InvalidInput(_) => {
            TargetValidationDisposition::Rebuild
        }
        _ => TargetValidationDisposition::Retry,
    }
}

type MaintenanceStoreAttempt<T> = std::result::Result<T, MaintenanceStoreAttemptError>;

fn begin_generation_failure(error: KanbanError) -> MaintenanceStoreAttemptError {
    MaintenanceStoreAttemptError::Store {
        kind: MaintenanceStoreFailureKind::Backend,
        error,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExplicitResumeInvariant {
    generation: String,
    descriptor: ProjectionStoreDescriptor,
}

fn require_explicit_resume_invariant(
    store: &ProjectionStoreStatus,
    invariant: &ExplicitResumeInvariant,
) -> MaintenanceStoreAttempt<()> {
    if store.building_generation.as_deref() != Some(invariant.generation.as_str())
        || !building_binding_matches_descriptor(store, &invariant.descriptor)
    {
        return Err(MaintenanceStoreAttemptError::Fatal(KanbanError::Conflict(
            format!(
                "explicit resume for projection store {} no longer matches unfinished generation {}; refusing to replace it",
                store.store_name, invariant.generation
            ),
        )));
    }
    Ok(())
}

fn prepare_snapshot_with_one_automatic_rebase(
    path: &Path,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    backend: &impl ProjectionStoreBackend,
    intent: MaintenanceStoreRunIntent,
    fresh_generation_created: bool,
) -> MaintenanceStoreAttempt<()> {
    let disposition =
        prepare_projection_snapshot_with_disposition(path, store_name, owner, lease_token, backend)
            .map_err(|error| MaintenanceStoreAttemptError::Store {
                kind: MaintenanceStoreFailureKind::Backend,
                error,
            })?;
    match disposition {
        ProjectionSnapshotPrepareDisposition::Prepared(_) => Ok(()),
        ProjectionSnapshotPrepareDisposition::CoverageChanged => {
            if intent == MaintenanceStoreRunIntent::Resume {
                return Err(MaintenanceStoreAttemptError::Fatal(KanbanError::Conflict(
                    format!(
                        "explicit resume for projection store {store_name} found an obsolete snapshot baseline; refusing to replace it"
                    ),
                )));
            }
            if intent == MaintenanceStoreRunIntent::Fresh && !fresh_generation_created {
                return Err(MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error: KanbanError::Conflict(format!(
                        "fresh rebuild for projection store {store_name} did not create the obsolete snapshot generation"
                    )),
                });
            }
            abort_projection_generation(path, store_name, owner, lease_token, backend).map_err(
                |error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                },
            )?;
            begin_projection_generation(path, store_name, owner, lease_token, backend).map_err(
                |error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                },
            )?;
            match prepare_projection_snapshot_with_disposition(
                path,
                store_name,
                owner,
                lease_token,
                backend,
            )
            .map_err(|error| MaintenanceStoreAttemptError::Store {
                kind: MaintenanceStoreFailureKind::Backend,
                error,
            })? {
                ProjectionSnapshotPrepareDisposition::Prepared(_) => Ok(()),
                ProjectionSnapshotPrepareDisposition::CoverageChanged => {
                    Err(MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Backend,
                        error: KanbanError::Conflict(format!(
                            "projection snapshot coverage changed again for store {store_name}; automatic maintenance will retry in a later pass"
                        )),
                    })
                }
            }
        }
    }
}

fn run_projection_store_once(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    backend: &impl ProjectionStoreBackend,
    intent: MaintenanceStoreRunIntent,
) -> Result<MaintenanceStoreRun> {
    let lease = acquire_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        session.options.lease_ttl_ms,
    )?;
    let heartbeat = ProjectionLeaseHeartbeat::new(session, &lease);
    let operation = heartbeat.run(|| {
        run_projection_store_operation_with_intent(
            session,
            store_name,
            display_name,
            &lease.lease_token,
            backend,
            intent,
        )
    });
    let operation = match operation {
        Err(error) => Err(error),
        Ok(Ok(report)) => Ok(report),
        Ok(Err(MaintenanceStoreAttemptError::Fatal(error))) => Err(error),
        Ok(Err(MaintenanceStoreAttemptError::Store { kind, error })) => {
            let kind = classify_store_failure(store_name, &error, kind);
            failed_store_run(
                session,
                store_name,
                display_name,
                &lease.lease_token,
                kind,
                error,
            )
        }
    };
    let release = release_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        &lease.lease_token,
    );
    match (operation, release) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn classify_store_failure(
    store_name: &str,
    error: &KanbanError,
    fallback: MaintenanceStoreFailureKind,
) -> MaintenanceStoreFailureKind {
    if matches!(store_name, LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE)
        && lancedb_failure_class(error) == LanceDbProjectionFailureClass::Provider
    {
        MaintenanceStoreFailureKind::Provider
    } else {
        fallback
    }
}

#[cfg(test)]
fn run_projection_store_operation(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    lease_token: &str,
    backend: &impl ProjectionStoreBackend,
    force_rebuild: bool,
) -> MaintenanceStoreAttempt<MaintenanceStoreRun> {
    run_projection_store_operation_with_intent(
        session,
        store_name,
        display_name,
        lease_token,
        backend,
        if force_rebuild {
            MaintenanceStoreRunIntent::Fresh
        } else {
            MaintenanceStoreRunIntent::Automatic
        },
    )
}

fn run_projection_store_operation_with_intent(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    lease_token: &str,
    backend: &impl ProjectionStoreBackend,
    intent: MaintenanceStoreRunIntent,
) -> MaintenanceStoreAttempt<MaintenanceStoreRun> {
    let mut action = "idle".to_owned();
    let initial_status =
        maintenance_status(&session.db_path).map_err(MaintenanceStoreAttemptError::Fatal)?;
    let initial_store = initial_status
        .stores
        .iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            MaintenanceStoreAttemptError::Fatal(KanbanError::Storage(format!(
                "{display_name} projection state is missing"
            )))
        })?;
    let resume_invariant = match intent {
        MaintenanceStoreRunIntent::Automatic => None,
        MaintenanceStoreRunIntent::Fresh => {
            if let Some(generation) = initial_store.building_generation.as_deref() {
                return Err(MaintenanceStoreAttemptError::Fatal(
                    KanbanError::InvalidInput(format!(
                        "projection store {store_name} has unfinished generation {generation}; use --resume"
                    )),
                ));
            }
            None
        }
        MaintenanceStoreRunIntent::Resume => {
            let generation = initial_store.building_generation.clone().ok_or_else(|| {
                MaintenanceStoreAttemptError::Fatal(KanbanError::InvalidInput(format!(
                    "projection store {store_name} has no unfinished generation to resume"
                )))
            })?;
            let descriptor =
                backend
                    .descriptor()
                    .map_err(|error| MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Backend,
                        error,
                    })?;
            let invariant = ExplicitResumeInvariant {
                generation,
                descriptor,
            };
            require_explicit_resume_invariant(initial_store, &invariant)?;
            Some(invariant)
        }
    };
    let incompatible_binding_reset = if resume_invariant.is_some() {
        false
    } else {
        recover_incompatible_projection_bindings(
            &session.db_path,
            store_name,
            &session.owner,
            lease_token,
            backend,
        )
        .map_err(|error| MaintenanceStoreAttemptError::Store {
            kind: classify_store_failure(store_name, &error, MaintenanceStoreFailureKind::Backend),
            error,
        })?
    };
    let status = if incompatible_binding_reset {
        maintenance_status(&session.db_path).map_err(MaintenanceStoreAttemptError::Fatal)?
    } else {
        initial_status
    };
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            MaintenanceStoreAttemptError::Fatal(KanbanError::Storage(format!(
                "{display_name} projection state is missing"
            )))
        })?;
    let physical_rebuild = matches!(
        store.fallback_reason.as_deref(),
        Some("physical_generation_unavailable" | "corpus_binding_upgrade_required")
    ) || incompatible_binding_reset;
    if intent.force_rebuild()
        || physical_rebuild
        || store.active_generation.is_none()
        || store.building_generation.is_some()
    {
        let fresh_generation_created = store.building_generation.is_none();
        if store.building_generation.is_none() {
            begin_projection_generation(
                &session.db_path,
                store_name,
                &session.owner,
                lease_token,
                backend,
            )
            .map_err(begin_generation_failure)?;
        }
        let rebuilding =
            projection_status(&session.db_path).map_err(MaintenanceStoreAttemptError::Fatal)?;
        let store = rebuilding
            .stores
            .iter()
            .find(|store| store.store_name == store_name)
            .ok_or_else(|| {
                MaintenanceStoreAttemptError::Fatal(KanbanError::Storage(format!(
                    "{display_name} projection state is missing"
                )))
            })?;
        if let Some(invariant) = resume_invariant.as_ref() {
            require_explicit_resume_invariant(store, invariant)?;
        }
        if store.building_generation.is_some() {
            let descriptor =
                backend
                    .descriptor()
                    .map_err(|error| MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Backend,
                        error,
                    })?;
            if !building_binding_matches_descriptor(store, &descriptor) {
                if let Some(invariant) = resume_invariant.as_ref() {
                    return Err(MaintenanceStoreAttemptError::Fatal(KanbanError::Conflict(
                        format!(
                            "explicit resume for projection store {store_name} cannot replace unfinished generation {} after its binding changed",
                            invariant.generation
                        ),
                    )));
                }
                abort_incompatible_projection_generation(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                )
                .map_err(|error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                })?;
                begin_projection_generation(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                )
                .map_err(begin_generation_failure)?;
            }
        }
        let rebuilding =
            projection_status(&session.db_path).map_err(MaintenanceStoreAttemptError::Fatal)?;
        let store = rebuilding
            .stores
            .iter()
            .find(|store| store.store_name == store_name)
            .ok_or_else(|| {
                MaintenanceStoreAttemptError::Fatal(KanbanError::Storage(format!(
                    "{display_name} projection state is missing"
                )))
            })?;
        if let Some(invariant) = resume_invariant.as_ref() {
            require_explicit_resume_invariant(store, invariant)?;
        }
        match store.building_phase.as_deref() {
            Some("snapshotting") => prepare_snapshot_with_one_automatic_rebase(
                &session.db_path,
                store_name,
                &session.owner,
                lease_token,
                backend,
                intent,
                fresh_generation_created,
            )?,
            Some("prepared" | "store_published") => {
                store.building_generation.as_deref().ok_or_else(|| {
                    MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Backend,
                        error: KanbanError::Conflict(format!(
                            "{display_name} rebuilding phase has no generation"
                        )),
                    }
                })?;
                if let Err(error) = validate_backend_for_target(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                ) {
                    if let Some(invariant) = resume_invariant.as_ref() {
                        return Err(MaintenanceStoreAttemptError::Fatal(KanbanError::Conflict(
                            format!(
                                "explicit resume for projection store {store_name} cannot replace unfinished generation {} after target validation failed: {error}",
                                invariant.generation
                            ),
                        )));
                    }
                    if target_validation_disposition(&error) == TargetValidationDisposition::Retry {
                        return Err(MaintenanceStoreAttemptError::Store {
                            kind: MaintenanceStoreFailureKind::Backend,
                            error,
                        });
                    }
                    let descriptor = backend.descriptor().map_err(|error| {
                        MaintenanceStoreAttemptError::Store {
                            kind: MaintenanceStoreFailureKind::Backend,
                            error,
                        }
                    })?;
                    let backend_still_matches_building =
                        building_binding_matches_descriptor(store, &descriptor);
                    let abort = if backend_still_matches_building {
                        abort_projection_generation(
                            &session.db_path,
                            store_name,
                            &session.owner,
                            lease_token,
                            backend,
                        )
                    } else {
                        abort_incompatible_projection_generation(
                            &session.db_path,
                            store_name,
                            &session.owner,
                            lease_token,
                            backend,
                        )
                    };
                    abort.map_err(|error| MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Backend,
                        error,
                    })?;
                    begin_projection_generation(
                        &session.db_path,
                        store_name,
                        &session.owner,
                        lease_token,
                        backend,
                    )
                    .map_err(begin_generation_failure)?;
                    prepare_projection_snapshot_with(
                        &session.db_path,
                        store_name,
                        &session.owner,
                        lease_token,
                        backend,
                    )
                    .map_err(|error| MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Backend,
                        error,
                    })?;
                }
            }
            other => {
                return Err(MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error: KanbanError::Conflict(format!(
                        "unsupported {display_name} rebuilding phase {other:?}"
                    )),
                });
            }
        }
        let processed =
            catch_up_generation(session, store_name, display_name, lease_token, backend)?;
        let rebuilding =
            projection_status(&session.db_path).map_err(MaintenanceStoreAttemptError::Fatal)?;
        let store = rebuilding
            .stores
            .iter()
            .find(|store| store.store_name == store_name)
            .ok_or_else(|| {
                MaintenanceStoreAttemptError::Fatal(KanbanError::Storage(format!(
                    "{display_name} projection state is missing"
                )))
            })?;
        if let Some(invariant) = resume_invariant.as_ref() {
            require_explicit_resume_invariant(store, invariant)?;
        }
        let physical_active = backend.inspect_active();
        let building_is_physically_active = store
            .building_generation
            .as_deref()
            .zip(
                physical_active
                    .as_ref()
                    .ok()
                    .and_then(|active| active.as_ref()),
            )
            .is_some_and(|(building, active)| active.manifest.generation == building);
        if physical_rebuild && !incompatible_binding_reset {
            if building_is_physically_active {
                reconcile_projection_generation_with(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                )
                .map_err(|error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                })?;
                action = "generation_reconciled".to_owned();
            } else {
                recover_projection_generation_with(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                )
                .map_err(|error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                })?;
                action = "generation_recovered".to_owned();
            }
        } else {
            physical_active.map_err(|error| MaintenanceStoreAttemptError::Store {
                kind: MaintenanceStoreFailureKind::Backend,
                error,
            })?;
            if store.building_phase.as_deref() == Some("store_published")
                || building_is_physically_active
            {
                reconcile_projection_generation_with(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                )
                .map_err(|error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                })?;
                action = "generation_reconciled".to_owned();
            } else {
                publish_projection_generation_with(
                    &session.db_path,
                    store_name,
                    &session.owner,
                    lease_token,
                    backend,
                )
                .map_err(|error| MaintenanceStoreAttemptError::Store {
                    kind: MaintenanceStoreFailureKind::Backend,
                    error,
                })?;
                action = "generation_published".to_owned();
            }
        }
        return store_run(
            &session.db_path,
            store_name,
            display_name,
            action,
            processed,
        )
        .map_err(MaintenanceStoreAttemptError::Fatal);
    }
    let batch = run_projection_batch_with(
        &session.db_path,
        store_name,
        &session.owner,
        lease_token,
        session.options.claim_ttl_ms,
        session.options.batch_size,
        backend,
    )
    .map_err(|error| MaintenanceStoreAttemptError::Store {
        kind: MaintenanceStoreFailureKind::Delivery,
        error,
    })?;
    let processed = batch.items.len();
    if processed > 0 {
        action = "batch_applied".to_owned();
    }
    store_run(
        &session.db_path,
        store_name,
        display_name,
        action,
        processed,
    )
    .map_err(MaintenanceStoreAttemptError::Fatal)
}

fn building_binding_matches_descriptor(
    store: &ProjectionStoreStatus,
    descriptor: &ProjectionStoreDescriptor,
) -> bool {
    store.building_generation.is_some()
        && projection_binding_matches_descriptor(
            &store.store_name,
            store.building_provider.as_deref(),
            store.building_provider_fingerprint.as_deref(),
            store.building_corpus.as_ref(),
            descriptor,
        )
}

fn projection_binding_matches_descriptor(
    store_name: &str,
    provider: Option<&str>,
    provider_fingerprint: Option<&str>,
    corpus: Option<&ProjectionCorpusMetadata>,
    descriptor: &ProjectionStoreDescriptor,
) -> bool {
    descriptor.store_name == store_name
        && provider == Some(descriptor.provider.as_str())
        && provider_fingerprint == Some(descriptor.provider_fingerprint.as_str())
        && corpus == descriptor.corpus.as_ref()
}

#[cfg(test)]
#[derive(Debug)]
struct ProjectionHeartbeatRenewBarrier {
    entered_tx: mpsc::Sender<()>,
    resume_rx: std::sync::Mutex<mpsc::Receiver<()>>,
}

#[derive(Clone)]
struct ProjectionLeaseHeartbeat {
    db_path: PathBuf,
    owner: String,
    maintenance_lease_token: String,
    maintenance_identity: MaintenanceRuntimeIdentity,
    store_name: String,
    store_lease_token: String,
    ttl_ms: i64,
    #[cfg(test)]
    before_transaction: Option<std::sync::Arc<ProjectionHeartbeatRenewBarrier>>,
}

impl fmt::Debug for ProjectionLeaseHeartbeat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionLeaseHeartbeat")
            .field("db_path", &self.db_path)
            .field("owner", &self.owner)
            .field("maintenance_lease_token", &"[REDACTED]")
            .field("maintenance_identity", &self.maintenance_identity)
            .field("store_name", &self.store_name)
            .field("store_lease_token", &"[REDACTED]")
            .field("ttl_ms", &self.ttl_ms)
            .finish()
    }
}

impl ProjectionLeaseHeartbeat {
    fn new(session: &MaintenanceSession, store_lease: &ProjectionLease) -> Self {
        Self {
            db_path: session.db_path.clone(),
            owner: session.owner.clone(),
            maintenance_lease_token: session.lease_token.clone(),
            maintenance_identity: session.identity.clone(),
            store_name: store_lease.store_name.clone(),
            store_lease_token: store_lease.lease_token.clone(),
            ttl_ms: session.options.lease_ttl_ms,
            #[cfg(test)]
            before_transaction: None,
        }
    }

    #[cfg(test)]
    fn pause_before_transaction_for_test(
        &mut self,
        entered_tx: mpsc::Sender<()>,
        resume_rx: mpsc::Receiver<()>,
    ) {
        self.before_transaction = Some(std::sync::Arc::new(ProjectionHeartbeatRenewBarrier {
            entered_tx,
            resume_rx: std::sync::Mutex::new(resume_rx),
        }));
    }

    #[cfg(test)]
    fn wait_before_transaction_for_test(&self) {
        let Some(barrier) = &self.before_transaction else {
            return;
        };
        barrier
            .entered_tx
            .send(())
            .expect("test waits for heartbeat renewal before transaction");
        barrier
            .resume_rx
            .lock()
            .expect("test heartbeat barrier lock")
            .recv()
            .expect("test resumes heartbeat renewal transaction");
    }

    fn renew(&self) -> Result<()> {
        #[cfg(test)]
        self.wait_before_transaction_for_test();
        renew_maintenance_and_store_lease(
            &self.db_path,
            &self.owner,
            &self.maintenance_lease_token,
            &self.maintenance_identity,
            &self.store_name,
            &self.store_lease_token,
            self.ttl_ms,
        )
    }

    fn run<T>(
        &self,
        operation: impl FnOnce() -> MaintenanceStoreAttempt<T>,
    ) -> Result<MaintenanceStoreAttempt<T>> {
        self.renew()?;
        let interval_ms = (self.ttl_ms / 3).clamp(1, 60_000) as u64;
        thread::scope(|scope| {
            let (stop_tx, stop_rx) = mpsc::channel();
            let heartbeat = scope.spawn(move || {
                loop {
                    match stop_rx.recv_timeout(Duration::from_millis(interval_ms)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                        Err(mpsc::RecvTimeoutError::Timeout) => self.renew()?,
                    }
                }
            });
            let operation_result = operation();
            let _ = stop_tx.send(());
            let heartbeat_result = heartbeat.join().map_err(|_| {
                KanbanError::Storage("projection lease heartbeat thread panicked".to_owned())
            })?;
            match heartbeat_result {
                Err(error) => Err(error),
                // The timer can have last renewed up to one interval before the
                // physical operation returns. Renew synchronously at that
                // completion boundary so a successful operation is never
                // reported while either authority has already expired.
                Ok(()) => {
                    self.renew()?;
                    Ok(operation_result)
                }
            }
        })
    }
}

fn renew_projection_lease_for_heartbeat_on_connection(
    conn: &rusqlite::Connection,
    store_name: &str,
    owner: &str,
    lease_token: &str,
    now: i64,
    expires_at: i64,
) -> Result<()> {
    // A recovery operation may advance its own fence while retaining the same
    // owner/token lease. Capture that current fence under this immediate
    // transaction, then use it in the renewal CAS so the two authority
    // renewals share one indivisible snapshot without accepting a successor.
    let fence_epoch: i64 = conn
        .query_row(
            "SELECT fence_epoch FROM projection_store_state
             WHERE store_name=?1 AND lease_owner=?2 AND lease_token=?3
               AND lease_expires_at>?4",
            params![store_name, owner, lease_token, now],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| {
            KanbanError::Conflict(format!(
                "projection lease is not owned by this worker for store {store_name}"
            ))
        })?;
    let changed = conn
        .execute(
            "UPDATE projection_store_state
             SET lease_expires_at=?1,updated_at=?2
             WHERE store_name=?3 AND lease_owner=?4 AND lease_token=?5
               AND fence_epoch=?6 AND lease_expires_at>?2",
            params![expires_at, now, store_name, owner, lease_token, fence_epoch],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Conflict(format!(
            "projection lease is not owned by this worker for store {store_name}"
        )));
    }
    Ok(())
}

fn renew_maintenance_and_store_lease(
    db_path: &Path,
    owner: &str,
    maintenance_lease_token: &str,
    maintenance_identity: &MaintenanceRuntimeIdentity,
    store_name: &str,
    store_lease_token: &str,
    ttl_ms: i64,
) -> Result<()> {
    let conn = connect_file(db_path)?;
    with_immediate_tx(&conn, || {
        let now = SystemClock.now_ms();
        let expires_at = checked_expiry(now, ttl_ms)?;
        renew_maintenance_owner_lease_on_connection(
            &conn,
            owner,
            maintenance_lease_token,
            maintenance_identity,
            now,
            expires_at,
        )?;
        renew_projection_lease_for_heartbeat_on_connection(
            &conn,
            store_name,
            owner,
            store_lease_token,
            now,
            expires_at,
        )
    })
}

fn renew_catch_up_authorities(
    session: &MaintenanceSession,
    store_name: &str,
    lease_token: &str,
) -> Result<()> {
    renew_maintenance_and_store_lease(
        &session.db_path,
        &session.owner,
        &session.lease_token,
        &session.identity,
        store_name,
        lease_token,
        session.options.lease_ttl_ms,
    )
}

fn catch_up_generation(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    lease_token: &str,
    backend: &impl ProjectionStoreBackend,
) -> MaintenanceStoreAttempt<usize> {
    let mut processed = 0;
    for _ in 0..MAX_REBUILD_CATCH_UP_BATCHES {
        renew_catch_up_authorities(session, store_name, lease_token)
            .map_err(MaintenanceStoreAttemptError::Fatal)?;
        let batch = run_projection_batch_with(
            &session.db_path,
            store_name,
            &session.owner,
            lease_token,
            session.options.claim_ttl_ms,
            session.options.batch_size,
            backend,
        )
        .map_err(|error| MaintenanceStoreAttemptError::Store {
            kind: MaintenanceStoreFailureKind::Delivery,
            error,
        })?;
        if batch.items.is_empty() {
            return Ok(processed);
        }
        processed += batch.items.len();
    }
    Err(MaintenanceStoreAttemptError::Store {
        kind: MaintenanceStoreFailureKind::Delivery,
        error: KanbanError::Conflict(format!(
            "{display_name} generation catch-up did not converge within the safety bound"
        )),
    })
}

fn failed_store_run(
    session: &MaintenanceSession,
    store_name: &str,
    display_name: &str,
    lease_token: &str,
    kind: MaintenanceStoreFailureKind,
    error: KanbanError,
) -> Result<MaintenanceStoreRun> {
    renew_maintenance_owner(session)?;
    let lease = renew_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        lease_token,
        session.options.lease_ttl_ms,
    )?;
    persist_store_failure(
        &session.db_path,
        store_name,
        display_name,
        &lease,
        kind,
        error,
    )
}

fn failed_store_run_without_store_lease(
    session: &MaintenanceSession,
    store_name: &str,
    display_name: &str,
    kind: MaintenanceStoreFailureKind,
    error: KanbanError,
) -> Result<MaintenanceStoreRun> {
    let message = error.to_string();
    let fallback_reason = store_failure_fallback_reason(store_name, &kind);
    let report = || MaintenanceStoreRun {
        store_name: store_name.to_owned(),
        result: MaintenanceStoreResult::Failed {
            kind: kind.clone(),
            message: message.clone(),
        },
        lifecycle_status: "error".to_owned(),
        fallback_reason: Some(fallback_reason.to_owned()),
    };

    // Constructor/provider failures happen before the normal projection lease
    // is acquired.  Refresh the singleton maintenance authority first; if it
    // is stale, return only the structured report rather than attempting an
    // unfenced store-name write.  Non-conflict database failures remain fatal.
    match renew_maintenance_owner(session) {
        Ok(()) => {}
        Err(KanbanError::Conflict(_)) => return Ok(report()),
        Err(error) => return Err(error),
    }
    let lease = match acquire_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        session.options.lease_ttl_ms,
    ) {
        Ok(lease) => lease,
        Err(KanbanError::Conflict(_)) => return Ok(report()),
        Err(error) => return Err(error),
    };

    // Persist only under the freshly acquired owner/token/fence/expiry CAS,
    // then release through the same service path even when persistence races
    // with a handoff.  A stale/conflicted writer is deliberately downgraded to
    // the structured local failure report and never touches the successor.
    let persisted = persist_store_failure(
        &session.db_path,
        store_name,
        display_name,
        &lease,
        kind.clone(),
        error,
    );
    let released = release_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        &lease.lease_token,
    );
    resolve_unleased_failure_results(persisted, released, report())
}

fn resolve_unleased_failure_results(
    persisted: Result<MaintenanceStoreRun>,
    released: Result<()>,
    report: MaintenanceStoreRun,
) -> Result<MaintenanceStoreRun> {
    match (persisted, released) {
        (Ok(run), Ok(())) => Ok(run),
        (Err(KanbanError::Conflict(_)), Ok(()))
        | (Err(KanbanError::Conflict(_)), Err(KanbanError::Conflict(_)))
        | (Ok(_), Err(KanbanError::Conflict(_))) => Ok(report),
        (Err(KanbanError::Conflict(_)), Err(release_error)) => Err(release_error),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn store_failure_fallback_reason(
    store_name: &str,
    kind: &MaintenanceStoreFailureKind,
) -> &'static str {
    match (kind, store_name) {
        (MaintenanceStoreFailureKind::Provider, _) => "provider_unavailable",
        (
            MaintenanceStoreFailureKind::Backend | MaintenanceStoreFailureKind::Delivery,
            LANCEDB_LABEL_ATOMS_STORE | LANCEDB_CHUNKS_STORE,
        ) => "helper_unavailable",
        (MaintenanceStoreFailureKind::Backend | MaintenanceStoreFailureKind::Delivery, _) => {
            "physical_generation_unavailable"
        }
    }
}

fn persist_store_failure(
    path: &Path,
    store_name: &str,
    display_name: &str,
    lease: &ProjectionLease,
    kind: MaintenanceStoreFailureKind,
    error: KanbanError,
) -> Result<MaintenanceStoreRun> {
    if lease.store_name != store_name {
        return Err(KanbanError::Conflict(format!(
            "{display_name} projection lease authority targets {}",
            lease.store_name
        )));
    }
    let message = error.to_string();
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let changed = conn
        .execute(
            "UPDATE projection_store_state
             SET lifecycle_status='error',last_error=?1,updated_at=?2
             WHERE store_name=?3 AND lease_owner=?4 AND lease_token=?5
               AND fence_epoch=?6 AND lease_expires_at>?2",
            params![
                message,
                now,
                store_name,
                lease.owner,
                lease.lease_token,
                lease.fence_epoch,
            ],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Conflict(format!(
            "{display_name} projection lease is stale while persisting failure"
        )));
    }
    let status = projection_status(path)?;
    let store = status
        .stores
        .into_iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            KanbanError::Storage(format!("{display_name} projection state is missing"))
        })?;
    let fallback_reason = match store.fallback_reason.as_deref() {
        Some("corpus_binding_upgrade_required" | "corpus_binding_invalid") => store.fallback_reason,
        _ => Some(store_failure_fallback_reason(store_name, &kind).to_owned()),
    };
    Ok(MaintenanceStoreRun {
        store_name: store.store_name,
        result: MaintenanceStoreResult::Failed { kind, message },
        lifecycle_status: store.lifecycle_status,
        fallback_reason,
    })
}

fn store_run(
    path: &Path,
    store_name: &str,
    display_name: &str,
    action: String,
    processed: usize,
) -> Result<MaintenanceStoreRun> {
    let status = maintenance_status(path)?;
    let store = status
        .stores
        .into_iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            KanbanError::Storage(format!("{display_name} projection state is missing"))
        })?;
    Ok(MaintenanceStoreRun {
        store_name: store.store_name,
        result: MaintenanceStoreResult::Succeeded { action, processed },
        lifecycle_status: store.lifecycle_status,
        fallback_reason: store.fallback_reason,
    })
}

fn validate_options(owner: &str, options: &MaintenanceRunOptions) -> Result<()> {
    if owner.trim().is_empty() {
        return Err(KanbanError::InvalidInput(
            "maintenance owner cannot be empty".to_owned(),
        ));
    }
    if options.lease_ttl_ms <= 0
        || options.claim_ttl_ms <= 0
        || options.claim_ttl_ms >= options.lease_ttl_ms
        || options.batch_size == 0
        || options.batch_size > 1_000
    {
        return Err(KanbanError::InvalidInput(
            "maintenance lease/claim TTLs and batch size are invalid".to_owned(),
        ));
    }
    Ok(())
}

fn checked_expiry(now: i64, ttl_ms: i64) -> Result<i64> {
    now.checked_add(ttl_ms)
        .ok_or_else(|| KanbanError::InvalidInput("maintenance lease TTL overflow".to_owned()))
}

fn runtime_build_identity() -> Result<String> {
    static BUILD_IDENTITY: OnceLock<std::result::Result<String, String>> = OnceLock::new();
    BUILD_IDENTITY
        .get_or_init(compute_runtime_build_identity)
        .clone()
        .map_err(KanbanError::Storage)
}

fn compute_runtime_build_identity() -> std::result::Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot resolve maintenance runtime executable: {error}"))?;
    let mut file = File::open(&executable)
        .map_err(|error| format!("cannot read maintenance runtime executable: {error}"))?;
    let mut hash = 0xcbf29ce484222325_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot fingerprint maintenance runtime: {error}"))?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let cohort = option_env!("KANBAN_BUILD_ID").unwrap_or("unassigned");
    Ok(format!(
        "kanban-sqlite/{};artifact=fnv64:{hash:016x};cohort={cohort}",
        env!("CARGO_PKG_VERSION")
    ))
}

#[cfg(test)]
mod target_validation_tests {
    use super::*;

    #[test]
    fn generation_begin_failure_is_not_classified_as_provider() {
        let attempt = begin_generation_failure(KanbanError::Storage(
            "local generation metadata write failed".to_owned(),
        ));
        let MaintenanceStoreAttemptError::Store { kind, error } = attempt else {
            panic!("generation begin failure must be a store failure");
        };
        assert_eq!(kind, MaintenanceStoreFailureKind::Backend);
        assert!(
            error
                .to_string()
                .contains("local generation metadata write failed")
        );
    }

    #[test]
    fn transient_target_validation_failure_requires_retry_without_abort() {
        assert_eq!(
            target_validation_disposition(&KanbanError::Storage(
                "transient helper timeout".to_owned()
            )),
            TargetValidationDisposition::Retry
        );
    }

    #[test]
    fn deterministic_target_validation_mismatch_requires_rebuild() {
        assert_eq!(
            target_validation_disposition(&KanbanError::Conflict(
                "physical generation evidence mismatch".to_owned()
            )),
            TargetValidationDisposition::Rebuild
        );
        assert_eq!(
            target_validation_disposition(&KanbanError::InvalidInput(
                "provider descriptor mismatch".to_owned()
            )),
            TargetValidationDisposition::Rebuild
        );
    }

    #[test]
    fn missing_lance_corpus_never_defaults_to_the_current_descriptor() {
        let descriptor = ProjectionStoreDescriptor {
            store_name: LANCEDB_CHUNKS_STORE.to_owned(),
            provider: "fake".to_owned(),
            provider_fingerprint: "fake-v1".to_owned(),
            corpus: Some(ProjectionCorpusMetadata {
                corpus_schema: "task-chunks-v2".to_owned(),
                corpus_fingerprint: "task-chunks-v2:fake-v1".to_owned(),
                embedding_model: "fake-model".to_owned(),
                embedding_dimensions: 3,
            }),
        };

        assert!(!projection_binding_matches_descriptor(
            LANCEDB_CHUNKS_STORE,
            Some("fake"),
            Some("fake-v1"),
            None,
            &descriptor,
        ));
        assert!(projection_binding_matches_descriptor(
            LANCEDB_CHUNKS_STORE,
            Some("fake"),
            Some("fake-v1"),
            descriptor.corpus.as_ref(),
            &descriptor,
        ));
    }
}

#[cfg(test)]
mod unleased_failure_tests {
    use std::cell::Cell;

    use rusqlite::params;
    use tempfile::tempdir;

    use super::*;
    use crate::init::init_database;
    use crate::service::{CreateTask, create_task};

    fn failure_report(message: &str) -> MaintenanceStoreRun {
        MaintenanceStoreRun {
            store_name: TANTIVY_TASKS_STORE.to_owned(),
            result: MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Backend,
                message: message.to_owned(),
            },
            lifecycle_status: "error".to_owned(),
            fallback_reason: Some("physical_generation_unavailable".to_owned()),
        }
    }

    #[test]
    fn unleased_result_resolution_propagates_non_conflict_release_error() {
        let error = resolve_unleased_failure_results(
            Err(KanbanError::Conflict(
                "failure persistence lost its lease".to_owned(),
            )),
            Err(KanbanError::Storage(
                "projection lease release failed".to_owned(),
            )),
            failure_report("report only"),
        )
        .expect_err("release storage failure must not be masked by persistence conflict");

        assert!(matches!(
            error,
            KanbanError::Storage(message) if message == "projection lease release failed"
        ));
    }

    #[test]
    fn unleased_result_resolution_keeps_ordinary_conflicts_report_only() -> anyhow::Result<()> {
        let report = failure_report("report only");
        let persist_conflict = resolve_unleased_failure_results(
            Err(KanbanError::Conflict(
                "failure persistence lost its lease".to_owned(),
            )),
            Ok(()),
            report.clone(),
        )?;
        assert_eq!(persist_conflict, report);

        let both_conflict = resolve_unleased_failure_results(
            Err(KanbanError::Conflict(
                "failure persistence lost its lease".to_owned(),
            )),
            Err(KanbanError::Conflict(
                "projection lease was already handed off".to_owned(),
            )),
            report.clone(),
        )?;
        assert_eq!(both_conflict, report);

        let release_conflict = resolve_unleased_failure_results(
            Ok(failure_report("persisted result")),
            Err(KanbanError::Conflict(
                "projection lease was already handed off".to_owned(),
            )),
            report.clone(),
        )?;
        assert_eq!(release_conflict, report);
        Ok(())
    }

    #[test]
    fn backend_open_failure_records_fenced_diagnostic_and_later_store_runs() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let task = create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("failure fencing canonical task"),
        )?;
        let canonical_before = {
            let conn = connect_file(&db_path)?;
            let task_state = conn.query_row(
                "SELECT status,title,description,metadata_json FROM tasks WHERE id=?1",
                [&task.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )?;
            let mut statement = conn.prepare(
                "SELECT id,kind,payload_json FROM task_events WHERE task_id=?1 ORDER BY id",
            )?;
            let events = statement
                .query_map([&task.id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            (task_state, events)
        };
        let session = MaintenanceSession::start(
            &db_path,
            "maintenance-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let before = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");

        let later_store_ran = Cell::new(false);
        let stores = [
            failed_store_run_without_store_lease(
                &session,
                TANTIVY_TASKS_STORE,
                "Tantivy",
                MaintenanceStoreFailureKind::Backend,
                KanbanError::Storage("backend could not be opened".to_owned()),
            )?,
            {
                later_store_ran.set(true);
                MaintenanceStoreRun {
                    store_name: LANCEDB_LABEL_ATOMS_STORE.to_owned(),
                    result: MaintenanceStoreResult::Succeeded {
                        action: "later_store_ran".to_owned(),
                        processed: 0,
                    },
                    lifecycle_status: "ready".to_owned(),
                    fallback_reason: None,
                }
            },
        ];
        assert!(later_store_ran.get());
        assert_eq!(stores.len(), 2);
        assert_eq!(stores[0].store_name, TANTIVY_TASKS_STORE);
        assert_eq!(stores[0].lifecycle_status, "error");
        assert_eq!(
            stores[0].fallback_reason.as_deref(),
            Some("physical_generation_unavailable")
        );
        assert!(matches!(
            &stores[0].result,
            MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Backend,
                message,
            } if message.contains("backend could not be opened")
        ));
        assert_eq!(stores[1].store_name, LANCEDB_LABEL_ATOMS_STORE);

        let after = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_ne!(after.lifecycle_status, before.lifecycle_status);
        assert_eq!(after.lifecycle_status, "error");
        assert!(
            after
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("backend could not be opened"))
        );
        assert_eq!(
            after.fallback_reason.as_deref(),
            Some("derived_store_error")
        );
        assert_eq!(after.owner, None);
        assert_eq!(after.lease_expires_at, None);
        let canonical_after = {
            let conn = connect_file(&db_path)?;
            let task_state = conn.query_row(
                "SELECT status,title,description,metadata_json FROM tasks WHERE id=?1",
                [&task.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )?;
            let mut statement = conn.prepare(
                "SELECT id,kind,payload_json FROM task_events WHERE task_id=?1 ORDER BY id",
            )?;
            let events = statement
                .query_map([&task.id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            (task_state, events)
        };
        assert_eq!(canonical_after, canonical_before);
        session.finish()?;
        Ok(())
    }

    #[test]
    fn unleased_failure_does_not_mutate_successor_projection_state() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let session = MaintenanceSession::start(
            &db_path,
            "maintenance-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let successor =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "successor-owner", 10_000)?;
        let before = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");

        let report = failed_store_run_without_store_lease(
            &session,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            MaintenanceStoreFailureKind::Backend,
            KanbanError::Storage("backend could not be opened".to_owned()),
        )?;
        assert!(matches!(
            report.result,
            MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Backend,
                ..
            }
        ));

        let after = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(after.lifecycle_status, before.lifecycle_status);
        assert_eq!(after.last_error, before.last_error);
        assert_eq!(after.fallback_reason, before.fallback_reason);
        assert_eq!(after.owner.as_deref(), Some("successor-owner"));
        assert_eq!(after.fence_epoch, successor.fence_epoch);
        assert_eq!(after.lease_expires_at, before.lease_expires_at);

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "successor-owner",
            &successor.lease_token,
        )?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn unleased_failure_retry_after_release_is_idempotently_fenced() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let session = MaintenanceSession::start(
            &db_path,
            "maintenance-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;

        let first = failed_store_run_without_store_lease(
            &session,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            MaintenanceStoreFailureKind::Backend,
            KanbanError::Storage("first open failure".to_owned()),
        )?;
        let retry = failed_store_run_without_store_lease(
            &session,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            MaintenanceStoreFailureKind::Provider,
            KanbanError::Storage("retry provider failure".to_owned()),
        )?;
        assert!(matches!(
            first.result,
            MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Backend,
                ..
            }
        ));
        assert!(matches!(
            retry.result,
            MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Provider,
                ..
            }
        ));

        let after = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(after.lifecycle_status, "error");
        assert!(
            after
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("retry provider failure"))
        );
        assert_eq!(after.owner, None);
        assert_eq!(after.lease_expires_at, None);
        session.finish()?;
        Ok(())
    }

    #[test]
    fn leased_failure_renews_after_recovery_fence_bump_before_persistence() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let session = MaintenanceSession::start(
            &db_path,
            "maintenance-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "maintenance-owner", 10_000)?;
        let old_fence = lease.fence_epoch;
        connect_file(&db_path)?.execute(
            "UPDATE projection_store_state
             SET fence_epoch=fence_epoch+1
             WHERE store_name=?1 AND lease_owner=?2 AND lease_token=?3",
            params![TANTIVY_TASKS_STORE, "maintenance-owner", lease.lease_token],
        )?;

        let stale = persist_store_failure(
            &db_path,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &lease,
            MaintenanceStoreFailureKind::Backend,
            KanbanError::Storage("stale pre-recovery fence".to_owned()),
        )
        .expect_err("pre-bump failure writer must fail closed");
        assert!(matches!(stale, KanbanError::Conflict(_)));

        let report = failed_store_run(
            &session,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &lease.lease_token,
            MaintenanceStoreFailureKind::Backend,
            KanbanError::Storage("post-recovery failure".to_owned()),
        )?;
        assert!(matches!(
            report.result,
            MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Backend,
                ..
            }
        ));
        let after = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(after.fence_epoch, old_fence + 1);
        assert_eq!(after.owner.as_deref(), Some("maintenance-owner"));
        assert!(
            after
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("post-recovery failure"))
        );

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "maintenance-owner",
            &lease.lease_token,
        )?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn unleased_failure_with_stale_maintenance_owner_is_report_only() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let session = MaintenanceSession::start(
            &db_path,
            "maintenance-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        connect_file(&db_path)?.execute(
            "UPDATE projection_maintenance_owner SET lease_expires_at=0 WHERE singleton=1",
            [],
        )?;
        let before = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");

        let report = failed_store_run_without_store_lease(
            &session,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            MaintenanceStoreFailureKind::Backend,
            KanbanError::Storage("stale maintenance owner".to_owned()),
        )?;
        assert!(matches!(
            report.result,
            MaintenanceStoreResult::Failed {
                kind: MaintenanceStoreFailureKind::Backend,
                ..
            }
        ));

        let after = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(after.lifecycle_status, before.lifecycle_status);
        assert_eq!(after.last_error, before.last_error);
        assert_eq!(after.fallback_reason, before.fallback_reason);
        assert_eq!(after.owner, before.owner);
        assert_eq!(after.fence_epoch, before.fence_epoch);
        drop(session);
        Ok(())
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
enum TestAuthorityProviderPolicy<'a> {
    Current(&'a ProjectionStoreDescriptor),
    Recovery(&'a ProjectionStoreDescriptor),
}

#[cfg(test)]
struct TestExactAuthorityGuard {
    _helper_guard: DerivedStoreWriteGuard,
    role: ProjectionGenerationRole,
    current_provider_binding: bool,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct TestSqliteGenerationBinding {
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

#[cfg(test)]
impl TestSqliteGenerationBinding {
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

    fn exact_binding(
        &self,
        store_name: &str,
        snapshot_cursor: Option<i64>,
    ) -> Result<ProjectionGenerationBinding> {
        let conflict = |field: &str| {
            test_authority_conflict(store_name, format!("live {field} binding is absent"))
        };
        Ok(ProjectionGenerationBinding {
            generation: self
                .generation
                .clone()
                .ok_or_else(|| conflict("generation"))?,
            fingerprint: self.fingerprint.clone(),
            fence_epoch: self.fence_epoch.ok_or_else(|| conflict("fence"))?,
            snapshot_cursor,
            provider: self.provider.clone().ok_or_else(|| conflict("provider"))?,
            provider_fingerprint: self
                .provider_fingerprint
                .clone()
                .ok_or_else(|| conflict("provider fingerprint"))?,
            canonical_count: self
                .canonical_count
                .ok_or_else(|| conflict("canonical count"))?,
            canonical_digest: self
                .canonical_digest
                .clone()
                .ok_or_else(|| conflict("canonical digest"))?,
            delivery_count: self
                .delivery_count
                .ok_or_else(|| conflict("delivery count"))?,
            delivery_digest: self
                .delivery_digest
                .clone()
                .ok_or_else(|| conflict("delivery digest"))?,
            corpus: super::projection_v2::projection_corpus_from_values(
                self.corpus_schema.clone(),
                self.corpus_fingerprint.clone(),
                self.embedding_model.clone(),
                self.embedding_dimensions,
                store_name,
                "test destructive authority",
            )
            .map_err(|_| test_authority_conflict(store_name, "live corpus binding is invalid"))?,
        })
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct TestSqliteAuthorityState {
    canonical_database_instance_id: String,
    canonical_protocol_version: i64,
    store_database_instance_id: String,
    store_protocol_version: i64,
    schema_version: i64,
    control_plane: String,
    fence_epoch: i64,
    lease_owner: Option<String>,
    lease_token: Option<String>,
    lease_expires_at: Option<i64>,
    active: TestSqliteGenerationBinding,
    previous: TestSqliteGenerationBinding,
    building: TestSqliteGenerationBinding,
    snapshot_cursor: i64,
    building_phase: Option<String>,
}

#[cfg(test)]
fn test_authority_conflict(store_name: &str, message: impl Into<String>) -> KanbanError {
    KanbanError::Conflict(format!(
        "test projection store {store_name} destructive authority is stale or inconsistent: {}",
        message.into()
    ))
}

#[cfg(test)]
fn acquire_test_exact_authority_guard(
    path: &Path,
    helper_lock_name: &str,
    store_name: &str,
    generation: &str,
    authority: &ProjectionDestructiveAuthority,
    provider_policy: TestAuthorityProviderPolicy<'_>,
) -> Result<TestExactAuthorityGuard> {
    let helper_guard = crate::db::acquire_derived_store_write_guard(path, helper_lock_name)?;
    let now = SystemClock.now_ms();
    if generation.trim().is_empty()
        || authority.generation != generation
        || authority.owner.trim().is_empty()
        || authority.lease_token.trim().is_empty()
        || authority.fence_epoch < 0
        || authority.lease_expires_at <= now
    {
        return Err(test_authority_conflict(
            store_name,
            "capability is incomplete or expired",
        ));
    }

    let conn = connect_file(path)?;
    let state = conn
        .query_row(
            "SELECT database.database_instance_id,database.protocol_version,
                    store.database_instance_id,store.protocol_version,store.schema_version,
                    store.control_plane,store.fence_epoch,store.lease_owner,store.lease_token,
                    store.lease_expires_at,
                    store.active_generation,store.active_fingerprint,store.active_fence_epoch,
                    store.active_snapshot_cursor,store.active_provider,
                    store.active_provider_fingerprint,store.active_canonical_count,
                    store.active_canonical_digest,store.active_delivery_count,
                    store.active_delivery_digest,store.active_corpus_schema,
                    store.active_corpus_fingerprint,store.active_embedding_model,
                    store.active_embedding_dimensions,
                    store.previous_generation,store.previous_fingerprint,
                    store.previous_fence_epoch,store.previous_snapshot_cursor,
                    store.previous_provider,store.previous_provider_fingerprint,
                    store.previous_canonical_count,store.previous_canonical_digest,
                    store.previous_delivery_count,store.previous_delivery_digest,
                    store.previous_corpus_schema,store.previous_corpus_fingerprint,
                    store.previous_embedding_model,store.previous_embedding_dimensions,
                    store.building_generation,store.building_fingerprint,
                    store.building_fence_epoch,store.snapshot_cursor,store.building_provider,
                    store.building_provider_fingerprint,store.building_canonical_count,
                    store.building_canonical_digest,store.building_delivery_count,
                    store.building_delivery_digest,store.building_corpus_schema,
                    store.building_corpus_fingerprint,store.building_embedding_model,
                    store.building_embedding_dimensions,store.snapshot_cursor,
                    store.building_phase
             FROM projection_database AS database
             JOIN projection_store_state AS store ON store.store_name=?1
             WHERE database.singleton=1",
            [store_name],
            |row| {
                Ok(TestSqliteAuthorityState {
                    canonical_database_instance_id: row.get(0)?,
                    canonical_protocol_version: row.get(1)?,
                    store_database_instance_id: row.get(2)?,
                    store_protocol_version: row.get(3)?,
                    schema_version: row.get(4)?,
                    control_plane: row.get(5)?,
                    fence_epoch: row.get(6)?,
                    lease_owner: row.get(7)?,
                    lease_token: row.get(8)?,
                    lease_expires_at: row.get(9)?,
                    active: TestSqliteGenerationBinding::from_row(row, 10)?,
                    previous: TestSqliteGenerationBinding::from_row(row, 24)?,
                    building: TestSqliteGenerationBinding::from_row(row, 38)?,
                    snapshot_cursor: row.get(52)?,
                    building_phase: row.get(53)?,
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| test_authority_conflict(store_name, "SQLite authority row is absent"))?;

    if state.canonical_database_instance_id != state.store_database_instance_id
        || state.canonical_protocol_version != super::projection_v2::PROJECTION_PROTOCOL_VERSION
        || state.store_protocol_version != super::projection_v2::PROJECTION_PROTOCOL_VERSION
        || state.schema_version != DERIVED_STORE_SCHEMA_VERSION
        || state.control_plane != "v2"
        || state.fence_epoch != authority.fence_epoch
        || state.lease_owner.as_deref() != Some(authority.owner.as_str())
        || state.lease_token.as_deref() != Some(authority.lease_token.as_str())
        || state
            .lease_expires_at
            .is_none_or(|lease_expires_at| lease_expires_at <= now)
    {
        return Err(test_authority_conflict(
            store_name,
            "database, protocol, schema, control-plane, owner, token, lease, or fence changed",
        ));
    }

    let mut live_role = None;
    for (role, candidate) in [
        (
            ProjectionGenerationRole::Active,
            state.active.generation.as_deref(),
        ),
        (
            ProjectionGenerationRole::Previous,
            state.previous.generation.as_deref(),
        ),
        (
            ProjectionGenerationRole::Building,
            state.building.generation.as_deref(),
        ),
    ] {
        if candidate == Some(generation) {
            if live_role.is_some() {
                return Err(test_authority_conflict(
                    store_name,
                    "generation is bound to more than one live role",
                ));
            }
            live_role = Some(role);
        }
    }
    let live_role = live_role.ok_or_else(|| {
        test_authority_conflict(
            store_name,
            "generation is not bound to an active, previous, or building role",
        )
    })?;
    if authority.role == ProjectionGenerationRole::Orphaned || authority.role != live_role {
        return Err(test_authority_conflict(
            store_name,
            "generation role does not match SQLite",
        ));
    }

    let (sqlite_binding, live_phase) = match live_role {
        ProjectionGenerationRole::Active => (
            state
                .active
                .exact_binding(store_name, state.active.snapshot_cursor)?,
            None,
        ),
        ProjectionGenerationRole::Previous => (
            state
                .previous
                .exact_binding(store_name, state.previous.snapshot_cursor)?,
            None,
        ),
        ProjectionGenerationRole::Building => {
            let phase = state.building_phase.clone();
            if !matches!(
                phase.as_deref(),
                Some("snapshotting" | "prepared" | "store_published")
            ) {
                return Err(test_authority_conflict(
                    store_name,
                    "building phase is invalid",
                ));
            }
            let binding_cursor = if phase.as_deref() == Some("snapshotting") {
                None
            } else {
                Some(state.snapshot_cursor)
            };
            (
                state.building.exact_binding(store_name, binding_cursor)?,
                phase,
            )
        }
        ProjectionGenerationRole::Orphaned => unreachable!("orphaned role is rejected above"),
    };
    if sqlite_binding.generation != generation
        || sqlite_binding != authority.expected_binding
        || authority.building_phase != live_phase
        || sqlite_binding.fence_epoch < 0
        || sqlite_binding.fence_epoch > state.fence_epoch
    {
        return Err(test_authority_conflict(
            store_name,
            "generation binding, role phase, or binding fence does not match SQLite",
        ));
    }

    let sqlite_manifest = sqlite_binding
        .fingerprint
        .as_ref()
        .map(|_| ProjectionArtifactManifest {
            store_name: store_name.to_owned(),
            database_instance_id: state.store_database_instance_id.clone(),
            protocol_version: state.store_protocol_version,
            schema_version: state.schema_version,
            generation: sqlite_binding.generation.clone(),
            fence_epoch: sqlite_binding.fence_epoch,
            snapshot_cursor: sqlite_binding
                .snapshot_cursor
                .unwrap_or(state.snapshot_cursor),
            provider: sqlite_binding.provider.clone(),
            provider_fingerprint: sqlite_binding.provider_fingerprint.clone(),
            corpus: sqlite_binding.corpus.clone(),
            canonical_item_count: sqlite_binding.canonical_count,
            canonical_digest: sqlite_binding.canonical_digest.clone(),
            delivery_item_count: sqlite_binding.delivery_count,
            delivery_digest: sqlite_binding.delivery_digest.clone(),
            fingerprint: sqlite_binding.fingerprint.clone(),
        });
    if authority.expected_manifest != sqlite_manifest {
        return Err(test_authority_conflict(
            store_name,
            "manifest does not match the exact SQLite binding",
        ));
    }
    let descriptor = match provider_policy {
        TestAuthorityProviderPolicy::Current(descriptor)
        | TestAuthorityProviderPolicy::Recovery(descriptor) => descriptor,
    };
    let current_provider_binding = descriptor.store_name == store_name
        && sqlite_binding.provider == descriptor.provider
        && sqlite_binding.provider_fingerprint == descriptor.provider_fingerprint
        && sqlite_binding.corpus == descriptor.corpus;
    if matches!(provider_policy, TestAuthorityProviderPolicy::Current(_))
        && !current_provider_binding
    {
        return Err(test_authority_conflict(
            store_name,
            "provider or corpus binding is not current",
        ));
    }

    Ok(TestExactAuthorityGuard {
        _helper_guard: helper_guard,
        role: live_role,
        current_provider_binding,
    })
}

#[cfg(test)]
mod legacy_binding_recovery_tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
        sync::{Arc, Mutex, mpsc},
        thread,
        time::Duration,
    };

    use kanban_local::DerivedStoreWriteGuard;
    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::{
        db::connect_file,
        init::init_database,
        service::{
            CreateTask, ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
            ProjectionBatchReceipt, ProjectionDestructiveAuthority, ProjectionGenerationBinding,
            ProjectionGenerationRole, ProjectionPublishReceipt, ProjectionSnapshot, create_task,
        },
    };

    const STORE: &str = LANCEDB_CHUNKS_STORE;
    const ACTIVE: &str = "gen_v29_lance_active";
    const PREVIOUS: &str = "gen_v29_lance_previous";
    const BUILDING: &str = "gen_v29_lance_building";

    #[derive(Default)]
    struct RecoveryBackendState {
        generations: BTreeMap<String, ProjectionArtifactEvidence>,
        active: Option<ProjectionArtifactEvidence>,
        prepared: Option<ProjectionArtifactEvidence>,
        published: BTreeSet<String>,
        quarantined: BTreeMap<String, ProjectionArtifactEvidence>,
        quarantine_attempts: Vec<String>,
        after_prepare: Option<Box<dyn FnOnce() + Send>>,
        before_active_inspect: Option<Box<dyn FnOnce() + Send>>,
        after_active_inspect: Option<Box<dyn FnOnce() + Send>>,
        promote_after_active_quarantine: Option<String>,
        fail_next_quarantine: Option<String>,
        fail_next_active_inspect: Option<String>,
    }

    struct RecoveryBackend {
        descriptor: ProjectionStoreDescriptor,
        state: Mutex<RecoveryBackendState>,
        helper_path: Option<PathBuf>,
    }

    impl RecoveryBackend {
        fn empty() -> Self {
            Self {
                descriptor: current_descriptor(),
                state: Mutex::new(RecoveryBackendState::default()),
                helper_path: None,
            }
        }

        fn empty_with_helper_path(path: &Path) -> Self {
            Self {
                descriptor: current_descriptor(),
                state: Mutex::new(RecoveryBackendState::default()),
                helper_path: Some(path.to_owned()),
            }
        }

        fn from_legacy_sqlite(
            path: &Path,
            active: bool,
            previous: bool,
            building: bool,
        ) -> anyhow::Result<Self> {
            let backend = Self::empty_with_helper_path(path);
            let mut state = backend.state.lock().expect("recovery backend lock");
            if active {
                let evidence = legacy_evidence(path, ACTIVE, 7)?;
                state
                    .generations
                    .insert(ACTIVE.to_owned(), evidence.clone());
                state.active = Some(evidence);
                state.published.insert(ACTIVE.to_owned());
            }
            if previous {
                state
                    .generations
                    .insert(PREVIOUS.to_owned(), legacy_evidence(path, PREVIOUS, 6)?);
                state.published.insert(PREVIOUS.to_owned());
            }
            if building {
                let evidence = legacy_evidence(path, BUILDING, 8)?;
                state
                    .generations
                    .insert(BUILDING.to_owned(), evidence.clone());
                state.prepared = Some(evidence);
            }
            drop(state);
            Ok(backend)
        }

        fn acquire_helper_guard(&self) -> Result<Option<DerivedStoreWriteGuard>> {
            self.helper_path
                .as_deref()
                .map(|path| {
                    crate::db::acquire_derived_store_write_guard(
                        path,
                        &format!("{STORE}-projection-helper"),
                    )
                })
                .transpose()
        }

        fn acquire_exact_authority_guard(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
            provider_policy: TestAuthorityProviderPolicy<'_>,
        ) -> Result<TestExactAuthorityGuard> {
            let path = self.helper_path.as_deref().ok_or_else(|| {
                KanbanError::Conflict(
                    "recovery fake authority has no SQLite/helper path".to_owned(),
                )
            })?;
            acquire_test_exact_authority_guard(
                path,
                &format!("{STORE}-projection-helper"),
                STORE,
                generation,
                authority,
                provider_policy,
            )
        }

        fn install_unknown_active(&self, path: &Path, generation: &str) -> anyhow::Result<()> {
            let evidence = evidence_for_descriptor(path, generation, 99, &self.descriptor)?;
            let mut state = self.state.lock().expect("recovery backend lock");
            state
                .generations
                .insert(generation.to_owned(), evidence.clone());
            state.active = Some(evidence);
            state.published.insert(generation.to_owned());
            Ok(())
        }

        fn bind_active_to_current_descriptor(&self, path: &Path) -> anyhow::Result<()> {
            let evidence = evidence_for_descriptor(path, ACTIVE, 7, &self.descriptor)?;
            let mut state = self.state.lock().expect("recovery backend lock");
            state
                .generations
                .insert(ACTIVE.to_owned(), evidence.clone());
            state.active = Some(evidence);
            Ok(())
        }

        fn bind_previous_to_current_descriptor(&self, path: &Path) -> anyhow::Result<()> {
            let evidence = evidence_for_descriptor(path, PREVIOUS, 6, &self.descriptor)?;
            self.state
                .lock()
                .expect("recovery backend lock")
                .generations
                .insert(PREVIOUS.to_owned(), evidence);
            Ok(())
        }

        fn prequarantine(&self, generation: &str) {
            self.quarantine_generation(generation)
                .expect("prequarantine legacy generation");
        }

        fn promote_after_active_quarantine(&self, generation: &str) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .promote_after_active_quarantine = Some(generation.to_owned());
        }

        fn set_before_active_inspect(&self, hook: impl FnOnce() + Send + 'static) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .before_active_inspect = Some(Box::new(hook));
        }

        fn set_after_active_inspect(&self, hook: impl FnOnce() + Send + 'static) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .after_active_inspect = Some(Box::new(hook));
        }

        fn set_after_prepare(&self, hook: impl FnOnce() + Send + 'static) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .after_prepare = Some(Box::new(hook));
        }

        fn fail_next_quarantine(&self, message: impl Into<String>) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .fail_next_quarantine = Some(message.into());
        }

        fn fail_next_active_inspect(&self, message: impl Into<String>) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .fail_next_active_inspect = Some(message.into());
        }

        fn quarantined_ids(&self) -> BTreeSet<String> {
            self.state
                .lock()
                .expect("recovery backend lock")
                .quarantined
                .keys()
                .cloned()
                .collect()
        }

        fn quarantine_attempts(&self) -> Vec<String> {
            self.state
                .lock()
                .expect("recovery backend lock")
                .quarantine_attempts
                .clone()
        }

        fn published_ids(&self) -> BTreeSet<String> {
            self.state
                .lock()
                .expect("recovery backend lock")
                .published
                .clone()
        }

        fn mark_published(&self, generation: &str) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .published
                .insert(generation.to_owned());
        }

        fn corrupt_generation_fingerprint(&self, generation: &str) {
            let mut state = self.state.lock().expect("recovery backend lock");
            let evidence = state
                .generations
                .get_mut(generation)
                .expect("generation evidence to corrupt");
            evidence.fingerprint = "fake:corrupt-resume-target".to_owned();
            evidence.manifest.fingerprint = Some(evidence.fingerprint.clone());
            let corrupted = evidence.clone();
            if state
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.manifest.generation == generation)
            {
                state.prepared = Some(corrupted);
            }
        }

        fn prepare_snapshot_while_helper_locked(
            &self,
            snapshot: &ProjectionSnapshot,
        ) -> ProjectionArtifactEvidence {
            let fingerprint = format!("fake:{}", snapshot.manifest.generation);
            let mut manifest = snapshot.manifest.clone();
            manifest.fingerprint = Some(fingerprint.clone());
            let evidence = ProjectionArtifactEvidence {
                manifest,
                fingerprint,
            };
            let hook = {
                let mut state = self.state.lock().expect("recovery backend lock");
                state
                    .generations
                    .insert(evidence.manifest.generation.clone(), evidence.clone());
                state.prepared = Some(evidence.clone());
                state.after_prepare.take()
            };
            if let Some(hook) = hook {
                hook();
            }
            evidence
        }

        fn apply_batch_while_helper_locked(
            &self,
            batch: &ProjectionBatch,
        ) -> ProjectionBatchReceipt {
            ProjectionBatchReceipt {
                store_name: batch.store_name.clone(),
                database_instance_id: batch.database_instance_id.clone(),
                protocol_version: batch.protocol_version,
                schema_version: batch.schema_version,
                provider: batch.provider.clone(),
                provider_fingerprint: batch.provider_fingerprint.clone(),
                target_generation: batch.target_generation.clone(),
                lease_token: batch.lease_token.clone(),
                fence_epoch: batch.fence_epoch,
                claim_token: batch.claim_token.clone(),
                applied_item_count: batch.items.len(),
            }
        }

        fn publish_generation_while_helper_locked(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            let mut state = self.state.lock().expect("recovery backend lock");
            if state.active.as_ref() != expected_active {
                return Err(KanbanError::Conflict(
                    "recovery fake active CAS mismatch".to_owned(),
                ));
            }
            if state.prepared.as_ref() != Some(prepared) {
                return Err(KanbanError::Conflict(
                    "recovery fake prepared evidence mismatch".to_owned(),
                ));
            }
            let retained_previous = state.active.clone();
            state.active = Some(prepared.clone());
            state
                .generations
                .insert(prepared.manifest.generation.clone(), prepared.clone());
            state.published.insert(prepared.manifest.generation.clone());
            Ok(ProjectionPublishReceipt {
                active: prepared.clone(),
                retained_previous,
            })
        }

        fn quarantine_generation_while_helper_locked(&self, generation: &str) {
            let mut state = self.state.lock().expect("recovery backend lock");
            state.quarantine_attempts.push(generation.to_owned());
            let evidence = state.generations.remove(generation).or_else(|| {
                state
                    .prepared
                    .as_ref()
                    .filter(|prepared| prepared.manifest.generation == generation)
                    .cloned()
            });
            let removed_active = state
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.generation == generation);
            if removed_active {
                state.active = None;
            }
            if state
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.manifest.generation == generation)
            {
                state.prepared = None;
            }
            state.published.remove(generation);
            if let Some(evidence) = evidence {
                state.quarantined.insert(generation.to_owned(), evidence);
            }
            if removed_active && let Some(promoted) = state.promote_after_active_quarantine.clone()
            {
                state.active = state.generations.get(&promoted).cloned();
            }
        }

        fn abort_generation_while_helper_locked(&self, generation: &str) {
            let mut state = self.state.lock().expect("recovery backend lock");
            state.generations.remove(generation);
            if state
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.manifest.generation == generation)
            {
                state.prepared = None;
            }
        }
    }

    impl ProjectionStoreBackend for RecoveryBackend {
        fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
            Ok(self.descriptor.clone())
        }

        fn prepare_snapshot(
            &self,
            snapshot: &ProjectionSnapshot,
        ) -> Result<ProjectionArtifactEvidence> {
            let _helper_guard = self.acquire_helper_guard()?;
            Ok(self.prepare_snapshot_while_helper_locked(snapshot))
        }

        fn prepare_snapshot_with_authority(
            &self,
            snapshot: &ProjectionSnapshot,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionArtifactEvidence> {
            let _authority_guard = self.acquire_exact_authority_guard(
                &snapshot.manifest.generation,
                authority,
                TestAuthorityProviderPolicy::Current(&self.descriptor),
            )?;
            Ok(self.prepare_snapshot_while_helper_locked(snapshot))
        }

        fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
            let _helper_guard = self.acquire_helper_guard()?;
            Ok(self.apply_batch_while_helper_locked(batch))
        }

        fn apply_batch_with_authority(
            &self,
            batch: &ProjectionBatch,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionBatchReceipt> {
            let _authority_guard = self.acquire_exact_authority_guard(
                &batch.target_generation,
                authority,
                TestAuthorityProviderPolicy::Current(&self.descriptor),
            )?;
            Ok(self.apply_batch_while_helper_locked(batch))
        }

        fn publish_generation(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            let _helper_guard = self.acquire_helper_guard()?;
            self.publish_generation_while_helper_locked(expected_active, prepared)
        }

        fn publish_generation_with_authority(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionPublishReceipt> {
            let _authority_guard = self.acquire_exact_authority_guard(
                &prepared.manifest.generation,
                authority,
                TestAuthorityProviderPolicy::Current(&self.descriptor),
            )?;
            self.publish_generation_while_helper_locked(expected_active, prepared)
        }

        fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
            let helper_guard = self.acquire_helper_guard()?;
            let (active, before_hook, after_hook, failure) = {
                let mut state = self.state.lock().expect("recovery backend lock");
                (
                    state.active.clone(),
                    state.before_active_inspect.take(),
                    state.after_active_inspect.take(),
                    state.fail_next_active_inspect.take(),
                )
            };
            if let Some(hook) = before_hook {
                hook();
            }
            drop(helper_guard);
            if let Some(hook) = after_hook {
                hook();
            }
            if let Some(message) = failure {
                return Err(KanbanError::Storage(message));
            }
            Ok(active)
        }

        fn inspect_generation(
            &self,
            generation: &str,
        ) -> Result<Option<ProjectionArtifactEvidence>> {
            let _helper_guard = self.acquire_helper_guard()?;
            Ok(self
                .state
                .lock()
                .expect("recovery backend lock")
                .generations
                .get(generation)
                .cloned())
        }

        fn validate_generation_publication(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            match self.inspect_generation(&expected.manifest.generation)? {
                Some(actual) if actual == *expected => Ok(()),
                _ => Err(KanbanError::Storage(
                    "recovery fake generation is not published".to_owned(),
                )),
            }
        }

        fn quarantine_generation(&self, generation: &str) -> Result<()> {
            let _helper_guard = self.acquire_helper_guard()?;
            self.quarantine_generation_while_helper_locked(generation);
            Ok(())
        }

        fn abort_generation(&self, generation: &str) -> Result<()> {
            let _helper_guard = self.acquire_helper_guard()?;
            let state = self.state.lock().expect("recovery backend lock");
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.generation == generation)
                || state.published.contains(generation)
            {
                return Err(KanbanError::Conflict(format!(
                    "recovery fake cannot abort published generation {generation}"
                )));
            }
            drop(state);
            self.abort_generation_while_helper_locked(generation);
            Ok(())
        }

        fn quarantine_generation_fenced(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            let authority_guard = self.acquire_exact_authority_guard(
                generation,
                authority,
                TestAuthorityProviderPolicy::Recovery(&self.descriptor),
            )?;
            if authority_guard.role == ProjectionGenerationRole::Active
                && authority_guard.current_provider_binding
            {
                let state = self.state.lock().expect("recovery backend lock");
                let exact_canonical_active = state.active.as_ref().is_some_and(|active| {
                    authority.expected_manifest.as_ref() == Some(&active.manifest)
                        && authority.expected_binding.fingerprint.as_deref()
                            == Some(active.fingerprint.as_str())
                });
                if exact_canonical_active {
                    return Err(KanbanError::Conflict(format!(
                        "cannot quarantine canonical active recovery fake generation {generation}"
                    )));
                }
            }
            if let Some(message) = self
                .state
                .lock()
                .expect("recovery backend lock")
                .fail_next_quarantine
                .take()
            {
                return Err(KanbanError::Storage(message));
            }
            self.quarantine_generation_while_helper_locked(generation);
            Ok(())
        }

        fn abort_generation_fenced(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            let authority_guard = self.acquire_exact_authority_guard(
                generation,
                authority,
                TestAuthorityProviderPolicy::Recovery(&self.descriptor),
            )?;
            let published = self
                .state
                .lock()
                .expect("recovery backend lock")
                .published
                .contains(generation);
            if authority_guard.role != ProjectionGenerationRole::Building
                || !matches!(
                    authority.building_phase.as_deref(),
                    Some("snapshotting" | "prepared")
                )
                || published
            {
                return Err(KanbanError::Conflict(format!(
                    "recovery fake can only abort an unpublished building generation: {generation}"
                )));
            }
            self.abort_generation_while_helper_locked(generation);
            Ok(())
        }
    }

    #[test]
    fn recovery_authority_without_a_helper_path_fails_closed() {
        let backend = RecoveryBackend::empty();
        let authority = ProjectionDestructiveAuthority {
            owner: "missing-helper-owner".to_owned(),
            lease_token: "missing-helper-token".to_owned(),
            fence_epoch: 1,
            lease_expires_at: SystemClock.now_ms() + 20_000,
            role: ProjectionGenerationRole::Active,
            generation: ACTIVE.to_owned(),
            expected_manifest: None,
            expected_binding: ProjectionGenerationBinding {
                generation: ACTIVE.to_owned(),
                fingerprint: None,
                fence_epoch: 1,
                snapshot_cursor: Some(0),
                provider: "fake-lance".to_owned(),
                provider_fingerprint: "fake-lance-v2".to_owned(),
                canonical_count: 0,
                canonical_digest: "canonical".to_owned(),
                delivery_count: 0,
                delivery_digest: "delivery".to_owned(),
                corpus: None,
            },
            building_phase: None,
        };

        let error = backend
            .quarantine_generation_fenced(ACTIVE, &authority)
            .expect_err("an authority-bearing fake mutation requires a live helper path");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(backend.quarantine_attempts().is_empty());
    }

    #[test]
    fn recovery_authority_rejects_a_wrong_live_role_without_physical_mutation() -> anyhow::Result<()>
    {
        let (_temp, path) = v29_lance_fixture(true, false, false)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, false, false)?;
        let lease = acquire_projection_lease(&path, STORE, "wrong-role-owner", 20_000)?;
        let mut authority = authority_for_evidence(
            &legacy_evidence(&path, ACTIVE, 7)?,
            "wrong-role-owner",
            &lease.lease_token,
            lease.fence_epoch,
            lease.lease_expires_at,
            ProjectionGenerationRole::Active,
        );
        authority.role = ProjectionGenerationRole::Previous;

        let before = backend.inspect_generation(ACTIVE)?;
        let error = backend
            .quarantine_generation_fenced(ACTIVE, &authority)
            .expect_err("the exact live SQLite role is part of destructive authority");

        assert!(matches!(error, KanbanError::Conflict(_)));
        assert_eq!(backend.inspect_generation(ACTIVE)?, before);
        assert!(backend.quarantine_attempts().is_empty());
        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    enum RecoveryAuthorityDrift {
        EmptyGeneration,
        EmptyOwner,
        EmptyToken,
        NegativeAuthorityFence,
        ExpiredAuthority,
        SameIdentityFenceRollover,
        OwnerTokenHandoff,
        ExpiredLiveLease,
        FutureBindingFence,
        WrongRole,
        WrongPhase,
        WrongBinding,
        WrongManifest,
        DatabaseMismatch,
        ProtocolMismatch,
        SchemaMismatch,
        ControlPlaneMismatch,
    }

    #[derive(Debug, Clone, Copy)]
    enum RecoveryFencedOperation {
        Quarantine,
        Abort,
    }

    #[test]
    fn recovery_exact_authority_negative_matrix_preserves_physical_state() -> anyhow::Result<()> {
        for operation in [
            RecoveryFencedOperation::Quarantine,
            RecoveryFencedOperation::Abort,
        ] {
            let (_temp, path) = v29_lance_fixture(true, false, false)?;
            let backend = RecoveryBackend::from_legacy_sqlite(&path, true, false, false)?;
            let owner = "negative-matrix-owner";
            let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
            let base_authority = authority_for_evidence(
                &legacy_evidence(&path, ACTIVE, 7)?,
                owner,
                &lease.lease_token,
                lease.fence_epoch,
                lease.lease_expires_at,
                ProjectionGenerationRole::Active,
            );
            let database_instance_id = base_authority
                .expected_manifest
                .as_ref()
                .expect("active manifest")
                .database_instance_id
                .clone();
            let physical_before = backend.inspect_generation(ACTIVE)?;
            let published_before = backend.published_ids();
            for drift in [
                RecoveryAuthorityDrift::EmptyGeneration,
                RecoveryAuthorityDrift::EmptyOwner,
                RecoveryAuthorityDrift::EmptyToken,
                RecoveryAuthorityDrift::NegativeAuthorityFence,
                RecoveryAuthorityDrift::ExpiredAuthority,
                RecoveryAuthorityDrift::SameIdentityFenceRollover,
                RecoveryAuthorityDrift::OwnerTokenHandoff,
                RecoveryAuthorityDrift::ExpiredLiveLease,
                RecoveryAuthorityDrift::FutureBindingFence,
                RecoveryAuthorityDrift::WrongRole,
                RecoveryAuthorityDrift::WrongPhase,
                RecoveryAuthorityDrift::WrongBinding,
                RecoveryAuthorityDrift::WrongManifest,
                RecoveryAuthorityDrift::DatabaseMismatch,
                RecoveryAuthorityDrift::ProtocolMismatch,
                RecoveryAuthorityDrift::SchemaMismatch,
                RecoveryAuthorityDrift::ControlPlaneMismatch,
            ] {
                let mut authority = base_authority.clone();

                match drift {
                    RecoveryAuthorityDrift::EmptyGeneration => authority.generation.clear(),
                    RecoveryAuthorityDrift::EmptyOwner => authority.owner.clear(),
                    RecoveryAuthorityDrift::EmptyToken => authority.lease_token.clear(),
                    RecoveryAuthorityDrift::NegativeAuthorityFence => authority.fence_epoch = -1,
                    RecoveryAuthorityDrift::ExpiredAuthority => authority.lease_expires_at = 0,
                    RecoveryAuthorityDrift::SameIdentityFenceRollover => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET fence_epoch=fence_epoch+1
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::OwnerTokenHandoff => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET lease_owner='successor-owner',
                                 lease_token='please_successor_token',
                                 lease_expires_at=?1,fence_epoch=fence_epoch+1
                             WHERE store_name=?2",
                            params![SystemClock.now_ms() + 20_000, STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::ExpiredLiveLease => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET lease_expires_at=0
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::FutureBindingFence => {
                        let future_fence = lease.fence_epoch + 1;
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET active_fence_epoch=?1
                             WHERE store_name=?2",
                            params![future_fence, STORE],
                        )?;
                        authority.expected_binding.fence_epoch = future_fence;
                        authority
                            .expected_manifest
                            .as_mut()
                            .expect("active manifest")
                            .fence_epoch = future_fence;
                    }
                    RecoveryAuthorityDrift::WrongRole => {
                        authority.role = ProjectionGenerationRole::Previous;
                    }
                    RecoveryAuthorityDrift::WrongPhase => {
                        authority.building_phase = Some("prepared".to_owned());
                    }
                    RecoveryAuthorityDrift::WrongBinding => {
                        authority.expected_binding.provider = "wrong-provider".to_owned();
                    }
                    RecoveryAuthorityDrift::WrongManifest => {
                        authority
                            .expected_manifest
                            .as_mut()
                            .expect("active manifest")
                            .database_instance_id = "db_wrong_manifest".to_owned();
                    }
                    RecoveryAuthorityDrift::DatabaseMismatch => {
                        let conn = connect_file(&path)?;
                        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
                        conn.execute(
                            "UPDATE projection_store_state
                             SET database_instance_id='db_mismatched_store'
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::ProtocolMismatch => {
                        let conn = connect_file(&path)?;
                        conn.execute_batch("PRAGMA ignore_check_constraints=ON;")?;
                        conn.execute(
                            "UPDATE projection_store_state
                             SET protocol_version=3
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::SchemaMismatch => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET schema_version=2
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::ControlPlaneMismatch => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET control_plane='legacy'
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                }

                let result = match operation {
                    RecoveryFencedOperation::Quarantine => {
                        backend.quarantine_generation_fenced(ACTIVE, &authority)
                    }
                    RecoveryFencedOperation::Abort => {
                        backend.abort_generation_fenced(ACTIVE, &authority)
                    }
                };
                let error =
                    result.expect_err("every stale or incomplete authority must fail closed");
                assert!(
                    matches!(error, KanbanError::Conflict(_)),
                    "{operation:?}/{drift:?} returned {error:?}"
                );
                assert_eq!(
                    backend.inspect_generation(ACTIVE)?,
                    physical_before,
                    "{operation:?}/{drift:?} changed physical evidence"
                );
                assert_eq!(
                    backend.published_ids(),
                    published_before,
                    "{operation:?}/{drift:?} changed published-marker evidence"
                );
                assert!(
                    backend.quarantine_attempts().is_empty(),
                    "{operation:?}/{drift:?} reached the physical mutator"
                );
                match drift {
                    RecoveryAuthorityDrift::SameIdentityFenceRollover => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET fence_epoch=?1
                             WHERE store_name=?2",
                            params![lease.fence_epoch, STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::OwnerTokenHandoff => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET lease_owner=?1,lease_token=?2,lease_expires_at=?3,
                                 fence_epoch=?4
                             WHERE store_name=?5",
                            params![
                                owner,
                                lease.lease_token,
                                lease.lease_expires_at,
                                lease.fence_epoch,
                                STORE
                            ],
                        )?;
                    }
                    RecoveryAuthorityDrift::ExpiredLiveLease => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET lease_expires_at=?1
                             WHERE store_name=?2",
                            params![lease.lease_expires_at, STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::FutureBindingFence => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET active_fence_epoch=7
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::DatabaseMismatch => {
                        let conn = connect_file(&path)?;
                        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
                        conn.execute(
                            "UPDATE projection_store_state
                             SET database_instance_id=?1
                             WHERE store_name=?2",
                            params![database_instance_id, STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::ProtocolMismatch => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET protocol_version=2
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::SchemaMismatch => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET schema_version=1
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::ControlPlaneMismatch => {
                        connect_file(&path)?.execute(
                            "UPDATE projection_store_state
                             SET control_plane='v2'
                             WHERE store_name=?1",
                            [STORE],
                        )?;
                    }
                    RecoveryAuthorityDrift::EmptyGeneration
                    | RecoveryAuthorityDrift::EmptyOwner
                    | RecoveryAuthorityDrift::EmptyToken
                    | RecoveryAuthorityDrift::NegativeAuthorityFence
                    | RecoveryAuthorityDrift::ExpiredAuthority
                    | RecoveryAuthorityDrift::WrongRole
                    | RecoveryAuthorityDrift::WrongPhase
                    | RecoveryAuthorityDrift::WrongBinding
                    | RecoveryAuthorityDrift::WrongManifest => {}
                }
            }
        }
        Ok(())
    }

    #[test]
    fn recovery_exact_historical_authority_allows_production_recovery_mutations()
    -> anyhow::Result<()> {
        for (role, generation, artifact_fence) in [
            (ProjectionGenerationRole::Active, ACTIVE, 7),
            (ProjectionGenerationRole::Previous, PREVIOUS, 6),
            (ProjectionGenerationRole::Building, BUILDING, 8),
        ] {
            let (_temp, path) = v29_lance_fixture(true, true, true)?;
            let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
            let owner = "historical-quarantine-owner";
            let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
            let mut authority = authority_for_evidence(
                &legacy_evidence(&path, generation, artifact_fence)?,
                owner,
                &lease.lease_token,
                lease.fence_epoch,
                lease.lease_expires_at,
                role,
            );
            if role == ProjectionGenerationRole::Building {
                authority.building_phase = Some("prepared".to_owned());
            }

            backend.quarantine_generation_fenced(generation, &authority)?;
            assert!(backend.inspect_generation(generation)?.is_none());
            assert!(backend.quarantined_ids().contains(generation));
        }

        let (_temp, path) = v29_lance_fixture(false, false, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, false, false, true)?;
        let owner = "prepared-abort-owner";
        let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
        let mut authority = authority_for_evidence(
            &legacy_evidence(&path, BUILDING, 8)?,
            owner,
            &lease.lease_token,
            lease.fence_epoch,
            lease.lease_expires_at,
            ProjectionGenerationRole::Building,
        );
        authority.building_phase = Some("prepared".to_owned());
        backend.abort_generation_fenced(BUILDING, &authority)?;
        assert!(backend.inspect_generation(BUILDING)?.is_none());
        assert!(!backend.quarantined_ids().contains(BUILDING));
        assert!(!backend.published_ids().contains(BUILDING));
        Ok(())
    }

    #[test]
    fn recovery_fenced_mutators_protect_canonical_or_published_generations() -> anyhow::Result<()> {
        for (role, generation, artifact_fence) in [
            (ProjectionGenerationRole::Active, ACTIVE, 7),
            (ProjectionGenerationRole::Previous, PREVIOUS, 6),
        ] {
            let (_temp, path) = v29_lance_fixture(true, true, false)?;
            let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;
            let owner = "published-abort-owner";
            let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
            let authority = authority_for_evidence(
                &legacy_evidence(&path, generation, artifact_fence)?,
                owner,
                &lease.lease_token,
                lease.fence_epoch,
                lease.lease_expires_at,
                role,
            );
            let physical_before = backend.inspect_generation(generation)?;
            let active_before = backend.inspect_active()?;
            let published_before = backend.published_ids();

            let error = backend
                .abort_generation_fenced(generation, &authority)
                .expect_err("active/previous generations are not abortable");
            assert!(matches!(error, KanbanError::Conflict(_)));
            assert_eq!(backend.inspect_generation(generation)?, physical_before);
            assert_eq!(backend.inspect_active()?, active_before);
            assert_eq!(backend.published_ids(), published_before);
            assert!(backend.quarantine_attempts().is_empty());
        }

        let (_temp, path) = v29_lance_fixture(false, false, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, false, false, true)?;
        let owner = "store-published-abort-owner";
        let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
        connect_file(&path)?.execute(
            "UPDATE projection_store_state
             SET building_phase='store_published'
             WHERE store_name=?1",
            [STORE],
        )?;
        backend.mark_published(BUILDING);
        let mut authority = authority_for_evidence(
            &legacy_evidence(&path, BUILDING, 8)?,
            owner,
            &lease.lease_token,
            lease.fence_epoch,
            lease.lease_expires_at,
            ProjectionGenerationRole::Building,
        );
        authority.building_phase = Some("store_published".to_owned());
        let physical_before = backend.inspect_generation(BUILDING)?;
        let published_before = backend.published_ids();
        let error = backend
            .abort_generation_fenced(BUILDING, &authority)
            .expect_err("a store-published building generation is not abortable");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert_eq!(backend.inspect_generation(BUILDING)?, physical_before);
        assert_eq!(backend.published_ids(), published_before);
        assert!(backend.quarantine_attempts().is_empty());

        let (_temp, path) = v29_lance_fixture(true, false, false)?;
        bind_phase_to_current_corpus(&path, "active")?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, false, false)?;
        backend.bind_active_to_current_descriptor(&path)?;
        let owner = "canonical-active-quarantine-owner";
        let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
        let authority = authority_for_evidence(
            &evidence_for_descriptor(&path, ACTIVE, 7, &current_descriptor())?,
            owner,
            &lease.lease_token,
            lease.fence_epoch,
            lease.lease_expires_at,
            ProjectionGenerationRole::Active,
        );
        let physical_before = backend.inspect_generation(ACTIVE)?;
        let active_before = backend.inspect_active()?;
        let published_before = backend.published_ids();
        let error = backend
            .quarantine_generation_fenced(ACTIVE, &authority)
            .expect_err("an exact current canonical active generation is protected");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert_eq!(backend.inspect_generation(ACTIVE)?, physical_before);
        assert_eq!(backend.inspect_active()?, active_before);
        assert_eq!(backend.published_ids(), published_before);
        assert!(backend.quarantine_attempts().is_empty());
        Ok(())
    }

    #[test]
    fn v29_active_and_previous_without_building_converge_to_a_bound_generation()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, false)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;

        run_store_to_completion(&path, &backend)?;

        let store = lance_store_status(&path)?;
        assert!(store.building_generation.is_none());
        assert!(store.previous_generation.is_none());
        assert_ne!(store.active_generation.as_deref(), Some(ACTIVE));
        assert_eq!(store.active_corpus, current_descriptor().corpus);
        assert_eq!(
            backend.quarantined_ids(),
            BTreeSet::from([ACTIVE.to_owned(), PREVIOUS.to_owned()])
        );
        Ok(())
    }

    #[test]
    fn v29_building_and_historical_generations_converge_to_a_bound_generation() -> anyhow::Result<()>
    {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;

        run_store_to_completion(&path, &backend)?;

        let store = lance_store_status(&path)?;
        assert!(store.building_generation.is_none());
        assert!(store.previous_generation.is_none());
        assert_ne!(store.active_generation.as_deref(), Some(ACTIVE));
        assert_ne!(store.active_generation.as_deref(), Some(BUILDING));
        assert_eq!(store.active_corpus, current_descriptor().corpus);
        assert_eq!(
            backend.quarantined_ids(),
            BTreeSet::from([ACTIVE.to_owned(), PREVIOUS.to_owned(), BUILDING.to_owned()])
        );
        Ok(())
    }

    #[test]
    fn compatible_building_does_not_mask_incompatible_historical_bindings() -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, false)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;
        let mut session = MaintenanceSession::start(
            &path,
            "legacy-binding-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &path,
            STORE,
            "legacy-binding-owner",
            session.options.lease_ttl_ms,
        )?;
        let compatible_building = begin_projection_generation(
            &path,
            STORE,
            "legacy-binding-owner",
            &lease.lease_token,
            &backend,
        )?
        .generation;

        run_projection_store_operation(
            &mut session,
            STORE,
            "LanceDB task chunks",
            &lease.lease_token,
            &backend,
            false,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;

        assert!(
            backend.quarantine_attempts().contains(&compatible_building),
            "a compatible building created over unbound history must be discarded and rebuilt"
        );
        let store = lance_store_status(&path)?;
        assert_ne!(
            store.active_generation.as_deref(),
            Some(compatible_building.as_str())
        );
        assert_eq!(store.active_corpus, current_descriptor().corpus);
        release_projection_lease(&path, STORE, "legacy-binding-owner", &lease.lease_token)?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn explicit_resume_never_replaces_its_bound_generation_after_target_validation_failure()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let path = temp.path().join("explicit-resume.db");
        init_database(&path, "tester")?;
        create_task(
            &path,
            "default",
            "tester",
            CreateTask::ready("explicit resume invariant"),
        )?;
        let backend = RecoveryBackend::empty_with_helper_path(&path);
        let seed = MaintenanceSession::start(
            &path,
            "resume-seed-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease =
            acquire_projection_lease(&path, STORE, "resume-seed-owner", seed.options.lease_ttl_ms)?;
        let building = begin_projection_generation(
            &path,
            STORE,
            "resume-seed-owner",
            &lease.lease_token,
            &backend,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &path,
            STORE,
            "resume-seed-owner",
            &lease.lease_token,
            &backend,
        )?;
        release_projection_lease(&path, STORE, "resume-seed-owner", &lease.lease_token)?;
        seed.finish()?;
        backend.corrupt_generation_fingerprint(&building);

        let mut takeover = MaintenanceSession::start(
            &path,
            "resume-takeover-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &path,
            STORE,
            "resume-takeover-owner",
            takeover.options.lease_ttl_ms,
        )?;
        let attempt = run_projection_store_operation_with_intent(
            &mut takeover,
            STORE,
            "LanceDB task chunks",
            &lease.lease_token,
            &backend,
            MaintenanceStoreRunIntent::Resume,
        );
        let Err(MaintenanceStoreAttemptError::Fatal(error)) = attempt else {
            anyhow::bail!("explicit resume must fail closed instead of replacing its generation");
        };
        assert!(error.to_string().contains("explicit resume"));
        assert!(error.to_string().contains(&building));
        assert_eq!(
            lance_store_status(&path)?.building_generation.as_deref(),
            Some(building.as_str())
        );
        assert!(
            backend.quarantine_attempts().is_empty(),
            "explicit resume must not quarantine or replace its bound generation"
        );
        assert!(
            backend.inspect_generation(&building)?.is_some(),
            "explicit resume failure must retain the physical target for operator recovery"
        );
        release_projection_lease(&path, STORE, "resume-takeover-owner", &lease.lease_token)?;
        takeover.finish()?;
        Ok(())
    }

    #[test]
    fn automatic_snapshot_coverage_drift_quarantines_once_and_rebases_to_latest_truth()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let path = temp.path().join("automatic-snapshot-rebase.db");
        init_database(&path, "tester")?;
        create_task(
            &path,
            "default",
            "tester",
            CreateTask::ready("present before snapshot"),
        )?;
        let backend = RecoveryBackend::empty_with_helper_path(&path);
        let mutation_path = path.clone();
        backend.set_after_prepare(move || {
            create_task(
                &mutation_path,
                "default",
                "concurrent-writer",
                CreateTask::ready("committed while provider was preparing"),
            )
            .expect("canonical mutation during physical snapshot");
        });
        let mut session = MaintenanceSession::start(
            &path,
            "automatic-rebase-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &path,
            STORE,
            "automatic-rebase-owner",
            session.options.lease_ttl_ms,
        )?;

        let result = run_projection_store_operation_with_intent(
            &mut session,
            STORE,
            "LanceDB task chunks",
            &lease.lease_token,
            &backend,
            MaintenanceStoreRunIntent::Automatic,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;

        assert!(matches!(
            result.result,
            MaintenanceStoreResult::Succeeded { .. }
        ));
        let quarantine_attempts = backend.quarantine_attempts();
        assert_eq!(
            quarantine_attempts.len(),
            1,
            "one pass may rebase at most the single obsolete snapshot it observed"
        );
        let stale_generation = &quarantine_attempts[0];
        assert!(backend.quarantined_ids().contains(stale_generation));
        let store = lance_store_status(&path)?;
        assert!(store.building_generation.is_none());
        assert_ne!(
            store.active_generation.as_deref(),
            Some(stale_generation.as_str())
        );
        assert_eq!(store.lifecycle_status, "ready");
        let conn = connect_file(&path)?;
        let (not_done, checkpoint, maximum_cursor): (i64, i64, i64) = conn.query_row(
            "SELECT
                 SUM(status!='done'),
                 (SELECT checkpoint_cursor FROM projection_store_state WHERE store_name=?1),
                 COALESCE(MAX(cursor),0)
             FROM projection_deliveries WHERE store_name=?1",
            [STORE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(not_done, 0);
        assert_eq!(checkpoint, maximum_cursor);
        release_projection_lease(&path, STORE, "automatic-rebase-owner", &lease.lease_token)?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn prepared_exact_abort_uses_the_persisted_building_phase_and_leaves_no_candidate()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let path = temp.path().join("prepared-exact-abort.db");
        init_database(&path, "tester")?;
        create_task(
            &path,
            "default",
            "tester",
            CreateTask::ready("prepared abort authority"),
        )?;
        let initial_snapshot = canonical_control_plane_snapshot(&path)?;
        assert_eq!(
            initial_snapshot.pending_deliveries,
            initial_snapshot.delivery_count
        );
        assert_eq!(initial_snapshot.checkpoint_cursor, 0);
        assert_eq!(initial_snapshot.legacy_checkpoint_cursor, 0);
        let backend = RecoveryBackend::empty_with_helper_path(&path);
        let lease = acquire_projection_lease(&path, STORE, "prepared-abort-owner", 20_000)?;
        let generation = begin_projection_generation(
            &path,
            STORE,
            "prepared-abort-owner",
            &lease.lease_token,
            &backend,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &path,
            STORE,
            "prepared-abort-owner",
            &lease.lease_token,
            &backend,
        )?;

        let state_value = |rows: &[(String, String)], name: &str| {
            rows.iter()
                .find_map(|(column, value)| (column == name).then_some(value.clone()))
                .unwrap_or_else(|| panic!("missing projection_store_state column {name}"))
        };
        let before_abort = canonical_control_plane_snapshot(&path)?;
        let prepared_store = lance_store_status(&path)?;
        assert_eq!(
            prepared_store.building_generation.as_deref(),
            Some(generation.as_str())
        );
        assert_eq!(prepared_store.building_phase.as_deref(), Some("prepared"));
        assert_eq!(before_abort.delivery_count, 1);
        assert_eq!(before_abort.pending_deliveries, 0);
        assert!(before_abort.published_deliveries > 0);
        assert!(before_abort.checkpoint_cursor > 0);
        assert!(
            state_value(&before_abort.store_state_row, "last_success_at") != "null",
            "prepare must advance last_success_at"
        );

        abort_projection_generation(
            &path,
            STORE,
            "prepared-abort-owner",
            &lease.lease_token,
            &backend,
        )?;

        let after_abort = canonical_control_plane_snapshot(&path)?;
        assert_eq!(after_abort.outbox_rows, initial_snapshot.outbox_rows);
        assert_eq!(
            after_abort.derived_store_row.len(),
            initial_snapshot.derived_store_row.len()
        );
        for ((name, before), (_, after)) in initial_snapshot
            .derived_store_row
            .iter()
            .zip(&after_abort.derived_store_row)
        {
            assert!(
                matches!(name.as_str(), "last_sync_at" | "last_error" | "updated_at")
                    || before == after,
                "derived_store_state canonical column {name} changed: {before:?} -> {after:?}"
            );
        }
        assert_eq!(
            after_abort.derived_store.0,
            initial_snapshot.derived_store.0
        );
        assert_eq!(
            after_abort.derived_store.1,
            initial_snapshot.derived_store.1
        );
        assert_eq!(after_abort.delivery_count, initial_snapshot.delivery_count);
        assert_eq!(
            after_abort.pending_deliveries,
            initial_snapshot.pending_deliveries
        );
        assert_eq!(
            after_abort.published_deliveries,
            initial_snapshot.published_deliveries
        );
        assert_eq!(
            after_abort.claimed_deliveries,
            initial_snapshot.claimed_deliveries
        );
        assert_eq!(
            after_abort.checkpoint_cursor,
            initial_snapshot.checkpoint_cursor
        );
        assert_eq!(
            after_abort.legacy_checkpoint_cursor,
            initial_snapshot.legacy_checkpoint_cursor
        );
        assert_eq!(
            after_abort.delivery_invariants,
            initial_snapshot.delivery_invariants
        );
        assert_eq!(
            after_abort.delivery_rows.len(),
            initial_snapshot.delivery_rows.len()
        );
        for (row_index, (before, after)) in initial_snapshot
            .delivery_rows
            .iter()
            .zip(&after_abort.delivery_rows)
            .enumerate()
        {
            assert_eq!(before.len(), after.len());
            for ((name, before), (_, after)) in before.iter().zip(after) {
                assert!(
                    matches!(name.as_str(), "last_error" | "updated_at") || before == after,
                    "projection_deliveries row {row_index} canonical column {name} changed: {before:?} -> {after:?}"
                );
            }
        }
        assert_eq!(
            after_abort.store_state_row.len(),
            initial_snapshot.store_state_row.len()
        );
        for ((name, before), (_, after)) in initial_snapshot
            .store_state_row
            .iter()
            .zip(&after_abort.store_state_row)
        {
            assert!(
                name.starts_with("building_")
                    || matches!(
                        name.as_str(),
                        "lease_owner"
                            | "lease_token"
                            | "lease_expires_at"
                            | "control_plane"
                            | "snapshot_cursor"
                            | "fence_epoch"
                            | "last_success_at"
                            | "last_error"
                            | "updated_at"
                    )
                    || before == after,
                "projection_store_state canonical column {name} changed: {before:?} -> {after:?}"
            );
        }
        let after_store = lance_store_status(&path)?;
        assert_eq!(after_store.building_generation, None);
        assert_eq!(after_store.building_phase, None);
        assert_eq!(
            state_value(&after_abort.store_state_row, "control_plane"),
            "text:v2"
        );
        assert_eq!(
            state_value(&after_abort.store_state_row, "snapshot_cursor"),
            state_value(&before_abort.store_state_row, "snapshot_cursor")
        );
        assert_eq!(
            state_value(&after_abort.store_state_row, "fence_epoch"),
            format!("integer:{}", lease.fence_epoch)
        );
        assert_eq!(
            state_value(&after_abort.store_state_row, "lease_owner"),
            "text:prepared-abort-owner"
        );
        assert_eq!(
            state_value(&after_abort.store_state_row, "lease_token"),
            format!("text:{}", lease.lease_token)
        );
        assert_eq!(
            state_value(&after_abort.store_state_row, "lease_expires_at"),
            format!("integer:{}", lease.lease_expires_at)
        );
        assert_eq!(
            state_value(&after_abort.store_state_row, "last_success_at"),
            state_value(&before_abort.store_state_row, "last_success_at")
        );
        for (name, value) in &after_abort.store_state_row {
            if name.starts_with("building_") {
                assert_eq!(value, "null", "abort must clear {name}");
            }
        }

        assert!(
            backend.inspect_generation(&generation)?.is_none(),
            "prepared physical candidate must be removed"
        );
        assert!(
            backend.quarantined_ids().contains(&generation),
            "the service-level exact abort intentionally quarantines recoverable evidence"
        );
        assert!(!backend.published_ids().contains(&generation));
        assert!(
            lance_store_status(&path)?.building_generation.is_none(),
            "prepared SQLite building binding must be cleared"
        );
        release_projection_lease(&path, STORE, "prepared-abort-owner", &lease.lease_token)?;
        Ok(())
    }

    #[test]
    fn explicit_resume_preserves_an_obsolete_snapshot_generation_for_automatic_recovery()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let path = temp.path().join("explicit-resume-obsolete-snapshot.db");
        init_database(&path, "tester")?;
        create_task(
            &path,
            "default",
            "tester",
            CreateTask::ready("resume baseline"),
        )?;
        let backend = RecoveryBackend::empty_with_helper_path(&path);
        let seed = MaintenanceSession::start(
            &path,
            "resume-seed-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease =
            acquire_projection_lease(&path, STORE, "resume-seed-owner", seed.options.lease_ttl_ms)?;
        let building = begin_projection_generation(
            &path,
            STORE,
            "resume-seed-owner",
            &lease.lease_token,
            &backend,
        )?
        .generation;
        let physical_before = backend.inspect_generation(&building)?;
        create_task(
            &path,
            "default",
            "concurrent-writer",
            CreateTask::ready("new truth after resume baseline"),
        )?;
        release_projection_lease(&path, STORE, "resume-seed-owner", &lease.lease_token)?;
        seed.finish()?;

        let mut takeover = MaintenanceSession::start(
            &path,
            "resume-takeover-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &path,
            STORE,
            "resume-takeover-owner",
            takeover.options.lease_ttl_ms,
        )?;
        let attempt = run_projection_store_operation_with_intent(
            &mut takeover,
            STORE,
            "LanceDB task chunks",
            &lease.lease_token,
            &backend,
            MaintenanceStoreRunIntent::Resume,
        );
        let Err(MaintenanceStoreAttemptError::Fatal(error)) = attempt else {
            anyhow::bail!("explicit resume must fail closed on an obsolete snapshot baseline");
        };
        assert!(error.to_string().contains("explicit resume"));
        assert!(error.to_string().contains("obsolete"));
        assert_eq!(
            lance_store_status(&path)?.building_generation.as_deref(),
            Some(building.as_str())
        );
        assert!(backend.quarantine_attempts().is_empty());
        assert_eq!(backend.inspect_generation(&building)?, physical_before);
        release_projection_lease(&path, STORE, "resume-takeover-owner", &lease.lease_token)?;
        takeover.finish()?;
        Ok(())
    }

    #[test]
    fn aliased_generation_ids_fail_before_any_physical_mutation() -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture_with_building_id(true, true, true, PREVIOUS)?;
        let backend = RecoveryBackend::empty_with_helper_path(&path);
        let lease = acquire_projection_lease(&path, STORE, "alias-owner", 20_000)?;
        let before = sqlite_recovery_control_snapshot(&path)?;

        let error = recover_incompatible_projection_bindings(
            &path,
            STORE,
            "alias-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("aliased generation identities must fail closed");

        assert!(error.to_string().contains("alias"));
        assert!(backend.quarantine_attempts().is_empty());
        assert_eq!(sqlite_recovery_control_snapshot(&path)?, before);
        Ok(())
    }

    #[test]
    fn lease_heartbeat_renewal_during_slow_quarantine_does_not_break_snapshot_cas()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, false)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;
        backend.set_before_active_inspect(|| {
            thread::sleep(Duration::from_millis(1_500));
        });
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 1_000,
            claim_ttl_ms: 300,
            batch_size: 25,
        };
        let mut session = MaintenanceSession::start(
            &path,
            "heartbeat-recovery-owner",
            MaintenanceMode::Once,
            options,
        )?;

        let run = run_projection_store_once(
            &mut session,
            STORE,
            "LanceDB task chunks",
            &backend,
            MaintenanceStoreRunIntent::Automatic,
        )?;

        assert!(matches!(
            run.result,
            MaintenanceStoreResult::Succeeded { .. }
        ));
        let store = lance_store_status(&path)?;
        assert_eq!(store.active_corpus, current_descriptor().corpus);
        assert!(store.building_generation.is_none());
        session.finish()?;
        Ok(())
    }

    #[test]
    fn busy_helper_mutation_lock_makes_no_sql_change_and_retry_is_idempotent() -> anyhow::Result<()>
    {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        let lease = acquire_projection_lease(&path, STORE, "busy-helper-owner", 20_000)?;
        let helper_guard =
            DerivedStoreWriteGuard::acquire(&path, &format!("{STORE}-projection-helper"))?;
        let before = sqlite_recovery_control_snapshot(&path)?;

        recover_incompatible_projection_bindings(
            &path,
            STORE,
            "busy-helper-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("busy helper mutation lock must defer SQLite recovery");

        assert_eq!(sqlite_recovery_control_snapshot(&path)?, before);
        drop(helper_guard);
        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "busy-helper-owner",
            &lease.lease_token,
            &backend,
        )?);
        Ok(())
    }

    #[test]
    fn late_helper_writer_cannot_cross_the_final_sqlite_cas_fence() -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, false)?;
        let backend = Arc::new(RecoveryBackend::from_legacy_sqlite(
            &path, true, true, false,
        )?);
        let lease = acquire_projection_lease(&path, STORE, "late-writer-owner", 20_000)?;
        let stale_authority = authority_for_evidence(
            &legacy_evidence(&path, ACTIVE, 7)?,
            "late-writer-owner",
            &lease.lease_token,
            lease.fence_epoch,
            lease.lease_expires_at,
            ProjectionGenerationRole::Active,
        );
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (outcome_tx, outcome_rx) = mpsc::channel();
        let writer_backend = Arc::clone(&backend);
        let writer_authority = stale_authority.clone();
        backend.set_before_active_inspect(move || {
            thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut reported_block = false;
                loop {
                    match writer_backend.quarantine_generation_fenced(ACTIVE, &writer_authority) {
                        Ok(()) => {
                            outcome_tx
                                .send("unexpected stale helper mutation succeeded".to_owned())
                                .expect("writer outcome receiver");
                            return;
                        }
                        Err(error)
                            if error.to_string().contains("active physical writer")
                                && std::time::Instant::now() < deadline =>
                        {
                            if !reported_block {
                                attempted_tx.send(()).expect("writer attempt receiver");
                                reported_block = true;
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => {
                            outcome_tx
                                .send(error.to_string())
                                .expect("writer outcome receiver");
                            return;
                        }
                    }
                }
            });
            attempted_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("late helper writer must block behind recovery");
        });
        backend.set_after_active_inspect(move || {
            let writer_outcome = outcome_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("queued helper must finish before final SQLite CAS");
            assert!(
                writer_outcome.contains("stale"),
                "queued pre-bump helper must fail closed after the recovery fence bump: {writer_outcome}"
            );
        });

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "late-writer-owner",
            &lease.lease_token,
            backend.as_ref(),
        )?);

        assert_eq!(
            backend.quarantine_attempts(),
            vec![ACTIVE.to_owned(), PREVIOUS.to_owned()]
        );
        assert_eq!(sqlite_generation_ids(&path)?, (None, None, None));
        Ok(())
    }

    #[test]
    fn recovery_fence_bump_survives_failure_before_physical_quarantine() -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        let owner = "pre-quarantine-crash-owner";
        let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
        let old_fence = lease.fence_epoch;
        let stale_authority = authority_for_evidence(
            &legacy_evidence(&path, BUILDING, 8)?,
            owner,
            &lease.lease_token,
            old_fence,
            lease.lease_expires_at,
            ProjectionGenerationRole::Building,
        );
        let mut before = canonical_control_plane_snapshot(&path)?;
        backend.fail_next_quarantine("simulated crash before physical quarantine");

        let error = recover_incompatible_projection_bindings(
            &path,
            STORE,
            owner,
            &lease.lease_token,
            &backend,
        )
        .expect_err("failure before the first physical quarantine must surface");
        assert!(error.to_string().contains("simulated crash"));
        let mut after_failure = canonical_control_plane_snapshot(&path)?;
        normalize_recovery_fence_bump(&mut before);
        normalize_recovery_fence_bump(&mut after_failure);
        assert_eq!(after_failure, before);
        let fence_after_failure: i64 = connect_file(&path)?.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| row.get(0),
        )?;
        assert_eq!(fence_after_failure, old_fence + 1);
        assert_eq!(
            sqlite_generation_ids(&path)?,
            (
                Some(ACTIVE.to_owned()),
                Some(PREVIOUS.to_owned()),
                Some(BUILDING.to_owned()),
            )
        );
        assert!(backend.quarantine_attempts().is_empty());
        let stale_error = backend
            .quarantine_generation_fenced(BUILDING, &stale_authority)
            .expect_err("pre-bump helper authority must be rejected after the bump");
        assert!(stale_error.to_string().contains("stale"));
        assert!(backend.inspect_generation(BUILDING)?.is_some());
        assert!(backend.quarantine_attempts().is_empty());

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            owner,
            &lease.lease_token,
            &backend,
        )?);
        let fence_after_retry: i64 = connect_file(&path)?.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| row.get(0),
        )?;
        assert_eq!(fence_after_retry, old_fence + 2);
        assert_eq!(sqlite_generation_ids(&path)?, (None, None, None));
        assert_eq!(
            backend.quarantine_attempts(),
            vec![BUILDING.to_owned(), ACTIVE.to_owned(), PREVIOUS.to_owned()]
        );
        let after = canonical_control_plane_snapshot(&path)?;
        assert_eq!(after.outbox, before.outbox);
        assert_eq!(after.derived_store, before.derived_store);
        assert_eq!(after.delivery_count, before.delivery_count);
        assert_eq!(
            after.legacy_checkpoint_cursor,
            before.legacy_checkpoint_cursor
        );
        assert_eq!(after.pending_deliveries, after.delivery_count);
        assert_eq!(after.checkpoint_cursor, 0);
        Ok(())
    }

    #[test]
    fn recovery_retry_after_physical_quarantine_before_final_cas_is_idempotent()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        let owner = "post-quarantine-crash-owner";
        let lease = acquire_projection_lease(&path, STORE, owner, 20_000)?;
        let old_fence = lease.fence_epoch;
        let mut before = canonical_control_plane_snapshot(&path)?;
        backend.fail_next_active_inspect("simulated crash after physical quarantine");

        let error = recover_incompatible_projection_bindings(
            &path,
            STORE,
            owner,
            &lease.lease_token,
            &backend,
        )
        .expect_err("failure after physical quarantine must surface before SQLite CAS");
        assert!(
            error
                .to_string()
                .contains("simulated crash after physical quarantine")
        );
        let mut after_failure = canonical_control_plane_snapshot(&path)?;
        normalize_recovery_fence_bump(&mut before);
        normalize_recovery_fence_bump(&mut after_failure);
        assert_eq!(after_failure, before);
        let fence_after_failure: i64 = connect_file(&path)?.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| row.get(0),
        )?;
        assert_eq!(fence_after_failure, old_fence + 1);
        assert_eq!(
            sqlite_generation_ids(&path)?,
            (
                Some(ACTIVE.to_owned()),
                Some(PREVIOUS.to_owned()),
                Some(BUILDING.to_owned()),
            )
        );
        assert_eq!(
            backend.quarantine_attempts(),
            vec![BUILDING.to_owned(), ACTIVE.to_owned(), PREVIOUS.to_owned()]
        );
        assert!(backend.inspect_generation(ACTIVE)?.is_none());
        assert!(backend.inspect_generation(PREVIOUS)?.is_none());
        assert!(backend.inspect_generation(BUILDING)?.is_none());

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            owner,
            &lease.lease_token,
            &backend,
        )?);
        let fence_after_retry: i64 = connect_file(&path)?.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| row.get(0),
        )?;
        assert_eq!(fence_after_retry, old_fence + 2);
        assert_eq!(sqlite_generation_ids(&path)?, (None, None, None));
        assert_eq!(
            backend.quarantine_attempts(),
            vec![
                BUILDING.to_owned(),
                ACTIVE.to_owned(),
                PREVIOUS.to_owned(),
                BUILDING.to_owned(),
                ACTIVE.to_owned(),
                PREVIOUS.to_owned(),
            ]
        );
        let after = canonical_control_plane_snapshot(&path)?;
        assert_eq!(after.outbox, before.outbox);
        assert_eq!(after.derived_store, before.derived_store);
        assert_eq!(after.delivery_count, before.delivery_count);
        assert_eq!(
            after.legacy_checkpoint_cursor,
            before.legacy_checkpoint_cursor
        );
        assert_eq!(after.pending_deliveries, after.delivery_count);
        assert_eq!(after.checkpoint_cursor, 0);
        Ok(())
    }

    #[test]
    fn physical_quarantine_is_idempotent_after_a_partial_crash() -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        backend.prequarantine(BUILDING);
        backend.prequarantine(ACTIVE);
        let lease = acquire_projection_lease(&path, STORE, "retry-owner", 20_000)?;

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "retry-owner",
            &lease.lease_token,
            &backend,
        )?);

        let store = lance_store_status(&path)?;
        assert!(store.active_generation.is_none());
        assert!(store.previous_generation.is_none());
        assert!(store.building_generation.is_none());
        assert_eq!(store.last_success_at, None);
        assert!(backend.inspect_generation(ACTIVE)?.is_none());
        assert!(backend.inspect_generation(PREVIOUS)?.is_none());
        assert!(backend.inspect_generation(BUILDING)?.is_none());
        Ok(())
    }

    #[test]
    fn incompatible_previous_is_removed_without_discarding_a_compatible_active()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, false)?;
        bind_phase_to_current_corpus(&path, "active")?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;
        backend.bind_active_to_current_descriptor(&path)?;
        let lease = acquire_projection_lease(&path, STORE, "previous-owner", 20_000)?;

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "previous-owner",
            &lease.lease_token,
            &backend,
        )?);

        let store = lance_store_status(&path)?;
        assert_eq!(store.active_generation.as_deref(), Some(ACTIVE));
        assert_eq!(store.active_corpus, current_descriptor().corpus);
        assert!(store.previous_generation.is_none());
        assert_eq!(store.last_success_at, Some(4_242));
        assert_eq!(
            backend
                .inspect_active()?
                .map(|active| active.manifest.generation),
            Some(ACTIVE.to_owned())
        );
        assert_eq!(
            backend.quarantined_ids(),
            BTreeSet::from([PREVIOUS.to_owned()])
        );
        Ok(())
    }

    #[test]
    fn retained_previous_requires_exact_physical_readback_before_sqlite_recovery()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        replace_legacy_phase_with_bound_generation(&path, "active")?;
        replace_legacy_phase_with_bound_generation(&path, "previous")?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        backend.bind_active_to_current_descriptor(&path)?;
        let lease = acquire_projection_lease(&path, STORE, "retained-previous-owner", 20_000)?;
        let mut before = sqlite_recovery_control_snapshot(&path)?;

        recover_incompatible_projection_bindings(
            &path,
            STORE,
            "retained-previous-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("mismatched retained previous evidence must fail closed");

        let mut after_failure = sqlite_recovery_control_snapshot(&path)?;
        let conn = connect_file(&path)?;
        let statement = conn.prepare("SELECT * FROM projection_store_state WHERE store_name=?1")?;
        let column_names = statement.column_names();
        let fence_index = column_names
            .iter()
            .position(|name| *name == "fence_epoch")
            .expect("fence_epoch column");
        let updated_at_index = column_names
            .iter()
            .position(|name| *name == "updated_at")
            .expect("updated_at column");
        let fence_after_failure: i64 = conn.query_row(
            "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| row.get(0),
        )?;
        assert_eq!(fence_after_failure, lease.fence_epoch + 1);
        for snapshot in [&mut before, &mut after_failure] {
            snapshot.store_state[fence_index] = "normalized:recovery-fence".to_owned();
            snapshot.store_state[updated_at_index] = "normalized:updated-at".to_owned();
        }
        assert_eq!(after_failure, before);
        drop(statement);
        drop(conn);
        backend.bind_previous_to_current_descriptor(&path)?;
        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "retained-previous-owner",
            &lease.lease_token,
            &backend,
        )?);
        assert_eq!(
            sqlite_generation_ids(&path)?,
            (Some(ACTIVE.to_owned()), Some(PREVIOUS.to_owned()), None)
        );
        assert_eq!(
            backend.quarantined_ids(),
            BTreeSet::from([BUILDING.to_owned()])
        );
        Ok(())
    }

    #[test]
    fn unattributed_physical_active_fails_closed_without_clearing_sqlite() -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        backend.install_unknown_active(&path, "gen_unattributed_x")?;
        let lease = acquire_projection_lease(&path, STORE, "unknown-owner", 20_000)?;

        let error = recover_incompatible_projection_bindings(
            &path,
            STORE,
            "unknown-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("unattributed physical active generation must fail closed");

        assert!(error.to_string().contains("unattributed"));
        assert!(
            !backend
                .quarantine_attempts()
                .iter()
                .any(|generation| generation == "gen_unattributed_x")
        );
        assert!(backend.inspect_generation("gen_unattributed_x")?.is_some());
        assert_eq!(
            sqlite_generation_ids(&path)?,
            (
                Some(ACTIVE.to_owned()),
                Some(PREVIOUS.to_owned()),
                Some(BUILDING.to_owned())
            )
        );
        Ok(())
    }

    #[test]
    fn active_quarantine_may_promote_previous_but_final_readback_still_converges()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, false)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;
        backend.promote_after_active_quarantine(PREVIOUS);
        let lease = acquire_projection_lease(&path, STORE, "promotion-owner", 20_000)?;

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "promotion-owner",
            &lease.lease_token,
            &backend,
        )?);

        assert_eq!(backend.inspect_active()?, None);
        assert_eq!(sqlite_generation_ids(&path)?, (None, None, None));
        assert_eq!(
            backend.quarantined_ids(),
            BTreeSet::from([ACTIVE.to_owned(), PREVIOUS.to_owned()])
        );
        Ok(())
    }

    #[test]
    fn binding_control_or_lease_race_after_quarantine_preserves_the_exact_raced_snapshot()
    -> anyhow::Result<()> {
        for (case, mutation) in [
            (
                "token",
                "UPDATE projection_store_state
                 SET lease_token='raced-lease-token'
                 WHERE store_name='lancedb_chunks'",
            ),
            (
                "fence",
                "UPDATE projection_store_state
                 SET fence_epoch=fence_epoch+1
                 WHERE store_name='lancedb_chunks'",
            ),
            (
                "previous_corpus",
                "UPDATE projection_store_state
                 SET previous_corpus_schema='task-chunks-v2',
                     previous_corpus_fingerprint='raced-previous-corpus',
                     previous_embedding_model='raced-previous-model',
                     previous_embedding_dimensions=7
                 WHERE store_name='lancedb_chunks'",
            ),
            (
                "building_corpus",
                "UPDATE projection_store_state
                 SET building_corpus_schema='task-chunks-v2',
                     building_corpus_fingerprint='raced-building-corpus',
                     building_embedding_model='raced-building-model',
                     building_embedding_dimensions=7
                 WHERE store_name='lancedb_chunks'",
            ),
            (
                "control",
                "UPDATE projection_store_state
                 SET control_plane='legacy'
                 WHERE store_name='lancedb_chunks'",
            ),
        ] {
            let (_temp, path) = v29_lance_fixture(true, true, true)?;
            if case == "previous_corpus" {
                replace_legacy_phase_with_bound_generation(&path, "previous")?;
            } else if case == "building_corpus" {
                replace_legacy_phase_with_bound_generation(&path, "building")?;
            }
            let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
            let lease = acquire_projection_lease(&path, STORE, "race-owner", 20_000)?;
            let race_path = path.clone();
            let raced_snapshot = Arc::new(Mutex::new(None));
            let hook_snapshot = Arc::clone(&raced_snapshot);
            backend.set_before_active_inspect(move || {
                connect_file(&race_path)
                    .and_then(|conn| {
                        conn.execute_batch(mutation)
                            .map_err(|error| KanbanError::Storage(error.to_string()))
                    })
                    .expect("inject recovery race");
                *hook_snapshot.lock().expect("raced snapshot lock") =
                    Some(sqlite_recovery_control_snapshot(&race_path).expect("raced snapshot"));
            });

            recover_incompatible_projection_bindings(
                &path,
                STORE,
                "race-owner",
                &lease.lease_token,
                &backend,
            )
            .unwrap_err();

            assert_eq!(
                sqlite_recovery_control_snapshot(&path)?,
                raced_snapshot
                    .lock()
                    .expect("raced snapshot lock")
                    .take()
                    .expect("hook captured raced snapshot"),
                "{case}: recovery must not modify any field after detecting the race"
            );
        }
        Ok(())
    }

    #[test]
    fn recovery_recomputes_checkpoint_without_touching_outbox_or_legacy_watermark()
    -> anyhow::Result<()> {
        let (_temp, path) = v29_lance_fixture(true, true, true)?;
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, true)?;
        let before = canonical_control_plane_snapshot(&path)?;
        assert!(before.delivery_count > 0);
        assert!(before.checkpoint_cursor > 0);
        let lease = acquire_projection_lease(&path, STORE, "control-owner", 20_000)?;

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "control-owner",
            &lease.lease_token,
            &backend,
        )?);

        let after = canonical_control_plane_snapshot(&path)?;
        assert_eq!(after.outbox, before.outbox);
        assert_eq!(after.derived_store, before.derived_store);
        assert_eq!(after.delivery_count, before.delivery_count);
        assert_eq!(after.pending_deliveries, after.delivery_count);
        assert_eq!(after.published_deliveries, 0);
        assert_eq!(after.claimed_deliveries, 0);
        assert_eq!(after.checkpoint_cursor, 0);
        assert_eq!(
            after.legacy_checkpoint_cursor,
            before.legacy_checkpoint_cursor
        );
        assert_eq!(after.delivery_controls, before.delivery_controls);
        Ok(())
    }

    #[test]
    fn corpus_upgrade_reason_survives_physical_health_failure_for_every_legacy_phase()
    -> anyhow::Result<()> {
        for (case, active, previous, building) in [
            ("active", true, false, false),
            ("previous", true, true, false),
            ("building", false, false, true),
        ] {
            let (_temp, path) = v29_lance_fixture(active, previous, building)?;
            if case == "previous" {
                bind_phase_to_current_corpus(&path, "active")?;
            }
            let mut status = projection_status(&path)?;
            let before = status
                .stores
                .iter()
                .find(|store| store.store_name == STORE)
                .expect("Lance status");
            assert_eq!(
                before.fallback_reason.as_deref(),
                Some("corpus_binding_upgrade_required"),
                "{case}"
            );

            enrich_physical_health(
                &path,
                &mut status,
                STORE,
                "LanceDB task chunks",
                &RecoveryBackend::empty(),
            )?;

            let after = status
                .stores
                .iter()
                .find(|store| store.store_name == STORE)
                .expect("Lance status");
            assert_eq!(
                after.fallback_reason.as_deref(),
                Some("corpus_binding_upgrade_required"),
                "{case}: physical health must not erase the actionable upgrade reason"
            );
        }
        Ok(())
    }

    fn bind_phase_to_current_corpus(path: &Path, phase: &str) -> anyhow::Result<()> {
        replace_legacy_phase_with_bound_generation(path, phase)
    }

    fn replace_legacy_phase_with_bound_generation(path: &Path, phase: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(phase, "active" | "previous" | "building"),
            "unsupported projection phase {phase}"
        );
        let corpus = current_descriptor().corpus.expect("Lance corpus binding");
        let mut conn = connect_file(path)?;
        let snapshot_select = if phase == "building" {
            // v29 has no building-scoped cursor column.  The v2 protocol
            // keeps the unfinished generation's cursor in the global
            // `snapshot_cursor` field, so carry that authority through the
            // corpus-binding upgrade fixture instead of manufacturing an
            // incomplete prepared binding.
            "snapshot_cursor".to_owned()
        } else {
            format!("{phase}_snapshot_cursor")
        };
        let phase_select = if phase == "building" {
            "building_phase".to_owned()
        } else {
            "NULL".to_owned()
        };
        let generation = conn.query_row(
            &format!(
                "SELECT {phase}_generation,{phase}_fingerprint,{phase}_fence_epoch,
                        {snapshot_select},{phase}_provider,{phase}_provider_fingerprint,
                        {phase}_canonical_count,{phase}_canonical_digest,
                        {phase}_delivery_count,{phase}_delivery_digest,{phase_select}
                 FROM projection_store_state WHERE store_name=?1"
            ),
            [STORE],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            },
        )?;
        let tx = conn.transaction()?;
        if phase == "building" {
            tx.execute(
                "UPDATE projection_store_state
                 SET building_generation=NULL,building_fingerprint=NULL,
                     building_fence_epoch=NULL,building_provider=NULL,
                     building_provider_fingerprint=NULL,
                     building_canonical_count=NULL,building_canonical_digest=NULL,
                     building_delivery_count=NULL,building_delivery_digest=NULL,
                     building_phase=NULL,building_corpus_schema=NULL,
                     building_corpus_fingerprint=NULL,building_embedding_model=NULL,
                     building_embedding_dimensions=NULL
                 WHERE store_name=?1",
                [STORE],
            )?;
            tx.execute(
                "UPDATE projection_store_state
                 SET building_generation=?1,building_fingerprint=?2,
                     building_fence_epoch=?3,snapshot_cursor=?4,
                     building_provider=?5,
                     building_provider_fingerprint=?6,
                     building_canonical_count=?7,building_canonical_digest=?8,
                     building_delivery_count=?9,building_delivery_digest=?10,
                     building_phase=?11,building_corpus_schema=?12,
                     building_corpus_fingerprint=?13,building_embedding_model=?14,
                     building_embedding_dimensions=?15
                 WHERE store_name=?16",
                rusqlite::params![
                    generation.0,
                    generation.1,
                    generation.2,
                    generation.3,
                    generation.4,
                    generation.5,
                    generation.6,
                    generation.7,
                    generation.8,
                    generation.9,
                    generation.10,
                    corpus.corpus_schema,
                    corpus.corpus_fingerprint,
                    corpus.embedding_model,
                    i64::try_from(corpus.embedding_dimensions)?,
                    STORE
                ],
            )?;
        } else {
            tx.execute(
                &format!(
                    "UPDATE projection_store_state
                     SET {phase}_generation=NULL,{phase}_fingerprint=NULL,
                         {phase}_fence_epoch=NULL,{phase}_snapshot_cursor=NULL,
                         {phase}_provider=NULL,{phase}_provider_fingerprint=NULL,
                         {phase}_canonical_count=NULL,{phase}_canonical_digest=NULL,
                         {phase}_delivery_count=NULL,{phase}_delivery_digest=NULL,
                         {phase}_corpus_schema=NULL,{phase}_corpus_fingerprint=NULL,
                         {phase}_embedding_model=NULL,{phase}_embedding_dimensions=NULL
                     WHERE store_name=?1"
                ),
                [STORE],
            )?;
            tx.execute(
                &format!(
                    "UPDATE projection_store_state
                     SET {phase}_generation=?1,{phase}_fingerprint=?2,
                         {phase}_fence_epoch=?3,{phase}_snapshot_cursor=?4,
                         {phase}_provider=?5,{phase}_provider_fingerprint=?6,
                         {phase}_canonical_count=?7,{phase}_canonical_digest=?8,
                         {phase}_delivery_count=?9,{phase}_delivery_digest=?10,
                         {phase}_corpus_schema=?11,{phase}_corpus_fingerprint=?12,
                         {phase}_embedding_model=?13,{phase}_embedding_dimensions=?14
                     WHERE store_name=?15"
                ),
                rusqlite::params![
                    generation.0,
                    generation.1,
                    generation.2,
                    generation.3,
                    generation.4,
                    generation.5,
                    generation.6,
                    generation.7,
                    generation.8,
                    generation.9,
                    corpus.corpus_schema,
                    corpus.corpus_fingerprint,
                    corpus.embedding_model,
                    i64::try_from(corpus.embedding_dimensions)?,
                    STORE
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn run_store_to_completion(path: &Path, backend: &RecoveryBackend) -> anyhow::Result<()> {
        let mut session = MaintenanceSession::start(
            path,
            "legacy-binding-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            path,
            STORE,
            "legacy-binding-owner",
            session.options.lease_ttl_ms,
        )?;
        let run = run_projection_store_operation(
            &mut session,
            STORE,
            "LanceDB task chunks",
            &lease.lease_token,
            backend,
            false,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        assert!(matches!(
            run.result,
            MaintenanceStoreResult::Succeeded { .. }
        ));
        release_projection_lease(path, STORE, "legacy-binding-owner", &lease.lease_token)?;
        session.finish()?;
        Ok(())
    }

    fn current_descriptor() -> ProjectionStoreDescriptor {
        ProjectionStoreDescriptor {
            store_name: STORE.to_owned(),
            provider: "fake-lance".to_owned(),
            provider_fingerprint: "fake-lance-v2".to_owned(),
            corpus: Some(ProjectionCorpusMetadata {
                corpus_schema: "task-chunks-v2".to_owned(),
                corpus_fingerprint: "task-chunks-v2:fake-corpus-v2".to_owned(),
                embedding_model: "fake-embedding-v2".to_owned(),
                embedding_dimensions: 3,
            }),
        }
    }

    fn legacy_evidence(
        path: &Path,
        generation: &str,
        fence_epoch: i64,
    ) -> anyhow::Result<ProjectionArtifactEvidence> {
        evidence(
            path,
            generation,
            fence_epoch,
            "fake-lance",
            "fake-lance-v2",
            None,
        )
    }

    fn evidence_for_descriptor(
        path: &Path,
        generation: &str,
        fence_epoch: i64,
        descriptor: &ProjectionStoreDescriptor,
    ) -> anyhow::Result<ProjectionArtifactEvidence> {
        evidence(
            path,
            generation,
            fence_epoch,
            &descriptor.provider,
            &descriptor.provider_fingerprint,
            descriptor.corpus.clone(),
        )
    }

    fn evidence(
        path: &Path,
        generation: &str,
        fence_epoch: i64,
        provider: &str,
        provider_fingerprint: &str,
        corpus: Option<ProjectionCorpusMetadata>,
    ) -> anyhow::Result<ProjectionArtifactEvidence> {
        let conn = connect_file(path)?;
        let (database_instance_id, protocol_version, schema_version): (String, i64, i64) = conn
            .query_row(
                "SELECT database_instance_id,protocol_version,schema_version
                 FROM projection_store_state WHERE store_name=?1",
                [STORE],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        let fingerprint = format!("fake:{generation}");
        Ok(ProjectionArtifactEvidence {
            manifest: ProjectionArtifactManifest {
                store_name: STORE.to_owned(),
                database_instance_id,
                protocol_version,
                schema_version,
                generation: generation.to_owned(),
                fence_epoch,
                snapshot_cursor: 0,
                provider: provider.to_owned(),
                provider_fingerprint: provider_fingerprint.to_owned(),
                corpus,
                canonical_item_count: 0,
                canonical_digest: format!("canonical:{generation}"),
                delivery_item_count: 0,
                delivery_digest: format!("delivery:{generation}"),
                fingerprint: Some(fingerprint.clone()),
            },
            fingerprint,
        })
    }

    fn authority_for_evidence(
        evidence: &ProjectionArtifactEvidence,
        owner: &str,
        lease_token: &str,
        fence_epoch: i64,
        lease_expires_at: i64,
        role: ProjectionGenerationRole,
    ) -> ProjectionDestructiveAuthority {
        let manifest = &evidence.manifest;
        ProjectionDestructiveAuthority {
            owner: owner.to_owned(),
            lease_token: lease_token.to_owned(),
            fence_epoch,
            lease_expires_at,
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

    fn v29_lance_fixture(
        active: bool,
        previous: bool,
        building: bool,
    ) -> anyhow::Result<(TempDir, PathBuf)> {
        v29_lance_fixture_with_building_id(active, previous, building, BUILDING)
    }

    fn v29_lance_fixture_with_building_id(
        active: bool,
        previous: bool,
        building: bool,
        building_generation: &str,
    ) -> anyhow::Result<(TempDir, PathBuf)> {
        let temp = tempdir()?;
        let path = temp.path().join("kanban.db");
        init_database(&path, "tester")?;
        create_task(
            &path,
            "default",
            "tester",
            CreateTask::ready("legacy Lance corpus binding"),
        )?;
        let conn = connect_file(&path)?;
        conn.execute_batch(
            "DROP TRIGGER projection_active_corpus_after_generation_reset;
             DROP TRIGGER projection_previous_corpus_after_generation_reset;
             DROP TRIGGER projection_building_corpus_after_generation_reset;
             DROP TRIGGER projection_active_corpus_generation_guard;
             DROP TRIGGER projection_previous_corpus_generation_guard;
             DROP TRIGGER projection_building_corpus_generation_guard;
             DROP TRIGGER projection_corpus_generation_insert_guard;
             ALTER TABLE projection_store_state DROP COLUMN active_embedding_dimensions;
             ALTER TABLE projection_store_state DROP COLUMN active_embedding_model;
             ALTER TABLE projection_store_state DROP COLUMN active_corpus_fingerprint;
             ALTER TABLE projection_store_state DROP COLUMN active_corpus_schema;
             ALTER TABLE projection_store_state DROP COLUMN previous_embedding_dimensions;
             ALTER TABLE projection_store_state DROP COLUMN previous_embedding_model;
             ALTER TABLE projection_store_state DROP COLUMN previous_corpus_fingerprint;
             ALTER TABLE projection_store_state DROP COLUMN previous_corpus_schema;
             ALTER TABLE projection_store_state DROP COLUMN building_embedding_dimensions;
             ALTER TABLE projection_store_state DROP COLUMN building_embedding_model;
             ALTER TABLE projection_store_state DROP COLUMN building_corpus_fingerprint;
             ALTER TABLE projection_store_state DROP COLUMN building_corpus_schema;
             DELETE FROM schema_migrations WHERE version=30;
             PRAGMA user_version=29;",
        )?;
        seed_v29_slot(&conn, "active", active, ACTIVE, 7)?;
        seed_v29_slot(&conn, "previous", previous, PREVIOUS, 6)?;
        seed_v29_slot(&conn, "building", building, building_generation, 8)?;
        conn.execute(
            "UPDATE projection_store_state
             SET control_plane='v2',lifecycle_status='ready',
                 fence_epoch=8,legacy_checkpoint_cursor=777,last_success_at=4242
             WHERE store_name=?1",
            [STORE],
        )?;
        if active {
            conn.execute(
                "UPDATE projection_deliveries
                 SET status='done',published_generation=?1,
                     attempts=4,next_attempt_at=9876543210,
                     last_error='preserve legacy delivery diagnostic',
                     claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                     claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL
                 WHERE store_name=?2",
                rusqlite::params![ACTIVE, STORE],
            )?;
            conn.execute(
                "UPDATE projection_store_state
                 SET checkpoint_cursor=(
                   SELECT COALESCE(MAX(cursor),0)
                   FROM projection_deliveries WHERE store_name=?1
                 )
                 WHERE store_name=?1",
                [STORE],
            )?;
        }
        drop(conn);
        init_database(&path, "upgrade")?;
        Ok((temp, path))
    }

    fn seed_v29_slot(
        conn: &rusqlite::Connection,
        phase: &str,
        present: bool,
        generation: &str,
        fence_epoch: i64,
    ) -> anyhow::Result<()> {
        let snapshot_column = if phase == "building" {
            // The building cursor is global in projection_store_state.  Make
            // the legacy prepared fixture explicit so destructive authority
            // reconstruction sees the same cursor as the physical evidence.
            ",snapshot_cursor=0".to_owned()
        } else {
            format!(",{phase}_snapshot_cursor=0")
        };
        let building_phase = if phase == "building" {
            ",building_phase='prepared'"
        } else {
            ""
        };
        if present {
            conn.execute(
                &format!(
                    "UPDATE projection_store_state
                     SET {phase}_generation=?1,{phase}_fingerprint=?2,
                         {phase}_fence_epoch=?3{snapshot_column},
                         {phase}_provider='fake-lance',
                         {phase}_provider_fingerprint='fake-lance-v2',
                         {phase}_canonical_count=0,
                         {phase}_canonical_digest=?4,
                         {phase}_delivery_count=0,
                         {phase}_delivery_digest=?5
                         {building_phase}
                     WHERE store_name=?6"
                ),
                rusqlite::params![
                    generation,
                    format!("fake:{generation}"),
                    fence_epoch,
                    format!("canonical:{generation}"),
                    format!("delivery:{generation}"),
                    STORE
                ],
            )?;
        }
        Ok(())
    }

    fn lance_store_status(path: &Path) -> anyhow::Result<ProjectionStoreStatus> {
        projection_status(path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == STORE)
            .ok_or_else(|| anyhow::anyhow!("Lance store status is missing"))
    }

    fn sqlite_generation_ids(
        path: &Path,
    ) -> anyhow::Result<(Option<String>, Option<String>, Option<String>)> {
        Ok(connect_file(path)?.query_row(
            "SELECT active_generation,previous_generation,building_generation
             FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SqliteRecoveryControlSnapshot {
        store_state: Vec<String>,
        deliveries: Vec<Vec<String>>,
    }

    fn sqlite_recovery_control_snapshot(
        path: &Path,
    ) -> anyhow::Result<SqliteRecoveryControlSnapshot> {
        let conn = connect_file(path)?;
        let mut state_statement =
            conn.prepare("SELECT * FROM projection_store_state WHERE store_name=?1")?;
        let state_column_count = state_statement.column_count();
        let store_state = state_statement
            .query_row([STORE], |row| sqlite_row_snapshot(row, state_column_count))?;

        let mut delivery_statement =
            conn.prepare("SELECT * FROM projection_deliveries WHERE store_name=?1 ORDER BY id")?;
        let delivery_column_count = delivery_statement.column_count();
        let deliveries = delivery_statement
            .query_map([STORE], |row| {
                sqlite_row_snapshot(row, delivery_column_count)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(SqliteRecoveryControlSnapshot {
            store_state,
            deliveries,
        })
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

    fn sqlite_named_row_snapshot(
        row: &rusqlite::Row<'_>,
        column_count: usize,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let values = sqlite_row_snapshot(row, column_count)?;
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| Ok((row.as_ref().column_name(index)?.to_owned(), value)))
            .collect()
    }

    #[derive(Debug, PartialEq, Eq)]
    struct CanonicalControlPlaneSnapshot {
        outbox: Vec<(i64, String, Option<String>, i64)>,
        outbox_rows: Vec<Vec<(String, String)>>,
        derived_store: (i64, Option<i64>, Option<i64>, Option<String>, i64),
        derived_store_row: Vec<(String, String)>,
        delivery_count: i64,
        pending_deliveries: i64,
        published_deliveries: i64,
        claimed_deliveries: i64,
        checkpoint_cursor: i64,
        legacy_checkpoint_cursor: i64,
        delivery_controls: Vec<(i64, i64, i64, Option<String>)>,
        store_state_row: Vec<(String, String)>,
        delivery_rows: Vec<Vec<(String, String)>>,
        delivery_invariants: Vec<Vec<(String, String)>>,
    }

    fn canonical_control_plane_snapshot(
        path: &Path,
    ) -> anyhow::Result<CanonicalControlPlaneSnapshot> {
        let conn = connect_file(path)?;
        let mut statement =
            conn.prepare("SELECT id,status,last_error,updated_at FROM index_outbox ORDER BY id")?;
        let outbox = statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut outbox_rows_statement = conn.prepare("SELECT * FROM index_outbox ORDER BY id")?;
        let outbox_column_count = outbox_rows_statement.column_count();
        let outbox_rows = outbox_rows_statement
            .query_map([], |row| {
                sqlite_named_row_snapshot(row, outbox_column_count)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let derived_store = conn.query_row(
            "SELECT dirty,last_event_id,last_sync_at,last_error,updated_at
             FROM derived_store_state WHERE store_name=?1",
            [STORE],
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
        let mut derived_store_statement =
            conn.prepare("SELECT * FROM derived_store_state WHERE store_name=?1")?;
        let derived_store_column_count = derived_store_statement.column_count();
        let derived_store_row = derived_store_statement.query_row([STORE], |row| {
            sqlite_named_row_snapshot(row, derived_store_column_count)
        })?;
        let (delivery_count, pending_deliveries, published_deliveries, claimed_deliveries) = conn
            .query_row(
            "SELECT COUNT(*),
                    SUM(status='pending'),
                    SUM(published_generation IS NOT NULL),
                    SUM(claim_token IS NOT NULL)
             FROM projection_deliveries WHERE store_name=?1",
            [STORE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let (checkpoint_cursor, legacy_checkpoint_cursor) = conn.query_row(
            "SELECT checkpoint_cursor,legacy_checkpoint_cursor
             FROM projection_store_state WHERE store_name=?1",
            [STORE],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut store_state_statement =
            conn.prepare("SELECT * FROM projection_store_state WHERE store_name=?1")?;
        let store_state_column_count = store_state_statement.column_count();
        let store_state_row = store_state_statement.query_row([STORE], |row| {
            sqlite_named_row_snapshot(row, store_state_column_count)
        })?;
        let mut delivery_control_statement = conn.prepare(
            "SELECT id,attempts,next_attempt_at,last_error
             FROM projection_deliveries WHERE store_name=?1 ORDER BY id",
        )?;
        let delivery_controls = delivery_control_statement
            .query_map([STORE], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut delivery_rows_statement =
            conn.prepare("SELECT * FROM projection_deliveries WHERE store_name=?1 ORDER BY id")?;
        let delivery_column_count = delivery_rows_statement.column_count();
        let delivery_rows = delivery_rows_statement
            .query_map([STORE], |row| {
                sqlite_named_row_snapshot(row, delivery_column_count)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut delivery_invariants_statement = conn.prepare(
            "SELECT id,status,attempts,next_attempt_at,claim_owner,claim_token,
                    claim_lease_token,claim_fence_epoch,claim_generation,
                    claim_expires_at,published_generation
             FROM projection_deliveries WHERE store_name=?1 ORDER BY id",
        )?;
        let delivery_invariants_column_count = delivery_invariants_statement.column_count();
        let delivery_invariants = delivery_invariants_statement
            .query_map([STORE], |row| {
                sqlite_named_row_snapshot(row, delivery_invariants_column_count)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(CanonicalControlPlaneSnapshot {
            outbox,
            outbox_rows,
            derived_store,
            derived_store_row,
            delivery_count,
            pending_deliveries,
            published_deliveries,
            claimed_deliveries,
            checkpoint_cursor,
            legacy_checkpoint_cursor,
            delivery_controls,
            store_state_row,
            delivery_rows,
            delivery_invariants,
        })
    }

    fn normalize_recovery_fence_bump(snapshot: &mut CanonicalControlPlaneSnapshot) {
        for (column, value) in &mut snapshot.store_state_row {
            match column.as_str() {
                "fence_epoch" => *value = "normalized:recovery-fence".to_owned(),
                "updated_at" => *value = "normalized:updated-at".to_owned(),
                _ => {}
            }
        }
    }
}

#[cfg(all(test, feature = "tantivy-backend"))]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use tempfile::tempdir;

    use super::*;
    use crate::init::init_database;
    use crate::service::{
        CreateTask, ProjectionArtifactEvidence, ProjectionBatch, ProjectionBatchReceipt,
        ProjectionDestructiveAuthority, ProjectionPublishReceipt, ProjectionSnapshot,
        ProjectionStoreDescriptor, create_task,
    };

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct TransientProjectionState {
        generations: BTreeMap<String, ProjectionArtifactEvidence>,
        prepared: Option<ProjectionArtifactEvidence>,
        active: Option<ProjectionArtifactEvidence>,
        published: BTreeSet<String>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TransientPhysicalMarker {
        Missing,
        Exact,
        Invalid,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TransientPhysicalGeneration {
        path: PathBuf,
        evidence: ProjectionArtifactEvidence,
        marker: TransientPhysicalMarker,
    }

    #[derive(Debug, Clone, Copy)]
    enum TransientMarkerRequirement {
        Unpublished,
        Published,
        Repairable,
    }

    struct TransientGenerationInspectStore {
        inner: TantivyProjectionStore,
        fail_next_inspect: AtomicBool,
        force_active_conflict: bool,
        descriptor_override: Option<ProjectionStoreDescriptor>,
        helper_path: Option<PathBuf>,
        state: Mutex<TransientProjectionState>,
    }

    impl TransientGenerationInspectStore {
        fn new(path: &Path, inner: TantivyProjectionStore) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(true),
                force_active_conflict: false,
                descriptor_override: None,
                helper_path: Some(path.to_owned()),
                state: Mutex::new(TransientProjectionState::default()),
            }
        }

        fn with_descriptor(
            path: &Path,
            inner: TantivyProjectionStore,
            descriptor: ProjectionStoreDescriptor,
        ) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(false),
                force_active_conflict: false,
                descriptor_override: Some(descriptor),
                helper_path: Some(path.to_owned()),
                state: Mutex::new(TransientProjectionState::default()),
            }
        }

        fn with_descriptor_and_active_conflict(
            path: &Path,
            inner: TantivyProjectionStore,
            descriptor: ProjectionStoreDescriptor,
        ) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(false),
                force_active_conflict: true,
                descriptor_override: Some(descriptor),
                helper_path: Some(path.to_owned()),
                state: Mutex::new(TransientProjectionState::default()),
            }
        }

        fn with_descriptor_without_helper_path(
            inner: TantivyProjectionStore,
            descriptor: ProjectionStoreDescriptor,
        ) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(false),
                force_active_conflict: false,
                descriptor_override: Some(descriptor),
                helper_path: None,
                state: Mutex::new(TransientProjectionState::default()),
            }
        }

        fn acquire_exact_authority_guard(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
            recovery: bool,
        ) -> Result<TestExactAuthorityGuard> {
            let path = self.helper_path.as_deref().ok_or_else(|| {
                KanbanError::Conflict(
                    "transient projection fixture authority has no SQLite/helper path".to_owned(),
                )
            })?;
            let descriptor = self.descriptor()?;
            let policy = if recovery {
                TestAuthorityProviderPolicy::Recovery(&descriptor)
            } else {
                TestAuthorityProviderPolicy::Current(&descriptor)
            };
            acquire_test_exact_authority_guard(
                path,
                "tantivy_tasks-projection-helper",
                TANTIVY_TASKS_STORE,
                generation,
                authority,
                policy,
            )
        }

        fn acquire_overlay_read_guard(&self) -> Result<kanban_local::DerivedStoreReadGuard> {
            let path = self.helper_path.as_deref().ok_or_else(|| {
                KanbanError::Conflict(
                    "transient projection fixture has no SQLite/helper path".to_owned(),
                )
            })?;
            crate::db::acquire_derived_store_read_guard(path, "tantivy_tasks-projection-helper")
        }

        fn generation_path_while_helper_locked(&self, generation: &str) -> Result<PathBuf> {
            let path = self.helper_path.as_deref().ok_or_else(|| {
                KanbanError::Conflict(
                    "transient projection fixture has no SQLite/helper path".to_owned(),
                )
            })?;
            let database_instance_id: String = connect_file(path)?
                .query_row(
                    "SELECT database_instance_id
                     FROM projection_store_state WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                    |row| row.get(0),
                )
                .map_err(storage)?;
            let generations_root = kanban_local::checked_projection_store_generations_path(
                path,
                &database_instance_id,
                TANTIVY_TASKS_STORE,
            )
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
            kanban_local::projection_generation_path(&generations_root, generation)
                .map_err(|error| KanbanError::Storage(error.to_string()))
        }

        fn inspect_physical_generation_while_helper_locked(
            &self,
            generation: &str,
        ) -> Result<Option<TransientPhysicalGeneration>> {
            let generation_path = self.generation_path_while_helper_locked(generation)?;
            match std::fs::symlink_metadata(&generation_path) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    return Err(KanbanError::Storage(format!(
                        "transient physical generation path is not a directory: {}",
                        generation_path.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(KanbanError::Storage(error.to_string())),
            }
            let database_instance_id = self
                .helper_path
                .as_deref()
                .ok_or_else(|| {
                    KanbanError::Conflict(
                        "transient projection fixture has no SQLite/helper path".to_owned(),
                    )
                })
                .and_then(|path| {
                    connect_file(path)?
                        .query_row(
                            "SELECT database_instance_id
                             FROM projection_store_state WHERE store_name=?1",
                            [TANTIVY_TASKS_STORE],
                            |row| row.get::<_, String>(0),
                        )
                        .map_err(storage)
                })?;
            let metadata = kanban_search::tantivy_backend::validate_task_projection_generation(
                &generation_path,
                &database_instance_id,
                generation,
            )
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
            let evidence = evidence_from_tantivy_metadata(metadata);
            let marker_path = generation_path.join("published");
            let marker = match std::fs::symlink_metadata(&marker_path) {
                Ok(metadata) if metadata.is_file() => {
                    let actual = std::fs::read(&marker_path)
                        .map_err(|error| KanbanError::Storage(error.to_string()))?;
                    if actual == transient_published_marker_contents(&evidence) {
                        TransientPhysicalMarker::Exact
                    } else {
                        TransientPhysicalMarker::Invalid
                    }
                }
                Ok(_) => TransientPhysicalMarker::Invalid,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    TransientPhysicalMarker::Missing
                }
                Err(error) => return Err(KanbanError::Storage(error.to_string())),
            };
            Ok(Some(TransientPhysicalGeneration {
                path: generation_path,
                evidence,
                marker,
            }))
        }

        fn inspect_physical_published_while_helper_locked(
            &self,
        ) -> Result<Vec<ProjectionArtifactEvidence>> {
            let sentinel_path = self.generation_path_while_helper_locked("gen_sentinel")?;
            let generations_root = sentinel_path.parent().ok_or_else(|| {
                KanbanError::Storage(
                    "transient projection generations root is unavailable".to_owned(),
                )
            })?;
            let entries = match std::fs::read_dir(generations_root) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Vec::new());
                }
                Err(error) => return Err(KanbanError::Storage(error.to_string())),
            };
            let mut published = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|error| KanbanError::Storage(error.to_string()))?;
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if !entry
                    .file_type()
                    .map_err(|error| KanbanError::Storage(error.to_string()))?
                    .is_dir()
                {
                    continue;
                }
                let marker_path = entry.path().join("published");
                match std::fs::symlink_metadata(&marker_path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(KanbanError::Storage(error.to_string())),
                    Ok(metadata) if metadata.is_file() => {}
                    Ok(_) => {
                        return Err(KanbanError::Storage(format!(
                            "transient physical publication marker is not a regular file: {}",
                            marker_path.display()
                        )));
                    }
                }
                let generation = entry.file_name().to_string_lossy().into_owned();
                let physical = self
                    .inspect_physical_generation_while_helper_locked(&generation)?
                    .ok_or_else(|| {
                        KanbanError::Storage(format!(
                            "transient published physical generation {generation} disappeared"
                        ))
                    })?;
                if physical.marker != TransientPhysicalMarker::Exact {
                    return Err(KanbanError::Storage(format!(
                        "transient physical publication marker is invalid for generation {generation}"
                    )));
                }
                published.push(physical.evidence);
            }
            published.sort_by(|left, right| {
                left.manifest
                    .fence_epoch
                    .cmp(&right.manifest.fence_epoch)
                    .then_with(|| left.manifest.generation.cmp(&right.manifest.generation))
            });
            Ok(published)
        }

        fn validate_snapshot_input_while_helper_locked(
            &self,
            snapshot: &ProjectionSnapshot,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            let path = self.helper_path.as_deref().ok_or_else(|| {
                KanbanError::Conflict(
                    "transient projection fixture has no SQLite/helper path".to_owned(),
                )
            })?;
            let descriptor = self.descriptor()?;
            let (database_instance_id, protocol_version, schema_version, snapshot_cursor) =
                connect_file(path)?
                    .query_row(
                        "SELECT database_instance_id,protocol_version,schema_version,snapshot_cursor
                         FROM projection_store_state WHERE store_name=?1",
                        [TANTIVY_TASKS_STORE],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, i64>(3)?,
                            ))
                        },
                    )
                    .map_err(storage)?;
            let manifest = &snapshot.manifest;
            let binding = &authority.expected_binding;
            if authority.role != ProjectionGenerationRole::Building
                || authority.building_phase.as_deref() != Some("snapshotting")
                || authority.expected_manifest.is_some()
                || manifest.fingerprint.is_some()
                || manifest.store_name != descriptor.store_name
                || manifest.database_instance_id != database_instance_id
                || manifest.protocol_version != protocol_version
                || manifest.schema_version != schema_version
                || manifest.generation != authority.generation
                || manifest.fence_epoch != binding.fence_epoch
                || manifest.snapshot_cursor != snapshot_cursor
                || manifest.provider != descriptor.provider
                || manifest.provider_fingerprint != descriptor.provider_fingerprint
                || manifest.corpus != descriptor.corpus
                || manifest.provider != binding.provider
                || manifest.provider_fingerprint != binding.provider_fingerprint
                || manifest.corpus != binding.corpus
                || manifest.canonical_item_count != binding.canonical_count
                || manifest.canonical_digest != binding.canonical_digest
                || manifest.delivery_item_count != binding.delivery_count
                || manifest.delivery_digest != binding.delivery_digest
                || binding.fingerprint.is_some()
                || binding.snapshot_cursor.is_some()
            {
                return Err(KanbanError::Conflict(format!(
                    "transient projection snapshot input does not match exact authority for generation {}",
                    manifest.generation
                )));
            }
            Ok(())
        }

        fn validate_batch_input_while_helper_locked(
            &self,
            batch: &ProjectionBatch,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            let path = self.helper_path.as_deref().ok_or_else(|| {
                KanbanError::Conflict(
                    "transient projection fixture has no SQLite/helper path".to_owned(),
                )
            })?;
            let descriptor = self.descriptor()?;
            let (database_instance_id, protocol_version, schema_version) = connect_file(path)?
                .query_row(
                    "SELECT database_instance_id,protocol_version,schema_version
                     FROM projection_store_state WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .map_err(storage)?;
            if authority.role != ProjectionGenerationRole::Building
                || authority.building_phase.as_deref() != Some("prepared")
                || batch.store_name != descriptor.store_name
                || batch.database_instance_id != database_instance_id
                || batch.protocol_version != protocol_version
                || batch.schema_version != schema_version
                || batch.provider != descriptor.provider
                || batch.provider_fingerprint != descriptor.provider_fingerprint
                || batch.corpus != descriptor.corpus
                || batch.owner != authority.owner
                || batch.lease_token != authority.lease_token
                || batch.fence_epoch != authority.fence_epoch
                || batch.target_generation != authority.generation
            {
                return Err(KanbanError::Conflict(format!(
                    "transient projection batch input does not match exact authority for generation {}",
                    batch.target_generation
                )));
            }
            Ok(())
        }

        fn validate_evidence_authority_while_helper_locked(
            &self,
            evidence: &ProjectionArtifactEvidence,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            let descriptor = self.descriptor()?;
            if !artifact_matches_descriptor(evidence, &descriptor)
                || authority.expected_manifest.as_ref() != Some(&evidence.manifest)
                || authority.expected_binding != binding_for_evidence(evidence)
            {
                return Err(KanbanError::Conflict(format!(
                    "transient projection evidence does not match exact authority for generation {}",
                    evidence.manifest.generation
                )));
            }
            Ok(())
        }

        fn ensure_prepare_target_absent_while_helper_locked(&self, generation: &str) -> Result<()> {
            let state = self.state.lock().expect("transient projection state");
            let overlay_exists = state.generations.contains_key(generation)
                || state
                    .prepared
                    .as_ref()
                    .is_some_and(|evidence| evidence.manifest.generation == generation)
                || state
                    .active
                    .as_ref()
                    .is_some_and(|evidence| evidence.manifest.generation == generation)
                || state.published.contains(generation);
            drop(state);
            let generation_path = self.generation_path_while_helper_locked(generation)?;
            let physical_exists = match std::fs::symlink_metadata(&generation_path) {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => return Err(KanbanError::Storage(error.to_string())),
            };
            if overlay_exists || physical_exists {
                return Err(KanbanError::Conflict(format!(
                    "transient projection generation {generation} already has overlay or physical evidence"
                )));
            }
            Ok(())
        }

        fn validate_evidence_seam_while_helper_locked(
            &self,
            expected: &ProjectionArtifactEvidence,
            marker_requirement: TransientMarkerRequirement,
        ) -> Result<Option<TransientPhysicalGeneration>> {
            let descriptor = self.descriptor()?;
            if !artifact_matches_descriptor(expected, &descriptor) {
                return Err(KanbanError::Conflict(format!(
                    "transient projection generation {} does not match the provider descriptor",
                    expected.manifest.generation
                )));
            }
            let (overlay, overlay_published) = {
                let state = self.state.lock().expect("transient projection state");
                (
                    state
                        .generations
                        .get(&expected.manifest.generation)
                        .cloned(),
                    state.published.contains(&expected.manifest.generation),
                )
            };
            let physical = self
                .inspect_physical_generation_while_helper_locked(&expected.manifest.generation)?;
            if overlay.is_none() && physical.is_none() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection generation {} has no overlay or physical evidence",
                    expected.manifest.generation
                )));
            }
            if overlay.as_ref().is_some_and(|actual| actual != expected) {
                return Err(KanbanError::Conflict(format!(
                    "transient projection overlay evidence changed for generation {}",
                    expected.manifest.generation
                )));
            }
            if physical
                .as_ref()
                .is_some_and(|actual| actual.evidence != *expected)
            {
                return Err(KanbanError::Conflict(format!(
                    "transient projection physical evidence conflicts with generation {}",
                    expected.manifest.generation
                )));
            }
            match marker_requirement {
                TransientMarkerRequirement::Unpublished => {
                    if overlay_published
                        || physical
                            .as_ref()
                            .is_some_and(|actual| actual.marker != TransientPhysicalMarker::Missing)
                    {
                        return Err(KanbanError::Conflict(format!(
                            "transient projection prepared generation {} is already published",
                            expected.manifest.generation
                        )));
                    }
                }
                TransientMarkerRequirement::Published => {
                    if overlay.is_some() && !overlay_published {
                        return Err(KanbanError::Storage(format!(
                            "transient overlay publication marker is missing for generation {}",
                            expected.manifest.generation
                        )));
                    }
                    if let Some(physical) = &physical
                        && physical.marker != TransientPhysicalMarker::Exact
                    {
                        return Err(KanbanError::Storage(format!(
                            "transient physical publication marker is missing or invalid for generation {}",
                            expected.manifest.generation
                        )));
                    }
                }
                TransientMarkerRequirement::Repairable => {}
            }
            Ok(physical)
        }

        fn effective_active_while_helper_locked(
            &self,
            descriptor: &ProjectionStoreDescriptor,
        ) -> Result<Option<ProjectionArtifactEvidence>> {
            let physical = self.inspect_physical_published_while_helper_locked()?;
            if physical
                .iter()
                .any(|evidence| !artifact_matches_descriptor(evidence, descriptor))
            {
                return Err(KanbanError::Conflict(
                    "strict backend found published evidence from another provider".to_owned(),
                ));
            }
            let (overlay, declared_active) = {
                let state = self.state.lock().expect("transient projection state");
                let mut overlay = Vec::new();
                for generation in &state.published {
                    let evidence = state.generations.get(generation).ok_or_else(|| {
                        KanbanError::Storage(format!(
                            "transient published overlay generation {generation} is missing"
                        ))
                    })?;
                    if !artifact_matches_descriptor(evidence, descriptor) {
                        return Err(KanbanError::Conflict(
                            "strict backend rejected published overlay evidence from another provider"
                                .to_owned(),
                        ));
                    }
                    overlay.push(evidence.clone());
                }
                overlay.sort_by(|left, right| {
                    left.manifest
                        .fence_epoch
                        .cmp(&right.manifest.fence_epoch)
                        .then_with(|| left.manifest.generation.cmp(&right.manifest.generation))
                });
                (overlay, state.active.clone())
            };
            if declared_active != overlay.last().cloned() {
                return Err(KanbanError::Storage(
                    "transient overlay active generation is not consistently published".to_owned(),
                ));
            }
            for overlay_evidence in &overlay {
                self.validate_evidence_seam_while_helper_locked(
                    overlay_evidence,
                    TransientMarkerRequirement::Published,
                )?;
            }
            for physical_evidence in &physical {
                if let Some(overlay_evidence) = overlay.iter().find(|overlay_evidence| {
                    overlay_evidence.manifest.generation == physical_evidence.manifest.generation
                }) && overlay_evidence != physical_evidence
                {
                    return Err(KanbanError::Conflict(format!(
                        "transient overlay and physical evidence disagree for generation {}",
                        physical_evidence.manifest.generation
                    )));
                }
            }
            let mut candidates = physical;
            candidates.extend(overlay);
            candidates.sort_by(|left, right| {
                left.manifest
                    .fence_epoch
                    .cmp(&right.manifest.fence_epoch)
                    .then_with(|| left.manifest.generation.cmp(&right.manifest.generation))
            });
            candidates.dedup();
            Ok(candidates.pop())
        }

        fn repair_physical_publication_while_helper_locked(
            &self,
            physical: &TransientPhysicalGeneration,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            if physical.evidence != *expected {
                return Err(KanbanError::Conflict(format!(
                    "transient physical generation {} changed before publication repair",
                    expected.manifest.generation
                )));
            }
            let marker_path = physical.path.join("published");
            match physical.marker {
                TransientPhysicalMarker::Exact => return Ok(()),
                TransientPhysicalMarker::Missing => {}
                TransientPhysicalMarker::Invalid => {
                    kanban_local::durable_quarantine_entry(&marker_path)
                        .map_err(|error| KanbanError::Storage(error.to_string()))?;
                }
            }
            kanban_local::durable_create_new_file(
                &marker_path,
                &transient_published_marker_contents(expected),
            )
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
            let repaired = self
                .inspect_physical_generation_while_helper_locked(&expected.manifest.generation)?
                .ok_or_else(|| {
                    KanbanError::Storage(format!(
                        "transient physical generation {} disappeared after publication repair",
                        expected.manifest.generation
                    ))
                })?;
            if repaired.evidence != *expected || repaired.marker != TransientPhysicalMarker::Exact {
                return Err(KanbanError::Storage(format!(
                    "transient physical publication repair did not converge for generation {}",
                    expected.manifest.generation
                )));
            }
            Ok(())
        }

        fn prepare_snapshot_while_helper_locked(
            &self,
            snapshot: &ProjectionSnapshot,
        ) -> ProjectionArtifactEvidence {
            let fingerprint = format!("transient:{}", snapshot.manifest.generation);
            let mut manifest = snapshot.manifest.clone();
            manifest.fingerprint = Some(fingerprint.clone());
            let evidence = ProjectionArtifactEvidence {
                manifest,
                fingerprint,
            };
            let mut state = self.state.lock().expect("transient projection state");
            state
                .generations
                .insert(evidence.manifest.generation.clone(), evidence.clone());
            state.prepared = Some(evidence.clone());
            evidence
        }

        fn apply_batch_while_helper_locked(
            &self,
            batch: &ProjectionBatch,
        ) -> ProjectionBatchReceipt {
            ProjectionBatchReceipt {
                store_name: batch.store_name.clone(),
                database_instance_id: batch.database_instance_id.clone(),
                protocol_version: batch.protocol_version,
                schema_version: batch.schema_version,
                provider: batch.provider.clone(),
                provider_fingerprint: batch.provider_fingerprint.clone(),
                target_generation: batch.target_generation.clone(),
                lease_token: batch.lease_token.clone(),
                fence_epoch: batch.fence_epoch,
                claim_token: batch.claim_token.clone(),
                applied_item_count: batch.items.len(),
            }
        }

        fn publish_generation_while_helper_locked(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            let descriptor = self.descriptor()?;
            if expected_active
                .is_some_and(|evidence| !artifact_matches_descriptor(evidence, &descriptor))
                || !artifact_matches_descriptor(prepared, &descriptor)
            {
                return Err(KanbanError::Conflict(
                    "strict backend rejected publish evidence from another provider".to_owned(),
                ));
            }
            let mut state = self.state.lock().expect("transient projection state");
            if state.active.as_ref() != expected_active {
                return Err(KanbanError::Conflict(
                    "transient projection active CAS mismatch".to_owned(),
                ));
            }
            if state.prepared.as_ref() != Some(prepared) {
                return Err(KanbanError::Conflict(
                    "transient projection prepared evidence mismatch".to_owned(),
                ));
            }
            let retained_previous = state.active.clone();
            state.active = Some(prepared.clone());
            state
                .generations
                .insert(prepared.manifest.generation.clone(), prepared.clone());
            state.published.insert(prepared.manifest.generation.clone());
            Ok(ProjectionPublishReceipt {
                active: prepared.clone(),
                retained_previous,
            })
        }

        fn validate_publication_while_helper_locked(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            let state = self.state.lock().expect("transient projection state");
            if let Some(actual) = state.generations.get(&expected.manifest.generation)
                && (actual != expected || !state.published.contains(&expected.manifest.generation))
            {
                return Err(KanbanError::Storage(format!(
                    "transient projection generation {} is not published",
                    expected.manifest.generation
                )));
            }
            Ok(())
        }

        fn repair_publication_while_helper_locked(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            let mut state = self.state.lock().expect("transient projection state");
            if let Some(actual) = state.generations.get(&expected.manifest.generation) {
                if actual != expected {
                    return Err(KanbanError::Conflict(format!(
                        "transient projection generation {} evidence changed before marker repair",
                        expected.manifest.generation
                    )));
                }
                state.published.insert(expected.manifest.generation.clone());
            }
            Ok(())
        }

        fn quarantine_generation_while_helper_locked(&self, generation: &str) -> Result<()> {
            let generation_path = self.generation_path_while_helper_locked(generation)?;
            match std::fs::symlink_metadata(&generation_path) {
                Ok(_) => kanban_local::durable_quarantine_entry(&generation_path)
                    .map(|_| ())
                    .map_err(|error| KanbanError::Storage(error.to_string())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(KanbanError::Storage(error.to_string())),
            }?;
            let mut state = self.state.lock().expect("transient projection state");
            state.generations.remove(generation);
            if state
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.manifest.generation == generation)
            {
                state.prepared = None;
            }
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.generation == generation)
            {
                state.active = None;
            }
            state.published.remove(generation);
            Ok(())
        }

        fn abort_generation_while_helper_locked(&self, generation: &str) -> Result<()> {
            {
                let state = self.state.lock().expect("transient projection state");
                if state.published.contains(generation) {
                    return Err(KanbanError::Conflict(format!(
                        "cannot abort published transient generation {generation}"
                    )));
                }
            }
            let generation_path = self.generation_path_while_helper_locked(generation)?;
            let metadata = match std::fs::symlink_metadata(&generation_path) {
                Ok(metadata) => Some(metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(KanbanError::Storage(error.to_string())),
            };
            if metadata.is_some() {
                match std::fs::symlink_metadata(generation_path.join("published")) {
                    Ok(_) => {
                        return Err(KanbanError::Conflict(format!(
                            "cannot abort published transient generation {generation}"
                        )));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(KanbanError::Storage(error.to_string())),
                }
            }
            if let Some(metadata) = metadata {
                if metadata.is_dir() {
                    kanban_local::durable_remove_directory(&generation_path)
                        .map_err(|error| KanbanError::Storage(error.to_string()))?;
                } else {
                    kanban_local::durable_quarantine_entry(&generation_path)
                        .map(|_| ())
                        .map_err(|error| KanbanError::Storage(error.to_string()))?;
                }
            }
            let mut state = self.state.lock().expect("transient projection state");
            state.generations.remove(generation);
            if state
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.manifest.generation == generation)
            {
                state.prepared = None;
            }
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.manifest.generation == generation)
            {
                state.active = None;
            }
            state.published.remove(generation);
            Ok(())
        }
    }

    impl ProjectionStoreBackend for TransientGenerationInspectStore {
        fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
            self.descriptor_override
                .clone()
                .map_or_else(|| self.inner.descriptor(), Ok)
        }

        fn prepare_snapshot(
            &self,
            snapshot: &ProjectionSnapshot,
        ) -> Result<ProjectionArtifactEvidence> {
            if self.descriptor_override.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection provider requires authority-bearing snapshot preparation for generation {}",
                    snapshot.manifest.generation
                )));
            }
            self.inner.prepare_snapshot(snapshot)
        }

        fn prepare_snapshot_with_authority(
            &self,
            snapshot: &ProjectionSnapshot,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionArtifactEvidence> {
            if self.descriptor_override.is_none() {
                return self
                    .inner
                    .prepare_snapshot_with_authority(snapshot, authority);
            }
            let _authority_guard = self.acquire_exact_authority_guard(
                &snapshot.manifest.generation,
                authority,
                false,
            )?;
            self.validate_snapshot_input_while_helper_locked(snapshot, authority)?;
            self.ensure_prepare_target_absent_while_helper_locked(&snapshot.manifest.generation)?;
            Ok(self.prepare_snapshot_while_helper_locked(snapshot))
        }

        fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
            if self.descriptor_override.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection provider requires authority-bearing batch apply for generation {}",
                    batch.target_generation
                )));
            }
            self.inner.apply_batch(batch)
        }

        fn apply_batch_with_authority(
            &self,
            batch: &ProjectionBatch,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionBatchReceipt> {
            if self.descriptor_override.is_none() {
                return self.inner.apply_batch_with_authority(batch, authority);
            }
            let _authority_guard =
                self.acquire_exact_authority_guard(&batch.target_generation, authority, false)?;
            self.validate_batch_input_while_helper_locked(batch, authority)?;
            let expected = evidence_from_authority(authority)?;
            self.validate_evidence_seam_while_helper_locked(
                &expected,
                TransientMarkerRequirement::Unpublished,
            )?;
            Ok(self.apply_batch_while_helper_locked(batch))
        }

        fn publish_generation(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            if self.descriptor_override.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection provider requires authority-bearing publication for generation {}",
                    prepared.manifest.generation
                )));
            }
            self.inner.publish_generation(expected_active, prepared)
        }

        fn publish_generation_with_authority(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<ProjectionPublishReceipt> {
            if self.descriptor_override.is_none() {
                return self.inner.publish_generation_with_authority(
                    expected_active,
                    prepared,
                    authority,
                );
            }
            let _authority_guard = self.acquire_exact_authority_guard(
                &prepared.manifest.generation,
                authority,
                false,
            )?;
            if authority.role != ProjectionGenerationRole::Building
                || authority.building_phase.as_deref() != Some("prepared")
            {
                return Err(KanbanError::Conflict(format!(
                    "transient projection publication requires a prepared building generation {}",
                    prepared.manifest.generation
                )));
            }
            self.validate_evidence_authority_while_helper_locked(prepared, authority)?;
            let descriptor = self.descriptor()?;
            let actual_active = self.effective_active_while_helper_locked(&descriptor)?;
            if actual_active.as_ref() != expected_active {
                return Err(KanbanError::Conflict(
                    "transient projection active generation changed before publish".to_owned(),
                ));
            }
            let physical = self.validate_evidence_seam_while_helper_locked(
                prepared,
                TransientMarkerRequirement::Repairable,
            )?;
            if let Some(physical) = &physical {
                self.repair_physical_publication_while_helper_locked(physical, prepared)?;
            }
            {
                let mut state = self.state.lock().expect("transient projection state");
                if state.prepared.is_none() {
                    state.prepared = Some(prepared.clone());
                    state
                        .generations
                        .insert(prepared.manifest.generation.clone(), prepared.clone());
                }
                if state.active.is_none()
                    && let Some(expected_active) = expected_active
                {
                    state.active = Some(expected_active.clone());
                    state.generations.insert(
                        expected_active.manifest.generation.clone(),
                        expected_active.clone(),
                    );
                    state
                        .published
                        .insert(expected_active.manifest.generation.clone());
                }
            }
            self.publish_generation_while_helper_locked(expected_active, prepared)
        }

        fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
            if self.force_active_conflict {
                return Err(KanbanError::Conflict(
                    "strict backend found an unattributed incompatible active generation"
                        .to_owned(),
                ));
            }
            let Some(descriptor) = &self.descriptor_override else {
                return self.inner.inspect_active();
            };
            let _authority_guard = self.acquire_overlay_read_guard()?;
            self.effective_active_while_helper_locked(descriptor)
        }

        fn inspect_generation(
            &self,
            generation: &str,
        ) -> Result<Option<ProjectionArtifactEvidence>> {
            if self.fail_next_inspect.swap(false, Ordering::SeqCst) {
                return Err(KanbanError::Storage(
                    "transient prepared generation inspection failure".to_owned(),
                ));
            }
            let Some(descriptor) = &self.descriptor_override else {
                return self.inner.inspect_generation(generation);
            };
            let _authority_guard = self.acquire_overlay_read_guard()?;
            let overlay = self
                .state
                .lock()
                .expect("transient projection state")
                .generations
                .get(generation)
                .cloned();
            let physical = self.inspect_physical_generation_while_helper_locked(generation)?;
            if overlay
                .as_ref()
                .is_some_and(|evidence| !artifact_matches_descriptor(evidence, descriptor))
                || physical.as_ref().is_some_and(|physical| {
                    !artifact_matches_descriptor(&physical.evidence, descriptor)
                })
            {
                return Err(KanbanError::Conflict(
                    "strict backend rejected generation evidence from another provider".to_owned(),
                ));
            }
            if let (Some(overlay), Some(physical)) = (&overlay, &physical)
                && *overlay != physical.evidence
            {
                return Err(KanbanError::Conflict(format!(
                    "transient overlay and physical evidence disagree for generation {generation}"
                )));
            }
            Ok(overlay.or_else(|| physical.map(|physical| physical.evidence)))
        }

        fn validate_generation_publication(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            if self.descriptor_override.is_none() {
                return self.inner.validate_generation_publication(expected);
            }
            let _authority_guard = self.acquire_overlay_read_guard()?;
            self.validate_evidence_seam_while_helper_locked(
                expected,
                TransientMarkerRequirement::Published,
            )?;
            self.validate_publication_while_helper_locked(expected)
        }

        fn validate_generation_publication_with_authority(
            &self,
            expected: &ProjectionArtifactEvidence,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            if self.descriptor_override.is_none() {
                return self
                    .inner
                    .validate_generation_publication_with_authority(expected, authority);
            }
            let _authority_guard = self.acquire_exact_authority_guard(
                &expected.manifest.generation,
                authority,
                false,
            )?;
            self.validate_evidence_authority_while_helper_locked(expected, authority)?;
            self.validate_evidence_seam_while_helper_locked(
                expected,
                TransientMarkerRequirement::Published,
            )?;
            self.validate_publication_while_helper_locked(expected)
        }

        fn repair_generation_publication(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            if self.descriptor_override.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection provider requires authority-bearing publication repair for generation {}",
                    expected.manifest.generation
                )));
            }
            self.inner.repair_generation_publication(expected)
        }

        fn repair_generation_publication_with_authority(
            &self,
            expected: &ProjectionArtifactEvidence,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            if self.descriptor_override.is_none() {
                return self
                    .inner
                    .repair_generation_publication_with_authority(expected, authority);
            }
            let _authority_guard = self.acquire_exact_authority_guard(
                &expected.manifest.generation,
                authority,
                false,
            )?;
            self.validate_evidence_authority_while_helper_locked(expected, authority)?;
            let physical = self.validate_evidence_seam_while_helper_locked(
                expected,
                TransientMarkerRequirement::Repairable,
            )?;
            if let Some(physical) = &physical {
                self.repair_physical_publication_while_helper_locked(physical, expected)?;
            }
            self.repair_publication_while_helper_locked(expected)
        }

        fn validate_active_contents(&self, active: &ProjectionArtifactEvidence) -> Result<()> {
            if self.descriptor_override.is_none() {
                return self.inner.validate_active_contents(active);
            }
            let _authority_guard = self.acquire_overlay_read_guard()?;
            let descriptor = self.descriptor()?;
            if !artifact_matches_descriptor(active, &descriptor) {
                return Err(KanbanError::Conflict(
                    "strict backend rejected active evidence from another provider".to_owned(),
                ));
            }
            self.validate_evidence_seam_while_helper_locked(
                active,
                TransientMarkerRequirement::Published,
            )?;
            if self
                .effective_active_while_helper_locked(&descriptor)?
                .as_ref()
                != Some(active)
            {
                return Err(KanbanError::Storage(
                    "transient projection active contents changed".to_owned(),
                ));
            }
            Ok(())
        }

        fn quarantine_generation(&self, generation: &str) -> Result<()> {
            if self.descriptor_override.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection provider requires fenced quarantine for generation {generation}"
                )));
            }
            self.inner.quarantine_generation(generation)
        }

        fn abort_generation(&self, generation: &str) -> Result<()> {
            if self.descriptor_override.is_some() {
                return Err(KanbanError::Conflict(format!(
                    "transient projection provider requires fenced abort for generation {generation}"
                )));
            }
            self.inner.abort_generation(generation)
        }

        fn quarantine_generation_fenced(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            if self.descriptor_override.is_none() {
                return self
                    .inner
                    .quarantine_generation_fenced(generation, authority);
            }
            let authority_guard =
                self.acquire_exact_authority_guard(generation, authority, true)?;
            if authority_guard.role == ProjectionGenerationRole::Active
                && authority_guard.current_provider_binding
            {
                let expected = evidence_from_authority(authority)?;
                let (overlay_present, overlay_exact) = {
                    let state = self.state.lock().expect("transient projection state");
                    let active = state
                        .active
                        .as_ref()
                        .filter(|active| active.manifest.generation == generation);
                    let stored = state.generations.get(generation);
                    (
                        active.is_some() || stored.is_some(),
                        active == Some(&expected) && stored == Some(&expected),
                    )
                };
                let generation_path = self.generation_path_while_helper_locked(generation)?;
                let physical_present = match std::fs::symlink_metadata(&generation_path) {
                    Ok(_) => true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                    Err(error) => {
                        return Err(KanbanError::Storage(error.to_string()));
                    }
                };
                let physical_exact = if physical_present {
                    matches!(
                        self.inspect_physical_generation_while_helper_locked(generation),
                        Ok(Some(physical)) if physical.evidence == expected
                    )
                } else {
                    false
                };
                if (overlay_present || physical_present)
                    && (!overlay_present || overlay_exact)
                    && (!physical_present || physical_exact)
                {
                    return Err(KanbanError::Conflict(format!(
                        "cannot quarantine canonical active transient generation {generation}"
                    )));
                }
            }
            self.quarantine_generation_while_helper_locked(generation)
        }

        fn abort_generation_fenced(
            &self,
            generation: &str,
            authority: &ProjectionDestructiveAuthority,
        ) -> Result<()> {
            if self.descriptor_override.is_none() {
                return self.inner.abort_generation_fenced(generation, authority);
            }
            let authority_guard =
                self.acquire_exact_authority_guard(generation, authority, true)?;
            if authority_guard.role != ProjectionGenerationRole::Building
                || !matches!(
                    authority.building_phase.as_deref(),
                    Some("snapshotting" | "prepared")
                )
            {
                return Err(KanbanError::Conflict(format!(
                    "cannot abort non-building transient generation {generation}"
                )));
            }
            self.abort_generation_while_helper_locked(generation)
        }
    }

    fn artifact_matches_descriptor(
        evidence: &ProjectionArtifactEvidence,
        descriptor: &ProjectionStoreDescriptor,
    ) -> bool {
        evidence.manifest.store_name == descriptor.store_name
            && evidence.manifest.provider == descriptor.provider
            && evidence.manifest.provider_fingerprint == descriptor.provider_fingerprint
            && evidence.manifest.corpus == descriptor.corpus
    }

    fn binding_for_evidence(evidence: &ProjectionArtifactEvidence) -> ProjectionGenerationBinding {
        ProjectionGenerationBinding {
            generation: evidence.manifest.generation.clone(),
            fingerprint: Some(evidence.fingerprint.clone()),
            fence_epoch: evidence.manifest.fence_epoch,
            snapshot_cursor: Some(evidence.manifest.snapshot_cursor),
            provider: evidence.manifest.provider.clone(),
            provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
            canonical_count: evidence.manifest.canonical_item_count,
            canonical_digest: evidence.manifest.canonical_digest.clone(),
            delivery_count: evidence.manifest.delivery_item_count,
            delivery_digest: evidence.manifest.delivery_digest.clone(),
            corpus: evidence.manifest.corpus.clone(),
        }
    }

    fn evidence_from_authority(
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionArtifactEvidence> {
        let manifest = authority.expected_manifest.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "transient projection authority has no manifest for generation {}",
                authority.generation
            ))
        })?;
        let fingerprint = authority
            .expected_binding
            .fingerprint
            .clone()
            .ok_or_else(|| {
                KanbanError::Conflict(format!(
                    "transient projection authority has no fingerprint for generation {}",
                    authority.generation
                ))
            })?;
        let evidence = ProjectionArtifactEvidence {
            manifest,
            fingerprint,
        };
        if binding_for_evidence(&evidence) != authority.expected_binding {
            return Err(KanbanError::Conflict(format!(
                "transient projection authority evidence is inconsistent for generation {}",
                authority.generation
            )));
        }
        Ok(evidence)
    }

    fn evidence_from_tantivy_metadata(
        metadata: kanban_search::tantivy_backend::TantivyTaskProjectionMetadata,
    ) -> ProjectionArtifactEvidence {
        ProjectionArtifactEvidence {
            fingerprint: metadata.fingerprint.clone(),
            manifest: ProjectionArtifactManifest {
                store_name: TANTIVY_TASKS_STORE.to_owned(),
                database_instance_id: metadata.database_instance_id,
                protocol_version: metadata.protocol_version,
                schema_version: metadata.schema_version,
                generation: metadata.generation,
                fence_epoch: metadata.fence_epoch,
                snapshot_cursor: metadata.snapshot_cursor,
                provider: metadata.provider,
                provider_fingerprint: metadata.provider_fingerprint,
                corpus: None,
                canonical_item_count: metadata.canonical_item_count,
                canonical_digest: metadata.canonical_digest,
                delivery_item_count: metadata.delivery_item_count,
                delivery_digest: metadata.delivery_digest,
                fingerprint: Some(metadata.fingerprint),
            },
        }
    }

    fn transient_published_marker_contents(evidence: &ProjectionArtifactEvidence) -> Vec<u8> {
        format!(
            "database_instance_id={}\ngeneration={}\nfence_epoch={}\n",
            evidence.manifest.database_instance_id,
            evidence.manifest.generation,
            evidence.manifest.fence_epoch
        )
        .into_bytes()
    }

    fn tantivy_metadata_for_evidence(
        evidence: &ProjectionArtifactEvidence,
    ) -> kanban_search::tantivy_backend::TantivyTaskProjectionMetadata {
        kanban_search::tantivy_backend::TantivyTaskProjectionMetadata {
            database_instance_id: evidence.manifest.database_instance_id.clone(),
            protocol_version: evidence.manifest.protocol_version,
            schema_version: evidence.manifest.schema_version,
            generation: evidence.manifest.generation.clone(),
            fence_epoch: evidence.manifest.fence_epoch,
            snapshot_cursor: evidence.manifest.snapshot_cursor,
            provider: evidence.manifest.provider.clone(),
            provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
            canonical_item_count: evidence.manifest.canonical_item_count,
            canonical_digest: evidence.manifest.canonical_digest.clone(),
            delivery_item_count: evidence.manifest.delivery_item_count,
            delivery_digest: evidence.manifest.delivery_digest.clone(),
            fingerprint: evidence.fingerprint.clone(),
        }
    }

    fn quarantined_generation_path(generation_path: &Path) -> anyhow::Result<PathBuf> {
        let parent = generation_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("generation has no parent"))?;
        let generation = generation_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("generation has no file name"))?
            .to_string_lossy();
        let prefix = format!(".{generation}.quarantine.");
        let matches = std::fs::read_dir(parent)?
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        anyhow::ensure!(
            matches.len() == 1,
            "expected one quarantine sibling for {generation}, got {}",
            matches.len()
        );
        Ok(matches.into_iter().next().expect("one quarantine sibling"))
    }

    fn snapshotting_authority(
        manifest: &ProjectionArtifactManifest,
        lease: &ProjectionLease,
    ) -> ProjectionDestructiveAuthority {
        ProjectionDestructiveAuthority {
            owner: lease.owner.clone(),
            lease_token: lease.lease_token.clone(),
            fence_epoch: lease.fence_epoch,
            lease_expires_at: lease.lease_expires_at,
            role: ProjectionGenerationRole::Building,
            generation: manifest.generation.clone(),
            expected_manifest: None,
            expected_binding: ProjectionGenerationBinding {
                generation: manifest.generation.clone(),
                fingerprint: None,
                fence_epoch: manifest.fence_epoch,
                snapshot_cursor: None,
                provider: manifest.provider.clone(),
                provider_fingerprint: manifest.provider_fingerprint.clone(),
                canonical_count: manifest.canonical_item_count,
                canonical_digest: manifest.canonical_digest.clone(),
                delivery_count: manifest.delivery_item_count,
                delivery_digest: manifest.delivery_digest.clone(),
                corpus: manifest.corpus.clone(),
            },
            building_phase: Some("snapshotting".to_owned()),
        }
    }

    fn evidence_authority(
        evidence: &ProjectionArtifactEvidence,
        lease: &ProjectionLease,
        role: ProjectionGenerationRole,
        building_phase: Option<&str>,
    ) -> ProjectionDestructiveAuthority {
        ProjectionDestructiveAuthority {
            owner: lease.owner.clone(),
            lease_token: lease.lease_token.clone(),
            fence_epoch: lease.fence_epoch,
            lease_expires_at: lease.lease_expires_at,
            role,
            generation: evidence.manifest.generation.clone(),
            expected_manifest: Some(evidence.manifest.clone()),
            expected_binding: binding_for_evidence(evidence),
            building_phase: building_phase.map(str::to_owned),
        }
    }

    fn empty_projection_batch(
        manifest: &ProjectionArtifactManifest,
        lease: &ProjectionLease,
    ) -> ProjectionBatch {
        ProjectionBatch {
            store_name: manifest.store_name.clone(),
            database_instance_id: manifest.database_instance_id.clone(),
            protocol_version: manifest.protocol_version,
            schema_version: manifest.schema_version,
            provider: manifest.provider.clone(),
            provider_fingerprint: manifest.provider_fingerprint.clone(),
            corpus: manifest.corpus.clone(),
            owner: lease.owner.clone(),
            lease_token: lease.lease_token.clone(),
            fence_epoch: lease.fence_epoch,
            target_generation: manifest.generation.clone(),
            claim_token: "transient-test-claim-token".to_owned(),
            claim_expires_at: lease.lease_expires_at,
            items: Vec::new(),
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum TransientAuthorityDrift {
        EmptyGeneration,
        EmptyOwner,
        EmptyToken,
        NegativeAuthorityFence,
        ExpiredAuthority,
        SameIdentityFenceRollover,
        OwnerTokenHandoff,
        ExpiredLiveLease,
        FutureBindingFence,
        WrongRole,
        WrongPhase,
        WrongBinding,
        WrongManifest,
        DatabaseMismatch,
        ProtocolMismatch,
        SchemaMismatch,
        ControlPlaneMismatch,
    }

    const TRANSIENT_AUTHORITY_DRIFTS: [TransientAuthorityDrift; 17] = [
        TransientAuthorityDrift::EmptyGeneration,
        TransientAuthorityDrift::EmptyOwner,
        TransientAuthorityDrift::EmptyToken,
        TransientAuthorityDrift::NegativeAuthorityFence,
        TransientAuthorityDrift::ExpiredAuthority,
        TransientAuthorityDrift::SameIdentityFenceRollover,
        TransientAuthorityDrift::OwnerTokenHandoff,
        TransientAuthorityDrift::ExpiredLiveLease,
        TransientAuthorityDrift::FutureBindingFence,
        TransientAuthorityDrift::WrongRole,
        TransientAuthorityDrift::WrongPhase,
        TransientAuthorityDrift::WrongBinding,
        TransientAuthorityDrift::WrongManifest,
        TransientAuthorityDrift::DatabaseMismatch,
        TransientAuthorityDrift::ProtocolMismatch,
        TransientAuthorityDrift::SchemaMismatch,
        TransientAuthorityDrift::ControlPlaneMismatch,
    ];

    fn apply_transient_building_drift(
        path: &Path,
        manifest: &ProjectionArtifactManifest,
        lease: &ProjectionLease,
        live_phase: &str,
        authority: &mut ProjectionDestructiveAuthority,
        drift: TransientAuthorityDrift,
    ) -> anyhow::Result<()> {
        match drift {
            TransientAuthorityDrift::EmptyGeneration => authority.generation.clear(),
            TransientAuthorityDrift::EmptyOwner => authority.owner.clear(),
            TransientAuthorityDrift::EmptyToken => authority.lease_token.clear(),
            TransientAuthorityDrift::NegativeAuthorityFence => authority.fence_epoch = -1,
            TransientAuthorityDrift::ExpiredAuthority => authority.lease_expires_at = 0,
            TransientAuthorityDrift::SameIdentityFenceRollover => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET fence_epoch=fence_epoch+1
                     WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::OwnerTokenHandoff => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET lease_owner='successor-owner',
                         lease_token='please_successor_token',
                         lease_expires_at=?1,fence_epoch=fence_epoch+1
                     WHERE store_name=?2",
                    params![SystemClock.now_ms() + 120_000, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::ExpiredLiveLease => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET lease_expires_at=0
                     WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::FutureBindingFence => {
                let future_fence = lease.fence_epoch + 1;
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET building_fence_epoch=?1
                     WHERE store_name=?2",
                    params![future_fence, TANTIVY_TASKS_STORE],
                )?;
                authority.expected_binding.fence_epoch = future_fence;
                if let Some(expected_manifest) = &mut authority.expected_manifest {
                    expected_manifest.fence_epoch = future_fence;
                }
            }
            TransientAuthorityDrift::WrongRole => {
                authority.role = ProjectionGenerationRole::Active;
            }
            TransientAuthorityDrift::WrongPhase => {
                authority.building_phase = Some(
                    if live_phase == "snapshotting" {
                        "prepared"
                    } else {
                        "snapshotting"
                    }
                    .to_owned(),
                );
            }
            TransientAuthorityDrift::WrongBinding => {
                authority.expected_binding.provider = "wrong-provider".to_owned();
            }
            TransientAuthorityDrift::WrongManifest => {
                if let Some(expected_manifest) = &mut authority.expected_manifest {
                    expected_manifest.database_instance_id = "db_wrong_manifest".to_owned();
                } else {
                    authority.expected_manifest = Some(manifest.clone());
                }
            }
            TransientAuthorityDrift::DatabaseMismatch => {
                let conn = connect_file(path)?;
                conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
                conn.execute(
                    "UPDATE projection_store_state
                     SET database_instance_id='db_mismatched_store'
                     WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::ProtocolMismatch => {
                let conn = connect_file(path)?;
                conn.execute_batch("PRAGMA ignore_check_constraints=ON;")?;
                conn.execute(
                    "UPDATE projection_store_state
                     SET protocol_version=?1
                     WHERE store_name=?2",
                    params![manifest.protocol_version + 1, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::SchemaMismatch => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET schema_version=?1
                     WHERE store_name=?2",
                    params![manifest.schema_version + 1, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::ControlPlaneMismatch => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET control_plane='legacy'
                     WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                )?;
            }
        }
        Ok(())
    }

    fn restore_transient_building_drift(
        path: &Path,
        manifest: &ProjectionArtifactManifest,
        lease: &ProjectionLease,
        drift: TransientAuthorityDrift,
    ) -> anyhow::Result<()> {
        match drift {
            TransientAuthorityDrift::SameIdentityFenceRollover => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET fence_epoch=?1
                     WHERE store_name=?2",
                    params![lease.fence_epoch, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::OwnerTokenHandoff => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET lease_owner=?1,lease_token=?2,lease_expires_at=?3,
                         fence_epoch=?4
                    WHERE store_name=?5",
                    params![
                        lease.owner.as_str(),
                        lease.lease_token.as_str(),
                        lease.lease_expires_at,
                        lease.fence_epoch,
                        TANTIVY_TASKS_STORE
                    ],
                )?;
            }
            TransientAuthorityDrift::ExpiredLiveLease => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET lease_expires_at=?1
                     WHERE store_name=?2",
                    params![lease.lease_expires_at, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::FutureBindingFence => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET building_fence_epoch=?1
                     WHERE store_name=?2",
                    params![manifest.fence_epoch, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::DatabaseMismatch => {
                let conn = connect_file(path)?;
                conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
                conn.execute(
                    "UPDATE projection_store_state
                     SET database_instance_id=?1
                     WHERE store_name=?2",
                    params![manifest.database_instance_id.as_str(), TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::ProtocolMismatch => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET protocol_version=?1
                     WHERE store_name=?2",
                    params![manifest.protocol_version, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::SchemaMismatch => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET schema_version=?1
                     WHERE store_name=?2",
                    params![manifest.schema_version, TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::ControlPlaneMismatch => {
                connect_file(path)?.execute(
                    "UPDATE projection_store_state
                     SET control_plane='v2'
                     WHERE store_name=?1",
                    [TANTIVY_TASKS_STORE],
                )?;
            }
            TransientAuthorityDrift::EmptyGeneration
            | TransientAuthorityDrift::EmptyOwner
            | TransientAuthorityDrift::EmptyToken
            | TransientAuthorityDrift::NegativeAuthorityFence
            | TransientAuthorityDrift::ExpiredAuthority
            | TransientAuthorityDrift::WrongRole
            | TransientAuthorityDrift::WrongPhase
            | TransientAuthorityDrift::WrongBinding
            | TransientAuthorityDrift::WrongManifest => {}
        }
        Ok(())
    }

    #[test]
    fn transient_authority_rejects_same_identity_fence_rollover_before_prepare_mutation()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-authority-red.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let backend = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            provider_a,
            descriptor.clone(),
        );
        let lease = acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "same-owner", 20_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "same-owner",
            &lease.lease_token,
            &backend,
        )?;
        let snapshot = ProjectionSnapshot {
            manifest: manifest.clone(),
            records: Vec::new(),
        };
        let authority = snapshotting_authority(&manifest, &lease);
        connect_file(&db_path)?.execute(
            "UPDATE projection_store_state
             SET fence_epoch=fence_epoch+1
             WHERE store_name=?1",
            [TANTIVY_TASKS_STORE],
        )?;

        let error = backend
            .prepare_snapshot_with_authority(&snapshot, &authority)
            .expect_err("same-owner/token fence rollover must reject stale prepare authority");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(
            backend
                .inner
                .inspect_generation(&manifest.generation)?
                .is_none(),
            "stale authority must not create physical generation evidence"
        );
        Ok(())
    }

    #[test]
    fn transient_current_authority_negative_matrix_preserves_physical_state() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-current-authority-matrix.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let backend =
            TransientGenerationInspectStore::with_descriptor(&db_path, provider_a, descriptor);
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "matrix-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &backend,
        )?;
        let snapshot = ProjectionSnapshot {
            manifest: manifest.clone(),
            records: Vec::new(),
        };
        let base_authority = snapshotting_authority(&manifest, &lease);

        for drift in TRANSIENT_AUTHORITY_DRIFTS {
            let mut authority = base_authority.clone();
            apply_transient_building_drift(
                &db_path,
                &manifest,
                &lease,
                "snapshotting",
                &mut authority,
                drift,
            )?;

            let error = backend
                .prepare_snapshot_with_authority(&snapshot, &authority)
                .expect_err("every stale or incomplete current authority must fail closed");
            assert!(
                matches!(error, KanbanError::Conflict(_)),
                "{drift:?} returned {error:?}"
            );
            assert!(
                backend
                    .state
                    .lock()
                    .expect("transient projection state")
                    .generations
                    .is_empty(),
                "{drift:?} reached the transient mutator"
            );
            assert!(
                backend
                    .inner
                    .inspect_generation(&manifest.generation)?
                    .is_none(),
                "{drift:?} changed physical generation evidence"
            );

            restore_transient_building_drift(&db_path, &manifest, &lease, drift)?;
        }
        Ok(())
    }

    #[test]
    fn transient_authority_surface_matrix_rejects_stale_fence_before_mutation() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-authority-surface-matrix.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let backend =
            TransientGenerationInspectStore::with_descriptor(&db_path, provider_a, descriptor);
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "surface-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &backend,
        )?;
        let snapshot = ProjectionSnapshot {
            manifest: manifest.clone(),
            records: Vec::new(),
        };
        let snapshotting_authority = snapshotting_authority(&manifest, &lease);
        let evidence = prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &backend,
        )?;
        let batch = empty_projection_batch(&evidence.manifest, &lease);
        let prepared_authority = evidence_authority(
            &evidence,
            &lease,
            ProjectionGenerationRole::Building,
            Some("prepared"),
        );
        let (generations_before, prepared_before, active_before, published_before) = {
            let state = backend.state.lock().expect("transient projection state");
            (
                state.generations.clone(),
                state.prepared.clone(),
                state.active.clone(),
                state.published.clone(),
            )
        };
        connect_file(&db_path)?.execute(
            "UPDATE projection_store_state
             SET fence_epoch=fence_epoch+1
             WHERE store_name=?1",
            [TANTIVY_TASKS_STORE],
        )?;

        macro_rules! assert_stale_conflict {
            ($label:literal, $operation:expr) => {
                let error = $operation.expect_err(concat!(
                    $label,
                    " must reject stale exact authority before mutation"
                ));
                assert!(
                    matches!(error, KanbanError::Conflict(_)),
                    "{} returned {error:?}",
                    $label
                );
            };
        }

        assert_stale_conflict!(
            "prepare",
            backend.prepare_snapshot_with_authority(&snapshot, &snapshotting_authority)
        );
        assert_stale_conflict!(
            "apply",
            backend.apply_batch_with_authority(&batch, &prepared_authority)
        );
        assert_stale_conflict!(
            "publish",
            backend.publish_generation_with_authority(None, &evidence, &prepared_authority)
        );
        assert_stale_conflict!(
            "validate publication",
            backend.validate_generation_publication_with_authority(&evidence, &prepared_authority)
        );
        assert_stale_conflict!(
            "repair publication",
            backend.repair_generation_publication_with_authority(&evidence, &prepared_authority)
        );
        assert_stale_conflict!(
            "quarantine",
            backend.quarantine_generation_fenced(&manifest.generation, &prepared_authority)
        );
        assert_stale_conflict!(
            "abort",
            backend.abort_generation_fenced(&manifest.generation, &prepared_authority)
        );
        let state = backend.state.lock().expect("transient projection state");
        assert_eq!(state.generations, generations_before);
        assert_eq!(state.prepared, prepared_before);
        assert_eq!(state.active, active_before);
        assert_eq!(state.published, published_before);
        drop(state);
        assert!(
            backend
                .inner
                .inspect_generation(&manifest.generation)?
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn transient_authority_without_helper_path_fails_closed() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-authority-no-helper.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let backend = TransientGenerationInspectStore::with_descriptor_without_helper_path(
            provider_a, descriptor,
        );
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "no-helper-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &backend,
        )?;
        let snapshot = ProjectionSnapshot {
            manifest: manifest.clone(),
            records: Vec::new(),
        };
        let authority = snapshotting_authority(&manifest, &lease);

        let error = backend
            .prepare_snapshot_with_authority(&snapshot, &authority)
            .expect_err("authority mutation without a helper path must fail closed");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(
            backend
                .state
                .lock()
                .expect("transient projection state")
                .generations
                .is_empty()
        );
        assert!(
            backend
                .inner
                .inspect_generation(&manifest.generation)?
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn transient_current_authority_rejects_live_provider_mismatch() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-current-provider-mismatch.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "provider-a-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_a,
        )?;
        let snapshot = ProjectionSnapshot {
            manifest: manifest.clone(),
            records: Vec::new(),
        };
        let authority = snapshotting_authority(&manifest, &lease);

        let mut provider_b_descriptor = provider_a.descriptor()?;
        provider_b_descriptor.provider = "tantivy-provider-b".to_owned();
        provider_b_descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            TantivyProjectionStore::new(&db_path)?,
            provider_b_descriptor,
        );
        let error = provider_b
            .prepare_snapshot_with_authority(&snapshot, &authority)
            .expect_err("current authority must not adopt another live provider binding");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(
            provider_b
                .state
                .lock()
                .expect("transient projection state")
                .generations
                .is_empty()
        );
        assert!(
            provider_b
                .inner
                .inspect_generation(&manifest.generation)?
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn transient_abort_rejects_store_published_and_any_physical_marker_entry() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-abort-published.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let provider_a_lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "provider-a-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &provider_a_lease.owner,
            &provider_a_lease.lease_token,
            &provider_a,
        )?;
        let evidence = prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &provider_a_lease.owner,
            &provider_a_lease.lease_token,
            &provider_a,
        )?;
        let status = projection_status(&db_path)?;
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&manifest.generation);
        let marker_path = generation_path.join("published");
        std::fs::create_dir(&marker_path)?;
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            &provider_a_lease.owner,
            &provider_a_lease.lease_token,
        )?;

        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            TantivyProjectionStore::new(&db_path)?,
            descriptor,
        );
        let provider_b_lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "provider-b-owner", 120_000)?;
        let prepared_authority = evidence_authority(
            &evidence,
            &provider_b_lease,
            ProjectionGenerationRole::Building,
            Some("prepared"),
        );
        let marker_error = provider_b
            .abort_generation_fenced(&manifest.generation, &prepared_authority)
            .expect_err("any physical publication marker entry must block abort");
        assert!(matches!(marker_error, KanbanError::Conflict(_)));
        assert!(generation_path.is_dir());
        assert!(std::fs::symlink_metadata(&marker_path)?.is_dir());

        connect_file(&db_path)?.execute(
            "UPDATE projection_store_state
             SET building_phase='store_published'
             WHERE store_name=?1",
            [TANTIVY_TASKS_STORE],
        )?;
        let store_published_authority = evidence_authority(
            &evidence,
            &provider_b_lease,
            ProjectionGenerationRole::Building,
            Some("store_published"),
        );
        let phase_error = provider_b
            .abort_generation_fenced(&manifest.generation, &store_published_authority)
            .expect_err("store-published building evidence must not be abortable");
        assert!(matches!(phase_error, KanbanError::Conflict(_)));
        assert!(generation_path.is_dir());
        assert!(std::fs::symlink_metadata(&marker_path)?.is_dir());
        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    enum TransientCoexistSurface {
        Prepare,
        Apply,
        Publish,
        Validate,
        Repair,
        Quarantine,
        Abort,
    }

    #[test]
    fn transient_authority_surface_matrix_reconciles_overlay_and_foreign_physical_coexistence()
    -> anyhow::Result<()> {
        for surface in [
            TransientCoexistSurface::Prepare,
            TransientCoexistSurface::Apply,
            TransientCoexistSurface::Publish,
            TransientCoexistSurface::Validate,
            TransientCoexistSurface::Repair,
            TransientCoexistSurface::Quarantine,
            TransientCoexistSurface::Abort,
        ] {
            let temp = tempdir()?;
            let db_path = temp
                .path()
                .join(format!("transient-coexist-{surface:?}.db"));
            init_database(&db_path, "tester")?;
            let provider_a = TantivyProjectionStore::new(&db_path)?;
            let provider_a_descriptor = provider_a.descriptor()?;
            let mut provider_b_descriptor = provider_a_descriptor.clone();
            provider_b_descriptor.provider = "tantivy-provider-b".to_owned();
            provider_b_descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
            let provider_b = TransientGenerationInspectStore::with_descriptor(
                &db_path,
                provider_a,
                provider_b_descriptor,
            );
            let lease =
                acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "coexist-owner", 120_000)?;
            let manifest = begin_projection_generation(
                &db_path,
                TANTIVY_TASKS_STORE,
                &lease.owner,
                &lease.lease_token,
                &provider_b,
            )?;
            let snapshot = ProjectionSnapshot {
                manifest: manifest.clone(),
                records: Vec::new(),
            };
            let snapshotting_authority = snapshotting_authority(&manifest, &lease);
            let prepared = if matches!(surface, TransientCoexistSurface::Prepare) {
                None
            } else {
                Some(prepare_projection_snapshot_with(
                    &db_path,
                    TANTIVY_TASKS_STORE,
                    &lease.owner,
                    &lease.lease_token,
                    &provider_b,
                )?)
            };
            let prepared_authority = prepared.as_ref().map(|evidence| {
                evidence_authority(
                    evidence,
                    &lease,
                    ProjectionGenerationRole::Building,
                    Some("prepared"),
                )
            });
            if matches!(surface, TransientCoexistSurface::Validate) {
                provider_b
                    .state
                    .lock()
                    .expect("transient projection state")
                    .published
                    .insert(manifest.generation.clone());
            }

            let mut foreign_manifest = manifest.clone();
            foreign_manifest.provider = provider_a_descriptor.provider;
            foreign_manifest.provider_fingerprint = provider_a_descriptor.provider_fingerprint;
            let foreign = provider_b.inner.prepare_snapshot(&ProjectionSnapshot {
                manifest: foreign_manifest,
                records: Vec::new(),
            })?;
            let overlay_before = provider_b
                .state
                .lock()
                .expect("transient projection state")
                .clone();

            let result = match surface {
                TransientCoexistSurface::Prepare => provider_b
                    .prepare_snapshot_with_authority(&snapshot, &snapshotting_authority)
                    .map(|_| ()),
                TransientCoexistSurface::Apply => provider_b
                    .apply_batch_with_authority(
                        &empty_projection_batch(
                            &prepared.as_ref().expect("prepared evidence").manifest,
                            &lease,
                        ),
                        prepared_authority.as_ref().expect("prepared authority"),
                    )
                    .map(|_| ()),
                TransientCoexistSurface::Publish => provider_b
                    .publish_generation_with_authority(
                        None,
                        prepared.as_ref().expect("prepared evidence"),
                        prepared_authority.as_ref().expect("prepared authority"),
                    )
                    .map(|_| ()),
                TransientCoexistSurface::Validate => provider_b
                    .validate_generation_publication_with_authority(
                        prepared.as_ref().expect("prepared evidence"),
                        prepared_authority.as_ref().expect("prepared authority"),
                    ),
                TransientCoexistSurface::Repair => provider_b
                    .repair_generation_publication_with_authority(
                        prepared.as_ref().expect("prepared evidence"),
                        prepared_authority.as_ref().expect("prepared authority"),
                    ),
                TransientCoexistSurface::Quarantine => provider_b.quarantine_generation_fenced(
                    &manifest.generation,
                    prepared_authority.as_ref().expect("prepared authority"),
                ),
                TransientCoexistSurface::Abort => provider_b.abort_generation_fenced(
                    &manifest.generation,
                    prepared_authority.as_ref().expect("prepared authority"),
                ),
            };

            match surface {
                TransientCoexistSurface::Quarantine | TransientCoexistSurface::Abort => {
                    result?;
                    assert!(
                        !provider_b
                            .state
                            .lock()
                            .expect("transient projection state")
                            .generations
                            .contains_key(&manifest.generation),
                        "{surface:?} left overlay evidence behind"
                    );
                    assert!(
                        provider_b
                            .inner
                            .inspect_generation(&manifest.generation)?
                            .is_none(),
                        "{surface:?} left foreign physical evidence behind"
                    );
                }
                _ => {
                    let error = result
                        .expect_err("foreign physical evidence must fail closed before write");
                    assert!(
                        matches!(error, KanbanError::Conflict(_) | KanbanError::Storage(_)),
                        "{surface:?} returned {error:?}"
                    );
                    assert_eq!(
                        *provider_b.state.lock().expect("transient projection state"),
                        overlay_before,
                        "{surface:?} changed overlay state"
                    );
                    assert_eq!(
                        provider_b.inner.inspect_generation(&manifest.generation)?,
                        Some(foreign),
                        "{surface:?} changed foreign physical evidence"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn transient_quarantine_protects_exact_physical_active_without_overlay_or_marker()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-physical-active.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut provider_b_descriptor = provider_a.descriptor()?;
        provider_b_descriptor.provider = "tantivy-provider-b".to_owned();
        provider_b_descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            provider_a,
            provider_b_descriptor,
        );
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "physical-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        let snapshot = ProjectionSnapshot {
            manifest,
            records: Vec::new(),
        };
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        let active = publish_projection_generation_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        provider_b.inner.prepare_snapshot(&snapshot)?;
        let status = projection_status(&db_path)?;
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&active.manifest.generation);
        std::fs::write(
            generation_path.join("kb-projection-meta.json"),
            serde_json::to_vec(&tantivy_metadata_for_evidence(&active))?,
        )?;
        let coexist_error = provider_b
            .inspect_active()
            .expect_err("published overlay must not hide a matching physical marker gap");
        assert!(matches!(
            coexist_error,
            KanbanError::Conflict(_) | KanbanError::Storage(_)
        ));
        *provider_b.state.lock().expect("transient projection state") =
            TransientProjectionState::default();
        assert!(
            std::fs::symlink_metadata(generation_path.join("published")).is_err(),
            "the exact physical active intentionally has no publication marker"
        );
        let authority = evidence_authority(&active, &lease, ProjectionGenerationRole::Active, None);

        let error = provider_b
            .quarantine_generation_fenced(&active.manifest.generation, &authority)
            .expect_err("exact physical active metadata must be protected without an overlay");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert!(generation_path.is_dir());
        assert_eq!(
            provider_b
                .inner
                .inspect_generation(&active.manifest.generation)?,
            Some(active)
        );
        Ok(())
    }

    #[test]
    fn transient_overlay_read_validation_rejects_foreign_descriptor_evidence() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-overlay-descriptor.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut provider_b_descriptor = provider_a.descriptor()?;
        provider_b_descriptor.provider = "tantivy-provider-b".to_owned();
        provider_b_descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            provider_a,
            provider_b_descriptor,
        );
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "descriptor-owner", 120_000)?;
        begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        let mut foreign = prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        foreign.manifest.provider = "foreign-provider".to_owned();
        foreign.manifest.provider_fingerprint = "foreign-provider-v1".to_owned();
        {
            let mut state = provider_b.state.lock().expect("transient projection state");
            state
                .generations
                .insert(foreign.manifest.generation.clone(), foreign.clone());
            state.active = Some(foreign.clone());
            state.published.insert(foreign.manifest.generation.clone());
        }

        let active_error = provider_b
            .validate_active_contents(&foreign)
            .expect_err("active validation must enforce the provider descriptor");
        assert!(matches!(active_error, KanbanError::Conflict(_)));
        let publication_error = provider_b
            .validate_generation_publication(&foreign)
            .expect_err("non-authority publication validation must enforce the descriptor");
        assert!(matches!(publication_error, KanbanError::Conflict(_)));
        Ok(())
    }

    #[test]
    fn transient_physical_active_scan_fails_closed_for_malformed_marker_entries()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-malformed-marker.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "marker-owner", 120_000)?;
        let manifest = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_a,
        )?;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_a,
        )?;
        let status = projection_status(&db_path)?;
        let marker_path = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&manifest.generation)
        .join("published");
        std::fs::create_dir(&marker_path)?;
        let mut provider_b_descriptor = provider_a.descriptor()?;
        provider_b_descriptor.provider = "tantivy-provider-b".to_owned();
        provider_b_descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            TantivyProjectionStore::new(&db_path)?,
            provider_b_descriptor,
        );

        let directory_error = provider_b
            .inspect_active()
            .expect_err("a marker directory must fail closed");
        assert!(matches!(directory_error, KanbanError::Storage(_)));

        #[cfg(unix)]
        {
            std::fs::remove_dir(&marker_path)?;
            std::os::unix::fs::symlink("missing-marker-target", &marker_path)?;
            let symlink_error = provider_b
                .inspect_active()
                .expect_err("a marker symlink must fail closed");
            assert!(matches!(symlink_error, KanbanError::Storage(_)));
        }
        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    enum TransientRecoveryOperation {
        Quarantine,
        Abort,
    }

    #[test]
    fn transient_recovery_authority_matrix_preserves_then_mutates_exact_historical_evidence()
    -> anyhow::Result<()> {
        for operation in [
            TransientRecoveryOperation::Quarantine,
            TransientRecoveryOperation::Abort,
        ] {
            let temp = tempdir()?;
            let db_path = temp
                .path()
                .join(format!("transient-recovery-{operation:?}.db"));
            init_database(&db_path, "tester")?;
            let provider_a = TantivyProjectionStore::new(&db_path)?;
            let provider_a_lease = acquire_projection_lease(
                &db_path,
                TANTIVY_TASKS_STORE,
                "provider-a-owner",
                120_000,
            )?;
            let manifest = begin_projection_generation(
                &db_path,
                TANTIVY_TASKS_STORE,
                &provider_a_lease.owner,
                &provider_a_lease.lease_token,
                &provider_a,
            )?;
            let evidence = prepare_projection_snapshot_with(
                &db_path,
                TANTIVY_TASKS_STORE,
                &provider_a_lease.owner,
                &provider_a_lease.lease_token,
                &provider_a,
            )?;
            let status = projection_status(&db_path)?;
            let generation_path = kanban_local::projection_store_root_path(
                &db_path,
                &status.database_instance_id,
                TANTIVY_TASKS_STORE,
            )?
            .join("generations")
            .join(&manifest.generation);
            release_projection_lease(
                &db_path,
                TANTIVY_TASKS_STORE,
                &provider_a_lease.owner,
                &provider_a_lease.lease_token,
            )?;

            let mut descriptor = provider_a.descriptor()?;
            descriptor.provider = "tantivy-provider-b".to_owned();
            descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
            let provider_b = TransientGenerationInspectStore::with_descriptor(
                &db_path,
                TantivyProjectionStore::new(&db_path)?,
                descriptor,
            );
            let provider_b_lease = acquire_projection_lease(
                &db_path,
                TANTIVY_TASKS_STORE,
                "provider-b-owner",
                120_000,
            )?;
            let base_authority = evidence_authority(
                &evidence,
                &provider_b_lease,
                ProjectionGenerationRole::Building,
                Some("prepared"),
            );
            let physical_before = provider_a
                .inspect_generation(&manifest.generation)?
                .expect("provider A prepared evidence");

            for drift in TRANSIENT_AUTHORITY_DRIFTS {
                let mut authority = base_authority.clone();
                apply_transient_building_drift(
                    &db_path,
                    &manifest,
                    &provider_b_lease,
                    "prepared",
                    &mut authority,
                    drift,
                )?;
                let result = match operation {
                    TransientRecoveryOperation::Quarantine => {
                        provider_b.quarantine_generation_fenced(&manifest.generation, &authority)
                    }
                    TransientRecoveryOperation::Abort => {
                        provider_b.abort_generation_fenced(&manifest.generation, &authority)
                    }
                };
                let error = result
                    .expect_err("stale historical authority must fail before physical change");
                assert!(
                    matches!(error, KanbanError::Conflict(_)),
                    "{operation:?}/{drift:?} returned {error:?}"
                );
                assert_eq!(
                    provider_a.inspect_generation(&manifest.generation)?,
                    Some(physical_before.clone()),
                    "{operation:?}/{drift:?} changed historical physical evidence"
                );
                assert!(
                    generation_path.is_dir(),
                    "{operation:?}/{drift:?} removed the generation directory"
                );
                restore_transient_building_drift(&db_path, &manifest, &provider_b_lease, drift)?;
            }

            match operation {
                TransientRecoveryOperation::Quarantine => {
                    provider_b
                        .quarantine_generation_fenced(&manifest.generation, &base_authority)?;
                    let quarantined = quarantined_generation_path(&generation_path)?;
                    assert!(
                        quarantined.join("kb-projection-meta.json").is_file(),
                        "exact historical quarantine must preserve provider A evidence"
                    );
                }
                TransientRecoveryOperation::Abort => {
                    provider_b.abort_generation_fenced(&manifest.generation, &base_authority)?;
                }
            }
            assert!(
                provider_a
                    .inspect_generation(&manifest.generation)?
                    .is_none(),
                "exact historical authority must remove the canonical physical entry"
            );
        }
        Ok(())
    }

    #[test]
    fn transient_new_provider_publishes_and_only_protects_exact_canonical_active()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-provider-b-success.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b =
            TransientGenerationInspectStore::with_descriptor(&db_path, provider_a, descriptor);
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "provider-b-owner", 120_000)?;
        begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        let prepared = prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        let batch = empty_projection_batch(&prepared.manifest, &lease);
        let prepared_authority = evidence_authority(
            &prepared,
            &lease,
            ProjectionGenerationRole::Building,
            Some("prepared"),
        );
        let receipt = provider_b.apply_batch_with_authority(&batch, &prepared_authority)?;
        assert_eq!(receipt.target_generation, prepared.manifest.generation);
        assert_eq!(receipt.applied_item_count, 0);
        let active = publish_projection_generation_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            &lease.owner,
            &lease.lease_token,
            &provider_b,
        )?;
        assert_eq!(active, prepared);
        assert_eq!(provider_b.inspect_active()?, Some(active.clone()));

        let authority = evidence_authority(&active, &lease, ProjectionGenerationRole::Active, None);
        provider_b
            .state
            .lock()
            .expect("transient projection state")
            .published
            .remove(&active.manifest.generation);
        let inspect_error = provider_b
            .inspect_active()
            .expect_err("an active without a publication marker must not read as active");
        assert!(matches!(inspect_error, KanbanError::Storage(_)));
        let active_validation_error = provider_b
            .validate_active_contents(&active)
            .expect_err("active validation must reject a missing publication marker");
        assert!(matches!(active_validation_error, KanbanError::Storage(_)));
        let validation_error = provider_b
            .validate_generation_publication_with_authority(&active, &authority)
            .expect_err("missing transient publication marker must be detected");
        assert!(matches!(validation_error, KanbanError::Storage(_)));
        provider_b.repair_generation_publication_with_authority(&active, &authority)?;
        provider_b.validate_generation_publication_with_authority(&active, &authority)?;

        let error = provider_b
            .quarantine_generation_fenced(&active.manifest.generation, &authority)
            .expect_err("an exact provider B canonical active must be protected");
        assert!(matches!(error, KanbanError::Conflict(_)));
        assert_eq!(
            provider_b.inspect_generation(&active.manifest.generation)?,
            Some(active.clone())
        );

        provider_b
            .state
            .lock()
            .expect("transient projection state")
            .active
            .as_mut()
            .expect("transient active")
            .fingerprint
            .push_str("-corrupt");
        provider_b.quarantine_generation_fenced(&active.manifest.generation, &authority)?;
        assert!(
            provider_b
                .inspect_generation(&active.manifest.generation)?
                .is_none(),
            "a corrupt provider B active must remain recoverable"
        );
        Ok(())
    }

    #[test]
    fn owner_heartbeat_renews_at_operation_completion_boundary() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 1_000,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;

        let operation_completed_at = session.run_with_owner_heartbeat(|| {
            thread::sleep(Duration::from_millis(20));
            Ok(SystemClock.now_ms())
        })?;
        let status = projection_status(&db_path)?;
        assert!(status.maintenance_owner.active);
        assert!(
            status
                .maintenance_owner
                .last_heartbeat_at
                .is_some_and(|heartbeat_at| heartbeat_at >= operation_completed_at)
        );
        session.finish()?;
        Ok(())
    }

    #[test]
    fn owner_heartbeat_final_renew_rejects_successor_handoff() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 10_000,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let successor_identity = session.identity.clone();
        let operation_completed = AtomicBool::new(false);
        let mut successor_token = None;

        let error = session
            .run_with_owner_heartbeat(|| {
                let conn = connect_file(&db_path).expect("open test database");
                with_immediate_tx(&conn, || {
                    conn.execute(
                        "UPDATE projection_maintenance_owner SET lease_expires_at=0 \
                         WHERE singleton=1 AND owner=?1 AND lease_token=?2",
                        params!["heartbeat-owner", session.lease_token.as_str()],
                    )
                    .map_err(storage)?;
                    Ok(())
                })
                .expect("expire old owner lease before lawful takeover");
                successor_token = Some(
                    acquire_maintenance_owner(
                        &db_path,
                        "successor-owner",
                        MaintenanceMode::Once,
                        10_000,
                        &successor_identity,
                    )
                    .expect("successor acquires expired owner lease"),
                );
                operation_completed.store(true, Ordering::SeqCst);
                Ok(())
            })
            .expect_err("final owner renewal must fail after successor handoff");

        assert!(operation_completed.load(Ordering::SeqCst));
        assert!(matches!(error, KanbanError::Conflict(_)));
        let status = projection_status(&db_path)?;
        assert!(status.maintenance_owner.active);
        assert_eq!(
            status.maintenance_owner.owner.as_deref(),
            Some("successor-owner")
        );
        release_maintenance_owner(
            &db_path,
            "successor-owner",
            successor_token.as_deref().expect("successor token"),
            &successor_identity,
        )?;
        Ok(())
    }

    #[test]
    fn catch_up_renewal_rolls_back_owner_extension_when_store_has_successor() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 10_000,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "catch-up-owner", MaintenanceMode::Once, options)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "catch-up-owner", 10_000)?;
        let owner_expiry_before = SystemClock.now_ms() + 5_000;
        let conn = connect_file(&db_path)?;
        with_immediate_tx(&conn, || {
            let owner_changed = conn
                .execute(
                    "UPDATE projection_maintenance_owner SET lease_expires_at=?1
                     WHERE singleton=1 AND owner=?2 AND lease_token=?3",
                    params![
                        owner_expiry_before,
                        "catch-up-owner",
                        session.lease_token.as_str()
                    ],
                )
                .map_err(storage)?;
            let store_expired = conn
                .execute(
                    "UPDATE projection_store_state SET lease_expires_at=0
                     WHERE store_name=?1 AND lease_owner=?2 AND lease_token=?3",
                    params![
                        TANTIVY_TASKS_STORE,
                        "catch-up-owner",
                        lease.lease_token.as_str()
                    ],
                )
                .map_err(storage)?;
            if owner_changed != 1 || store_expired != 1 {
                return Err(KanbanError::Storage(
                    "test failed to prepare catch-up successor handoff".to_owned(),
                ));
            }
            Ok(())
        })?;
        let successor =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "successor-owner", 10_000)?;

        let error = renew_catch_up_authorities(&session, TANTIVY_TASKS_STORE, &lease.lease_token)
            .expect_err("catch-up renewal must fail after store handoff");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let conn = connect_file(&db_path)?;
        let owner_expiry_after: i64 = conn.query_row(
            "SELECT lease_expires_at FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        let (store_owner, store_token, store_fence_epoch): (Option<String>, Option<String>, i64) =
            conn.query_row(
                "SELECT lease_owner,lease_token,fence_epoch
                 FROM projection_store_state WHERE store_name=?1",
                [TANTIVY_TASKS_STORE],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        assert_eq!(owner_expiry_after, owner_expiry_before);
        assert_eq!(store_owner.as_deref(), Some("successor-owner"));
        assert_eq!(store_token.as_deref(), Some(successor.lease_token.as_str()));
        assert_eq!(store_fence_epoch, successor.fence_epoch);

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "successor-owner",
            &successor.lease_token,
        )?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn physical_operation_heartbeat_keeps_both_leases_fenced() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 1_000,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "heartbeat-owner", 1_000)?;
        let heartbeat = ProjectionLeaseHeartbeat::new(&session, &lease);

        let operation_completed_at = heartbeat
            .run(|| {
                thread::sleep(Duration::from_millis(2_500));
                let conflict =
                    acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "competitor", 1_000)
                        .expect_err("heartbeat must prevent a competing store lease");
                assert!(matches!(conflict, KanbanError::Conflict(_)));
                Ok(SystemClock.now_ms())
            })?
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let status = projection_status(&db_path)?;
        assert!(status.maintenance_owner.active);
        assert!(
            status
                .maintenance_owner
                .last_heartbeat_at
                .is_some_and(|heartbeat_at| heartbeat_at >= operation_completed_at)
        );
        assert_eq!(
            status.maintenance_owner.owner.as_deref(),
            Some("heartbeat-owner")
        );

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "heartbeat-owner",
            &lease.lease_token,
        )?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn maintenance_owner_renew_does_not_revive_expired_lease_after_writer_delay()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 1_000,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let (entered_tx, entered_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let renewal_path = db_path.clone();
        let renewal_token = session.lease_token.clone();
        let renewal_identity = session.identity.clone();
        let renewal = thread::spawn(move || {
            renew_maintenance_owner_lease_with_before_transaction(
                &renewal_path,
                "heartbeat-owner",
                &renewal_token,
                1_000,
                &renewal_identity,
                || {
                    entered_tx
                        .send(())
                        .expect("test observes timestamp sampling before writer lock");
                    resume_rx
                        .recv()
                        .expect("test resumes owner renewal against writer lock");
                },
            )
        });
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("owner renewal reached pre-transaction barrier");

        let expires_at = SystemClock.now_ms() + 75;
        let conn = connect_file(&db_path)?;
        with_immediate_tx(&conn, || {
            let changed = conn
                .execute(
                    "UPDATE projection_maintenance_owner SET lease_expires_at=?1
                     WHERE singleton=1 AND owner=?2 AND lease_token=?3",
                    params![expires_at, "heartbeat-owner", session.lease_token.as_str()],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::Storage(
                    "test failed to shorten maintenance owner lease".to_owned(),
                ));
            }
            Ok(())
        })?;

        let writer = connect_file(&db_path)?;
        writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
        resume_tx
            .send(())
            .expect("resume owner renewal against held writer lock");
        while SystemClock.now_ms() <= expires_at {
            thread::sleep(Duration::from_millis(1));
        }
        writer.execute_batch("COMMIT").map_err(storage)?;

        let error = renewal
            .join()
            .expect("owner renewal thread must not panic")
            .expect_err("owner renewal delayed beyond expiry must not revive the lease");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let conn = connect_file(&db_path)?;
        let actual_expiry: i64 = conn.query_row(
            "SELECT lease_expires_at FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(actual_expiry, expires_at);
        Ok(())
    }

    #[test]
    fn heartbeat_renew_does_not_revive_expired_authorities_after_writer_delay() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 1_000,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "heartbeat-owner", 1_000)?;
        let mut heartbeat = ProjectionLeaseHeartbeat::new(&session, &lease);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        heartbeat.pause_before_transaction_for_test(entered_tx, resume_rx);

        let renewal = thread::spawn(move || heartbeat.renew());
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("heartbeat reached pre-transaction barrier");

        let expires_at = SystemClock.now_ms() + 75;
        let conn = connect_file(&db_path)?;
        with_immediate_tx(&conn, || {
            let owner_changed = conn
                .execute(
                    "UPDATE projection_maintenance_owner SET lease_expires_at=?1
                     WHERE singleton=1 AND owner=?2 AND lease_token=?3",
                    params![expires_at, "heartbeat-owner", session.lease_token.as_str()],
                )
                .map_err(storage)?;
            let store_changed = conn
                .execute(
                    "UPDATE projection_store_state SET lease_expires_at=?1
                     WHERE store_name=?2 AND lease_owner=?3 AND lease_token=?4",
                    params![
                        expires_at,
                        TANTIVY_TASKS_STORE,
                        "heartbeat-owner",
                        lease.lease_token.as_str()
                    ],
                )
                .map_err(storage)?;
            if owner_changed != 1 || store_changed != 1 {
                return Err(KanbanError::Storage(
                    "test failed to shorten both heartbeat authorities".to_owned(),
                ));
            }
            Ok(())
        })?;

        let writer = connect_file(&db_path)?;
        writer.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
        resume_tx
            .send(())
            .expect("resume heartbeat renewal against held writer lock");
        while SystemClock.now_ms() <= expires_at {
            thread::sleep(Duration::from_millis(1));
        }
        writer.execute_batch("COMMIT").map_err(storage)?;

        let error = renewal
            .join()
            .expect("heartbeat renewal thread must not panic")
            .expect_err("renewal delayed beyond expiry must not revive old authorities");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let conn = connect_file(&db_path)?;
        let owner_expiry: i64 = conn.query_row(
            "SELECT lease_expires_at FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        let store_expiry: i64 = conn.query_row(
            "SELECT lease_expires_at FROM projection_store_state WHERE store_name=?1",
            [TANTIVY_TASKS_STORE],
            |row| row.get(0),
        )?;
        assert_eq!(owner_expiry, expires_at);
        assert_eq!(store_expiry, expires_at);
        Ok(())
    }

    #[test]
    fn physical_operation_final_renew_rejects_maintenance_owner_handoff() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let lease_ttl_ms = 10_000;
        let options = MaintenanceRunOptions {
            lease_ttl_ms,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "heartbeat-owner",
            lease_ttl_ms,
        )?;
        let heartbeat = ProjectionLeaseHeartbeat::new(&session, &lease);
        let successor_identity = heartbeat.maintenance_identity.clone();
        let operation_completed = AtomicBool::new(false);
        let mut store_expires_before_handoff = None;
        let mut successor_token = None;

        let error = heartbeat
            .run(|| {
                let conn = connect_file(&db_path).expect("open test database");
                store_expires_before_handoff = Some(
                    conn.query_row(
                        "SELECT lease_expires_at FROM projection_store_state WHERE store_name=?1",
                        [TANTIVY_TASKS_STORE],
                        |row| row.get::<_, i64>(0),
                    )
                    .expect("read store expiry before maintenance handoff"),
                );
                with_immediate_tx(&conn, || {
                    conn.execute(
                        "UPDATE projection_maintenance_owner SET lease_expires_at=0 \
                         WHERE singleton=1 AND owner=?1 AND lease_token=?2",
                        params!["heartbeat-owner", session.lease_token.as_str()],
                    )
                    .map_err(storage)?;
                    Ok(())
                })
                .expect("expire old maintenance lease before lawful takeover");
                successor_token = Some(
                    acquire_maintenance_owner(
                        &db_path,
                        "successor-owner",
                        MaintenanceMode::Once,
                        lease_ttl_ms,
                        &successor_identity,
                    )
                    .expect("successor acquires expired maintenance lease"),
                );
                operation_completed.store(true, Ordering::SeqCst);
                Ok(())
            })
            .expect_err("final heartbeat renewal must fail after maintenance ownership changes");

        assert!(operation_completed.load(Ordering::SeqCst));
        assert!(matches!(error, KanbanError::Conflict(_)));
        let status = projection_status(&db_path)?;
        assert!(status.maintenance_owner.active);
        assert_eq!(
            status.maintenance_owner.owner.as_deref(),
            Some("successor-owner")
        );
        let conn = connect_file(&db_path)?;
        let store_expires_after_handoff: i64 = conn.query_row(
            "SELECT lease_expires_at FROM projection_store_state WHERE store_name=?1",
            [TANTIVY_TASKS_STORE],
            |row| row.get(0),
        )?;
        assert_eq!(
            store_expires_after_handoff,
            store_expires_before_handoff.expect("store expiry before handoff"),
            "a rejected maintenance renewal must not extend the paired store lease"
        );

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "heartbeat-owner",
            &lease.lease_token,
        )?;
        release_maintenance_owner(
            &db_path,
            "successor-owner",
            successor_token.as_deref().expect("successor token"),
            &successor_identity,
        )?;
        Ok(())
    }

    #[test]
    fn physical_operation_final_renew_rejects_store_successor_without_reporting_success()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let lease_ttl_ms = 10_000;
        let options = MaintenanceRunOptions {
            lease_ttl_ms,
            claim_ttl_ms: 250,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "heartbeat-owner",
            lease_ttl_ms,
        )?;
        let heartbeat = ProjectionLeaseHeartbeat::new(&session, &lease);
        let operation_completed = AtomicBool::new(false);
        let mut maintenance_expires_before_handoff = None;
        let mut successor = None;

        let error = heartbeat
            .run(|| {
                let conn = connect_file(&db_path).expect("open test database");
                maintenance_expires_before_handoff = Some(
                    conn.query_row(
                        "SELECT lease_expires_at FROM projection_maintenance_owner WHERE singleton=1",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .expect("read maintenance expiry before store handoff"),
                );
                with_immediate_tx(&conn, || {
                    conn.execute(
                        "UPDATE projection_store_state SET lease_expires_at=0 \
                         WHERE store_name=?1 AND lease_owner='heartbeat-owner' AND lease_token=?2",
                        params![TANTIVY_TASKS_STORE, lease.lease_token],
                    )
                    .map_err(storage)?;
                    Ok(())
                })
                .expect("expire old store lease before lawful takeover");
                successor = Some(
                    acquire_projection_lease(
                        &db_path,
                        TANTIVY_TASKS_STORE,
                        "successor-owner",
                        lease_ttl_ms,
                    )
                    .expect("successor acquires expired store lease"),
                );
                thread::sleep(Duration::from_millis(20));
                operation_completed.store(true, Ordering::SeqCst);
                Ok(())
            })
            .expect_err("final heartbeat renewal must fail after store ownership changes");

        assert!(operation_completed.load(Ordering::SeqCst));
        assert!(matches!(error, KanbanError::Conflict(_)));
        let successor = successor.expect("successor store lease");
        assert_eq!(successor.fence_epoch, lease.fence_epoch + 1);
        let conn = connect_file(&db_path)?;
        let maintenance_expires_after_handoff: i64 = conn.query_row(
            "SELECT lease_expires_at FROM projection_maintenance_owner WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(
            maintenance_expires_after_handoff,
            maintenance_expires_before_handoff.expect("maintenance expiry before handoff"),
            "a rejected store renewal must roll back the paired maintenance extension"
        );
        let (owner, token, fence_epoch): (Option<String>, Option<String>, i64) = conn.query_row(
            "SELECT lease_owner,lease_token,fence_epoch \
             FROM projection_store_state WHERE store_name=?1",
            [TANTIVY_TASKS_STORE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(owner.as_deref(), Some("successor-owner"));
        assert_eq!(token.as_deref(), Some(successor.lease_token.as_str()));
        assert_eq!(fence_epoch, successor.fence_epoch);

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "successor-owner",
            &successor.lease_token,
        )?;
        session.finish()?;
        Ok(())
    }

    #[test]
    fn failure_persistence_rejects_handoff_after_failure_before_persist() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;

        let old = acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "old-owner", 10_000)?;
        let renewed = renew_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "old-owner",
            &old.lease_token,
            10_000,
        )?;
        release_projection_lease(&db_path, TANTIVY_TASKS_STORE, "old-owner", &old.lease_token)?;
        let successor =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "successor-owner", 10_000)?;
        let before = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");

        let error = persist_store_failure(
            &db_path,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &renewed,
            MaintenanceStoreFailureKind::Backend,
            KanbanError::Storage("failure from the previous owner".to_owned()),
        )
        .expect_err("a handed-off lease must reject stale failure persistence");
        assert!(matches!(error, KanbanError::Conflict(_)));

        let after = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(after.lifecycle_status, before.lifecycle_status);
        assert_eq!(after.last_error, before.last_error);
        assert_eq!(after.fallback_reason, before.fallback_reason);
        assert_eq!(after.owner.as_deref(), Some("successor-owner"));
        assert_eq!(after.fence_epoch, successor.fence_epoch);

        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "successor-owner",
            &successor.lease_token,
        )?;
        Ok(())
    }

    #[test]
    fn runtime_reconciles_crash_after_physical_publish() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("first generation"),
        )?;
        maintenance_run_once(
            &db_path,
            "bootstrap-owner",
            MaintenanceRunOptions::default(),
        )?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("crash catch-up"),
        )?;

        let mut crashed = MaintenanceSession::start(
            &db_path,
            "crashed-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let backend = TantivyProjectionStore::new(&db_path)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "crashed-owner",
            crashed.options.lease_ttl_ms,
        )?;
        begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "crashed-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "crashed-owner",
            &lease.lease_token,
            &backend,
        )?;
        catch_up_generation(
            &mut crashed,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &lease.lease_token,
            &backend,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let state = projection_status(&db_path)?;
        let store = state
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        let building = store
            .building_generation
            .clone()
            .expect("building generation");
        let prepared = backend
            .inspect_generation(&building)?
            .expect("prepared generation");
        let expected_active = backend.inspect_active()?;
        backend.publish_generation(expected_active.as_ref(), &prepared)?;
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "crashed-owner",
            &lease.lease_token,
        )?;
        drop(crashed);

        let report =
            maintenance_run_once(&db_path, "takeover-owner", MaintenanceRunOptions::default())?;
        assert!(matches!(
            &report.stores[0].result,
            MaintenanceStoreResult::Succeeded { action, .. }
                if action == "generation_reconciled"
        ));
        let recovered = maintenance_status(&db_path)?;
        let store = recovered
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(store.active_generation.as_deref(), Some(building.as_str()));
        assert!(store.building_generation.is_none());
        assert_eq!(store.lifecycle_status, "ready");
        Ok(())
    }

    #[test]
    fn runtime_repairs_corrupt_building_marker_before_normal_publish() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("corrupt building marker"),
        )?;
        let mut interrupted = MaintenanceSession::start(
            &db_path,
            "interrupted-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let backend = TantivyProjectionStore::new(&db_path)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            interrupted.options.lease_ttl_ms,
        )?;
        begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        catch_up_generation(
            &mut interrupted,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &lease.lease_token,
            &backend,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let status = projection_status(&db_path)?;
        let building = status
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .and_then(|store| store.building_generation.clone())
            .expect("building generation");
        let database_instance_id = projection_status(&db_path)?.database_instance_id;
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&building);
        let marker = generation_path.join("published");
        std::fs::write(&marker, b"truncated-marker")?;
        assert_eq!(backend.inspect_active()?, None);
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
        )?;
        interrupted.finish()?;

        let report =
            maintenance_run_once(&db_path, "takeover-owner", MaintenanceRunOptions::default())?;
        let store = report
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy result");
        assert!(matches!(
            &store.result,
            MaintenanceStoreResult::Succeeded { action, .. }
                if action == "generation_published"
        ));
        let recovered = projection_status(&db_path)?;
        let store = recovered
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(store.active_generation.as_deref(), Some(building.as_str()));
        assert!(store.building_generation.is_none());
        assert!(marker.is_file());
        assert!(std::fs::read_dir(generation_path)?.flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("published.quarantine")
        }));
        Ok(())
    }

    #[test]
    fn runtime_restarts_building_that_only_exists_in_legacy_unscoped_v2_root() -> anyhow::Result<()>
    {
        for phase in ["prepared", "store_published"] {
            let temp = tempdir()?;
            let db_path = temp.path().join(format!("{phase}.db"));
            init_database(&db_path, "tester")?;
            create_task(
                &db_path,
                "default",
                "tester",
                CreateTask::ready(format!("legacy unscoped {phase}")),
            )?;
            let session = MaintenanceSession::start(
                &db_path,
                "legacy-owner",
                MaintenanceMode::Once,
                MaintenanceRunOptions::default(),
            )?;
            let backend = TantivyProjectionStore::new(&db_path)?;
            let lease = acquire_projection_lease(
                &db_path,
                TANTIVY_TASKS_STORE,
                "legacy-owner",
                session.options.lease_ttl_ms,
            )?;
            begin_projection_generation(
                &db_path,
                TANTIVY_TASKS_STORE,
                "legacy-owner",
                &lease.lease_token,
                &backend,
            )?;
            prepare_projection_snapshot_with(
                &db_path,
                TANTIVY_TASKS_STORE,
                "legacy-owner",
                &lease.lease_token,
                &backend,
            )?;
            let status = projection_status(&db_path)?;
            let building = status
                .stores
                .iter()
                .find(|store| store.store_name == TANTIVY_TASKS_STORE)
                .and_then(|store| store.building_generation.clone())
                .expect("building generation");
            let scoped_generation = kanban_local::projection_store_root_path(
                &db_path,
                &status.database_instance_id,
                TANTIVY_TASKS_STORE,
            )?
            .join("generations")
            .join(&building);
            let legacy_generation = temp
                .path()
                .join("index")
                .join("v2")
                .join(TANTIVY_TASKS_STORE)
                .join("generations")
                .join(&building);
            std::fs::create_dir_all(legacy_generation.parent().expect("legacy parent"))?;
            std::fs::rename(&scoped_generation, &legacy_generation)?;
            let sentinel = legacy_generation.join("legacy-sentinel");
            std::fs::write(&sentinel, b"must-remain-unmodified")?;
            if phase == "store_published" {
                connect_file(&db_path)?.execute(
                    "UPDATE projection_store_state
                     SET building_phase='store_published'
                     WHERE store_name=?1 AND building_generation=?2",
                    params![TANTIVY_TASKS_STORE, building],
                )?;
            }
            release_projection_lease(
                &db_path,
                TANTIVY_TASKS_STORE,
                "legacy-owner",
                &lease.lease_token,
            )?;
            session.finish()?;

            let report = maintenance_run_once(
                &db_path,
                "namespaced-owner",
                MaintenanceRunOptions::default(),
            )?;
            let result = report
                .stores
                .iter()
                .find(|store| store.store_name == TANTIVY_TASKS_STORE)
                .expect("Tantivy result");
            assert!(
                matches!(&result.result, MaintenanceStoreResult::Succeeded { .. }),
                "{phase}: {:?}",
                result.result
            );
            let recovered = projection_status(&db_path)?;
            let recovered = recovered
                .stores
                .iter()
                .find(|store| store.store_name == TANTIVY_TASKS_STORE)
                .expect("Tantivy status");
            assert_ne!(
                recovered.active_generation.as_deref(),
                Some(building.as_str()),
                "{phase}"
            );
            assert!(recovered.active_generation.is_some(), "{phase}");
            assert!(recovered.building_generation.is_none(), "{phase}");
            assert_eq!(
                std::fs::read(&sentinel)?,
                b"must-remain-unmodified",
                "{phase}"
            );
        }
        Ok(())
    }

    #[test]
    fn runtime_quarantines_incompatible_prepared_generation_and_publishes_new_provider()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("provider-migration.db");
        init_database(&db_path, "tester")?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("provider migration"),
        )?;
        let interrupted = MaintenanceSession::start(
            &db_path,
            "provider-a-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            interrupted.options.lease_ttl_ms,
        )?;
        let first_active = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?;
        publish_projection_generation_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?;
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
        )?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            interrupted.options.lease_ttl_ms,
        )?;
        let second_active = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?;
        publish_projection_generation_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?;
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
        )?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            interrupted.options.lease_ttl_ms,
        )?;
        let incompatible_generation = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?;
        let status = projection_status(&db_path)?;
        let generations_root = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations");
        let generation_paths = [
            first_active.clone(),
            second_active.clone(),
            incompatible_generation.clone(),
        ]
        .map(|generation| generations_root.join(generation));
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
        )?;
        interrupted.finish()?;

        let mut provider_b_descriptor = provider_a.descriptor()?;
        provider_b_descriptor.provider = "tantivy-provider-b".to_owned();
        provider_b_descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor(
            &db_path,
            TantivyProjectionStore::new(&db_path)?,
            provider_b_descriptor.clone(),
        );
        let mut takeover = MaintenanceSession::start(
            &db_path,
            "provider-b-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-b-owner",
            takeover.options.lease_ttl_ms,
        )?;

        let result = run_projection_store_operation(
            &mut takeover,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &lease.lease_token,
            &provider_b,
            false,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;

        assert!(matches!(
            result.result,
            MaintenanceStoreResult::Succeeded { .. }
        ));
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-b-owner",
            &lease.lease_token,
        )?;
        takeover.finish()?;
        let recovered = projection_status(&db_path)?;
        let store = recovered
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(
            store.active_provider.as_deref(),
            Some(provider_b_descriptor.provider.as_str())
        );
        assert_eq!(
            store.active_provider_fingerprint.as_deref(),
            Some(provider_b_descriptor.provider_fingerprint.as_str())
        );
        assert!(store.building_generation.is_none());
        assert_ne!(
            store.active_generation.as_deref(),
            Some(incompatible_generation.as_str())
        );
        assert!(provider_b.inspect_active()?.is_some());
        for (generation, generation_path) in
            [first_active, second_active, incompatible_generation.clone()]
                .iter()
                .zip(&generation_paths)
        {
            assert!(provider_b.inspect_generation(generation)?.is_none());
            let quarantined = quarantined_generation_path(generation_path)?;
            assert!(
                quarantined.join("kb-projection-meta.json").is_file(),
                "incompatible generation {generation} evidence must remain in quarantine"
            );
        }
        Ok(())
    }

    #[test]
    fn incompatible_abort_does_not_swallow_unattributed_active_conflict() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("provider-conflict.db");
        init_database(&db_path, "tester")?;
        let provider_a = TantivyProjectionStore::new(&db_path)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "provider-a-owner", 20_000)?;
        let building = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
            &provider_a,
        )?;
        let status = projection_status(&db_path)?;
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&building);
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-a-owner",
            &lease.lease_token,
        )?;

        let mut descriptor = provider_a.descriptor()?;
        descriptor.provider = "tantivy-provider-b".to_owned();
        descriptor.provider_fingerprint = "tantivy-provider-b-v1".to_owned();
        let provider_b = TransientGenerationInspectStore::with_descriptor_and_active_conflict(
            &db_path,
            TantivyProjectionStore::new(&db_path)?,
            descriptor,
        );
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "provider-b-owner", 20_000)?;
        let error = abort_incompatible_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "provider-b-owner",
            &lease.lease_token,
            &provider_b,
        )
        .expect_err("an unattributed active conflict must fail closed");

        assert!(
            error
                .to_string()
                .contains("unattributed incompatible active")
        );
        let store = projection_status(&db_path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(
            store.building_generation.as_deref(),
            Some(building.as_str()),
            "SQLite must not reset until all physical active evidence is attributable"
        );
        assert!(
            quarantined_generation_path(&generation_path)?
                .join("kb-projection-meta.json")
                .is_file(),
            "the already quarantined building evidence remains recoverable"
        );
        Ok(())
    }

    #[test]
    fn runtime_preserves_prepared_generation_on_transient_target_inspection_failure()
    -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("transient-target-inspection.db");
        init_database(&db_path, "tester")?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("prepared transient inspection"),
        )?;
        let interrupted = MaintenanceSession::start(
            &db_path,
            "interrupted-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let backend = TantivyProjectionStore::new(&db_path)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            interrupted.options.lease_ttl_ms,
        )?;
        let building = begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?
        .generation;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
        )?;
        interrupted.finish()?;

        let mut takeover = MaintenanceSession::start(
            &db_path,
            "takeover-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "takeover-owner",
            takeover.options.lease_ttl_ms,
        )?;
        let backend =
            TransientGenerationInspectStore::new(&db_path, TantivyProjectionStore::new(&db_path)?);
        let attempt = run_projection_store_operation(
            &mut takeover,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &lease.lease_token,
            &backend,
            false,
        );
        let Err(MaintenanceStoreAttemptError::Store { kind, error }) = attempt else {
            anyhow::bail!("transient target inspection must fail the store attempt");
        };
        assert_eq!(kind, MaintenanceStoreFailureKind::Backend);
        assert!(
            error
                .to_string()
                .contains("transient prepared generation inspection")
        );
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "takeover-owner",
            &lease.lease_token,
        )?;
        takeover.finish()?;

        let state = projection_status(&db_path)?;
        let store = state
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_eq!(
            store.building_generation.as_deref(),
            Some(building.as_str())
        );
        assert_eq!(store.building_phase.as_deref(), Some("prepared"));
        assert!(
            backend.inspect_generation(&building)?.is_some(),
            "transient inspection must preserve the prepared physical generation"
        );
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &state.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&building);
        assert!(generation_path.is_dir());
        let prefix = format!(".{building}.quarantine.");
        assert!(
            std::fs::read_dir(generation_path.parent().expect("generation parent"))?
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().starts_with(&prefix)),
            "transient inspection must not quarantine the prepared generation"
        );
        Ok(())
    }

    #[test]
    fn runtime_restarts_prepared_generation_with_sqlite_evidence_mismatch() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("evidence-mismatch.db");
        init_database(&db_path, "tester")?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("prepared evidence mismatch"),
        )?;
        let session = MaintenanceSession::start(
            &db_path,
            "interrupted-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let backend = TantivyProjectionStore::new(&db_path)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            session.options.lease_ttl_ms,
        )?;
        begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        let status = projection_status(&db_path)?;
        let store = status
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        let building = store
            .building_generation
            .clone()
            .expect("building generation");
        let expected_schema_version = store.schema_version;
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&building);
        let metadata_path = generation_path.join("kb-projection-meta.json");
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
        metadata["schema_version"] = serde_json::json!(expected_schema_version + 1);
        std::fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;
        let mismatched = backend
            .inspect_generation(&building)?
            .expect("self-consistent physical generation");
        assert_ne!(mismatched.manifest.schema_version, expected_schema_version);
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
        )?;
        session.finish()?;

        let report =
            maintenance_run_once(&db_path, "takeover-owner", MaintenanceRunOptions::default())?;
        let result = report
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy result");
        assert!(
            matches!(&result.result, MaintenanceStoreResult::Succeeded { .. }),
            "{:?}",
            result.result
        );
        let recovered = projection_status(&db_path)?;
        let recovered = recovered
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_ne!(
            recovered.active_generation.as_deref(),
            Some(building.as_str())
        );
        assert!(recovered.active_generation.is_some());
        assert!(recovered.building_generation.is_none());
        assert!(
            std::fs::symlink_metadata(&generation_path).is_err(),
            "mismatched prepared evidence must leave the authoritative namespace"
        );
        let quarantined = quarantined_generation_path(&generation_path)?;
        assert!(
            quarantined.join("kb-projection-meta.json").is_file(),
            "quarantine must preserve prepared evidence"
        );
        Ok(())
    }

    #[test]
    fn runtime_quarantines_published_building_with_sqlite_evidence_mismatch() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let db_path = temp.path().join("published-evidence-mismatch.db");
        init_database(&db_path, "tester")?;
        create_task(
            &db_path,
            "default",
            "tester",
            CreateTask::ready("published building evidence mismatch"),
        )?;
        let session = MaintenanceSession::start(
            &db_path,
            "interrupted-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let backend = TantivyProjectionStore::new(&db_path)?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            session.options.lease_ttl_ms,
        )?;
        begin_projection_generation(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        prepare_projection_snapshot_with(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
            &backend,
        )?;
        let status = projection_status(&db_path)?;
        let store = status
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        let building = store
            .building_generation
            .clone()
            .expect("building generation");
        let expected_schema_version = store.schema_version;
        let prepared = backend
            .inspect_generation(&building)?
            .expect("prepared generation");
        backend.publish_generation(None, &prepared)?;
        let generation_path = kanban_local::projection_store_root_path(
            &db_path,
            &status.database_instance_id,
            TANTIVY_TASKS_STORE,
        )?
        .join("generations")
        .join(&building);
        let metadata_path = generation_path.join("kb-projection-meta.json");
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
        metadata["schema_version"] = serde_json::json!(expected_schema_version + 1);
        std::fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;
        connect_file(&db_path)?.execute(
            "UPDATE projection_store_state
             SET building_phase='store_published'
             WHERE store_name=?1 AND building_generation=?2",
            params![TANTIVY_TASKS_STORE, building],
        )?;
        let mismatched_active = backend
            .inspect_active()?
            .expect("self-consistent mismatched generation remains physically published");
        assert_ne!(
            mismatched_active.manifest.schema_version,
            expected_schema_version
        );
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "interrupted-owner",
            &lease.lease_token,
        )?;
        session.finish()?;

        let report =
            maintenance_run_once(&db_path, "takeover-owner", MaintenanceRunOptions::default())?;
        let result = report
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy result");
        assert!(
            matches!(&result.result, MaintenanceStoreResult::Succeeded { .. }),
            "{:?}",
            result.result
        );
        let recovered = projection_status(&db_path)?;
        let recovered = recovered
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .expect("Tantivy status");
        assert_ne!(
            recovered.active_generation.as_deref(),
            Some(building.as_str())
        );
        assert!(recovered.active_generation.is_some());
        assert!(recovered.building_generation.is_none());
        assert!(
            std::fs::symlink_metadata(&generation_path).is_err(),
            "mismatched published generation must leave the authoritative namespace"
        );
        let quarantined = quarantined_generation_path(&generation_path)?;
        assert!(
            quarantined.join("published").is_file(),
            "whole-directory quarantine must preserve publication evidence"
        );
        Ok(())
    }

    #[cfg(all(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    #[test]
    fn partial_continuous_owner_is_rejected_without_monopolizing_singleton() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let partial_identity = MaintenanceRuntimeIdentity::for_test(
            vec![TANTIVY_TASKS_STORE.to_owned()],
            "test-build",
        );
        let error = MaintenanceSession::start_with_identity(
            &db_path,
            "tantivy-only",
            MaintenanceMode::Continuous,
            MaintenanceRunOptions::default(),
            partial_identity,
        )
        .expect_err("a partial runtime must not hold the continuous singleton lease");
        assert!(
            error
                .to_string()
                .contains("continuous maintenance requires capabilities"),
            "{error}"
        );

        let status = maintenance_status(&db_path)?;
        assert!(
            !status.maintenance_owner.active,
            "rejected partial owner must leave the singleton immediately available"
        );
        let full_identity = MaintenanceRuntimeIdentity::for_test(
            vec![
                TANTIVY_TASKS_STORE.to_owned(),
                OXIGRAPH_RELATIONS_STORE.to_owned(),
                LANCEDB_LABEL_ATOMS_STORE.to_owned(),
                LANCEDB_CHUNKS_STORE.to_owned(),
            ],
            "full-test-build",
        );
        let full = MaintenanceSession::start_with_identity(
            &db_path,
            "full-owner",
            MaintenanceMode::Continuous,
            MaintenanceRunOptions::default(),
            full_identity,
        )?;
        full.finish()?;
        Ok(())
    }

    #[test]
    fn maintenance_session_debug_redacts_lease_token() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let session = MaintenanceSession::start(
            &db_path,
            "debug-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let secret = session.lease_token.clone();

        let rendered = format!("{session:?}");

        assert!(!rendered.contains(&secret), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
        session.finish()?;
        Ok(())
    }

    #[test]
    fn projection_heartbeat_debug_redacts_both_lease_tokens() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let session = MaintenanceSession::start(
            &db_path,
            "debug-owner",
            MaintenanceMode::Once,
            MaintenanceRunOptions::default(),
        )?;
        let lease = acquire_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "debug-owner",
            MaintenanceRunOptions::default().lease_ttl_ms,
        )?;
        let heartbeat = ProjectionLeaseHeartbeat::new(&session, &lease);

        let rendered = format!("{heartbeat:?}");

        assert!(!rendered.contains(&session.lease_token), "{rendered}");
        assert!(!rendered.contains(&lease.lease_token), "{rendered}");
        assert_eq!(rendered.matches("[REDACTED]").count(), 2, "{rendered}");
        release_projection_lease(
            &db_path,
            TANTIVY_TASKS_STORE,
            "debug-owner",
            &lease.lease_token,
        )?;
        session.finish()?;
        Ok(())
    }
}
