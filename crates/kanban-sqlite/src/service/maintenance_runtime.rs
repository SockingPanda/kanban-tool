use std::path::{Path, PathBuf};
#[cfg(feature = "tantivy-backend")]
use std::{sync::mpsc, thread, time::Duration};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use kanban_indexer::TANTIVY_TASKS_STORE;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::connect_file;

use super::{ProjectionStatus, projection_status, storage, with_immediate_tx};
#[cfg(feature = "tantivy-backend")]
use super::{
    ProjectionStoreBackend, acquire_projection_lease, begin_projection_generation,
    prepare_projection_snapshot_with, publish_projection_generation_with,
    reconcile_projection_generation_with, recover_projection_generation_with,
    release_projection_lease, renew_projection_lease, run_projection_batch_with,
    tantivy_projection::TantivyProjectionStore, validate_physical_active_artifact_with,
};

pub const DEFAULT_MAINTENANCE_LEASE_TTL_MS: i64 = 3_600_000;
pub const DEFAULT_MAINTENANCE_CLAIM_TTL_MS: i64 = 300_000;
pub const DEFAULT_MAINTENANCE_BATCH_SIZE: usize = 250;
#[cfg(feature = "tantivy-backend")]
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
pub struct MaintenanceStoreRun {
    pub store_name: String,
    pub action: String,
    pub processed: usize,
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

#[derive(Debug)]
pub struct MaintenanceSession {
    db_path: PathBuf,
    owner: String,
    lease_token: String,
    mode: MaintenanceMode,
    options: MaintenanceRunOptions,
    released: bool,
}

impl MaintenanceSession {
    pub fn start(
        path: impl AsRef<Path>,
        owner: &str,
        mode: MaintenanceMode,
        options: MaintenanceRunOptions,
    ) -> Result<Self> {
        validate_options(owner, &options)?;
        let db_path = path.as_ref().to_path_buf();
        drop(super::maintenance::connect_existing_database(&db_path)?);
        let lease_token = acquire_maintenance_owner(&db_path, owner, mode, options.lease_ttl_ms)?;
        Ok(Self {
            db_path,
            owner: owner.to_owned(),
            lease_token,
            mode,
            options,
            released: false,
        })
    }

    pub fn run_once(&mut self) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        let stores = vec![run_tantivy_once(self, false)?];
        self.report(stores)
    }

    pub fn rebuild(&mut self, store_name: &str) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        let store = match store_name {
            TANTIVY_TASKS_STORE => run_tantivy_once(self, true)?,
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "projection store {store_name} is not yet wired to the unified maintenance runtime"
                )));
            }
        };
        self.report(vec![store])
    }

    pub fn rebuild_all(&mut self) -> Result<MaintenanceRunReport> {
        renew_maintenance_owner(self)?;
        let stores = vec![run_tantivy_once(self, true)?];
        self.report(stores)
    }

    pub fn heartbeat(&mut self) -> Result<()> {
        renew_maintenance_owner(self)
    }

    pub fn lease_ttl_ms(&self) -> i64 {
        self.options.lease_ttl_ms
    }

    pub fn finish(mut self) -> Result<()> {
        let result = release_maintenance_owner(&self.db_path, &self.owner, &self.lease_token);
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
            let _ = release_maintenance_owner(&self.db_path, &self.owner, &self.lease_token);
        }
    }
}

pub fn maintenance_status(path: impl AsRef<Path>) -> Result<ProjectionStatus> {
    let path = path.as_ref();
    let status = projection_status(path)?;
    #[cfg(feature = "tantivy-backend")]
    {
        let mut status = status;
        enrich_tantivy_physical_health(path, &mut status)?;
        Ok(status)
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        Ok(status)
    }
}

#[cfg(feature = "tantivy-backend")]
fn enrich_tantivy_physical_health(path: &Path, status: &mut ProjectionStatus) -> Result<()> {
    let Some(store) = status
        .stores
        .iter_mut()
        .find(|store| store.store_name == TANTIVY_TASKS_STORE && store.control_plane == "v2")
    else {
        return Ok(());
    };
    let Some(generation) = store.active_generation.as_deref() else {
        return Ok(());
    };
    let backend = TantivyProjectionStore::new(path)?;
    let physical = validate_physical_active_artifact_with(path, TANTIVY_TASKS_STORE, &backend)
        .and_then(|evidence| {
            evidence.ok_or_else(|| {
                KanbanError::Storage(format!(
                    "active Tantivy generation {generation} is missing from SQLite"
                ))
            })
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

fn acquire_maintenance_owner(
    path: &Path,
    owner: &str,
    mode: MaintenanceMode,
    ttl_ms: i64,
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
                     started_at=?5,last_heartbeat_at=?5,updated_at=?5
                 WHERE singleton=1
                   AND (lease_token IS NULL OR lease_expires_at<=?5)",
                params![owner, lease_token, expires_at, mode.as_str(), now],
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
    )
}

fn renew_maintenance_owner_lease(
    path: &Path,
    owner: &str,
    lease_token: &str,
    ttl_ms: i64,
) -> Result<()> {
    let now = SystemClock.now_ms();
    let expires_at = checked_expiry(now, ttl_ms)?;
    let conn = connect_file(path)?;
    let changed = conn
        .execute(
            "UPDATE projection_maintenance_owner
             SET lease_expires_at=?1,last_heartbeat_at=?2,updated_at=?2
             WHERE singleton=1 AND owner=?3 AND lease_token=?4
               AND lease_expires_at>?2",
            params![expires_at, now, owner, lease_token],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Conflict(
            "projection maintenance owner lease is stale".to_owned(),
        ));
    }
    Ok(())
}

fn release_maintenance_owner(path: &Path, owner: &str, lease_token: &str) -> Result<()> {
    let now = SystemClock.now_ms();
    let conn = connect_file(path)?;
    let changed = conn
        .execute(
            "UPDATE projection_maintenance_owner
         SET owner=NULL,lease_token=NULL,lease_expires_at=NULL,mode=NULL,
             started_at=NULL,last_heartbeat_at=NULL,updated_at=?1
         WHERE singleton=1 AND owner=?2 AND lease_token=?3",
            params![now, owner, lease_token],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::Conflict(
            "projection maintenance owner token is stale".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
fn run_tantivy_once(
    session: &mut MaintenanceSession,
    force_rebuild: bool,
) -> Result<MaintenanceStoreRun> {
    let backend = TantivyProjectionStore::new(&session.db_path)?;
    let lease = acquire_projection_lease(
        &session.db_path,
        TANTIVY_TASKS_STORE,
        &session.owner,
        session.options.lease_ttl_ms,
    )?;
    let heartbeat = ProjectionLeaseHeartbeat::new(session, &lease.lease_token);
    let operation = heartbeat.run(|| {
        let mut action = "idle".to_owned();
        let status = maintenance_status(&session.db_path)?;
        let store = status
            .stores
            .iter()
            .find(|store| store.store_name == TANTIVY_TASKS_STORE)
            .ok_or_else(|| {
                KanbanError::Storage("Tantivy projection state is missing".to_owned())
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
                    TANTIVY_TASKS_STORE,
                    &session.owner,
                    &lease.lease_token,
                    &backend,
                )?;
            }
            let rebuilding = projection_status(&session.db_path)?;
            let store = rebuilding
                .stores
                .iter()
                .find(|store| store.store_name == TANTIVY_TASKS_STORE)
                .expect("projection store seed is stable");
            match store.building_phase.as_deref() {
                Some("snapshotting") => {
                    prepare_projection_snapshot_with(
                        &session.db_path,
                        TANTIVY_TASKS_STORE,
                        &session.owner,
                        &lease.lease_token,
                        &backend,
                    )?;
                }
                Some("prepared" | "store_published") => {}
                other => {
                    return Err(KanbanError::Conflict(format!(
                        "unsupported Tantivy rebuilding phase {other:?}"
                    )));
                }
            }
            let processed = catch_up_generation(session, &lease.lease_token, &backend)?;
            let rebuilding = projection_status(&session.db_path)?;
            let store = rebuilding
                .stores
                .iter()
                .find(|store| store.store_name == TANTIVY_TASKS_STORE)
                .expect("projection store seed is stable");
            let physical_active = backend.inspect_active()?;
            let building_is_physically_active = store
                .building_generation
                .as_deref()
                .zip(physical_active.as_ref())
                .is_some_and(|(building, active)| active.manifest.generation == building);
            if store.building_phase.as_deref() == Some("store_published")
                || building_is_physically_active
            {
                reconcile_projection_generation_with(
                    &session.db_path,
                    TANTIVY_TASKS_STORE,
                    &session.owner,
                    &lease.lease_token,
                    &backend,
                )?;
                action = "generation_reconciled".to_owned();
            } else {
                if physical_rebuild {
                    recover_projection_generation_with(
                        &session.db_path,
                        TANTIVY_TASKS_STORE,
                        &session.owner,
                        &lease.lease_token,
                        &backend,
                    )?;
                    action = "generation_recovered".to_owned();
                } else {
                    publish_projection_generation_with(
                        &session.db_path,
                        TANTIVY_TASKS_STORE,
                        &session.owner,
                        &lease.lease_token,
                        &backend,
                    )?;
                    action = "generation_published".to_owned();
                }
            }
            return store_run(&session.db_path, action, processed);
        }
        let batch = run_projection_batch_with(
            &session.db_path,
            TANTIVY_TASKS_STORE,
            &session.owner,
            &lease.lease_token,
            session.options.claim_ttl_ms,
            session.options.batch_size,
            &backend,
        )?;
        let processed = batch.items.len();
        if processed > 0 {
            action = "batch_applied".to_owned();
        }
        store_run(&session.db_path, action, processed)
    });
    let release = release_projection_lease(
        &session.db_path,
        TANTIVY_TASKS_STORE,
        &session.owner,
        &lease.lease_token,
    );
    match (operation, release) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

#[cfg(feature = "tantivy-backend")]
#[derive(Debug, Clone)]
struct ProjectionLeaseHeartbeat {
    db_path: PathBuf,
    owner: String,
    maintenance_lease_token: String,
    store_lease_token: String,
    ttl_ms: i64,
}

#[cfg(feature = "tantivy-backend")]
impl ProjectionLeaseHeartbeat {
    fn new(session: &MaintenanceSession, store_lease_token: &str) -> Self {
        Self {
            db_path: session.db_path.clone(),
            owner: session.owner.clone(),
            maintenance_lease_token: session.lease_token.clone(),
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
        )?;
        renew_projection_lease(
            &self.db_path,
            TANTIVY_TASKS_STORE,
            &self.owner,
            &self.store_lease_token,
            self.ttl_ms,
        )
        .map(|_| ())
    }

    fn run<T>(&self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
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
            match (operation_result, heartbeat_result) {
                (Err(error), _) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Ok(value), Ok(())) => Ok(value),
            }
        })
    }
}

#[cfg(not(feature = "tantivy-backend"))]
fn run_tantivy_once(
    _session: &mut MaintenanceSession,
    _force_rebuild: bool,
) -> Result<MaintenanceStoreRun> {
    Err(KanbanError::InvalidInput(
        "unified Tantivy maintenance requires the tantivy-backend feature".to_owned(),
    ))
}

#[cfg(feature = "tantivy-backend")]
fn catch_up_generation(
    session: &mut MaintenanceSession,
    lease_token: &str,
    backend: &TantivyProjectionStore,
) -> Result<usize> {
    let mut processed = 0;
    for _ in 0..MAX_REBUILD_CATCH_UP_BATCHES {
        renew_maintenance_owner(session)?;
        renew_projection_lease(
            &session.db_path,
            TANTIVY_TASKS_STORE,
            &session.owner,
            lease_token,
            session.options.lease_ttl_ms,
        )?;
        let batch = run_projection_batch_with(
            &session.db_path,
            TANTIVY_TASKS_STORE,
            &session.owner,
            lease_token,
            session.options.claim_ttl_ms,
            session.options.batch_size,
            backend,
        )?;
        if batch.items.is_empty() {
            return Ok(processed);
        }
        processed += batch.items.len();
    }
    Err(KanbanError::Conflict(
        "Tantivy generation catch-up did not converge within the safety bound".to_owned(),
    ))
}

#[cfg(feature = "tantivy-backend")]
fn store_run(path: &Path, action: String, processed: usize) -> Result<MaintenanceStoreRun> {
    let status = maintenance_status(path)?;
    let store = status
        .stores
        .into_iter()
        .find(|store| store.store_name == TANTIVY_TASKS_STORE)
        .ok_or_else(|| KanbanError::Storage("Tantivy projection state is missing".to_owned()))?;
    Ok(MaintenanceStoreRun {
        store_name: store.store_name,
        action,
        processed,
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

#[cfg(all(test, feature = "tantivy-backend"))]
mod tests {
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::init::init_database;
    use crate::service::{CreateTask, create_task};

    #[test]
    fn physical_operation_heartbeat_keeps_both_leases_fenced() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "tester")?;
        let options = MaintenanceRunOptions {
            lease_ttl_ms: 120,
            claim_ttl_ms: 30,
            batch_size: 1,
        };
        let session =
            MaintenanceSession::start(&db_path, "heartbeat-owner", MaintenanceMode::Once, options)?;
        let lease =
            acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "heartbeat-owner", 120)?;
        let heartbeat = ProjectionLeaseHeartbeat::new(&session, &lease.lease_token);

        heartbeat.run(|| {
            thread::sleep(Duration::from_millis(350));
            let conflict =
                acquire_projection_lease(&db_path, TANTIVY_TASKS_STORE, "competitor", 120)
                    .expect_err("heartbeat must prevent a competing store lease");
            assert!(matches!(conflict, KanbanError::Conflict(_)));
            Ok(())
        })?;
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
        catch_up_generation(&mut crashed, &lease.lease_token, &backend)?;
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
        assert_eq!(report.stores[0].action, "generation_reconciled");
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
}
