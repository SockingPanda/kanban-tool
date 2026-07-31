use std::{
    fmt,
    mem::{self, ManuallyDrop},
    ops::Deref,
    path::Path,
};

use rusqlite::Connection;

use crate::DatabaseLifecycleSharedGuard;

/// A SQLite connection that retains its shared lifecycle authority until the
/// SQLite handle has closed successfully.
pub struct DatabaseLifecycleSharedConnection {
    inner: Option<Connection>,
    lifecycle: Option<DatabaseLifecycleSharedGuard>,
}

/// Failure phase for a shared lifecycle SQLite opener.
#[derive(Debug)]
pub enum DatabaseLifecycleSharedConnectionOpenError {
    Lifecycle(std::io::Error),
    BeforeOpen(std::io::Error),
    SQLite(rusqlite::Error),
}

impl fmt::Display for DatabaseLifecycleSharedConnectionOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) | Self::BeforeOpen(error) => error.fmt(formatter),
            Self::SQLite(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DatabaseLifecycleSharedConnectionOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) | Self::BeforeOpen(error) => Some(error),
            Self::SQLite(error) => Some(error),
        }
    }
}

impl DatabaseLifecycleSharedConnection {
    fn new(inner: Connection, lifecycle: DatabaseLifecycleSharedGuard) -> Self {
        Self {
            inner: Some(inner),
            lifecycle: Some(lifecycle),
        }
    }

    /// Explicitly closes SQLite before releasing lifecycle authority.
    #[allow(clippy::result_large_err)]
    pub fn close(mut self) -> std::result::Result<(), (Self, rusqlite::Error)> {
        let connection = self.inner.take().expect("database connection is open");
        let lifecycle = self
            .lifecycle
            .take()
            .expect("an open database connection owns a lifecycle guard");
        match close_connection_with(connection, lifecycle, Connection::close) {
            Ok(()) => Ok(()),
            Err((connection, lifecycle, error)) => {
                self.inner = Some(connection);
                self.lifecycle = Some(lifecycle);
                Err((self, error))
            }
        }
    }
}

impl Deref for DatabaseLifecycleSharedConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("database connection is open")
    }
}

impl fmt::Debug for DatabaseLifecycleSharedConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseLifecycleSharedConnection")
            .field("open", &self.inner.is_some())
            .finish()
    }
}

impl Drop for DatabaseLifecycleSharedConnection {
    fn drop(&mut self) {
        let Some(connection) = self.inner.take() else {
            return;
        };
        let lifecycle = self
            .lifecycle
            .take()
            .expect("an open database connection owns a lifecycle guard");
        match close_connection_with(connection, lifecycle, Connection::close) {
            Ok(()) => {}
            Err((connection, lifecycle, _error)) => {
                // SQLite still owns a handle. Keep both resources until
                // process exit so an exclusive replacement cannot enter.
                mem::forget(connection);
                mem::forget(lifecycle);
            }
        }
    }
}

#[allow(clippy::result_large_err)]
fn close_connection_with<Lifecycle>(
    connection: Connection,
    lifecycle: Lifecycle,
    close: impl FnOnce(Connection) -> std::result::Result<(), (Connection, rusqlite::Error)>,
) -> std::result::Result<(), (Connection, Lifecycle, rusqlite::Error)> {
    let mut lifecycle = ManuallyDrop::new(lifecycle);
    match close(connection) {
        Ok(()) => {
            // SAFETY: a successful close proves SQLite released its handles.
            unsafe { ManuallyDrop::drop(&mut lifecycle) };
            Ok(())
        }
        Err((connection, error)) => Err((connection, ManuallyDrop::into_inner(lifecycle), error)),
    }
}

/// Acquires shared lifecycle authority, invokes the caller's post-lock check,
/// opens SQLite, then validates that the namespace still names the held inode.
pub fn open_database_with_shared_lifecycle(
    path: &Path,
    before_open: impl FnOnce(&Path) -> std::io::Result<()>,
) -> std::result::Result<
    DatabaseLifecycleSharedConnection,
    DatabaseLifecycleSharedConnectionOpenError,
> {
    let lifecycle = DatabaseLifecycleSharedGuard::acquire_or_create(path)
        .map_err(DatabaseLifecycleSharedConnectionOpenError::Lifecycle)?;
    before_open(lifecycle.path())
        .map_err(DatabaseLifecycleSharedConnectionOpenError::BeforeOpen)?;
    // This is the lifecycle-bound helper SQLite opener audited by raw_file_open_audit.
    let connection = Connection::open(lifecycle.path())
        .map_err(DatabaseLifecycleSharedConnectionOpenError::SQLite)?;
    lifecycle
        .validate_path_identity()
        .map_err(DatabaseLifecycleSharedConnectionOpenError::Lifecycle)?;
    Ok(DatabaseLifecycleSharedConnection::new(
        connection, lifecycle,
    ))
}
