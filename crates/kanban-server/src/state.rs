use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use kanban_application::ApplicationService;
use kanban_core::{KanbanError, Result};
use kanban_store_turso::TursoStore;

use crate::adapter::TursoApplicationStore;

pub(crate) type HostApplicationService = ApplicationService<TursoApplicationStore>;

#[derive(Clone)]
pub struct AppState {
    application: HostApplicationService,
    db_path: Arc<PathBuf>,
    default_actor: Arc<str>,
}

impl AppState {
    /// Open and initialize the canonical Turso database owned by this host.
    pub async fn open(
        db_path: impl Into<PathBuf>,
        default_actor: impl Into<String>,
    ) -> Result<Self> {
        let db_path = db_path.into();
        ensure_parent_directory(&db_path).await?;
        let store = TursoStore::open(&db_path)
            .await
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        store
            .initialize()
            .await
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(Self {
            application: ApplicationService::new(TursoApplicationStore::new(store)),
            db_path: Arc::new(db_path),
            default_actor: Arc::from(default_actor.into()),
        })
    }

    pub(crate) fn application(&self) -> &HostApplicationService {
        &self.application
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
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
