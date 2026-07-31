//! Test and benchmark support for kanban-tool.
//!
//! This crate centralizes fixtures and raw inspection helpers so production
//! adapters do not depend on storage implementation internals.

use std::path::{Path, PathBuf};

use kanban_sqlite::db::DatabaseConnection;

/// Temporary SQLite database initialized with the canonical kanban schema.
pub struct TempDb {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl TempDb {
    pub fn new(actor: &str) -> kanban_core::Result<Self> {
        let dir = tempfile::tempdir().map_err(|error| {
            kanban_core::KanbanError::Storage(format!("create tempdir: {error}"))
        })?;
        let path = dir.path().join("test.db");
        kanban_sqlite::init::init_database(&path, actor)?;
        Ok(Self { _dir: dir, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn connect(&self) -> kanban_core::Result<DatabaseConnection> {
        kanban_sqlite::db::connect_file(&self.path)
    }
}

/// Open a test database for raw SQL inspection.
pub fn connect_file(path: impl AsRef<Path>) -> kanban_core::Result<DatabaseConnection> {
    kanban_sqlite::db::connect_file(path.as_ref())
}
