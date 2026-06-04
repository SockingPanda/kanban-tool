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
