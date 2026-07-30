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
fn init_upgrades_v26_with_singleton_maintenance_owner_idempotently() -> anyhow::Result<()> {
    let temp = TempDb::new("init_upgrades_v26_maintenance_owner")?;
    init_database(&temp.path, "first")?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch(
        "DROP TABLE projection_maintenance_owner;
         DELETE FROM schema_migrations WHERE version IN (27,28);
         PRAGMA user_version=26;",
    )?;
    drop(conn);

    init_database(&temp.path, "upgrade")?;
    init_database(&temp.path, "idempotent")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let migration_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations
         WHERE version=27 AND name='027_projection_maintenance_owner'
           AND checksum GLOB 'fnv64:*'",
        [],
        |row| row.get(0),
    )?;
    let owner_rows: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_maintenance_owner
         WHERE singleton=1 AND owner IS NULL AND lease_token IS NULL",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(user_version, 29);
    assert_eq!(migration_count, 1);
    assert_eq!(owner_rows, 1);
    Ok(())
}

#[test]
fn init_upgrades_v27_owner_with_runtime_identity_and_invalidates_unknown_lease()
-> anyhow::Result<()> {
    let temp = TempDb::new("init_upgrades_v27_owner_identity")?;
    init_database(&temp.path, "first")?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch(
        "ALTER TABLE projection_maintenance_owner DROP COLUMN capabilities_json;
         ALTER TABLE projection_maintenance_owner DROP COLUMN build_identity;
         UPDATE projection_maintenance_owner
         SET owner='legacy-owner',lease_token='legacy-token',lease_expires_at=4102444800000,
             mode='continuous',started_at=1,last_heartbeat_at=1,updated_at=1
         WHERE singleton=1;
         DELETE FROM schema_migrations WHERE version=28;
         PRAGMA user_version=27;",
    )?;
    drop(conn);

    init_database(&temp.path, "upgrade")?;
    init_database(&temp.path, "idempotent")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let migration_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations
         WHERE version=28 AND name='028_projection_maintenance_runtime_identity'
           AND checksum GLOB 'fnv64:*'",
        [],
        |row| row.get(0),
    )?;
    let owner_row: (Option<String>, Option<String>, String, Option<String>) = conn.query_row(
        "SELECT owner,lease_token,capabilities_json,build_identity
         FROM projection_maintenance_owner WHERE singleton=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    assert_eq!(user_version, 29);
    assert_eq!(migration_count, 1);
    assert_eq!(owner_row, (None, None, "[]".to_owned(), None));
    Ok(())
}

#[test]
fn init_v26_preflight_reports_unresolved_and_cross_board_outbox_rows() -> anyhow::Result<()> {
    for case_name in ["unresolved", "cross_board"] {
        let temp = TempDb::new(&format!("init_v26_preflight_{case_name}"))?;
        init_database(&temp.path, "tester")?;
        if case_name == "cross_board" {
            create_task(
                &temp.path,
                "default",
                "tester",
                CreateTask::ready("v26 board scope source"),
            )?;
        }

        let conn = connect_file(&temp.path)?;
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS projection_deliveries_after_outbox_insert;
             DROP TRIGGER IF EXISTS projection_deliveries_after_legacy_outbox_done;
             DROP TABLE projection_deliveries;
             DROP TABLE projection_store_state;
             DROP TABLE projection_database;
             DELETE FROM schema_migrations WHERE version=26;
             PRAGMA user_version=25;",
        )?;

        let expected_key = if case_name == "unresolved" {
            conn.execute(
                "INSERT INTO index_outbox(
                   source_event_id,target,entity_uri,action,payload_json,status,attempts,created_at,updated_at
                 ) VALUES(NULL,'tantivy','kb://task/missing','upsert','{}','pending',0,1,1)",
                [],
            )?;
            "kb://task/missing".to_owned()
        } else {
            conn.execute(
                "INSERT INTO boards(id,slug,name,created_at,updated_at)
                 VALUES('b_v26_other','v26-other','V26 Other',1,1)",
                [],
            )?;
            let entity_uri: String = conn.query_row(
                "SELECT o.entity_uri
                 FROM index_outbox o
                 JOIN task_events e ON e.id=o.source_event_id
                 JOIN entities entity ON entity.uri=o.entity_uri
                 LIMIT 1",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "UPDATE entities SET board_id='b_v26_other' WHERE uri=?1",
                [&entity_uri],
            )?;
            entity_uri
        };
        drop(conn);

        let err = result_err(init_database(&temp.path, "tester"))?;
        let message = err.to_string();
        assert!(
            message.contains("cannot apply migration 026_projection_v2"),
            "{case_name}: {message}"
        );
        assert!(message.contains(&expected_key), "{case_name}: {message}");
        assert!(
            message.contains("repair the canonical board mapping"),
            "{case_name}: {message}"
        );
    }
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
    assert_eq!(user_version, 29);
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
    assert_eq!(user_version, 29);
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
        "signal_observations",
        "signals",
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
    assert_eq!(user_version, 29);
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
    downgrade_task_history_to_v19_shape(&conn)?;
    conn.execute("DELETE FROM schema_migrations WHERE version=17", [])?;
    conn.execute("DELETE FROM schema_migrations WHERE version=20", [])?;
    conn.pragma_update(None, "user_version", 16)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 29);
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
    assert_eq!(user_version, 29);
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
    assert_eq!(user_version, 29);
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
fn init_v20_hardens_task_history_tables_without_losing_rows() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v20_hardens_task_history_tables_without_losing_rows")?;
    seed_v20_task_history_fixture(&temp)?;

    let conn = connect_file(&temp.path)?;
    let before_counts = v20_task_history_counts(&conn)?;
    downgrade_task_history_to_v19_shape(&conn)?;
    conn.execute("DELETE FROM schema_migrations WHERE version=20", [])?;
    conn.pragma_update(None, "user_version", 19)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 29);
    assert_eq!(v20_task_history_counts(&conn)?, before_counts);
    let fk_errors = foreign_key_check_rows(&conn)?;
    assert!(fk_errors.is_empty(), "{fk_errors:#?}");
    Ok(())
}

#[test]
fn init_v20_preflight_reports_cross_board_task_history_rows() -> anyhow::Result<()> {
    for table in ["task_comments", "task_events", "task_attachments"] {
        let temp = TempDb::new(&format!("init_v20_preflight_{table}"))?;
        let fixture = seed_v20_task_history_fixture(&temp)?;
        let conn = connect_file(&temp.path)?;
        downgrade_task_history_to_v19_shape(&conn)?;
        conn.execute("DELETE FROM schema_migrations WHERE version=20", [])?;
        conn.pragma_update(None, "user_version", 19)?;
        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        let expected_row_key = match table {
            "task_comments" => {
                conn.execute(
                    "UPDATE task_comments SET board_id=?1 WHERE id='c_v20'",
                    [&fixture.other_board_id],
                )?;
                "c_v20".to_owned()
            }
            "task_events" => {
                conn.execute(
                    "UPDATE task_events SET board_id=?1 WHERE event_id='e_v20'",
                    [&fixture.other_board_id],
                )?;
                "e_v20".to_owned()
            }
            "task_attachments" => {
                conn.execute(
                    "UPDATE task_attachments SET board_id=?1 WHERE id='a_v20'",
                    [&fixture.other_board_id],
                )?;
                "a_v20".to_owned()
            }
            _ => unreachable!(),
        };
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        drop(conn);

        let err = result_err(init_database(&temp.path, "tester"))?;
        let message = err.to_string();
        assert!(
            message.contains("cannot apply migration 020_board_isolation_task_history"),
            "{}: {message}",
            table
        );
        assert!(message.contains(table), "{}: {message}", table);
        assert!(message.contains(&expected_row_key), "{}: {message}", table);
    }
    Ok(())
}

#[test]
fn init_v20_preflight_reports_orphan_task_history_rows() -> anyhow::Result<()> {
    for case in [
        ("task_comments", "c_v20"),
        ("task_events_task", "e_v20"),
        ("task_events_run", "e_v20"),
        ("task_attachments", "a_v20"),
    ] {
        let (case_name, expected_row_key) = case;
        let temp = TempDb::new(&format!("init_v20_preflight_orphan_{case_name}"))?;
        seed_v20_task_history_fixture(&temp)?;
        let conn = connect_file(&temp.path)?;
        downgrade_task_history_to_v19_shape(&conn)?;
        conn.execute("DELETE FROM schema_migrations WHERE version=20", [])?;
        conn.pragma_update(None, "user_version", 19)?;
        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        match case_name {
            "task_comments" => {
                conn.execute(
                    "UPDATE task_comments SET task_id='t_missing' WHERE id='c_v20'",
                    [],
                )?;
            }
            "task_events_task" => {
                conn.execute(
                    "UPDATE task_events SET task_id='t_missing' WHERE event_id='e_v20'",
                    [],
                )?;
            }
            "task_events_run" => {
                conn.execute(
                    "UPDATE task_events SET run_id='r_missing' WHERE event_id='e_v20'",
                    [],
                )?;
            }
            "task_attachments" => {
                conn.execute(
                    "UPDATE task_attachments SET task_id='t_missing' WHERE id='a_v20'",
                    [],
                )?;
            }
            _ => unreachable!(),
        };
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        drop(conn);

        let err = result_err(init_database(&temp.path, "tester"))?;
        let message = err.to_string();
        assert!(
            message.contains("cannot apply migration 020_board_isolation_task_history"),
            "{}: {message}",
            case_name
        );
        assert!(message.contains("task_"), "{}: {message}", case_name);
        assert!(
            message.contains(expected_row_key),
            "{}: {message}",
            case_name
        );
        assert!(message.contains("missing"), "{}: {message}", case_name);
    }
    Ok(())
}

#[test]
fn init_v21_hardens_ontology_links_without_losing_rows() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v21_hardens_ontology_links_without_losing_rows")?;
    seed_v21_ontology_link_fixture(&temp)?;

    let conn = connect_file(&temp.path)?;
    let before_counts = v21_ontology_link_counts(&conn)?;
    downgrade_ontology_links_to_v20_shape(&conn)?;
    conn.execute("DELETE FROM schema_migrations WHERE version=21", [])?;
    conn.pragma_update(None, "user_version", 20)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 29);
    assert_eq!(v21_ontology_link_counts(&conn)?, before_counts);
    let fk_errors = foreign_key_check_rows(&conn)?;
    assert!(fk_errors.is_empty(), "{fk_errors:#?}");
    Ok(())
}

#[test]
fn init_v21_preflight_reports_cross_board_and_orphan_ontology_links() -> anyhow::Result<()> {
    for case in [
        (
            "proposal_resolved_label",
            "label_semantic_proposals",
            "lp_v21",
        ),
        ("signal_observation", "label_ontology_signals", "los_v21"),
        ("signal_target_label", "label_ontology_signals", "los_v21"),
        ("signal_supersede", "label_ontology_signals", "los_v21"),
        ("action_parent", "label_ontology_actions", "loa_v21"),
        ("action_target_label", "label_ontology_actions", "loa_v21"),
        ("action_result_label", "label_ontology_actions", "loa_v21"),
        (
            "action_result_proposal",
            "label_ontology_actions",
            "loa_v21",
        ),
        (
            "action_signal_orphan",
            "label_ontology_action_signals",
            "loa_v21:los_missing",
        ),
    ] {
        let (case_name, expected_table, expected_row_key) = case;
        let temp = TempDb::new(&format!("init_v21_preflight_{case_name}"))?;
        let fixture = seed_v21_ontology_link_fixture(&temp)?;
        let conn = connect_file(&temp.path)?;
        downgrade_ontology_links_to_v20_shape(&conn)?;
        conn.execute("DELETE FROM schema_migrations WHERE version=21", [])?;
        conn.pragma_update(None, "user_version", 20)?;
        conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        match case_name {
            "proposal_resolved_label" => {
                conn.execute(
                    "UPDATE label_semantic_proposals SET resolved_label_id=?1 WHERE id='lp_v21'",
                    [&fixture.other_label_id],
                )?;
            }
            "signal_observation" => {
                conn.execute(
                    "UPDATE label_ontology_signals SET observation_id=?1 WHERE id='los_v21'",
                    [&fixture.other_observation_id],
                )?;
            }
            "signal_target_label" => {
                conn.execute(
                    "UPDATE label_ontology_signals SET target_label_id=?1 WHERE id='los_v21'",
                    [&fixture.other_label_id],
                )?;
            }
            "signal_supersede" => {
                conn.execute(
                    "UPDATE label_ontology_signals SET superseded_by_signal_id=?1 WHERE id='los_v21'",
                    [&fixture.other_signal_id],
                )?;
            }
            "action_parent" => {
                conn.execute(
                    "UPDATE label_ontology_actions SET parent_action_id=?1 WHERE id='loa_v21'",
                    [&fixture.other_action_id],
                )?;
            }
            "action_target_label" => {
                conn.execute(
                    "UPDATE label_ontology_actions SET target_label_id=?1 WHERE id='loa_v21'",
                    [&fixture.other_label_id],
                )?;
            }
            "action_result_label" => {
                conn.execute(
                    "UPDATE label_ontology_actions SET result_label_id=?1 WHERE id='loa_v21'",
                    [&fixture.other_label_id],
                )?;
            }
            "action_result_proposal" => {
                conn.execute(
                    "UPDATE label_ontology_actions SET result_proposal_id=?1 WHERE id='loa_v21'",
                    [&fixture.other_proposal_id],
                )?;
            }
            "action_signal_orphan" => {
                conn.execute(
                    "UPDATE label_ontology_action_signals SET signal_id='los_missing'
                     WHERE action_id='loa_v21' AND signal_id='los_v21'",
                    [],
                )?;
            }
            _ => unreachable!(),
        };
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        drop(conn);

        let err = result_err(init_database(&temp.path, "tester"))?;
        let message = err.to_string();
        assert!(
            message.contains("cannot apply migration 021_board_isolation_ontology_links"),
            "{}: {message}",
            case_name
        );
        assert!(message.contains(expected_table), "{}: {message}", case_name);
        assert!(
            message.contains(expected_row_key),
            "{}: {message}",
            case_name
        );
    }
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
    run_id: String,
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
        run_id: "r_v17".to_owned(),
    })
}

struct V20TaskHistoryFixture {
    other_board_id: String,
}

fn seed_v20_task_history_fixture(temp: &TempDb) -> anyhow::Result<V20TaskHistoryFixture> {
    let fixture = seed_v17_board_isolation_fixture(temp)?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO task_comments(id, board_id, task_id, author, author_type, body, kind, metadata_json, created_at) \
         VALUES ('c_v20', ?1, ?2, 'tester', 'user', 'v20 task history note', 'note', '{}', 1)",
        params![fixture.board_id, fixture.task_id],
    )?;
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, payload_json, created_at) \
         VALUES ('e_v20', ?1, ?2, ?3, 'test.history', '{}', 1)",
        params![fixture.board_id, fixture.task_id, fixture.run_id],
    )?;
    conn.execute(
        "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, size_bytes, created_by, created_at) \
         VALUES ('a_v20', ?1, ?2, 'history.txt', 'attachments/history.txt', 0, 'tester', 1)",
        params![fixture.board_id, fixture.task_id],
    )?;
    Ok(V20TaskHistoryFixture {
        other_board_id: fixture.other_board_id,
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

fn v20_task_history_counts(conn: &Connection) -> anyhow::Result<Vec<(&'static str, i64)>> {
    [
        "task_runs",
        "task_comments",
        "task_events",
        "task_attachments",
    ]
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

struct V21OntologyLinkFixture {
    other_label_id: String,
    other_observation_id: String,
    other_signal_id: String,
    other_action_id: String,
    other_proposal_id: String,
}

fn seed_v21_ontology_link_fixture(temp: &TempDb) -> anyhow::Result<V21OntologyLinkFixture> {
    let fixture = seed_v17_board_isolation_fixture(temp)?;
    let other_task = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("v21 other task"),
    )?;
    let default_label = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "v21-default-label".to_owned(),
            color: None,
        },
    )?;
    let other_label = create_label(
        &temp.path,
        "other",
        CreateLabel {
            name: "v21-other-label".to_owned(),
            color: None,
        },
    )?;
    let conn = connect_file(&temp.path)?;
    for (observation_id, board_id, task_id, task_ref, fingerprint) in [
        (
            "lor_v21",
            fixture.board_id.as_str(),
            fixture.task_id.as_str(),
            "default#fixture",
            "v21-default-fingerprint",
        ),
        (
            "lor_v21_other",
            other_task.board_id.as_str(),
            other_task.id.as_str(),
            "other#fixture",
            "v21-other-fingerprint",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_observations(
             id, board_id, task_id, task_ref_snapshot, task_snapshot_json, suggest_input_hash,
             agent_candidates_json, suggestion_snapshot_json, final_decision_json,
             diagnostics_json, capture_fingerprint, created_by, created_by_type, created_at)
             VALUES (?1, ?2, ?3, ?4, '{}', 'v21hash', '[]', '{}', '{}', '[]',
             ?5, 'tester', 'user', 1)",
            params![observation_id, board_id, task_id, task_ref, fingerprint],
        )?;
    }
    for (proposal_id, board_id, task_id, name) in [
        (
            "lp_v21",
            fixture.board_id.as_str(),
            fixture.task_id.as_str(),
            "v21-default-proposal",
        ),
        (
            "lp_v21_other",
            other_task.board_id.as_str(),
            other_task.id.as_str(),
            "v21-other-proposal",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_semantic_proposals(
             id, board_id, task_id, status, name, applies_when, excludes_when,
             positive_examples, negative_examples, heuristic_coverage,
             heuristic_coverage_cosine, heuristic_residual_norm, diagnostics_json,
             created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'proposed', ?4, '[]', '[]', '[]', '[]',
             0.1, 0.1, 0.9, '[]', 'tester', 1, 1)",
            params![proposal_id, board_id, task_id, name],
        )?;
    }
    for (signal_id, observation_id, board_id, label_id, signal_key) in [
        (
            "los_v21",
            "lor_v21",
            fixture.board_id.as_str(),
            default_label.id.as_str(),
            "v21-default-signal",
        ),
        (
            "los_v21_other",
            "lor_v21_other",
            other_task.board_id.as_str(),
            other_label.id.as_str(),
            "v21-other-signal",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_signals(
             id, observation_id, board_id, kind, status, target_label_id, related_labels_json,
             proposed_action, proposal_json, agent_selected, final_selected, rationale, signal_key,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, 'false_negative', 'open', ?4, '[]',
             'add_positive_atom', '{}', 1, 1, 'v21 signal fixture', ?5, 1, 1)",
            params![signal_id, observation_id, board_id, label_id, signal_key],
        )?;
    }
    for (action_id, board_id, label_id, proposal_id) in [
        (
            "loa_v21",
            fixture.board_id.as_str(),
            default_label.id.as_str(),
            "lp_v21",
        ),
        (
            "loa_v21_other",
            other_task.board_id.as_str(),
            other_label.id.as_str(),
            "lp_v21_other",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_actions(
             id, board_id, action_type, reason, target_label_id, result_label_id,
             result_proposal_id, change_json, validation_requirement, validation_status,
             validation_json, created_by, created_by_type, created_at)
             VALUES (?1, ?2, 'create_label_proposal', 'v21 action fixture',
             ?3, ?3, ?4, '{}', 'none', 'not_required', '{}', 'tester', 'user', 1)",
            params![action_id, board_id, label_id, proposal_id],
        )?;
    }
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
         VALUES (?1, 'loa_v21', 'los_v21', 1)",
        [&fixture.board_id],
    )?;

    Ok(V21OntologyLinkFixture {
        other_label_id: other_label.id,
        other_observation_id: "lor_v21_other".to_owned(),
        other_signal_id: "los_v21_other".to_owned(),
        other_action_id: "loa_v21_other".to_owned(),
        other_proposal_id: "lp_v21_other".to_owned(),
    })
}

fn v21_ontology_link_counts(conn: &Connection) -> anyhow::Result<Vec<(&'static str, i64)>> {
    [
        "label_semantic_proposals",
        "label_ontology_observations",
        "label_ontology_signals",
        "label_ontology_actions",
        "label_ontology_action_signals",
    ]
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

fn downgrade_ontology_links_to_v20_shape(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys=OFF;
        DROP TRIGGER IF EXISTS trg_label_ontology_signals_board_insert;
        DROP TRIGGER IF EXISTS trg_label_ontology_signals_board_update;
        DROP TRIGGER IF EXISTS trg_label_ontology_actions_board_insert;
        DROP TRIGGER IF EXISTS trg_label_ontology_actions_board_update;
        DROP TRIGGER IF EXISTS trg_label_semantic_proposals_board_insert;
        DROP TRIGGER IF EXISTS trg_label_semantic_proposals_board_update;
        DROP INDEX IF EXISTS idx_label_ontology_action_signals_signal;
        DROP TABLE IF EXISTS label_ontology_action_signals_v20;
        CREATE TABLE label_ontology_action_signals_v20 (
          board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
          action_id TEXT NOT NULL REFERENCES label_ontology_actions(id) ON DELETE CASCADE,
          signal_id TEXT NOT NULL REFERENCES label_ontology_signals(id) ON DELETE CASCADE,
          created_at INTEGER NOT NULL,
          PRIMARY KEY(action_id, signal_id)
        );
        INSERT INTO label_ontology_action_signals_v20(board_id, action_id, signal_id, created_at)
        SELECT board_id, action_id, signal_id, created_at
        FROM label_ontology_action_signals;
        DROP TABLE label_ontology_action_signals;
        ALTER TABLE label_ontology_action_signals_v20 RENAME TO label_ontology_action_signals;
        CREATE INDEX IF NOT EXISTS idx_label_ontology_action_signals_signal
          ON label_ontology_action_signals(signal_id, action_id);
        DROP INDEX IF EXISTS idx_label_ontology_observations_id_board;
        DROP INDEX IF EXISTS idx_label_ontology_signals_id_board;
        DROP INDEX IF EXISTS idx_label_semantic_proposals_id_board;
        PRAGMA foreign_keys=ON;
        ",
    )?;
    Ok(())
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

fn downgrade_task_history_to_v19_shape(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys=OFF;
        DROP TRIGGER IF EXISTS trg_task_events_board_insert;
        DROP TRIGGER IF EXISTS trg_task_events_board_update;
        DROP INDEX IF EXISTS idx_task_runs_id_board;
        DROP INDEX IF EXISTS idx_comments_task_created;
        DROP TABLE IF EXISTS task_comments_v19;
        CREATE TABLE task_comments_v19 (
          id TEXT PRIMARY KEY CHECK(id LIKE 'c_%'),
          board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
          task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
          author TEXT NOT NULL,
          author_type TEXT NOT NULL DEFAULT 'user' CHECK(author_type IN ('user', 'agent')),
          agent_type TEXT CHECK(author_type = 'agent' OR agent_type IS NULL),
          body TEXT NOT NULL CHECK(length(trim(body)) > 0),
          kind TEXT NOT NULL DEFAULT 'note' CHECK(kind IN ('note', 'decision')),
          metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json) = 'object'),
          created_at INTEGER NOT NULL
        );
        INSERT INTO task_comments_v19(
          id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at
        )
        SELECT id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at
        FROM task_comments;
        DROP TABLE task_comments;
        ALTER TABLE task_comments_v19 RENAME TO task_comments;
        CREATE INDEX IF NOT EXISTS idx_comments_task_created
          ON task_comments(task_id, created_at ASC);

        DROP TABLE IF EXISTS task_attachments_v19;
        CREATE TABLE task_attachments_v19 (
          id TEXT PRIMARY KEY CHECK(id LIKE 'a_%'),
          board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
          task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
          filename TEXT NOT NULL CHECK(length(trim(filename)) > 0),
          rel_path TEXT NOT NULL CHECK(length(trim(rel_path)) > 0),
          content_type TEXT,
          size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
          sha256 TEXT,
          created_by TEXT NOT NULL,
          created_at INTEGER NOT NULL
        );
        INSERT INTO task_attachments_v19(
          id, board_id, task_id, filename, rel_path, content_type, size_bytes, sha256, created_by, created_at
        )
        SELECT id, board_id, task_id, filename, rel_path, content_type, size_bytes, sha256, created_by, created_at
        FROM task_attachments;
        DROP TABLE task_attachments;
        ALTER TABLE task_attachments_v19 RENAME TO task_attachments;
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
