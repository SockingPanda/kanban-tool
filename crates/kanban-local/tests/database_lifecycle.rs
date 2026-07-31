use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use kanban_local::{
    DatabaseLifecycleExclusiveGuard, DatabaseLifecycleSharedGuard, DerivedStoreWriteGuard,
    database_maintenance_lock_path,
};

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_shared_guards_coexist_and_block_exclusive() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let first = DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap();
    let second = DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap();

    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    drop(second);
    drop(first);
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_create_publishes_one_regular_single_link_authority() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");

    let guard = DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap();

    let metadata = std::fs::symlink_metadata(guard.path()).unwrap();
    assert!(metadata.is_file());
    #[cfg(unix)]
    assert_eq!(link_count(&metadata), 1);
    assert_eq!(guard.path(), std::fs::canonicalize(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_rejects_hardlinked_database_authority() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let alias = tempdir.path().join("alias.db");
    std::fs::write(&db_path, b"").unwrap();
    std::fs::hard_link(&db_path, &alias).unwrap();

    let shared = DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap_err();
    let exclusive =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&alias).unwrap_err();

    assert_eq!(shared.kind(), io::ErrorKind::InvalidData);
    assert_eq!(exclusive.kind(), io::ErrorKind::InvalidData);
}

#[cfg(unix)]
#[test]
fn database_lifecycle_symlink_alias_uses_canonical_non_symlink_authority() {
    use std::os::unix::fs::symlink;

    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let alias = tempdir.path().join("alias.db");
    std::fs::write(&db_path, b"").unwrap();
    symlink(&db_path, &alias).unwrap();
    let expected = std::fs::canonicalize(&db_path).unwrap();

    let shared = DatabaseLifecycleSharedGuard::acquire_existing(&alias).unwrap();

    assert_eq!(shared.path(), expected);
    assert!(
        !std::fs::symlink_metadata(shared.path())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
}

#[cfg(windows)]
#[test]
fn database_lifecycle_windows_reparse_alias_uses_canonical_non_reparse_authority() {
    use std::os::windows::fs::symlink_file;

    const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let alias = tempdir.path().join("alias.db");
    std::fs::write(&db_path, b"").unwrap();
    if let Err(error) = symlink_file(&db_path, &alias) {
        assert_eq!(
            error.raw_os_error(),
            Some(ERROR_PRIVILEGE_NOT_HELD),
            "unexpected Windows symlink creation failure: {error}"
        );
        return;
    }
    let expected = std::fs::canonicalize(&db_path).unwrap();

    let shared = DatabaseLifecycleSharedGuard::acquire_existing(&alias).unwrap();

    assert_eq!(shared.path(), expected);
    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
}

#[cfg(unix)]
#[test]
fn unrelated_sqlite_descriptor_close_does_not_release_lifecycle_guard() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let shared = DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap();
    let unrelated = rusqlite::Connection::open(&db_path).unwrap();
    drop(unrelated);

    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    drop(shared);
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_shared_guard_blocks_exclusive_across_processes() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let ready_path = tempdir.path().join("ready");
    let release_path = tempdir.path().join("release");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("database_lifecycle_shared_probe_child")
        .arg("--nocapture")
        .env("KANBAN_LIFECYCLE_CHILD_DB", &db_path)
        .env("KANBAN_LIFECYCLE_CHILD_READY", &ready_path)
        .env("KANBAN_LIFECYCLE_CHILD_RELEASE", &release_path)
        .spawn()
        .unwrap();
    wait_for_path_or_child_exit(&ready_path, &mut child);

    let error =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);

    std::fs::write(&release_path, b"release").unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "lifecycle child failed with {status}");
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_shared_guards_coexist_across_processes_before_exclusive_release() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let ready_path = tempdir.path().join("shared-coexist-ready");
    let release_path = tempdir.path().join("shared-coexist-release");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("database_lifecycle_shared_coexist_probe_child")
        .arg("--nocapture")
        .env("KANBAN_LIFECYCLE_SHARED_COEXIST_DB", &db_path)
        .env("KANBAN_LIFECYCLE_SHARED_COEXIST_READY", &ready_path)
        .env("KANBAN_LIFECYCLE_SHARED_COEXIST_RELEASE", &release_path)
        .spawn()
        .unwrap();
    wait_for_path_or_child_exit(&ready_path, &mut child);

    let parent_shared = DatabaseLifecycleSharedGuard::acquire_existing(&db_path)
        .expect("a shared lifecycle guard must coexist across processes");
    assert_eq!(
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );

    drop(parent_shared);
    std::fs::write(&release_path, b"release").unwrap();
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "shared coexistence child failed with {status}"
    );
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_shared_probe_child() {
    let Some(db_path) = std::env::var_os("KANBAN_LIFECYCLE_CHILD_DB") else {
        return;
    };
    let ready_path = PathBuf::from(std::env::var_os("KANBAN_LIFECYCLE_CHILD_READY").unwrap());
    let release_path = PathBuf::from(std::env::var_os("KANBAN_LIFECYCLE_CHILD_RELEASE").unwrap());
    let _guard = DatabaseLifecycleSharedGuard::acquire_existing(Path::new(&db_path)).unwrap();
    std::fs::write(ready_path, b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !release_path.exists() {
        assert!(
            Instant::now() < deadline,
            "parent did not release lifecycle child"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_shared_coexist_probe_child() {
    let Some(db_path) = std::env::var_os("KANBAN_LIFECYCLE_SHARED_COEXIST_DB") else {
        return;
    };
    let ready_path = PathBuf::from(
        std::env::var_os("KANBAN_LIFECYCLE_SHARED_COEXIST_READY").unwrap(),
    );
    let release_path = PathBuf::from(
        std::env::var_os("KANBAN_LIFECYCLE_SHARED_COEXIST_RELEASE").unwrap(),
    );
    let _guard =
        DatabaseLifecycleSharedGuard::acquire_existing(Path::new(&db_path)).unwrap();
    std::fs::write(ready_path, b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !release_path.exists() {
        assert!(
            Instant::now() < deadline,
            "parent did not release shared coexistence child"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(unix, windows))]
fn wait_for_path_or_child_exit(path: &Path, child: &mut std::process::Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("lifecycle child exited before readiness with {status}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("lifecycle child did not become ready");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_exclusive_guard_blocks_shared_and_exclusive_across_processes() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let ready_path = tempdir.path().join("exclusive-ready");
    let release_path = tempdir.path().join("exclusive-release");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("database_lifecycle_exclusive_probe_child")
        .arg("--nocapture")
        .env("KANBAN_LIFECYCLE_EXCLUSIVE_DB", &db_path)
        .env("KANBAN_LIFECYCLE_EXCLUSIVE_READY", &ready_path)
        .env("KANBAN_LIFECYCLE_EXCLUSIVE_RELEASE", &release_path)
        .spawn()
        .unwrap();
    wait_for_path_or_child_exit(&ready_path, &mut child);

    assert_eq!(
        DatabaseLifecycleSharedGuard::acquire_existing(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );

    std::fs::write(&release_path, b"release").unwrap();
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "exclusive lifecycle child failed with {status}"
    );
    drop(DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn database_lifecycle_exclusive_probe_child() {
    let Some(db_path) = std::env::var_os("KANBAN_LIFECYCLE_EXCLUSIVE_DB") else {
        return;
    };
    let ready_path = PathBuf::from(std::env::var_os("KANBAN_LIFECYCLE_EXCLUSIVE_READY").unwrap());
    let release_path =
        PathBuf::from(std::env::var_os("KANBAN_LIFECYCLE_EXCLUSIVE_RELEASE").unwrap());
    let _guard =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(Path::new(&db_path)).unwrap();
    std::fs::write(ready_path, b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !release_path.exists() {
        assert!(
            Instant::now() < deadline,
            "parent did not release exclusive lifecycle child"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn derived_store_guard_rechecks_stable_maintenance_fence_after_shared_lifecycle() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());
    let marker = database_maintenance_lock_path(&db_path);
    std::fs::write(&marker, b"live maintenance fence").unwrap();

    let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(!kanban_local::derived_store_write_lock_path(&db_path, "tantivy_tasks").exists());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn exclusive_authority_acquires_legacy_store_guards_without_shared_reentry() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());
    let exclusive =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap();

    let authority = exclusive
        .into_derived_store_authority(&["tantivy_tasks", "oxigraph_relations"])
        .unwrap();

    assert_eq!(authority.path(), std::fs::canonicalize(&db_path).unwrap());
    assert_eq!(
        DatabaseLifecycleSharedGuard::acquire_existing(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(authority);
    drop(DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn current_and_staged_exclusive_guards_fence_both_inodes_across_rename() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let staged_path = tempdir.path().join("staged.db");
    let previous_path = tempdir.path().join("previous.db");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&staged_path).unwrap());
    let current = DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap();
    let staged =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&staged_path).unwrap();

    std::fs::rename(&db_path, &previous_path).unwrap();
    std::fs::rename(&staged_path, &db_path).unwrap();

    assert_eq!(
        DatabaseLifecycleSharedGuard::acquire_existing(&previous_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        DatabaseLifecycleSharedGuard::acquire_existing(&db_path)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(staged);
    drop(current);
    drop(DatabaseLifecycleSharedGuard::acquire_existing(&db_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn exclusive_guard_rebinds_and_revalidates_the_renamed_namespace_identity() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let previous_path = tempdir.path().join("previous.db");
    drop(DatabaseLifecycleSharedGuard::acquire_or_create(&db_path).unwrap());
    let mut guard =
        DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db_path).unwrap();

    std::fs::rename(&db_path, &previous_path).unwrap();

    assert!(guard.validate_path_identity().is_err());
    guard.validate_identity_at(&previous_path).unwrap();
    guard.rebind_after_rename(&previous_path).unwrap();
    guard.validate_path_identity().unwrap();
    assert_eq!(guard.path(), std::fs::canonicalize(&previous_path).unwrap());
}

#[cfg(any(unix, windows))]
#[test]
fn exclusive_create_for_replace_removes_an_unpublished_placeholder_on_drop() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("kanban.db");
    let marker = database_maintenance_lock_path(&db_path);
    std::fs::write(&marker, b"test namespace fence").unwrap();

    let guard = DatabaseLifecycleExclusiveGuard::acquire_or_create_for_replace(&db_path).unwrap();

    assert!(guard.created_authority_file());
    assert!(db_path.is_file());
    drop(guard);
    assert!(!db_path.exists());
}

#[cfg(unix)]
#[test]
fn database_maintenance_marker_preserves_non_utf8_database_path_bytes() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join(OsStr::from_bytes(b"kanban-\xff.db"));
    let marker = database_maintenance_lock_path(&db_path);
    let mut expected = db_path.as_os_str().as_bytes().to_vec();
    expected.extend_from_slice(b".maintenance.lock");

    assert_eq!(marker.as_os_str().as_bytes(), expected);
}

#[cfg(unix)]
fn link_count(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;

    metadata.nlink()
}
