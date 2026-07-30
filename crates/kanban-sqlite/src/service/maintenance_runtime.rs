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
    ProjectionRuntimeAvailability, ProjectionStatus, projection_status, storage, with_immediate_tx,
};
use super::{
    ProjectionStoreBackend, abort_incompatible_projection_generation, abort_projection_generation,
    acquire_projection_lease, begin_projection_generation, prepare_projection_snapshot_with,
    publish_projection_generation_with, reconcile_projection_generation_with,
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
            run_tantivy_once(self, false)?,
            #[cfg(feature = "oxigraph-backend")]
            run_oxigraph_once(self, false)?,
            run_lancedb_once(
                self,
                LANCEDB_LABEL_ATOMS_STORE,
                "LanceDB label atoms",
                false,
            )?,
            run_lancedb_once(self, LANCEDB_CHUNKS_STORE, "LanceDB task chunks", false)?,
        ];
        renew_maintenance_owner(self)?;
        self.report(stores)
    }

    pub fn rebuild(&mut self, store_name: &str) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        let store = match store_name {
            TANTIVY_TASKS_STORE => run_tantivy_once(self, true)?,
            OXIGRAPH_RELATIONS_STORE => run_oxigraph_once(self, true)?,
            LANCEDB_LABEL_ATOMS_STORE => {
                run_lancedb_once(self, store_name, "LanceDB label atoms", true)?
            }
            LANCEDB_CHUNKS_STORE => {
                run_lancedb_once(self, store_name, "LanceDB task chunks", true)?
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
        let stores = vec![
            #[cfg(feature = "tantivy-backend")]
            run_tantivy_once(self, true)?,
            #[cfg(feature = "oxigraph-backend")]
            run_oxigraph_once(self, true)?,
            run_lancedb_once(self, LANCEDB_LABEL_ATOMS_STORE, "LanceDB label atoms", true)?,
            run_lancedb_once(self, LANCEDB_CHUNKS_STORE, "LanceDB task chunks", true)?,
        ];
        renew_maintenance_owner(self)?;
        self.report(stores)
    }

    pub fn heartbeat(&mut self) -> Result<()> {
        renew_maintenance_owner(self)
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
                store.fallback_reason = Some(
                    match failure {
                        LanceDbProjectionFailureClass::Provider => "provider_unavailable",
                        LanceDbProjectionFailureClass::Backend
                        | LanceDbProjectionFailureClass::Delivery => "helper_unavailable",
                    }
                    .to_owned(),
                );
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
        store.fallback_reason = Some("physical_generation_unavailable".to_owned());
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
    force_rebuild: bool,
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
        run_projection_store_once(
            session,
            TANTIVY_TASKS_STORE,
            "Tantivy",
            &backend,
            force_rebuild,
        )
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = (session, force_rebuild);
        Err(KanbanError::InvalidInput(
            "unified Tantivy maintenance requires the tantivy-backend feature".to_owned(),
        ))
    }
}

fn run_oxigraph_once(
    session: &mut MaintenanceSession,
    force_rebuild: bool,
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
            force_rebuild,
        )
    }
    #[cfg(not(feature = "oxigraph-backend"))]
    {
        let _ = (session, force_rebuild);
        Err(KanbanError::InvalidInput(
            "unified Oxigraph maintenance requires the oxigraph-backend feature".to_owned(),
        ))
    }
}

fn run_lancedb_once(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    force_rebuild: bool,
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
    run_projection_store_once(session, store_name, display_name, &backend, force_rebuild)
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

fn run_projection_store_once(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    backend: &impl ProjectionStoreBackend,
    force_rebuild: bool,
) -> Result<MaintenanceStoreRun> {
    let lease = acquire_projection_lease(
        &session.db_path,
        store_name,
        &session.owner,
        session.options.lease_ttl_ms,
    )?;
    let heartbeat = ProjectionLeaseHeartbeat::new(session, store_name, &lease.lease_token);
    let operation = heartbeat.run(|| {
        run_projection_store_operation(
            session,
            store_name,
            display_name,
            &lease.lease_token,
            backend,
            force_rebuild,
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

fn run_projection_store_operation(
    session: &mut MaintenanceSession,
    store_name: &str,
    display_name: &str,
    lease_token: &str,
    backend: &impl ProjectionStoreBackend,
    force_rebuild: bool,
) -> MaintenanceStoreAttempt<MaintenanceStoreRun> {
    let mut action = "idle".to_owned();
    let status =
        maintenance_status(&session.db_path).map_err(MaintenanceStoreAttemptError::Fatal)?;
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == store_name)
        .ok_or_else(|| {
            MaintenanceStoreAttemptError::Fatal(KanbanError::Storage(format!(
                "{display_name} projection state is missing"
            )))
        })?;
    let physical_rebuild =
        store.fallback_reason.as_deref() == Some("physical_generation_unavailable");
    if force_rebuild
        || physical_rebuild
        || store.active_generation.is_none()
        || store.building_generation.is_some()
    {
        if store.building_generation.is_none() {
            begin_projection_generation(
                &session.db_path,
                store_name,
                &session.owner,
                lease_token,
                backend,
            )
            .map_err(|error| MaintenanceStoreAttemptError::Store {
                kind: MaintenanceStoreFailureKind::Provider,
                error,
            })?;
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
        match store.building_phase.as_deref() {
            Some("snapshotting") => {
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
                    let backend_still_matches_building = descriptor.store_name == store_name
                        && store.building_provider.as_deref() == Some(descriptor.provider.as_str())
                        && store.building_provider_fingerprint.as_deref()
                            == Some(descriptor.provider_fingerprint.as_str());
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
                    .map_err(|error| MaintenanceStoreAttemptError::Store {
                        kind: MaintenanceStoreFailureKind::Provider,
                        error,
                    })?;
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
        if physical_rebuild {
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
