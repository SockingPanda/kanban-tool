use std::{path::PathBuf, time::Duration};

#[cfg(test)]
use std::future::Future;

use kanban_core::Locale;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct AppState {
    db_path: PathBuf,
    default_actor: String,
    locale: Locale,
    vector_config_path: Option<PathBuf>,
    vector_helper_path: Option<PathBuf>,
    graph_helper_path: Option<PathBuf>,
}

impl AppState {
    pub fn new(db_path: impl Into<PathBuf>, default_actor: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
            default_actor: default_actor.into(),
            locale: Locale::En,
            vector_config_path: None,
            vector_helper_path: None,
            graph_helper_path: None,
        }
    }

    pub fn with_vector_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.vector_config_path = Some(path.into());
        self
    }

    pub fn with_vector_helper_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.vector_helper_path = Some(path.into());
        self
    }

    pub fn with_graph_helper_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.graph_helper_path = Some(path.into());
        self
    }

    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = locale;
        self
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    pub fn application(&self) -> kanban_sqlite::application::SqliteApplication {
        kanban_sqlite::application::SqliteApplication::new(self.db_path.clone())
    }

    pub fn default_actor(&self) -> &str {
        &self.default_actor
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn vector_config_path(&self) -> Option<&std::path::Path> {
        self.vector_config_path.as_deref()
    }

    pub fn vector_helper_path(&self) -> Option<&std::path::Path> {
        self.vector_helper_path.as_deref()
    }

    pub fn graph_helper_path(&self) -> Option<&std::path::Path> {
        self.graph_helper_path.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct MaintenanceConfig {
    owner: String,
    interval: Duration,
    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    options: kanban_sqlite::api::MaintenanceRunOptions,
}

impl MaintenanceConfig {
    pub fn new(owner: impl Into<String>, interval: Duration) -> Self {
        Self {
            owner: owner.into(),
            interval,
            #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
            options: kanban_sqlite::api::MaintenanceRunOptions::default(),
        }
    }

    pub fn disabled(owner: impl Into<String>) -> Self {
        Self::new(owner, Duration::ZERO)
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    #[cfg(all(test, any(feature = "tantivy-backend", feature = "oxigraph-backend")))]
    fn with_options(
        owner: impl Into<String>,
        interval: Duration,
        options: kanban_sqlite::api::MaintenanceRunOptions,
    ) -> Self {
        Self {
            owner: owner.into(),
            interval,
            options,
        }
    }
}

pub fn maintenance_task_enabled(config: &MaintenanceConfig) -> bool {
    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    {
        !config.interval.is_zero()
    }
    #[cfg(not(any(feature = "tantivy-backend", feature = "oxigraph-backend")))]
    {
        let _ = config;
        false
    }
}

pub fn spawn_maintenance_task(
    state: AppState,
    config: MaintenanceConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    if !maintenance_task_enabled(&config) {
        return None;
    }

    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    {
        Some(tokio::spawn(async move {
            run_maintenance_session_until_shutdown(state.db_path, config, CancellationToken::new())
                .await;
        }))
    }
    #[cfg(not(any(feature = "tantivy-backend", feature = "oxigraph-backend")))]
    {
        let _ = (state, config);
        None
    }
}

pub(crate) fn spawn_maintenance_task_until_shutdown(
    state: AppState,
    config: MaintenanceConfig,
    shutdown: CancellationToken,
) -> Option<tokio::task::JoinHandle<()>> {
    if !maintenance_task_enabled(&config) {
        return None;
    }

    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    {
        Some(tokio::spawn(async move {
            run_maintenance_session_until_shutdown(state.db_path, config, shutdown).await;
        }))
    }
    #[cfg(not(any(feature = "tantivy-backend", feature = "oxigraph-backend")))]
    {
        let _ = (state, config, shutdown);
        None
    }
}

#[cfg(test)]
async fn run_maintenance_loop_until_shutdown<Run, RunFut, Wait, WaitFut>(
    shutdown: CancellationToken,
    mut run_once: Run,
    mut wait_interval: Wait,
) where
    Run: FnMut() -> RunFut,
    RunFut: Future<Output = ()>,
    Wait: FnMut() -> WaitFut,
    WaitFut: Future<Output = ()>,
{
    run_once().await;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = wait_interval() => {}
        }
        if shutdown.is_cancelled() {
            break;
        }
        run_once().await;
    }
}

#[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
async fn run_maintenance_session_until_shutdown(
    db_path: PathBuf,
    config: MaintenanceConfig,
    shutdown: CancellationToken,
) {
    loop {
        if shutdown.is_cancelled() {
            return;
        }
        let start_path = db_path.clone();
        let start_owner = config.owner.clone();
        let start_options = config.options.clone();
        let started = tokio::task::spawn_blocking(move || {
            kanban_sqlite::api::MaintenanceSession::start(
                start_path,
                &start_owner,
                kanban_sqlite::api::MaintenanceMode::Continuous,
                start_options,
            )
        })
        .await;
        let mut session = match started {
            Ok(Ok(session)) => session,
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "Projection maintenance owner could not start");
                if wait_for_retry(&shutdown, config.interval).await {
                    return;
                }
                continue;
            }
            Err(error) => {
                tracing::warn!(error = %error, "Projection maintenance startup task failed");
                if wait_for_retry(&shutdown, config.interval).await {
                    return;
                }
                continue;
            }
        };
        let mut restart = false;
        while !shutdown.is_cancelled() {
            let run = tokio::task::spawn_blocking(move || {
                let result = session.run_once();
                (session, result)
            })
            .await;
            let (next_session, result) = match run {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(error = %error, "Projection maintenance worker task failed");
                    return;
                }
            };
            session = next_session;
            if let Err(error) = result {
                tracing::warn!(error = %error, "Projection maintenance pass failed; owner will be reacquired");
                restart = true;
                break;
            }
            let (next_session, wait_result) =
                wait_with_maintenance_heartbeats(session, config.interval, &shutdown).await;
            session = next_session;
            match wait_result {
                MaintenanceWait::Continue => {}
                MaintenanceWait::Shutdown => break,
                MaintenanceWait::Reacquire(error) => {
                    tracing::warn!(error = %error, "Projection maintenance heartbeat failed; owner will be reacquired");
                    restart = true;
                    break;
                }
            }
        }
        match tokio::task::spawn_blocking(move || session.finish()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "Projection maintenance owner release failed");
            }
            Err(error) => {
                tracing::warn!(error = %error, "Projection maintenance release task failed");
            }
        }
        if shutdown.is_cancelled() {
            return;
        }
        if restart && wait_for_retry(&shutdown, config.interval).await {
            return;
        }
    }
}

#[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
enum MaintenanceWait {
    Continue,
    Shutdown,
    Reacquire(kanban_core::KanbanError),
}

#[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
async fn wait_with_maintenance_heartbeats(
    mut session: kanban_sqlite::api::MaintenanceSession,
    interval: Duration,
    shutdown: &CancellationToken,
) -> (kanban_sqlite::api::MaintenanceSession, MaintenanceWait) {
    let deadline = tokio::time::Instant::now() + interval;
    let heartbeat_interval =
        Duration::from_millis((session.lease_ttl_ms() / 3).clamp(1, 60_000) as u64);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return (session, MaintenanceWait::Continue);
        }
        let wait = (deadline - now).min(heartbeat_interval);
        tokio::select! {
            _ = shutdown.cancelled() => return (session, MaintenanceWait::Shutdown),
            _ = tokio::time::sleep(wait) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return (session, MaintenanceWait::Continue);
        }
        if let Err(error) = session.heartbeat() {
            return (session, MaintenanceWait::Reacquire(error));
        }
    }
}

#[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
async fn wait_for_retry(shutdown: &CancellationToken, interval: Duration) -> bool {
    let delay = interval
        .min(Duration::from_secs(5))
        .max(Duration::from_millis(1));
    tokio::select! {
        _ = shutdown.cancelled() => true,
        _ = tokio::time::sleep(delay) => false,
    }
}
#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use tokio_util::sync::CancellationToken;

    use super::{MaintenanceConfig, maintenance_task_enabled, run_maintenance_loop_until_shutdown};

    #[test]
    fn maintenance_zero_interval_is_disabled() {
        let config = MaintenanceConfig::disabled("server-test");

        assert_eq!(config.owner(), "server-test");
        assert_eq!(config.interval(), Duration::ZERO);
        assert!(!maintenance_task_enabled(&config));
    }

    #[cfg(not(any(feature = "tantivy-backend", feature = "oxigraph-backend")))]
    #[test]
    fn maintenance_is_disabled_without_tantivy_backend() {
        let config = MaintenanceConfig::new("server-test", Duration::from_millis(5_000));

        assert!(!maintenance_task_enabled(&config));
    }

    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    #[test]
    fn maintenance_positive_interval_is_enabled_with_tantivy_backend() {
        let config = MaintenanceConfig::new("server-test", Duration::from_millis(5_000));

        assert!(maintenance_task_enabled(&config));
    }

    #[cfg(any(feature = "tantivy-backend", feature = "oxigraph-backend"))]
    #[tokio::test]
    async fn maintenance_reacquires_after_conflict_heartbeats_and_releases_on_shutdown()
    -> anyhow::Result<()> {
        use kanban_sqlite::api::{
            MaintenanceMode, MaintenanceRunOptions, MaintenanceSession, maintenance_status,
        };
        use kanban_sqlite::init::init_database;

        let temp = tempfile::tempdir()?;
        let db_path = temp.path().join("kanban.db");
        init_database(&db_path, "test")?;
        let blocker = MaintenanceSession::start(
            &db_path,
            "blocker",
            MaintenanceMode::Continuous,
            MaintenanceRunOptions::default(),
        )?;
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(super::run_maintenance_session_until_shutdown(
            db_path.clone(),
            MaintenanceConfig::with_options(
                "server-test",
                Duration::from_millis(500),
                MaintenanceRunOptions {
                    lease_ttl_ms: 120,
                    claim_ttl_ms: 60,
                    batch_size: 20,
                },
            ),
            shutdown.clone(),
        ));
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(
            maintenance_status(&db_path)?
                .maintenance_owner
                .owner
                .as_deref(),
            Some("blocker")
        );
        blocker.finish()?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if maintenance_status(&db_path)?
                .maintenance_owner
                .owner
                .as_deref()
                == Some("server-test")
            {
                break;
            }
            anyhow::ensure!(
                tokio::time::Instant::now() < deadline,
                "server maintenance owner was not reacquired"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(350)).await;
        let conflict = MaintenanceSession::start(
            &db_path,
            "intruder",
            MaintenanceMode::Once,
            MaintenanceRunOptions {
                lease_ttl_ms: 120,
                claim_ttl_ms: 60,
                batch_size: 20,
            },
        )
        .expect_err("heartbeat must keep the server owner live");
        assert!(matches!(conflict, kanban_core::KanbanError::Conflict(_)));

        shutdown.cancel();
        task.await?;
        assert!(
            maintenance_status(&db_path)?
                .maintenance_owner
                .owner
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn maintenance_loop_stops_when_shutdown_is_cancelled_after_interval_wake() {
        let shutdown = CancellationToken::new();
        let wait_shutdown = shutdown.clone();
        let sync_count = Arc::new(AtomicUsize::new(0));

        run_maintenance_loop_until_shutdown(
            shutdown,
            {
                let sync_count = Arc::clone(&sync_count);
                move || {
                    let sync_count = Arc::clone(&sync_count);
                    async move {
                        sync_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
            },
            move || {
                let wait_shutdown = wait_shutdown.clone();
                async move {
                    wait_shutdown.cancel();
                }
            },
        )
        .await;

        assert_eq!(sync_count.load(Ordering::SeqCst), 1);
    }
}
