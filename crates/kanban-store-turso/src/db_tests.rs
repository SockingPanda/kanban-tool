#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn fresh_database_bootstraps_canonical_tables() {
        let (_directory, store, _path) = store("bootstrap").await;
        store.initialize().await.expect("initialize");

        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_internal_%' ORDER BY name",
                (),
            )
            .await
            .expect("table query");
        let mut names = Vec::new();
        while let Some(row) = rows.next().await.expect("next table row") {
            names.push(
                text_value(row.get_value(0).expect("table name"), "sqlite_master.name")
                    .expect("text table name"),
            );
        }
        for required in [
            "board_columns",
            "boards",
            "schema_migrations",
            "task_comments",
            "task_dependencies",
            "task_events",
            "task_execution_plans",
            "task_runs",
            "task_steps",
            "tasks",
            "task_attachments",
            "labels",
            "task_labels",
            "entities",
            "entity_relations",
            "projection_jobs",
            "projection_state",
            "signal_observations",
            "signals",
            "projection_maintenance_owner",
            "import_journal",
            "attachment_staging",
            "retrieval_documents",
            "retrieval_vectors",
        ] {
            assert!(
                names.iter().any(|name| name == required),
                "missing table {required}"
            );
        }
    }

    #[tokio::test]
    async fn fresh_database_records_full_turso_lineage() {
        let (_directory, store, _path) = store("lineage").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query(
                "SELECT schema_family, name, checksum FROM schema_migrations ORDER BY version",
                (),
            )
            .await
            .expect("ledger query");
        let mut ledger = Vec::new();
        while let Some(row) = rows.next().await.expect("next ledger row") {
            ledger.push((
                text_value(row.get_value(0).expect("family"), "family").expect("family text"),
                text_value(row.get_value(1).expect("name"), "name").expect("name text"),
                text_value(row.get_value(2).expect("checksum"), "checksum").expect("checksum text"),
            ));
        }
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger[0].0, "kanban.turso");
        assert_eq!(ledger[0].1, "001_canonical_baseline");
        assert_eq!(ledger[0].2, crate::schema::CURRENT_V1_SCHEMA_FINGERPRINT);
        assert_eq!(ledger[1].1, "002_turso_full_feature_baseline");
        assert_eq!(ledger[1].2, crate::migration::full_schema_fingerprint());
    }

    #[tokio::test]
    async fn current_v1_fixture_is_adopted_and_upgraded() {
        let (_directory, store, path) = store("v1-upgrade").await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute_batch(crate::schema::CANONICAL_SCHEMA)
            .await
            .expect("v1 schema");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_default', 'default', 'Default', 1, 1)",
                (),
            )
            .await
            .expect("v1 board");
        connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, title, status, priority, position, created_by, created_at, updated_at, metadata_json) VALUES ('t_fixture', 'b_default', 1, 'Fixture', 'todo', 1, 1024, 'fixture', 1, 1, '{}')",
                (),
            )
            .await
            .expect("v1 task");
        connection
            .execute(
                "INSERT INTO task_comments(id, board_id, task_id, idempotency_key, author, body, kind, created_at) VALUES ('c_fixture', 'b_default', 't_fixture', 'comment-key', 'fixture', 'fixture body', 'decision', 1)",
                (),
            )
            .await
            .expect("v1 comment");
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at) VALUES (1, '001_canonical_baseline', '', 1)",
                (),
            )
            .await
            .expect("v1 ledger");
        drop(connection);

        store.initialize().await.expect("upgrade");
        let backup_prefix = format!(
            "{}.pre-v2-",
            path.file_name()
                .and_then(|name| name.to_str())
                .expect("database file name")
        );
        let backups = std::fs::read_dir(path.parent().expect("database parent"))
            .expect("backup directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with(&backup_prefix) && name.ends_with(".turso-backup")
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1, "upgrade must create exactly one backup");
        assert!(
            backups[0].metadata().expect("backup metadata").len() > 0,
            "upgrade backup must be non-empty"
        );
        let reopened = crate::TursoStore::open(&path).await.expect("reopen");
        reopened.initialize().await.expect("idempotent upgrade");
        let backups_after_reopen = std::fs::read_dir(path.parent().expect("database parent"))
            .expect("backup directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with(&backup_prefix) && name.ends_with(".turso-backup")
                })
            })
            .count();
        assert_eq!(
            backups_after_reopen, 1,
            "idempotent startup must not create another backup"
        );
        let report = reopened
            .capability_report()
            .await
            .expect("capability report");
        assert!(report.iter().any(|item| item.capability == "vector32"));
        let connection = reopened.connection().await.expect("upgraded connection");
        let mut rows = connection
            .query(
                "SELECT idempotency_key, kind, body FROM task_comments WHERE id='c_fixture'",
                (),
            )
            .await
            .expect("comment query");
        let row = rows
            .next()
            .await
            .expect("comment row")
            .expect("comment result");
        assert_eq!(
            text_value(row.get_value(0).expect("comment key"), "comment key")
                .expect("comment key text"),
            "comment-key"
        );
        assert_eq!(
            text_value(row.get_value(1).expect("comment kind"), "comment kind")
                .expect("comment kind text"),
            "decision"
        );
        assert_eq!(
            text_value(row.get_value(2).expect("comment body"), "comment body")
                .expect("comment body text"),
            "fixture body"
        );
        connection
            .execute(
                "INSERT INTO task_comments(id, board_id, task_id, idempotency_key, author, body, kind, created_at) VALUES ('c_signal', 'b_default', 't_fixture', 'signal-key', 'fixture', 'signal body', 'signal', 2)",
                (),
            )
            .await
            .expect("signal comment kind");
    }

    #[tokio::test]
    async fn unknown_same_number_schema_is_rejected_without_adoption() {
        let (_directory, store, _path) = store("unknown-schema").await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute_batch(crate::schema::CANONICAL_SCHEMA)
            .await
            .expect("v1 schema");
        connection
            .execute(
                "CREATE TABLE projection_database(singleton INTEGER PRIMARY KEY)",
                (),
            )
            .await
            .expect("foreign schema marker");
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at) VALUES (1, '001_canonical_baseline', '', 1)",
                (),
            )
            .await
            .expect("v1 ledger");
        let error = store
            .initialize()
            .await
            .expect_err("must reject unknown schema");
        assert!(error.to_string().contains("schema 不匹配"));
    }

    #[tokio::test]
    async fn backup_hook_failure_leaves_current_v1_untouched() {
        struct Reject;
        impl crate::UpgradeBackupHook for Reject {
            fn before_upgrade(
                &self,
                _request: &crate::UpgradeBackupRequest,
            ) -> Result<(), crate::StoreError> {
                Err(crate::StoreError::BackupRequired("test refusal".to_owned()))
            }
        }

        let (_directory, store, _path) = store("backup-hook").await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute_batch(crate::schema::CANONICAL_SCHEMA)
            .await
            .expect("v1 schema");
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at) VALUES (1, '001_canonical_baseline', '', 1)",
                (),
            )
            .await
            .expect("v1 ledger");
        let error = store
            .initialize_requiring_backup(&Reject)
            .await
            .expect_err("backup hook must block upgrade");
        assert!(error.to_string().contains("需要备份"));
        let connection = store.connection().await.expect("connection after refusal");
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
                (),
            )
            .await
            .expect("ledger check");
        let row = rows.next().await.expect("row").expect("count row");
        assert_eq!(
            integer_value(row.get_value(0).expect("count"), "count").expect("count"),
            0
        );
    }

    #[tokio::test]
    async fn migration_failure_rolls_back_schema_and_ledger_changes() {
        let (_directory, store, _path) = store("migration-rollback").await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute_batch(crate::schema::CANONICAL_SCHEMA)
            .await
            .expect("v1 schema");
        connection
            .execute_batch(
                r#"
PRAGMA foreign_keys = OFF;
INSERT INTO boards(id, slug, name, created_at, updated_at)
VALUES ('b_one', 'one', 'One', 1, 1), ('b_two', 'two', 'Two', 1, 1);
INSERT INTO tasks(
  id, board_id, seq, title, status, priority, position, created_by,
  created_at, updated_at, metadata_json
) VALUES ('t_two', 'b_two', 1, 'Two', 'todo', 1, 1024, 'fixture', 1, 1, '{}');
INSERT INTO task_events(event_id, board_id, task_id, kind, payload_json, created_at)
VALUES ('e_cross_board', 'b_one', 't_two', 'fixture', '{}', 1);
INSERT INTO schema_migrations(version, name, checksum, applied_at)
VALUES (1, '001_canonical_baseline', '', 1);
PRAGMA foreign_keys = ON;
"#,
            )
            .await
            .expect("v1 fixture with invalid reference");
        drop(connection);

        let error = store
            .initialize()
            .await
            .expect_err("foreign key preflight must abort migration");
        assert!(error.to_string().contains("board isolation preflight"));

        let connection = store.connection().await.expect("connection after rollback");
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
                (),
            )
            .await
            .expect("ledger query");
        let row = rows.next().await.expect("ledger row").expect("count row");
        assert_eq!(
            integer_value(row.get_value(0).expect("count"), "count").expect("count"),
            0
        );
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_identity'",
                (),
            )
            .await
            .expect("identity table query");
        let row = rows.next().await.expect("identity row").expect("count row");
        assert_eq!(
            integer_value(row.get_value(0).expect("count"), "count").expect("count"),
            0
        );
        let mut rows = connection
            .query("PRAGMA table_info('schema_migrations')", ())
            .await
            .expect("schema_migrations columns");
        let mut has_schema_family = false;
        while let Some(row) = rows.next().await.expect("column row") {
            has_schema_family |= text_value(row.get_value(1).expect("column name"), "column name")
                .expect("column name text")
                == "schema_family";
        }
        assert!(!has_schema_family);
    }

    #[tokio::test]
    async fn upgraded_schema_rejects_column_and_trigger_drift() {
        let (_directory, shape_store, path) = store("full-shape-drift").await;
        shape_store.initialize().await.expect("initialize");
        let connection = shape_store.connection().await.expect("connection");
        connection
            .execute("ALTER TABLE label_semantics ADD COLUMN unexpected TEXT", ())
            .await
            .expect("tamper columns");
        drop(connection);
        drop(shape_store);

        let reopened = crate::TursoStore::open(&path).await.expect("reopen");
        let error = reopened
            .initialize()
            .await
            .expect_err("column drift must fail closed");
        assert!(error.to_string().contains("column fingerprint"));

        let (_directory, trigger_store, trigger_path) = store("full-trigger-drift").await;
        trigger_store
            .initialize()
            .await
            .expect("trigger initialize");
        let connection = trigger_store
            .connection()
            .await
            .expect("trigger connection");
        connection
            .execute("DROP TRIGGER task_events_board_guard_insert", ())
            .await
            .expect("tamper trigger");
        drop(connection);
        drop(trigger_store);
        let trigger_reopened = crate::TursoStore::open(trigger_path)
            .await
            .expect("reopen trigger database");
        let error = trigger_reopened
            .initialize()
            .await
            .expect_err("trigger drift must fail closed");
        assert!(error.to_string().contains("trigger"));
    }

    #[tokio::test]
    async fn initialize_is_idempotent_and_seeds_default_board_columns() {
        let (_directory, store, path) = store("idempotent").await;
        store.initialize().await.expect("first initialize");
        store.initialize().await.expect("second initialize");

        let boards = store.list_boards(false).await.expect("list boards");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].slug, "default");

        let columns = store
            .list_board_columns("default")
            .await
            .expect("list columns");
        assert_eq!(columns.len(), 9);
        assert_eq!(
            columns
                .iter()
                .map(|column| (column.status.as_str(), column.position, column.hidden))
                .collect::<Vec<_>>(),
            vec![
                ("triage", 10, false),
                ("todo", 20, false),
                ("scheduled", 30, false),
                ("ready", 40, false),
                ("running", 50, false),
                ("blocked", 60, false),
                ("review", 70, false),
                ("done", 80, false),
                ("archived", 90, true),
            ]
        );

        drop(store);
        let reopened = TursoStore::open(path).await.expect("reopen database");
        reopened.initialize().await.expect("reinitialize database");
        assert_eq!(
            reopened
                .list_boards(false)
                .await
                .expect("list after reopen")
                .len(),
            1
        );
        assert_eq!(
            reopened
                .list_board_columns("b_default")
                .await
                .expect("columns by id")
                .len(),
            9
        );
    }

    #[tokio::test]
    async fn include_archived_filters_and_orders_boards() {
        let (_directory, store, _path) = store("archived").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
                ("b_archived", "archive", "Archive", 2_i64, 3_i64),
            )
            .await
            .expect("insert archived board");

        let active = store.list_boards(false).await.expect("active boards");
        assert_eq!(
            active
                .iter()
                .map(|board| board.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["default"]
        );
        let all = store.list_boards(true).await.expect("all boards");
        assert_eq!(
            all.iter()
                .map(|board| board.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["default", "archive"]
        );
    }
}
