use std::{path::PathBuf, time::Duration};

use kanban_core::Locale;

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
pub struct SearchSyncConfig {
    board: String,
    interval: Duration,
}

impl SearchSyncConfig {
    pub fn new(board: impl Into<String>, interval: Duration) -> Self {
        Self {
            board: board.into(),
            interval,
        }
    }

    pub fn disabled(board: impl Into<String>) -> Self {
        Self::new(board, Duration::ZERO)
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn board(&self) -> &str {
        &self.board
    }
}

pub fn search_sync_task_enabled(config: &SearchSyncConfig) -> bool {
    #[cfg(feature = "tantivy-backend")]
    {
        !config.interval.is_zero()
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = config;
        false
    }
}

pub fn spawn_search_sync_task(
    state: AppState,
    config: SearchSyncConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    if !search_sync_task_enabled(&config) {
        return None;
    }

    #[cfg(feature = "tantivy-backend")]
    {
        Some(tokio::spawn(async move {
            run_search_sync_once(state.db_path.clone(), config.board.clone()).await;
            loop {
                tokio::time::sleep(config.interval).await;
                run_search_sync_once(state.db_path.clone(), config.board.clone()).await;
            }
        }))
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = (state, config);
        None
    }
}

pub(crate) fn spawn_search_sync_task_until_shutdown(
    state: AppState,
    config: SearchSyncConfig,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Option<tokio::task::JoinHandle<()>> {
    if !search_sync_task_enabled(&config) {
        return None;
    }

    #[cfg(feature = "tantivy-backend")]
    {
        Some(tokio::spawn(async move {
            let mut shutdown = shutdown;
            run_search_sync_once(state.db_path.clone(), config.board.clone()).await;
            loop {
                tokio::select! {
                    _ = shutdown.changed() => break,
                    _ = tokio::time::sleep(config.interval) => {}
                }
                if *shutdown.borrow() {
                    break;
                }
                run_search_sync_once(state.db_path.clone(), config.board.clone()).await;
            }
        }))
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = (state, config, shutdown);
        None
    }
}

#[cfg(feature = "tantivy-backend")]
async fn run_search_sync_once(db_path: PathBuf, board: String) {
    let _ =
        tokio::task::spawn_blocking(move || kanban_sqlite::api::sync_search_index(db_path, &board))
            .await;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{SearchSyncConfig, search_sync_task_enabled};

    #[test]
    fn search_sync_zero_interval_is_disabled() {
        let config = SearchSyncConfig::disabled("default");

        assert_eq!(config.board(), "default");
        assert_eq!(config.interval(), Duration::ZERO);
        assert!(!search_sync_task_enabled(&config));
    }

    #[cfg(not(feature = "tantivy-backend"))]
    #[test]
    fn search_sync_is_disabled_without_tantivy_backend() {
        let config = SearchSyncConfig::new("default", Duration::from_millis(5_000));

        assert!(!search_sync_task_enabled(&config));
    }

    #[cfg(feature = "tantivy-backend")]
    #[test]
    fn search_sync_positive_interval_is_enabled_with_tantivy_backend() {
        let config = SearchSyncConfig::new("default", Duration::from_millis(5_000));

        assert!(search_sync_task_enabled(&config));
    }
}
