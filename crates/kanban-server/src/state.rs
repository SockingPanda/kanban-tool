use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use kanban_service::{ApplicationService, TursoApplicationStore};
use kanban_service::{KanbanError, Result};

pub(crate) type HostApplicationService = ApplicationService<TursoApplicationStore>;

#[derive(Clone)]
pub struct AppState {
    application: HostApplicationService,
    vector_store: TursoApplicationStore,
    db_path: Arc<PathBuf>,
    attachment_root: Arc<PathBuf>,
    default_actor: Arc<str>,
}

impl AppState {
    /// Open and initialize the canonical Turso database owned by this host.
    pub async fn open(
        db_path: impl Into<PathBuf>,
        default_actor: impl Into<String>,
    ) -> Result<Self> {
        Self::open_with_run_log_root(db_path, default_actor, None).await
    }

    /// Open the canonical database and optionally configure the trusted run-log root.
    pub async fn open_with_run_log_root(
        db_path: impl Into<PathBuf>,
        default_actor: impl Into<String>,
        run_log_root: Option<PathBuf>,
    ) -> Result<Self> {
        let db_path = db_path.into();
        ensure_parent_directory(&db_path).await?;
        let run_log_root = match run_log_root {
            Some(path) => Some(Arc::new(ensure_run_log_root(&path).await?)),
            None => None,
        };
        let attachment_root = Arc::new(ensure_attachment_root(&db_path).await?);
        let application_store =
            TursoApplicationStore::open_with_roots(&db_path, run_log_root, attachment_root.clone())
                .await
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
        let vector_store = application_store.clone();
        Ok(Self {
            application: ApplicationService::new(application_store),
            vector_store,
            db_path: Arc::new(db_path),
            attachment_root,
            default_actor: Arc::from(default_actor.into()),
        })
    }

    pub(crate) fn application(&self) -> &HostApplicationService {
        &self.application
    }

    pub(crate) fn vector_store(&self) -> &TursoApplicationStore {
        &self.vector_store
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub fn attachment_root(&self) -> &Path {
        self.attachment_root.as_path()
    }

    pub fn default_actor(&self) -> &str {
        &self.default_actor
    }
}

async fn ensure_parent_directory(db_path: &Path) -> Result<()> {
    let parent = db_path.parent().ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "database path has no parent directory: {}",
            db_path.display()
        ))
    })?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| KanbanError::Storage(error.to_string()))
}

async fn ensure_run_log_root(path: &Path) -> Result<PathBuf> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    tokio::fs::canonicalize(path)
        .await
        .map_err(|error| KanbanError::Storage(error.to_string()))
}

async fn ensure_attachment_root(db_path: &Path) -> Result<PathBuf> {
    let parent = db_path.parent().ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "database path has no parent directory: {}",
            db_path.display()
        ))
    })?;
    let root = parent.join("attachments");
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    tokio::fs::create_dir_all(root.join(".trash"))
        .await
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    tokio::fs::canonicalize(root)
        .await
        .map_err(|error| KanbanError::Storage(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::{Builder, TempDir};

    use super::{AppState, ensure_run_log_root};

    fn relative_temp_dir() -> (TempDir, PathBuf) {
        let directory = Builder::new()
            .prefix(".kanban-run-log-root-")
            .tempdir_in(".")
            .expect("temporary run-log root");
        let relative = directory
            .path()
            .file_name()
            .expect("temporary directory name")
            .into();
        (directory, relative)
    }

    #[tokio::test]
    async fn open_without_dispatcher_keeps_run_log_root_unset() {
        let db = tempfile::tempdir().expect("temporary database directory");
        let state = AppState::open(db.path().join("kanban.db"), "test")
            .await
            .expect("open state");

        let _ = state;
    }

    #[tokio::test]
    async fn open_canonicalizes_relative_run_log_root() {
        let db = tempfile::tempdir().expect("temporary database directory");
        let (root, relative_root) = relative_temp_dir();
        let expected = std::fs::canonicalize(root.path()).expect("canonical run-log root");
        let actual = ensure_run_log_root(&relative_root)
            .await
            .expect("canonicalize run-log root");

        assert_eq!(actual, expected);

        let _ = db;
    }
}
