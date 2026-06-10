use std::{path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct AppState {
    db_path: PathBuf,
    default_actor: String,
}

impl AppState {
    pub fn new(db_path: impl Into<PathBuf>, default_actor: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
            default_actor: default_actor.into(),
        }
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    pub fn default_actor(&self) -> &str {
        &self.default_actor
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

#[cfg(feature = "tantivy-backend")]
async fn run_search_sync_once(db_path: PathBuf, board: String) {
    let _ = tokio::task::spawn_blocking(move || kanban_sqlite::sync_search_index(db_path, &board))
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
