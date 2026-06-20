use crate::common::*;

#[test]
fn init_creates_schema_default_board_and_columns() -> anyhow::Result<()> {
    let temp = TempDb::new("init_creates_schema_default_board_and_columns")?;

    let result = init_database(&temp.path, "test-actor")?;

    assert_eq!(result.board_slug, "default");
    let conn = connect_file(&temp.path)?;
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

    let conn = connect_file(&temp.path)?;
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
    assert_eq!(user_version, 19);
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
    assert_eq!(user_version, 19);
    for table in [
        "entities",
        "relation_predicates",
        "entity_relations",
        "index_outbox",
        "derived_store_state",
        "label_semantics",
        "label_atoms",
        "label_atom_index_boards",
        "label_semantic_proposals",
        "label_ontology_observations",
        "label_ontology_signals",
        "label_ontology_actions",
        "label_ontology_action_atom_effects",
        "label_ontology_action_signals",
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
    assert_eq!(derived_count, 4);
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
    assert_eq!(user_version, 19);
    let task_entity_title: String = conn.query_row(
        "SELECT title FROM entities WHERE uri='kb://task/t_test'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(task_entity_title, "Upgrade task");
    Ok(())
}

#[test]
fn init_v17_rebuilds_key_relationship_tables_without_losing_rows() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v17_rebuilds_key_relationship_tables_without_losing_rows")?;
    let fixture = seed_v17_board_isolation_fixture(&temp)?;

    let conn = connect_file(&temp.path)?;
    let before_counts = v17_relationship_counts(&conn)?;
    conn.execute("DELETE FROM schema_migrations WHERE version=17", [])?;
    conn.pragma_update(None, "user_version", 16)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 19);
    assert_eq!(v17_relationship_counts(&conn)?, before_counts);
    assert_eq!(
        task_label_board_for(&conn, &fixture.task_id, &fixture.label_id)?,
        fixture.board_id
    );
    let fk_errors = foreign_key_check_rows(&conn)?;
    assert!(fk_errors.is_empty(), "{fk_errors:#?}");
    Ok(())
}

#[test]
fn init_v18_adds_root_action_atom_effects_table_to_v17_database() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v18_adds_root_action_atom_effects_table_to_v17_database")?;
    seed_v17_board_isolation_fixture(&temp)?;

    let conn = connect_file(&temp.path)?;
    conn.execute("DROP TABLE label_ontology_action_atom_effects", [])?;
    conn.execute(
        "DROP INDEX IF EXISTS idx_label_ontology_actions_id_board",
        [],
    )?;
    conn.execute("DELETE FROM schema_migrations WHERE version=18", [])?;
    conn.pragma_update(None, "user_version", 17)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 19);
    let table_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='label_ontology_action_atom_effects'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(table_exists, 1);
    let action_board_index_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_label_ontology_actions_id_board'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(action_board_index_exists, 1);
    let fk_errors = foreign_key_check_rows(&conn)?;
    assert!(fk_errors.is_empty(), "{fk_errors:#?}");
    Ok(())
}

#[test]
fn init_v19_adds_validation_requirement_and_backfills_v18_actions() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v19_adds_validation_requirement_and_backfills_v18_actions")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    downgrade_label_ontology_actions_to_v18_shape(&conn)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    for (id, action_type, status) in [
        ("loa_required_positive", "add_positive_atom", "pending"),
        ("loa_required_bootstrap", "bootstrap_label", "pending"),
        ("loa_unsupported_update", "update_semantics", "pending"),
        (
            "loa_unsupported_revert",
            "revert_ontology_mutation",
            "pending",
        ),
        ("loa_unsupported_structure", "rename_label", "pending"),
        ("loa_none_lifecycle", "confirm", "pending"),
        ("loa_none_adoption", "adopt_existing_atom", "pending"),
        ("loa_none_proposal", "create_label_proposal", "pending"),
        ("loa_none_validate", "validate", "passed"),
        ("loa_none_failed_add", "add_positive_atom", "failed"),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_actions(
             id, board_id, action_type, reason, change_json, validation_status, validation_json,
             created_by, created_by_type, created_at)
             VALUES (?1, ?2, ?3, 'v18 validation requirement fixture', '{}', ?4, '{}',
             'tester', 'user', 1)",
            params![id, board_id, action_type, status],
        )?;
    }
    conn.execute("DELETE FROM schema_migrations WHERE version=19", [])?;
    conn.pragma_update(None, "user_version", 18)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 19);
    let column_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('label_ontology_actions') WHERE name='validation_requirement'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(column_exists, 1);
    let rows = conn
        .prepare(
            "SELECT id, validation_status, validation_requirement
             FROM label_ontology_actions
             ORDER BY id ASC",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(
        rows,
        vec![
            (
                "loa_none_adoption".to_owned(),
                "pending".to_owned(),
                "none".to_owned()
            ),
            (
                "loa_none_failed_add".to_owned(),
                "failed".to_owned(),
                "none".to_owned()
            ),
            (
                "loa_none_lifecycle".to_owned(),
                "pending".to_owned(),
                "none".to_owned()
            ),
            (
                "loa_none_proposal".to_owned(),
                "pending".to_owned(),
                "none".to_owned()
            ),
            (
                "loa_none_validate".to_owned(),
                "passed".to_owned(),
                "none".to_owned()
            ),
            (
                "loa_required_bootstrap".to_owned(),
                "pending".to_owned(),
                "required".to_owned()
            ),
            (
                "loa_required_positive".to_owned(),
                "pending".to_owned(),
                "required".to_owned()
            ),
            (
                "loa_unsupported_revert".to_owned(),
                "pending".to_owned(),
                "unsupported".to_owned()
            ),
            (
                "loa_unsupported_structure".to_owned(),
                "pending".to_owned(),
                "unsupported".to_owned()
            ),
            (
                "loa_unsupported_update".to_owned(),
                "pending".to_owned(),
                "unsupported".to_owned()
            ),
        ]
    );
    let check_error = result_err(conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, action_type, reason, validation_requirement, change_json,
         validation_status, validation_json, created_by, created_by_type, created_at)
         VALUES ('loa_bad_requirement', ?1, 'confirm', 'bad requirement', 'later',
         '{}', 'not_required', '{}', 'tester', 'user', 2)",
        [&board_id],
    ))?;
    assert!(check_error.to_string().contains("CHECK"));
    let fk_errors = foreign_key_check_rows(&conn)?;
    assert!(fk_errors.is_empty(), "{fk_errors:#?}");
    Ok(())
}

#[test]
fn init_v17_preflight_reports_cross_board_key_relationship_rows() -> anyhow::Result<()> {
    for table in ["task_labels", "task_dependencies", "task_runs"] {
        let temp = TempDb::new(&format!("init_v17_preflight_{table}"))?;
        let fixture = seed_v17_board_isolation_fixture(&temp)?;
        let conn = connect_file(&temp.path)?;
        conn.execute("DELETE FROM schema_migrations WHERE version=17", [])?;
        conn.pragma_update(None, "user_version", 16)?;
        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        let expected_row_key = match table {
            "task_labels" => {
                conn.execute(
                    "UPDATE task_labels SET board_id=?1 WHERE task_id=?2 AND label_id=?3",
                    params![fixture.other_board_id, fixture.task_id, fixture.label_id],
                )?;
                format!("{}:{}", fixture.task_id, fixture.label_id)
            }
            "task_dependencies" => {
                conn.execute(
                    "UPDATE task_dependencies SET board_id=?1 WHERE parent_task_id=?2 AND child_task_id=?3",
                    params![
                        fixture.other_board_id,
                        fixture.parent_task_id,
                        fixture.child_task_id
                    ],
                )?;
                format!("{}->{}", fixture.parent_task_id, fixture.child_task_id)
            }
            "task_runs" => {
                conn.execute(
                    "UPDATE task_runs SET board_id=?1 WHERE id='r_v17'",
                    [&fixture.other_board_id],
                )?;
                "r_v17".to_owned()
            }
            _ => unreachable!(),
        };
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        drop(conn);

        let err = result_err(init_database(&temp.path, "tester"))?;
        let message = err.to_string();
        assert!(
            message.contains("cannot apply migration 017_board_isolation_composite_fk"),
            "{}: {message}",
            table
        );
        assert!(message.contains(table), "{}: {message}", table);
        assert!(message.contains(&expected_row_key), "{}: {message}", table);
    }
    Ok(())
}

struct V17BoardIsolationFixture {
    board_id: String,
    other_board_id: String,
    task_id: String,
    parent_task_id: String,
    child_task_id: String,
    label_id: String,
}

fn seed_v17_board_isolation_fixture(temp: &TempDb) -> anyhow::Result<V17BoardIsolationFixture> {
    init_database(&temp.path, "tester")?;
    let other_board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("v17 task label source"),
    )?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("v17 parent"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("v17 child"),
    )?;
    let label = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "v17-label".to_owned(),
            color: None,
        },
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO task_labels(board_id, task_id, label_id, created_at) VALUES (?1, ?2, ?3, 1)",
        params![task.board_id, task.id, label.id],
    )?;
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, metadata_json) \
         VALUES ('r_v17', ?1, ?2, 'failed', 'claim', 'tester', 1, 1, '{}')",
        params![task.board_id, task.id],
    )?;

    Ok(V17BoardIsolationFixture {
        board_id: task.board_id,
        other_board_id: other_board.id,
        task_id: task.id,
        parent_task_id: parent.id,
        child_task_id: child.id,
        label_id: label.id,
    })
}

fn v17_relationship_counts(conn: &Connection) -> anyhow::Result<Vec<(&'static str, i64)>> {
    ["task_labels", "task_dependencies", "task_runs"]
        .into_iter()
        .map(|table| {
            Ok((
                table,
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?,
            ))
        })
        .collect()
}

fn task_label_board_for(
    conn: &Connection,
    task_id: &str,
    label_id: &str,
) -> anyhow::Result<String> {
    Ok(conn.query_row(
        "SELECT board_id FROM task_labels WHERE task_id=?1 AND label_id=?2",
        params![task_id, label_id],
        |row| row.get(0),
    )?)
}

fn foreign_key_check_rows(conn: &Connection) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let rows = stmt
        .query_map([], |row| {
            let table: String = row.get(0)?;
            let rowid: i64 = row.get(1)?;
            let parent: String = row.get(2)?;
            let fkid: i64 = row.get(3)?;
            Ok(format!("{table}:{rowid}->{parent}:{fkid}"))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn downgrade_label_ontology_actions_to_v18_shape(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys=OFF;
        DROP INDEX IF EXISTS idx_label_ontology_actions_board_type_created;
        DROP INDEX IF EXISTS idx_label_ontology_actions_label_created;
        DROP INDEX IF EXISTS idx_label_ontology_actions_unique_create_proposal;
        DROP INDEX IF EXISTS idx_label_ontology_actions_id_board;
        CREATE TABLE label_ontology_actions_v18 (
          id TEXT PRIMARY KEY CHECK(id LIKE 'loa_%'),
          board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
          parent_action_id TEXT REFERENCES label_ontology_actions(id) ON DELETE SET NULL,
          action_type TEXT NOT NULL CHECK(action_type IN (
            'confirm',
            'reject',
            'supersede',
            'resolve_no_change',
            'add_positive_atom',
            'add_negative_atom',
            'adopt_existing_atom',
            'update_semantics',
            'create_label_proposal',
            'bootstrap_label',
            'rename_label',
            'split_label',
            'merge_labels',
            'validate',
            'revert_ontology_mutation'
          )),
          reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
          target_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
          result_label_id TEXT REFERENCES labels(id) ON DELETE SET NULL,
          result_atom_id TEXT,
          result_atom_content_hash TEXT,
          result_proposal_id TEXT REFERENCES label_semantic_proposals(id) ON DELETE SET NULL,
          canonical_before_hash TEXT,
          canonical_after_hash TEXT,
          change_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(change_json)),
          validation_status TEXT NOT NULL DEFAULT 'not_required' CHECK(validation_status IN (
            'not_required',
            'pending',
            'passed',
            'failed',
            'partial'
          )),
          validation_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(validation_json)),
          created_by TEXT NOT NULL CHECK(length(trim(created_by)) > 0),
          created_by_type TEXT NOT NULL CHECK(created_by_type IN ('user', 'agent')),
          agent_type TEXT,
          created_at INTEGER NOT NULL
        );
        INSERT INTO label_ontology_actions_v18(
          id, board_id, parent_action_id, action_type, reason, target_label_id,
          result_label_id, result_atom_id, result_atom_content_hash, result_proposal_id,
          canonical_before_hash, canonical_after_hash, change_json, validation_status,
          validation_json, created_by, created_by_type, agent_type, created_at
        )
        SELECT
          id, board_id, parent_action_id, action_type, reason, target_label_id,
          result_label_id, result_atom_id, result_atom_content_hash, result_proposal_id,
          canonical_before_hash, canonical_after_hash, change_json, validation_status,
          validation_json, created_by, created_by_type, agent_type, created_at
        FROM label_ontology_actions;
        DROP TABLE label_ontology_actions;
        ALTER TABLE label_ontology_actions_v18 RENAME TO label_ontology_actions;
        CREATE INDEX IF NOT EXISTS idx_label_ontology_actions_board_type_created
          ON label_ontology_actions(board_id, action_type, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_label_ontology_actions_label_created
          ON label_ontology_actions(board_id, target_label_id, created_at DESC);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_actions_unique_create_proposal
          ON label_ontology_actions(board_id, result_proposal_id)
          WHERE action_type = 'create_label_proposal'
            AND result_proposal_id IS NOT NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_actions_id_board
          ON label_ontology_actions(id, board_id);
        PRAGMA foreign_keys=ON;
        ",
    )?;
    Ok(())
}

#[test]
fn init_upgrades_legacy_priority_values_to_p0_p3_range() -> anyhow::Result<()> {
    let temp = TempDb::new("init_upgrades_legacy_priority_values_to_p0_p3_range")?;

    let v1_sql = include_str!("../../../../migrations/001_initial.sql");
    let conn = Connection::open(&temp.path)?;
    conn.execute_batch(v1_sql)?;
    conn.execute(
        "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_test', 'default', 'Default', NULL, 1, 1, NULL)",
        [],
    )?;
    for (seq, title, priority) in [
        (1, "negative", -5),
        (2, "zero", 0),
        (3, "two", 2),
        (4, "eighty", 80),
    ] {
        conn.execute(
            "INSERT INTO tasks(id, board_id, seq, title, description, status, priority, created_by, created_at, updated_at, metadata_json) VALUES (?1, 'b_test', ?2, ?3, 'ready spec', 'ready', ?4, 'tester', 2, 2, '{}')",
            params![format!("t_test_{seq}"), seq, title, priority],
        )?;
    }
    conn.pragma_update(None, "user_version", 1)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = Connection::open(&temp.path)?;
    let priorities = conn
        .prepare("SELECT title, priority FROM tasks ORDER BY seq")?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(
        priorities,
        vec![
            ("negative".to_owned(), 0),
            ("zero".to_owned(), 0),
            ("two".to_owned(), 2),
            ("eighty".to_owned(), 3),
        ]
    );
    let check_error = result_err(conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, priority, created_by, created_at, updated_at, metadata_json) VALUES ('t_bad', 'b_test', 5, 'bad', 'ready spec', 'ready', 4, 'tester', 2, 2, '{}')",
        [],
    ))?;
    assert!(check_error.to_string().contains("CHECK"));
    Ok(())
}

#[test]
fn init_backfill_preserves_existing_entity_relations() -> anyhow::Result<()> {
    let temp = TempDb::new("init_backfill_preserves_existing_entity_relations")?;

    init_database(&temp.path, "tester")?;
    let first = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("relation subject"),
    )?;
    let second = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("relation object"),
    )?;

    let conn = Connection::open(&temp.path)?;
    conn.execute(
        "INSERT INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         VALUES (?1, 'related_to', ?2, 'kb://graph/indexed', 'graph', 'test', 'relation-1', NULL, '{}', 10, 10)",
        params![
            format!("kb://task/{}", first.id),
            format!("kb://task/{}", second.id)
        ],
    )?;
    let first_entity_rowid: i64 = conn.query_row(
        "SELECT rowid FROM entities WHERE uri=?1",
        [format!("kb://task/{}", first.id)],
        |row| row.get(0),
    )?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = Connection::open(&temp.path)?;
    let refreshed_first_entity_rowid: i64 = conn.query_row(
        "SELECT rowid FROM entities WHERE uri=?1",
        [format!("kb://task/{}", first.id)],
        |row| row.get(0),
    )?;
    assert_eq!(refreshed_first_entity_rowid, first_entity_rowid);
    let relation_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_relations \
         WHERE subject_uri=?1 AND predicate='related_to' AND object_uri=?2",
        params![
            format!("kb://task/{}", first.id),
            format!("kb://task/{}", second.id)
        ],
        |row| row.get(0),
    )?;
    assert_eq!(relation_count, 1);
    Ok(())
}
