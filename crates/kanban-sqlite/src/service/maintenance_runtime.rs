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
use kanban_indexer::{
    LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE, OXIGRAPH_RELATIONS_STORE, TANTIVY_TASKS_STORE,
};
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
use super::{
    ProjectionCorpusMetadata, ProjectionRuntimeAvailability, ProjectionStatus,
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
                (Ok(value), Ok(())) => Ok(value),
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
    let now = SystemClock.now_ms();
    let expires_at = checked_expiry(now, ttl_ms)?;
    let conn = connect_file(path)?;
    renew_maintenance_owner_lease_on_connection(
        &conn,
        owner,
        lease_token,
        identity,
        now,
        expires_at,
    )
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
    let heartbeat = ProjectionLeaseHeartbeat::new(session, store_name, &lease.lease_token);
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

#[derive(Clone)]
struct ProjectionLeaseHeartbeat {
    db_path: PathBuf,
    owner: String,
    maintenance_lease_token: String,
    maintenance_identity: MaintenanceRuntimeIdentity,
    store_name: String,
    store_lease_token: String,
    ttl_ms: i64,
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
    fn new(session: &MaintenanceSession, store_name: &str, store_lease_token: &str) -> Self {
        Self {
            db_path: session.db_path.clone(),
            owner: session.owner.clone(),
            maintenance_lease_token: session.lease_token.clone(),
            maintenance_identity: session.identity.clone(),
            store_name: store_name.to_owned(),
            store_lease_token: store_lease_token.to_owned(),
            ttl_ms: session.options.lease_ttl_ms,
        }
    }

    fn renew(&self) -> Result<()> {
        renew_maintenance_owner_lease(
            &self.db_path,
            &self.owner,
            &self.maintenance_lease_token,
            self.ttl_ms,
            &self.maintenance_identity,
        )?;
        renew_projection_lease(
            &self.db_path,
            &self.store_name,
            &self.owner,
            &self.store_lease_token,
            self.ttl_ms,
        )
        .map(|_| ())
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
                Ok(()) => Ok(operation_result),
            }
        })
    }
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
        renew_maintenance_owner(session).map_err(MaintenanceStoreAttemptError::Fatal)?;
        renew_projection_lease(
            &session.db_path,
            store_name,
            &session.owner,
            lease_token,
            session.options.lease_ttl_ms,
        )
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
    renew_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        lease_token,
        session.options.lease_ttl_ms,
    )?;
    persist_store_failure(&session.db_path, store_name, display_name, kind, error)
}

fn failed_store_run_without_store_lease(
    session: &MaintenanceSession,
    store_name: &str,
    display_name: &str,
    kind: MaintenanceStoreFailureKind,
    error: KanbanError,
) -> Result<MaintenanceStoreRun> {
    renew_maintenance_owner(session)?;
    persist_store_failure(&session.db_path, store_name, display_name, kind, error)
}

fn persist_store_failure(
    path: &Path,
    store_name: &str,
    display_name: &str,
    kind: MaintenanceStoreFailureKind,
    error: KanbanError,
) -> Result<MaintenanceStoreRun> {
    let message = error.to_string();
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let changed = conn
        .execute(
            "UPDATE projection_store_state
             SET lifecycle_status='error',last_error=?1,updated_at=?2
             WHERE store_name=?3",
            params![message, now, store_name],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Storage(format!(
            "{display_name} projection state is missing"
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
    Ok(MaintenanceStoreRun {
        store_name: store.store_name,
        result: MaintenanceStoreResult::Failed { kind, message },
        lifecycle_status: store.lifecycle_status,
        fallback_reason: store.fallback_reason,
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
mod legacy_binding_recovery_tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        io::ErrorKind,
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
            ProjectionBatchReceipt, ProjectionPublishReceipt, ProjectionSnapshot, create_task,
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
        quarantined: BTreeMap<String, ProjectionArtifactEvidence>,
        quarantine_attempts: Vec<String>,
        after_prepare: Option<Box<dyn FnOnce() + Send>>,
        before_active_inspect: Option<Box<dyn FnOnce() + Send>>,
        promote_after_active_quarantine: Option<String>,
    }

    struct RecoveryBackend {
        descriptor: ProjectionStoreDescriptor,
        state: Mutex<RecoveryBackendState>,
    }

    impl RecoveryBackend {
        fn empty() -> Self {
            Self {
                descriptor: current_descriptor(),
                state: Mutex::new(RecoveryBackendState::default()),
            }
        }

        fn from_legacy_sqlite(
            path: &Path,
            active: bool,
            previous: bool,
            building: bool,
        ) -> anyhow::Result<Self> {
            let backend = Self::empty();
            let mut state = backend.state.lock().expect("recovery backend lock");
            if active {
                let evidence = legacy_evidence(path, ACTIVE, 7)?;
                state
                    .generations
                    .insert(ACTIVE.to_owned(), evidence.clone());
                state.active = Some(evidence);
            }
            if previous {
                state
                    .generations
                    .insert(PREVIOUS.to_owned(), legacy_evidence(path, PREVIOUS, 6)?);
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

        fn install_unknown_active(&self, path: &Path, generation: &str) -> anyhow::Result<()> {
            let evidence = evidence_for_descriptor(path, generation, 99, &self.descriptor)?;
            let mut state = self.state.lock().expect("recovery backend lock");
            state
                .generations
                .insert(generation.to_owned(), evidence.clone());
            state.active = Some(evidence);
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

        fn set_after_prepare(&self, hook: impl FnOnce() + Send + 'static) {
            self.state
                .lock()
                .expect("recovery backend lock")
                .after_prepare = Some(Box::new(hook));
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
    }

    impl ProjectionStoreBackend for RecoveryBackend {
        fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
            Ok(self.descriptor.clone())
        }

        fn prepare_snapshot(
            &self,
            snapshot: &ProjectionSnapshot,
        ) -> Result<ProjectionArtifactEvidence> {
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
            Ok(evidence)
        }

        fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
            Ok(ProjectionBatchReceipt {
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
            })
        }

        fn publish_generation(
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
            Ok(ProjectionPublishReceipt {
                active: prepared.clone(),
                retained_previous,
            })
        }

        fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
            let (active, hook) = {
                let mut state = self.state.lock().expect("recovery backend lock");
                (state.active.clone(), state.before_active_inspect.take())
            };
            if let Some(hook) = hook {
                hook();
            }
            Ok(active)
        }

        fn inspect_generation(
            &self,
            generation: &str,
        ) -> Result<Option<ProjectionArtifactEvidence>> {
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
            if let Some(evidence) = evidence {
                state.quarantined.insert(generation.to_owned(), evidence);
            }
            if removed_active && let Some(promoted) = state.promote_after_active_quarantine.clone()
            {
                state.active = state.generations.get(&promoted).cloned();
            }
            Ok(())
        }
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
        let backend = RecoveryBackend::empty();
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
        let backend = RecoveryBackend::empty();
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
        let backend = RecoveryBackend::empty();
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
        let backend = RecoveryBackend::empty();
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
            thread::sleep(Duration::from_millis(450));
        });
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 300,
            claim_ttl_ms: 100,
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
        let backend = RecoveryBackend::from_legacy_sqlite(&path, true, true, false)?;
        let lease = acquire_projection_lease(&path, STORE, "late-writer-owner", 20_000)?;
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (observed_tx, observed_rx) = mpsc::channel();
        let writer_path = path.clone();
        backend.set_before_active_inspect(move || {
            thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut reported_block = false;
                loop {
                    match DerivedStoreWriteGuard::acquire(
                        &writer_path,
                        &format!("{STORE}-projection-helper"),
                    ) {
                        Ok(_guard) => {
                            observed_tx
                                .send(sqlite_generation_ids(&writer_path).expect("writer readback"))
                                .expect("writer observation receiver");
                            return;
                        }
                        Err(error)
                            if error.kind() == ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            if !reported_block {
                                attempted_tx.send(()).expect("writer attempt receiver");
                                reported_block = true;
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("late helper writer lock failed: {error}"),
                    }
                }
            });
            attempted_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("late helper writer must block behind recovery");
        });

        assert!(recover_incompatible_projection_bindings(
            &path,
            STORE,
            "late-writer-owner",
            &lease.lease_token,
            &backend,
        )?);

        assert_eq!(
            observed_rx.recv_timeout(Duration::from_secs(2))?,
            (None, None, None),
            "the queued helper writer may acquire only after SQLite commits the recovery CAS"
        );
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
        let before = sqlite_recovery_control_snapshot(&path)?;

        recover_incompatible_projection_bindings(
            &path,
            STORE,
            "retained-previous-owner",
            &lease.lease_token,
            &backend,
        )
        .expect_err("mismatched retained previous evidence must fail closed");

        assert_eq!(sqlite_recovery_control_snapshot(&path)?, before);
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
            "NULL".to_owned()
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
                     building_fence_epoch=?3,building_provider=?4,
                     building_provider_fingerprint=?5,
                     building_canonical_count=?6,building_canonical_digest=?7,
                     building_delivery_count=?8,building_delivery_digest=?9,
                     building_phase=?10,building_corpus_schema=?11,
                     building_corpus_fingerprint=?12,building_embedding_model=?13,
                     building_embedding_dimensions=?14
                 WHERE store_name=?15",
                rusqlite::params![
                    generation.0,
                    generation.1,
                    generation.2,
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
                 legacy_checkpoint_cursor=777,last_success_at=4242
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
            String::new()
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

    #[derive(Debug, PartialEq, Eq)]
    struct CanonicalControlPlaneSnapshot {
        outbox: Vec<(i64, String, Option<String>, i64)>,
        derived_store: (i64, Option<i64>, Option<i64>, Option<String>, i64),
        delivery_count: i64,
        pending_deliveries: i64,
        published_deliveries: i64,
        claimed_deliveries: i64,
        checkpoint_cursor: i64,
        legacy_checkpoint_cursor: i64,
        delivery_controls: Vec<(i64, i64, i64, Option<String>)>,
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
        let mut delivery_control_statement = conn.prepare(
            "SELECT id,attempts,next_attempt_at,last_error
             FROM projection_deliveries WHERE store_name=?1 ORDER BY id",
        )?;
        let delivery_controls = delivery_control_statement
            .query_map([STORE], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(CanonicalControlPlaneSnapshot {
            outbox,
            derived_store,
            delivery_count,
            pending_deliveries,
            published_deliveries,
            claimed_deliveries,
            checkpoint_cursor,
            legacy_checkpoint_cursor,
            delivery_controls,
        })
    }
}

#[cfg(all(test, feature = "tantivy-backend"))]
mod tests {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    use tempfile::tempdir;

    use super::*;
    use crate::init::init_database;
    use crate::service::{
        CreateTask, ProjectionArtifactEvidence, ProjectionBatch, ProjectionBatchReceipt,
        ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreDescriptor, create_task,
    };

    struct TransientGenerationInspectStore {
        inner: TantivyProjectionStore,
        fail_next_inspect: AtomicBool,
        force_active_conflict: bool,
        descriptor_override: Option<ProjectionStoreDescriptor>,
    }

    impl TransientGenerationInspectStore {
        fn new(inner: TantivyProjectionStore) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(true),
                force_active_conflict: false,
                descriptor_override: None,
            }
        }

        fn with_descriptor(
            inner: TantivyProjectionStore,
            descriptor: ProjectionStoreDescriptor,
        ) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(false),
                force_active_conflict: false,
                descriptor_override: Some(descriptor),
            }
        }

        fn with_descriptor_and_active_conflict(
            inner: TantivyProjectionStore,
            descriptor: ProjectionStoreDescriptor,
        ) -> Self {
            Self {
                inner,
                fail_next_inspect: AtomicBool::new(false),
                force_active_conflict: true,
                descriptor_override: Some(descriptor),
            }
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
            self.inner.prepare_snapshot(snapshot)
        }

        fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
            self.inner.apply_batch(batch)
        }

        fn publish_generation(
            &self,
            expected_active: Option<&ProjectionArtifactEvidence>,
            prepared: &ProjectionArtifactEvidence,
        ) -> Result<ProjectionPublishReceipt> {
            if let Some(descriptor) = &self.descriptor_override
                && (expected_active
                    .is_some_and(|evidence| !artifact_matches_descriptor(evidence, descriptor))
                    || !artifact_matches_descriptor(prepared, descriptor))
            {
                return Err(KanbanError::Conflict(
                    "strict backend rejected publish evidence from another provider".to_owned(),
                ));
            }
            self.inner.publish_generation(expected_active, prepared)
        }

        fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
            if self.force_active_conflict {
                return Err(KanbanError::Conflict(
                    "strict backend found an unattributed incompatible active generation"
                        .to_owned(),
                ));
            }
            let active = self.inner.inspect_active()?;
            if let (Some(descriptor), Some(evidence)) = (&self.descriptor_override, &active)
                && !artifact_matches_descriptor(evidence, descriptor)
            {
                return Err(KanbanError::Conflict(
                    "strict backend rejected active evidence from another provider".to_owned(),
                ));
            }
            Ok(active)
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
            let evidence = self.inner.inspect_generation(generation)?;
            if let (Some(descriptor), Some(evidence)) = (&self.descriptor_override, &evidence)
                && !artifact_matches_descriptor(evidence, descriptor)
            {
                return Err(KanbanError::Conflict(
                    "strict backend rejected generation evidence from another provider".to_owned(),
                ));
            }
            Ok(evidence)
        }

        fn validate_generation_publication(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            self.inner.validate_generation_publication(expected)
        }

        fn repair_generation_publication(
            &self,
            expected: &ProjectionArtifactEvidence,
        ) -> Result<()> {
            self.inner.repair_generation_publication(expected)
        }

        fn validate_active_contents(&self, active: &ProjectionArtifactEvidence) -> Result<()> {
            self.inner.validate_active_contents(active)
        }

        fn quarantine_generation(&self, generation: &str) -> Result<()> {
            self.inner.quarantine_generation(generation)
        }

        fn abort_generation(&self, generation: &str) -> Result<()> {
            self.inner.abort_generation(generation)
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
        let heartbeat =
            ProjectionLeaseHeartbeat::new(&session, TANTIVY_TASKS_STORE, &lease.lease_token);

        heartbeat
            .run(|| {
                thread::sleep(Duration::from_millis(2_500));
                let conflict =
                    acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "competitor", 1_000)
                        .expect_err("heartbeat must prevent a competing store lease");
                assert!(matches!(conflict, KanbanError::Conflict(_)));
                Ok(())
            })?
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let status = projection_status(&db_path)?;
        assert!(status.maintenance_owner.active);
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
        let backend = TransientGenerationInspectStore::new(TantivyProjectionStore::new(&db_path)?);
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
        let heartbeat =
            ProjectionLeaseHeartbeat::new(&session, TANTIVY_TASKS_STORE, &lease.lease_token);

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
