use std::{
    fmt, fs,
    mem::{self, ManuallyDrop},
    ops::Deref,
    path::{Path, PathBuf},
    time::Duration,
};

use kanban_core::{KanbanError, Result};
use kanban_local::{
    DatabaseLifecycleExclusiveAuthority, DatabaseLifecycleExclusiveGuard,
    DatabaseLifecycleSharedGuard, DerivedStoreReadGuard, DerivedStoreWriteGuard,
    database_maintenance_lock_path,
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};

/// A SQLite connection physically bound to the canonical database lifecycle.
///
/// Immutable dereferencing preserves the existing read/query surface without
/// exposing a way to move the raw connection out from under its guard.
pub struct DatabaseConnection {
    inner: Option<Connection>,
    lifecycle: Option<DatabaseConnectionLifecycle>,
}

/// A SQLite connection inspected while a replacement owns every exclusive
/// lifecycle and legacy derived-store authority for its canonical database.
///
/// This type never exposes ownership of the raw connection. Its explicit
/// close returns the authority only after SQLite has fully closed, so callers
/// can restore that authority to a replacement guard without a lock gap.
pub(crate) struct DatabaseExclusiveAuthorityConnection {
    inner: Option<Connection>,
    authority: Option<DatabaseLifecycleExclusiveAuthority>,
}

#[derive(Debug)]
enum DatabaseConnectionLifecycle {
    Shared(DatabaseLifecycleSharedGuard),
    Exclusive(DatabaseLifecycleExclusiveGuard),
}

impl DatabaseConnectionLifecycle {
    fn path(&self) -> &Path {
        match self {
            Self::Shared(guard) => guard.path(),
            Self::Exclusive(guard) => guard.path(),
        }
    }
}

impl DatabaseConnection {
    fn new(inner: Connection, lifecycle: DatabaseLifecycleSharedGuard) -> Self {
        Self {
            inner: Some(inner),
            lifecycle: Some(DatabaseConnectionLifecycle::Shared(lifecycle)),
        }
    }

    fn new_quiescent(inner: Connection, lifecycle: DatabaseLifecycleExclusiveGuard) -> Self {
        Self {
            inner: Some(inner),
            lifecycle: Some(DatabaseConnectionLifecycle::Exclusive(lifecycle)),
        }
    }

    /// Starts a checked transaction without exposing `DerefMut<Connection>`.
    pub fn transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        self.inner
            .as_mut()
            .expect("database connection is open")
            .transaction()
    }

    /// Starts a checked transaction with the requested behavior.
    pub fn transaction_with_behavior(
        &mut self,
        behavior: TransactionBehavior,
    ) -> rusqlite::Result<Transaction<'_>> {
        self.inner
            .as_mut()
            .expect("database connection is open")
            .transaction_with_behavior(behavior)
    }

    /// Explicitly closes SQLite before releasing the lifecycle guard.
    ///
    /// A failed SQLite close returns the complete guarded connection so the
    /// caller may finish outstanding statements and retry.
    #[allow(clippy::result_large_err)] // mirrors rusqlite::Connection::close ownership recovery
    pub fn close(mut self) -> std::result::Result<(), (Self, rusqlite::Error)> {
        let connection = self.inner.take().expect("database connection is open");
        let lifecycle = self
            .lifecycle
            .take()
            .expect("an open database connection owns a lifecycle guard");
        match close_connection_with(connection, lifecycle, Connection::close, |lifecycle| {
            drop(lifecycle)
        }) {
            Ok(()) => Ok(()),
            Err((connection, lifecycle, error)) => {
                self.inner = Some(connection);
                self.lifecycle = Some(lifecycle);
                Err((self, error))
            }
        }
    }
}

impl Deref for DatabaseConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("database connection is open")
    }
}

impl fmt::Debug for DatabaseConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseConnection")
            .field(
                "path",
                &self
                    .lifecycle
                    .as_ref()
                    .map(DatabaseConnectionLifecycle::path),
            )
            .field("open", &self.inner.is_some())
            .finish()
    }
}

impl Drop for DatabaseConnection {
    fn drop(&mut self) {
        let Some(connection) = self.inner.take() else {
            return;
        };
        let lifecycle = self
            .lifecycle
            .take()
            .expect("an open database connection owns a lifecycle guard");
        match close_connection_with(connection, lifecycle, Connection::close, |lifecycle| {
            drop(lifecycle)
        }) {
            Ok(()) => {}
            Err((connection, lifecycle, _error)) => {
                // Releasing the lifecycle byte while SQLite still owns main,
                // pager, statement, or WAL handles would make replacement
                // unsafe. Deliberately retain both resources until process
                // exit; the kernel then closes the handles and releases locks.
                mem::forget(connection);
                mem::forget(lifecycle);
            }
        }
    }
}

impl DatabaseExclusiveAuthorityConnection {
    fn new(inner: Connection, authority: DatabaseLifecycleExclusiveAuthority) -> Self {
        Self {
            inner: Some(inner),
            authority: Some(authority),
        }
    }

    /// Explicitly closes SQLite, returning the still-exclusive authority only
    /// after SQLite proves that no handles remain.
    #[allow(clippy::result_large_err)] // mirrors rusqlite::Connection::close ownership recovery
    pub(crate) fn close(
        mut self,
    ) -> std::result::Result<DatabaseLifecycleExclusiveAuthority, (Self, rusqlite::Error)> {
        let connection = self.inner.take().expect("database connection is open");
        let authority = self
            .authority
            .take()
            .expect("an open replacement inspection owns lifecycle authority");
        match close_connection_with(connection, authority, Connection::close, |authority| {
            authority
        }) {
            Ok(authority) => Ok(authority),
            Err((connection, authority, error)) => {
                self.inner = Some(connection);
                self.authority = Some(authority);
                Err((self, error))
            }
        }
    }
}

impl Deref for DatabaseExclusiveAuthorityConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("database connection is open")
    }
}

impl fmt::Debug for DatabaseExclusiveAuthorityConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseExclusiveAuthorityConnection")
            .field(
                "path",
                &self.authority.as_ref().map(|authority| authority.path()),
            )
            .field("open", &self.inner.is_some())
            .finish()
    }
}

impl Drop for DatabaseExclusiveAuthorityConnection {
    fn drop(&mut self) {
        let Some(connection) = self.inner.take() else {
            return;
        };
        let authority = self
            .authority
            .take()
            .expect("an open replacement inspection owns lifecycle authority");
        match close_connection_with(connection, authority, Connection::close, |authority| {
            drop(authority)
        }) {
            Ok(()) => {}
            Err((connection, authority, _error)) => {
                // A failed close must retain every lifecycle and legacy-store
                // byte until process exit, otherwise replacement could publish
                // while SQLite still owns a pager, statement, or WAL handle.
                mem::forget(connection);
                mem::forget(authority);
            }
        }
    }
}

#[allow(clippy::result_large_err)] // fail-closed close must return both owned resources on BUSY
fn close_connection_with<Lifecycle, Output>(
    connection: Connection,
    lifecycle: Lifecycle,
    close: impl FnOnce(Connection) -> std::result::Result<(), (Connection, rusqlite::Error)>,
    after_close: impl FnOnce(Lifecycle) -> Output,
) -> std::result::Result<Output, (Connection, Lifecycle, rusqlite::Error)> {
    // ManuallyDrop makes unexpected panic/unwind fail closed: the lifecycle
    // file handle is intentionally leaked unless close returns a proven result.
    let lifecycle = ManuallyDrop::new(lifecycle);
    match close(connection) {
        Ok(()) => {
            // Successful SQLite close proves that no main, pager, statement,
            // or WAL handles remain. The caller may now either release the
            // authority or retain it for a following replacement phase.
            Ok(after_close(ManuallyDrop::into_inner(lifecycle)))
        }
        Err((connection, error)) => Err((connection, ManuallyDrop::into_inner(lifecycle), error)),
    }
}

/// Opens a canonical database only while the caller owns the replacement's
/// exclusive lifecycle plus legacy derived-store authority.
///
/// The authority stays inside the returned wrapper until an explicit,
/// successful SQLite close. This is the sole replacement inspection opener
/// audited by `raw_file_open_audit`.
pub(crate) fn open_database_with_exclusive_authority(
    authority: DatabaseLifecycleExclusiveAuthority,
) -> Result<DatabaseExclusiveAuthorityConnection> {
    authority
        .validate_path_identity()
        .map_err(lifecycle_storage)?;
    let connection = Connection::open(authority.path())
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    let guarded = DatabaseExclusiveAuthorityConnection::new(connection, authority);
    if let Err(error) = guarded
        .authority
        .as_ref()
        .expect("replacement inspection authority is installed above")
        .validate_path_identity()
    {
        // Drop closes SQLite before releasing the authority; a BUSY close is
        // deliberately leaked by the wrapper, preserving fail-closed safety.
        drop(guarded);
        return Err(lifecycle_storage(error));
    }
    Ok(guarded)
}

pub fn connect_file(path: &Path) -> Result<DatabaseConnection> {
    connect_file_with_setup(path, default_pragmas)
}

fn connect_file_with_setup(
    path: &Path,
    setup: impl FnOnce(&Connection) -> Result<()>,
) -> Result<DatabaseConnection> {
    connect_file_with_hooks(path, |_| Ok(()), setup)
}

fn connect_file_with_hooks(
    path: &Path,
    after_lifecycle_acquired: impl FnOnce(&Path) -> Result<()>,
    setup: impl FnOnce(&Connection) -> Result<()>,
) -> Result<DatabaseConnection> {
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    let lifecycle =
        DatabaseLifecycleSharedGuard::acquire_or_create(path).map_err(lifecycle_storage)?;
    after_lifecycle_acquired(lifecycle.path())?;
    let lock_path = maintenance_lock_path(lifecycle.path());
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            lifecycle.path().display()
        )));
    }
    // This is the guarded constructor audited by raw_file_open_audit.
    let conn =
        Connection::open(lifecycle.path()).map_err(|err| KanbanError::Storage(err.to_string()))?;
    lifecycle
        .validate_path_identity()
        .map_err(lifecycle_storage)?;
    let conn = DatabaseConnection::new(conn, lifecycle);
    setup(&conn)?;
    Ok(conn)
}

pub fn connect(path: impl AsRef<Path>) -> Result<DatabaseConnection> {
    connect_file(path.as_ref())
}

/// Opens an existing canonical database read-only while retaining shared
/// database lifecycle authority for the complete connection lifetime.
///
/// This constructor never creates a missing database and is the infrastructure
/// boundary for derived helpers that must not race database replacement.
pub fn connect_existing_read_only(path: &Path) -> Result<DatabaseConnection> {
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    let lifecycle =
        DatabaseLifecycleSharedGuard::acquire_existing(path).map_err(lifecycle_storage)?;
    let lock_path = maintenance_lock_path(lifecycle.path());
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            lifecycle.path().display()
        )));
    }
    // This is the guarded read-only constructor audited by raw_file_open_audit.
    let conn = Connection::open_with_flags(
        lifecycle.path(),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    lifecycle
        .validate_path_identity()
        .map_err(lifecycle_storage)?;
    let conn = DatabaseConnection::new(conn, lifecycle);
    conn.busy_timeout(Duration::from_secs(120))
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(conn)
}

/// Opens a stable, byte-for-byte read-only snapshot of an existing database.
///
/// The exclusive lifecycle byte first drains every managed SQLite opener. A
/// non-empty WAL or rollback journal is rejected because SQLite's immutable
/// mode must never ignore canonical frames. Holding that lifecycle authority
/// for the complete connection lifetime then makes immutable reads consistent
/// without creating or updating WAL/SHM sidecars.
pub(crate) fn connect_existing_quiescent_read_only(path: &Path) -> Result<DatabaseConnection> {
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    let lifecycle = DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(path)
        .map_err(lifecycle_storage)?;
    let lock_path = maintenance_lock_path(lifecycle.path());
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            lifecycle.path().display()
        )));
    }
    require_checkpointed_snapshot(lifecycle.path())?;
    let uri = immutable_sqlite_uri(lifecycle.path())?;
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    lifecycle
        .validate_path_identity()
        .map_err(lifecycle_storage)?;
    let conn = DatabaseConnection::new_quiescent(conn, lifecycle);
    conn.busy_timeout(Duration::from_secs(120))
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(conn)
}

fn require_checkpointed_snapshot(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-journal"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
        match fs::symlink_metadata(&sidecar) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(KanbanError::Conflict(format!(
                    "strict read-only database snapshot found a non-file sidecar: {}",
                    sidecar.display()
                )));
            }
            Ok(metadata) if metadata.len() != 0 => {
                return Err(KanbanError::Conflict(format!(
                    "strict read-only database snapshot requires a checkpointed database; non-empty sidecar: {}",
                    sidecar.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
    }
    Ok(())
}

fn immutable_sqlite_uri(path: &Path) -> Result<String> {
    let raw = path.to_str().ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "strict read-only database path is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    let normalized = raw.replace('\\', "/");
    let mut uri = if normalized.starts_with('/') {
        String::from("file:")
    } else {
        String::from("file:/")
    };
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'.' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    uri.push_str("?mode=ro&immutable=1");
    Ok(uri)
}

pub fn default_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;",
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))
}

pub(crate) fn lifecycle_storage(error: std::io::Error) -> KanbanError {
    if error.kind() == std::io::ErrorKind::WouldBlock {
        KanbanError::Conflict(error.to_string())
    } else {
        KanbanError::Storage(error.to_string())
    }
}

pub fn maintenance_lock_path(path: &Path) -> PathBuf {
    database_maintenance_lock_path(path)
}

pub fn runtime_lock_path(path: &Path) -> PathBuf {
    let normalized = normalized_database_path(path);
    PathBuf::from(format!("{}.runtime.lock", normalized.display()))
}

pub fn maintenance_lock_blocks(lock_path: &Path) -> Result<bool> {
    lock_file_blocks(lock_path)
}

pub fn runtime_lock_blocks(lock_path: &Path) -> Result<bool> {
    lock_file_blocks(lock_path)
}

pub fn acquire_derived_store_write_guard(
    path: &Path,
    store_name: &str,
) -> Result<DerivedStoreWriteGuard> {
    DerivedStoreWriteGuard::acquire(path, store_name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            KanbanError::Conflict(error.to_string())
        } else {
            KanbanError::Storage(error.to_string())
        }
    })
}

/// Acquire the shared physical suffix authority for a projection reader.
/// Recovery/quarantine takes the matching exclusive guard, so a reader never
/// observes a generation while its directory is being moved aside. A busy
/// suffix is surfaced as a transient conflict for callers to retry.
pub fn acquire_derived_store_read_guard(
    path: &Path,
    store_name: &str,
) -> Result<DerivedStoreReadGuard> {
    DerivedStoreReadGuard::acquire(path, store_name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            KanbanError::Conflict(error.to_string())
        } else {
            KanbanError::Storage(error.to_string())
        }
    })
}

fn lock_file_blocks(lock_path: &Path) -> Result<bool> {
    if !lock_path.exists() {
        return Ok(false);
    }
    if lock_is_stale(lock_path) {
        fs::remove_file(lock_path).map_err(|err| KanbanError::Storage(err.to_string()))?;
        return Ok(false);
    }
    Ok(true)
}

fn normalized_database_path(path: &Path) -> PathBuf {
    if path.exists()
        && let Ok(canonical) = fs::canonicalize(path)
    {
        return canonical;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().unwrap_or_default();
    if let Ok(canonical_parent) = fs::canonicalize(parent) {
        return canonical_parent.join(file_name);
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn lock_is_stale(lock_path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(lock_path) else {
        return false;
    };
    let Some(pid) = content
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|pid| pid.trim().parse::<u32>().ok())
    else {
        return false;
    };
    !process_is_alive(pid)
}

#[cfg(target_os = "linux")]
fn process_is_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    std::process::Command::new("tasklist")
        .args(["/FI", &filter])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(true)
}

#[cfg(not(any(unix, windows)))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_local::{DatabaseLifecycleExclusiveGuard, DatabaseLifecycleSharedGuard};

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn setup_failure_keeps_lifecycle_guard_until_sqlite_is_closed() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("kanban.db");

        let error = connect_file_with_setup(&path, |_| {
            let conflict =
                DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path).unwrap_err();
            assert_eq!(conflict.kind(), std::io::ErrorKind::WouldBlock);
            Err(KanbanError::Storage("forced setup failure".to_owned()))
        })
        .unwrap_err();

        assert!(error.to_string().contains("forced setup failure"));
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path).unwrap());
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn unsafe_database_authority_fails_closed_before_sqlite_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("kanban.db");
        std::fs::create_dir(&path).unwrap();

        let error = connect_file(&path).unwrap_err();

        #[cfg(target_os = "linux")]
        assert!(error.to_string().contains("regular non-reparse file"));
        #[cfg(windows)]
        assert!(
            !error.to_string().is_empty(),
            "Windows must reject a directory authority before SQLite opens it"
        );
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
        drop(connect_file(&path).unwrap());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn maintenance_marker_created_after_shared_acquisition_fails_before_sqlite_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("kanban.db");
        let marker = maintenance_lock_path(&path);

        let error = connect_file_with_hooks(
            &path,
            |_| {
                std::fs::write(&marker, b"concurrent maintenance").map_err(|error| {
                    KanbanError::Storage(format!("failed to create test marker: {error}"))
                })
            },
            default_pragmas,
        )
        .unwrap_err();

        assert!(error.to_string().contains("locked for maintenance"));
        std::fs::remove_file(marker).unwrap();
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path).unwrap());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn post_open_identity_recheck_rejects_path_swap() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("kanban.db");
        let previous = tempdir.path().join("previous.db");

        let error = connect_file_with_hooks(
            &path,
            |guarded_path| {
                std::fs::rename(guarded_path, &previous)
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                std::fs::write(guarded_path, b"")
                    .map_err(|error| KanbanError::Storage(error.to_string()))
            },
            default_pragmas,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("does not identify the opened lock authority")
        );
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path).unwrap());
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&previous).unwrap());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn exclusive_authority_connection_retains_replace_fence_until_authority_returns() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("kanban.db");
        drop(connect_file(&path).unwrap());
        let authority = DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path)
            .unwrap()
            .into_derived_store_authority(&[])
            .unwrap();

        let connection = open_database_with_exclusive_authority(authority).unwrap();
        assert_eq!(
            DatabaseLifecycleSharedGuard::acquire_existing(&path)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );

        let authority = connection
            .close()
            .map_err(|(_, error)| error)
            .expect("explicit close must preserve the exclusive authority");
        assert_eq!(
            DatabaseLifecycleSharedGuard::acquire_existing(&path)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        drop(authority);
        drop(DatabaseLifecycleSharedGuard::acquire_existing(&path).unwrap());
    }

    #[allow(clippy::result_large_err)] // forced closure mirrors rusqlite::Connection::close
    #[cfg(any(unix, windows))]
    #[test]
    fn panic_during_close_retains_lifecycle_authority() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("kanban.db");
        let mut guarded = connect_file(&path).unwrap();
        let connection = guarded.inner.take().unwrap();
        let lifecycle = guarded.lifecycle.take().unwrap();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = close_connection_with(
                connection,
                lifecycle,
                |connection| {
                    std::mem::forget(connection);
                    panic!("forced close panic");
                },
                drop,
            );
        }));

        assert!(panic.is_err());
        assert_eq!(
            DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
