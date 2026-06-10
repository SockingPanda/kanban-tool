use crate::common::*;

#[test]
fn init_creates_schema_default_board_and_columns() -> anyhow::Result<()> {
    let temp = TempDb::new("init_creates_schema_default_board_and_columns")?;

    let result = init_database(&temp.path, "test-actor")?;

    assert_eq!(result.board_slug, "default");
    let conn = Connection::open(&temp.path)?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    assert_eq!(integrity, "ok");
    let board_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM boards WHERE slug = 'default'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(board_count, 1);
    let visible_columns: i64 = conn.query_row(
        "SELECT COUNT(*) FROM board_columns WHERE hidden = 0",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(visible_columns, 8);
    let archived_columns: i64 = conn.query_row(
        "SELECT COUNT(*) FROM board_columns WHERE status = 'archived' AND hidden = 1",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(archived_columns, 1);
    Ok(())
}

#[test]
fn init_is_idempotent() -> anyhow::Result<()> {
    let temp = TempDb::new("init_is_idempotent")?;

    init_database(&temp.path, "first")?;
    init_database(&temp.path, "second")?;

    let conn = Connection::open(&temp.path)?;
    let board_count: i64 = conn.query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))?;
    let column_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM board_columns", [], |row| row.get(0))?;
    assert_eq!(board_count, 1);
    assert_eq!(column_count, 9);
    Ok(())
}

#[test]
fn init_records_and_enforces_migration_checksum() -> anyhow::Result<()> {
    let temp = TempDb::new("init_records_and_enforces_migration_checksum")?;

    init_database(&temp.path, "first")?;

    let conn = Connection::open(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let (name, checksum): (String, String) = conn.query_row(
        "SELECT name, checksum FROM schema_migrations WHERE version = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(user_version, 2);
    assert_eq!(name, "001_initial");
    assert!(checksum.starts_with("fnv64:"), "checksum: {checksum}");

    conn.execute(
        "UPDATE schema_migrations SET checksum='fnv64:wrong' WHERE version=1",
        [],
    )?;
    drop(conn);

    let err = result_err(init_database(&temp.path, "second"))?;
    assert!(
        err.to_string().contains("migration checksum mismatch"),
        "err: {err}"
    );
    Ok(())
}

#[test]
fn init_creates_knowledge_substrate_tables_and_seeds() -> anyhow::Result<()> {
    let temp = TempDb::new("init_creates_knowledge_substrate_tables_and_seeds")?;

    init_database(&temp.path, "tester")?;

    let conn = Connection::open(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 2);
    for table in [
        "entities",
        "relation_predicates",
        "entity_relations",
        "index_outbox",
        "derived_store_state",
    ] {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "missing table {table}");
    }
    let predicate_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM relation_predicates", [], |row| {
            row.get(0)
        })?;
    assert!(predicate_count >= 13);
    let derived_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM derived_store_state", [], |row| {
            row.get(0)
        })?;
    assert_eq!(derived_count, 3);
    let board_entities: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE kind='board'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(board_entities, 1);
    Ok(())
}

#[test]
fn init_upgrades_v1_database_and_backfills_task_entities() -> anyhow::Result<()> {
    let temp = TempDb::new("init_upgrades_v1_database_and_backfills_task_entities")?;

    let v1_sql = include_str!("../../../../migrations/001_initial.sql");
    let conn = Connection::open(&temp.path)?;
    conn.execute_batch(v1_sql)?;
    conn.execute(
        "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_test', 'default', 'Default', NULL, 1, 1, NULL)",
        [],
    )
    ?;
    conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, created_by, created_at, updated_at, metadata_json) VALUES ('t_test', 'b_test', 1, 'Upgrade task', 'ready spec', 'ready', 'tester', 2, 2, '{}')",
        [],
    )
    ?;
    conn.pragma_update(None, "user_version", 1)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = Connection::open(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 2);
    let task_entity_title: String = conn.query_row(
        "SELECT title FROM entities WHERE uri='kb://task/t_test'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(task_entity_title, "Upgrade task");
    Ok(())
}
