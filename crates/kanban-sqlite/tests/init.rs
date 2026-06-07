use std::path::Path;

use kanban_sqlite::init_database;
use rusqlite::Connection;

#[test]
fn init_creates_schema_default_board_and_columns() {
    let temp = TempDb::new("init_creates_schema_default_board_and_columns");

    let result = init_database(&temp.path, "test-actor").expect("init succeeds");

    assert_eq!(result.board_slug, "default");
    let conn = Connection::open(&temp.path).unwrap();
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let board_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM boards WHERE slug = 'default'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(board_count, 1);
    let visible_columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM board_columns WHERE hidden = 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(visible_columns, 8);
    let archived_columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM board_columns WHERE status = 'archived' AND hidden = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(archived_columns, 1);
}

#[test]
fn init_is_idempotent() {
    let temp = TempDb::new("init_is_idempotent");

    init_database(&temp.path, "first").expect("first init succeeds");
    init_database(&temp.path, "second").expect("second init succeeds");

    let conn = Connection::open(&temp.path).unwrap();
    let board_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .unwrap();
    let column_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM board_columns", [], |row| row.get(0))
        .unwrap();
    assert_eq!(board_count, 1);
    assert_eq!(column_count, 9);
}

#[test]
fn init_records_and_enforces_migration_checksum() {
    let temp = TempDb::new("init_records_and_enforces_migration_checksum");

    init_database(&temp.path, "first").expect("first init succeeds");

    let conn = Connection::open(&temp.path).unwrap();
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let (name, checksum): (String, String) = conn
        .query_row(
            "SELECT name, checksum FROM schema_migrations WHERE version = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(user_version, 1);
    assert_eq!(name, "001_initial");
    assert!(checksum.starts_with("fnv64:"), "checksum: {checksum}");

    conn.execute(
        "UPDATE schema_migrations SET checksum='fnv64:wrong' WHERE version=1",
        [],
    )
    .unwrap();
    drop(conn);

    let err = init_database(&temp.path, "second").unwrap_err();
    assert!(
        err.to_string().contains("migration checksum mismatch"),
        "err: {err}"
    );
}

struct TempDb {
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("kb-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path.push("kb.db");
        Self { path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        if let Some(parent) = Path::new(&self.path).parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
