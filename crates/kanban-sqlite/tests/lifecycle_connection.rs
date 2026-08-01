use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use kanban_local::DatabaseLifecycleExclusiveGuard;
use kanban_sqlite::api::lifecycle::{begin_database_replace, publish_staged_database};
use kanban_sqlite::db::{DatabaseConnection, connect_existing_read_only, connect_file};
use kanban_sqlite::init::init_database;

#[test]
fn guarded_read_only_constructor_does_not_create_a_missing_database() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("missing.db");

    assert!(connect_existing_read_only(&db_path).is_err());
    assert!(!db_path.exists());
}

#[cfg(any(unix, windows))]
#[test]
fn guarded_read_only_connection_holds_shared_lifecycle_until_drop() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    drop(connect_file(&db_path).unwrap());

    let connection = connect_existing_read_only(&db_path).unwrap();
    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);

    drop(connection);
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_connection_holds_shared_lifecycle_until_close() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let connection: DatabaseConnection = connect_file(&db_path).unwrap();

    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);

    connection
        .close()
        .map_err(|(_, error)| error)
        .expect("explicit close must succeed");
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_connection_drop_releases_only_after_sqlite_close() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let connection = connect_file(&db_path).unwrap();
    connection
        .execute_batch("CREATE TABLE lifecycle_probe(value INTEGER);")
        .unwrap();

    assert!(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).is_err());
    drop(connection);
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_connection_explicit_busy_close_returns_guarded_wrapper_for_retry() {
    use std::{ffi::CString, ptr};

    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let connection = connect_file(&db_path).unwrap();
    let sql = CString::new("SELECT 1").unwrap();
    let mut statement = ptr::null_mut();
    // SAFETY: the guarded connection owns a live SQLite handle and `sql` is
    // NUL-terminated. The raw statement is finalized before the retry.
    let result = unsafe {
        rusqlite::ffi::sqlite3_prepare_v2(
            connection.handle(),
            sql.as_ptr(),
            -1,
            &mut statement,
            ptr::null_mut(),
        )
    };
    assert_eq!(result, rusqlite::ffi::SQLITE_OK);
    assert!(!statement.is_null());

    let (connection, error) = connection.close().unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy)
    );
    assert_eq!(
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );

    // SAFETY: `statement` is the live statement prepared above and is finalized
    // exactly once before the guarded connection retries close.
    assert_eq!(
        unsafe { rusqlite::ffi::sqlite3_finalize(statement) },
        rusqlite::ffi::SQLITE_OK
    );
    connection
        .close()
        .map_err(|(_, error)| error)
        .expect("retry close must succeed after finalizing the raw statement");
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_connection_busy_close_retains_guard_until_process_exit() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let ready_path = tempdir.path().join("ready");
    let release_path = tempdir.path().join("release");
    drop(connect_file(&db_path).unwrap());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("database_connection_busy_close_probe_child")
        .arg("--nocapture")
        .env("KANBAN_CONNECTION_BUSY_DB", &db_path)
        .env("KANBAN_CONNECTION_BUSY_READY", &ready_path)
        .env("KANBAN_CONNECTION_BUSY_RELEASE", &release_path)
        .spawn()
        .unwrap();
    wait_for_path_or_child_exit(&ready_path, &mut child);

    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);

    std::fs::write(&release_path, b"release").unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "busy-close child failed with {status}");
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_connection_busy_close_probe_child() {
    use std::{ffi::CString, ptr};

    let Some(db_path) = std::env::var_os("KANBAN_CONNECTION_BUSY_DB") else {
        return;
    };
    let ready_path = PathBuf::from(std::env::var_os("KANBAN_CONNECTION_BUSY_READY").unwrap());
    let release_path = PathBuf::from(std::env::var_os("KANBAN_CONNECTION_BUSY_RELEASE").unwrap());
    let connection = connect_file(Path::new(&db_path)).unwrap();
    let sql = CString::new("SELECT 1").unwrap();
    let mut statement = ptr::null_mut();
    // SAFETY: connection owns a live SQLite handle, sql is NUL terminated,
    // and statement points to writable storage. Deliberately leaving this raw
    // statement unfinalized forces sqlite3_close to return SQLITE_BUSY.
    let result = unsafe {
        rusqlite::ffi::sqlite3_prepare_v2(
            connection.handle(),
            sql.as_ptr(),
            -1,
            &mut statement,
            ptr::null_mut(),
        )
    };
    assert_eq!(result, rusqlite::ffi::SQLITE_OK);
    assert!(!statement.is_null());

    drop(connection);
    std::fs::write(ready_path, b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !release_path.exists() {
        assert!(
            Instant::now() < deadline,
            "parent did not release busy-close child"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(unix, windows))]
fn wait_for_path_or_child_exit(path: &Path, child: &mut std::process::Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("connection child exited before readiness with {status}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("connection child did not become ready");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(unix, windows))]
#[test]
fn database_replace_marker_then_exclusive_rejects_live_connection_and_cleans_marker() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    init_database(&db_path, "tester").unwrap();
    let connection = connect_file(&db_path).unwrap();

    let error = begin_database_replace(&db_path).unwrap_err();

    assert!(error.to_string().contains("lifecycle"));
    assert!(!kanban_sqlite::db::maintenance_lock_path(&db_path).exists());
    drop(connection);
    drop(begin_database_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_replace_can_fence_staged_inode_before_namespace_publish() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let staged_path = tempdir.path().join("staged.db");
    let previous_path = tempdir.path().join("previous.db");
    init_database(&db_path, "tester").unwrap();
    init_database(&staged_path, "tester").unwrap();
    let mut replace = begin_database_replace(&db_path).unwrap();

    replace
        .fence_staged_database_for_replace(&staged_path)
        .unwrap();
    std::fs::rename(&db_path, &previous_path).unwrap();
    std::fs::rename(&staged_path, &db_path).unwrap();

    assert!(replace.validate_database_identities().is_err());
    replace
        .rebind_after_namespace_publish(&previous_path, &db_path)
        .unwrap();
    replace.validate_database_identities().unwrap();
    assert_eq!(
        kanban_local::DatabaseLifecycleSharedGuard::acquire_existing(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(replace);
    drop(kanban_local::DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_replace_publishes_staged_file_atomically_and_retains_evidence() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let staged_path = tempdir.path().join("staged.db");
    let previous_path = tempdir.path().join("previous.db");
    let journal_path = tempdir.path().join("replace.journal");
    init_database(&db_path, "tester").unwrap();
    init_database(&staged_path, "tester").unwrap();

    let mut replace = begin_database_replace(&db_path).unwrap();
    let report = publish_staged_database(
        &mut replace,
        &db_path,
        &staged_path,
        &previous_path,
        &journal_path,
    )
    .unwrap();

    let expected_canonical_path = std::fs::canonicalize(&db_path).unwrap();
    let expected_previous_path = std::fs::canonicalize(&previous_path).unwrap();
    let expected_journal_path = std::fs::canonicalize(&journal_path).unwrap();
    assert_eq!(report.canonical_path, expected_canonical_path);
    assert_eq!(report.previous_path, expected_previous_path);
    assert_eq!(report.journal_path, expected_journal_path);
    assert!(db_path.is_file());
    assert!(previous_path.is_file());
    assert!(!staged_path.exists());
    assert!(journal_path.is_file());
    assert!(
        std::fs::read_to_string(&journal_path)
            .unwrap()
            .contains("completed")
    );
    replace.validate_database_identities().unwrap();
    drop(replace);
    drop(kanban_local::DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_replace_rejects_existing_previous_or_journal_without_mutation() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let staged_path = tempdir.path().join("staged.db");
    let previous_path = tempdir.path().join("previous.db");
    let journal_path = tempdir.path().join("replace.journal");
    init_database(&db_path, "tester").unwrap();
    init_database(&staged_path, "tester").unwrap();
    std::fs::write(&previous_path, b"unknown previous").unwrap();
    let before = std::fs::read(&db_path).unwrap();

    let mut replace = begin_database_replace(&db_path).unwrap();
    let error = publish_staged_database(
        &mut replace,
        &db_path,
        &staged_path,
        &previous_path,
        &journal_path,
    )
    .unwrap_err();
    assert!(error.to_string().contains("previous") || error.to_string().contains("destination"));
    assert_eq!(std::fs::read(&db_path).unwrap(), before);
    assert!(!journal_path.exists());
    assert!(staged_path.exists());
    drop(replace);
}

#[cfg(any(unix, windows))]
#[test]
fn database_replace_missing_target_uses_and_cleans_a_fenced_placeholder() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");

    let replace = begin_database_replace(&db_path).unwrap();

    assert!(db_path.is_file());
    replace.validate_database_identities().unwrap();
    drop(replace);
    assert!(!db_path.exists());
    assert!(!kanban_sqlite::db::maintenance_lock_path(&db_path).exists());
}

#[test]
fn database_replace_rejects_existing_unknown_sqlite_schema() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("unknown.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch("CREATE TABLE unrelated(value TEXT);")
        .unwrap();
    drop(conn);

    let error = begin_database_replace(&db_path).unwrap_err();

    assert!(error.to_string().contains("not initialized"), "{error}");
    assert!(db_path.exists());
    assert!(!kanban_sqlite::db::maintenance_lock_path(&db_path).exists());
}
