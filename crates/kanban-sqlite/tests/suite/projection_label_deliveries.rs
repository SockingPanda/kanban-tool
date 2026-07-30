use crate::common::*;

const LABEL_ATOMS_STORE: &str = "lancedb_label_atoms";
const LABEL_ATOMS_PAYLOAD: &str = r#"{"scope":"board","version":1}"#;

const LEGACY_PROJECTION_FANOUT_TRIGGER: &str = r#"
CREATE TRIGGER projection_deliveries_after_outbox_insert
AFTER INSERT ON index_outbox
BEGIN
  INSERT INTO projection_deliveries(
    outbox_id, store_name, board_id, source_event_id, cursor, action,
    entity_uri, payload_json, status, attempts, next_attempt_at,
    created_at, updated_at
  )
  SELECT
    NEW.id,
    stores.store_name,
    COALESCE(
      (SELECT board_id FROM task_events WHERE id=NEW.source_event_id),
      (SELECT board_id FROM entities WHERE uri=NEW.entity_uri)
    ),
    NEW.source_event_id,
    NEW.id,
    NEW.action,
    NEW.entity_uri,
    NEW.payload_json,
    CASE WHEN NEW.status='done' THEN 'legacy_done' ELSE 'pending' END,
    NEW.attempts,
    0,
    NEW.created_at,
    NEW.updated_at
  FROM (
    SELECT 'tantivy_tasks' AS store_name WHERE NEW.target IN ('tantivy', 'all')
    UNION ALL
    SELECT 'oxigraph_relations' WHERE NEW.target IN ('oxigraph', 'all')
    UNION ALL
    SELECT 'lancedb_chunks' WHERE NEW.target IN ('lancedb', 'all')
  ) stores;
END;
"#;

fn downgrade_projection_label_delivery_to_v28(conn: &Connection) -> anyhow::Result<()> {
    let has_projection_store: bool = conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM pragma_table_info('index_outbox') WHERE name='projection_store'
         )",
        [],
        |row| row.get(0),
    )?;
    if has_projection_store {
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS label_atom_delivery_after_board_insert;
             DROP TRIGGER IF EXISTS label_atom_delivery_after_board_update;
             DROP TRIGGER IF EXISTS index_outbox_projection_route_immutable;
             DROP INDEX IF EXISTS idx_index_outbox_projection_route;
             DROP TRIGGER projection_deliveries_after_outbox_insert;
             ALTER TABLE index_outbox DROP COLUMN projection_store;",
        )?;
        conn.execute_batch(LEGACY_PROJECTION_FANOUT_TRIGGER)?;
    }
    conn.execute("DELETE FROM schema_migrations WHERE version=29", [])?;
    conn.pragma_update(None, "user_version", 28)?;
    Ok(())
}

fn board_id(conn: &Connection, slug: &str) -> anyhow::Result<String> {
    conn.query_row("SELECT id FROM boards WHERE slug=?1", [slug], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn insert_board_entity(
    conn: &Connection,
    board_id: &str,
    slug: &str,
    updated_at: i64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO boards(id,slug,name,created_at,updated_at)
         VALUES (?1,?2,?2,?3,?3)",
        params![board_id, slug, updated_at],
    )?;
    conn.execute(
        "INSERT INTO entities(
           uri,kind,source_table,source_id,board_id,task_id,title,summary,
           content_hash,created_at,updated_at,archived_at
         ) VALUES (
           'kb://board/' || ?1,'board','boards',?1,?1,NULL,?2,NULL,
           NULL,?3,?3,NULL
         )",
        params![board_id, slug, updated_at],
    )?;
    Ok(())
}

fn insert_outbox(
    conn: &Connection,
    target: &str,
    projection_store: Option<&str>,
    entity_uri: &str,
    payload_json: &str,
    now: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,projection_store,entity_uri,action,payload_json,
           status,attempts,last_error,created_at,updated_at
         ) VALUES (
           NULL,?1,?2,?3,'rebuild',?4,'pending',0,NULL,?5,?5
         )",
        params![target, projection_store, entity_uri, payload_json, now],
    )?;
    Ok(conn.last_insert_rowid())
}

fn insert_label_outbox_shape(
    conn: &Connection,
    source_event_id: Option<i64>,
    entity_uri: &str,
    action: &str,
    payload_json: &str,
    now: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,projection_store,entity_uri,action,payload_json,
           status,attempts,last_error,created_at,updated_at
         ) VALUES (
           ?1,'lancedb','lancedb_label_atoms',?2,?3,?4,
           'pending',0,NULL,?5,?5
         )",
        params![source_event_id, entity_uri, action, payload_json, now],
    )?;
    Ok(conn.last_insert_rowid())
}

fn delivery_stores(conn: &Connection, outbox_id: i64) -> anyhow::Result<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT store_name
         FROM projection_deliveries
         WHERE outbox_id=?1
         ORDER BY store_name",
    )?;
    let rows = statement.query_map([outbox_id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn foreign_key_violation_count(conn: &Connection) -> anyhow::Result<i64> {
    let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    let mut count = 0_i64;
    while rows.next()?.is_some() {
        count += 1;
    }
    Ok(count)
}

#[test]
fn projection_label_deliveries_upgrade_is_additive_and_preserves_ids_and_sequences()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_label_deliveries_additive_upgrade")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    downgrade_projection_label_delivery_to_v28(&conn)?;
    let default_board_id = board_id(&conn, "default")?;
    let entity_uri = format!("kb://board/{default_board_id}");

    conn.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,entity_uri,action,payload_json,status,attempts,
           last_error,created_at,updated_at
         ) VALUES (NULL,'lancedb',?1,'rebuild','{}','pending',0,NULL,10,10)",
        [&entity_uri],
    )?;
    let preserved_outbox_id = conn.last_insert_rowid();
    let preserved_delivery_id: i64 = conn.query_row(
        "SELECT id FROM projection_deliveries
         WHERE outbox_id=?1 AND store_name='lancedb_chunks'",
        [preserved_outbox_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,entity_uri,action,payload_json,status,attempts,
           last_error,created_at,updated_at
         ) VALUES (NULL,'tantivy',?1,'rebuild','{}','pending',0,NULL,11,11)",
        [&entity_uri],
    )?;
    let outbox_sequence_before: i64 = conn.query_row(
        "SELECT seq FROM sqlite_sequence WHERE name='index_outbox'",
        [],
        |row| row.get(0),
    )?;
    let delivery_sequence_before: i64 = conn.query_row(
        "SELECT seq FROM sqlite_sequence WHERE name='projection_deliveries'",
        [],
        |row| row.get(0),
    )?;
    drop(conn);

    init_database(&temp.path, "upgrade")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let migration_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations
         WHERE version=29 AND name='029_projection_label_atom_deliveries'
           AND checksum GLOB 'fnv64:*'",
        [],
        |row| row.get(0),
    )?;
    let preserved: (i64, i64, Option<String>) = conn.query_row(
        "SELECT o.id,d.id,o.projection_store
         FROM index_outbox o
         JOIN projection_deliveries d ON d.outbox_id=o.id
         WHERE o.id=?1 AND d.store_name='lancedb_chunks'",
        [preserved_outbox_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let outbox_sequence_after: i64 = conn.query_row(
        "SELECT seq FROM sqlite_sequence WHERE name='index_outbox'",
        [],
        |row| row.get(0),
    )?;
    let delivery_sequence_after: i64 = conn.query_row(
        "SELECT seq FROM sqlite_sequence WHERE name='projection_deliveries'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(user_version, 29);
    assert_eq!(migration_count, 1);
    assert_eq!(
        preserved,
        (preserved_outbox_id, preserved_delivery_id, None)
    );
    assert_eq!(outbox_sequence_after, outbox_sequence_before);
    assert_eq!(delivery_sequence_after, delivery_sequence_before);
    assert_eq!(foreign_key_violation_count(&conn)?, 0);
    Ok(())
}

#[test]
fn projection_label_deliveries_fanout_keeps_legacy_routes_and_isolates_selector()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_label_deliveries_fanout")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let default_board_id = board_id(&conn, "default")?;
    let entity_uri = format!("kb://board/{default_board_id}");

    let all_id = insert_outbox(&conn, "all", None, &entity_uri, "{}", 20)?;
    let chunks_id = insert_outbox(&conn, "lancedb", None, &entity_uri, "{}", 21)?;
    let labels_id = insert_outbox(
        &conn,
        "lancedb",
        Some(LABEL_ATOMS_STORE),
        &entity_uri,
        LABEL_ATOMS_PAYLOAD,
        22,
    )?;

    assert_eq!(
        delivery_stores(&conn, all_id)?,
        vec![
            "lancedb_chunks".to_owned(),
            "oxigraph_relations".to_owned(),
            "tantivy_tasks".to_owned(),
        ]
    );
    assert_eq!(
        delivery_stores(&conn, chunks_id)?,
        vec!["lancedb_chunks".to_owned()]
    );
    assert_eq!(
        delivery_stores(&conn, labels_id)?,
        vec![LABEL_ATOMS_STORE.to_owned()]
    );
    let label_delivery: (String, String, String, Option<i64>) = conn.query_row(
        "SELECT board_id,entity_uri,payload_json,source_event_id
         FROM projection_deliveries
         WHERE outbox_id=?1 AND store_name=?2",
        params![labels_id, LABEL_ATOMS_STORE],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    assert_eq!(
        label_delivery,
        (
            default_board_id,
            entity_uri.clone(),
            LABEL_ATOMS_PAYLOAD.to_owned(),
            None,
        )
    );

    let invalid_target = insert_outbox(
        &conn,
        "all",
        Some(LABEL_ATOMS_STORE),
        &entity_uri,
        LABEL_ATOMS_PAYLOAD,
        23,
    )
    .expect_err("selector with target=all must fail");
    assert!(
        invalid_target
            .to_string()
            .contains("CHECK constraint failed")
    );
    let invalid_selector = insert_outbox(
        &conn,
        "lancedb",
        Some("lancedb_chunks"),
        &entity_uri,
        LABEL_ATOMS_PAYLOAD,
        24,
    )
    .expect_err("unknown exact selector must fail");
    assert!(
        invalid_selector
            .to_string()
            .contains("CHECK constraint failed")
    );

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("selector shape guard"),
    )?;
    let source_event_id: i64 = conn.query_row(
        "SELECT MAX(id) FROM task_events WHERE task_id=?1",
        [&task.id],
        |row| row.get(0),
    )?;
    let task_uri = format!("kb://task/{}", task.id);
    for (source_event_id, invalid_uri, action, payload, reason) in [
        (
            Some(source_event_id),
            entity_uri.as_str(),
            "rebuild",
            LABEL_ATOMS_PAYLOAD,
            "source_event_id must be NULL",
        ),
        (
            None,
            task_uri.as_str(),
            "rebuild",
            LABEL_ATOMS_PAYLOAD,
            "entity_uri must be board-scoped",
        ),
        (
            None,
            entity_uri.as_str(),
            "upsert",
            LABEL_ATOMS_PAYLOAD,
            "action must be rebuild",
        ),
        (
            None,
            entity_uri.as_str(),
            "rebuild",
            "{}",
            "payload must match the exact board rebuild contract",
        ),
    ] {
        let error =
            insert_label_outbox_shape(&conn, source_event_id, invalid_uri, action, payload, 25)
                .expect_err(reason);
        assert!(error.to_string().contains("CHECK constraint failed"));
    }
    Ok(())
}

#[test]
fn projection_label_deliveries_route_is_immutable_after_insert() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_label_deliveries_immutable_route")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let default_board_id = board_id(&conn, "default")?;
    let entity_uri = format!("kb://board/{default_board_id}");
    let labels_id = insert_outbox(
        &conn,
        "lancedb",
        Some(LABEL_ATOMS_STORE),
        &entity_uri,
        LABEL_ATOMS_PAYLOAD,
        30,
    )?;
    let chunks_id = insert_outbox(&conn, "lancedb", None, &entity_uri, "{}", 31)?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("selector update guard"),
    )?;
    let source_event_id: i64 = conn.query_row(
        "SELECT MAX(id) FROM task_events WHERE task_id=?1",
        [&task.id],
        |row| row.get(0),
    )?;
    insert_board_entity(&conn, "b_selector_other", "selector-other", 32)?;
    let other_board_uri = "kb://board/b_selector_other";
    let delivery_route_before: (String, String) = conn.query_row(
        "SELECT board_id,entity_uri FROM projection_deliveries
         WHERE outbox_id=?1 AND store_name=?2",
        params![labels_id, LABEL_ATOMS_STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let selector_error = conn
        .execute(
            "UPDATE index_outbox SET projection_store=NULL WHERE id=?1",
            [labels_id],
        )
        .expect_err("selector route mutation must fail");
    assert!(
        selector_error
            .to_string()
            .contains("index_outbox projection route is immutable")
    );
    let target_error = conn
        .execute(
            "UPDATE index_outbox SET target='oxigraph' WHERE id=?1",
            [chunks_id],
        )
        .expect_err("legacy target mutation must fail");
    assert!(
        target_error
            .to_string()
            .contains("index_outbox projection route is immutable")
    );
    for (sql, reason) in [
        (
            "UPDATE index_outbox SET action='upsert' WHERE id=?1",
            "exact selector action mutation must fail",
        ),
        (
            "UPDATE index_outbox SET payload_json='{}' WHERE id=?1",
            "exact selector payload mutation must fail",
        ),
        (
            "UPDATE index_outbox SET entity_uri='kb://task/not-a-board' WHERE id=?1",
            "exact selector entity mutation must fail",
        ),
    ] {
        let error = conn.execute(sql, [labels_id]).expect_err(reason);
        assert!(
            error
                .to_string()
                .contains("index_outbox projection route is immutable")
        );
    }
    let source_event_error = conn
        .execute(
            "UPDATE index_outbox SET source_event_id=?1 WHERE id=?2",
            params![source_event_id, labels_id],
        )
        .expect_err("exact selector source event mutation must fail");
    assert!(
        source_event_error
            .to_string()
            .contains("index_outbox projection route is immutable")
    );
    let valid_shape_route_error = conn
        .execute(
            "UPDATE index_outbox SET entity_uri=?1 WHERE id=?2",
            params![other_board_uri, labels_id],
        )
        .expect_err("exact selector route must not move to another valid board");
    assert!(
        valid_shape_route_error
            .to_string()
            .contains("index_outbox projection route is immutable")
    );
    let delivery_route_after: (String, String) = conn.query_row(
        "SELECT board_id,entity_uri FROM projection_deliveries
         WHERE outbox_id=?1 AND store_name=?2",
        params![labels_id, LABEL_ATOMS_STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(delivery_route_after, delivery_route_before);
    assert_eq!(
        delivery_stores(&conn, labels_id)?,
        vec![LABEL_ATOMS_STORE.to_owned()]
    );
    assert_eq!(
        delivery_stores(&conn, chunks_id)?,
        vec!["lancedb_chunks".to_owned()]
    );
    Ok(())
}

#[test]
fn projection_label_deliveries_dirty_triggers_enqueue_and_coalesce_without_lost_running_work()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_label_deliveries_dirty_triggers")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let default_board_id = board_id(&conn, "default")?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES (?1,?2,0,NULL,NULL,40)",
        params![LABEL_ATOMS_STORE, default_board_id],
    )?;

    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error=NULL,updated_at=41
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
    )?;
    let first_delivery_id: i64 = conn.query_row(
        "SELECT id FROM projection_deliveries
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
        |row| row.get(0),
    )?;
    let first_route: (String, Option<String>, String, String, String) = conn.query_row(
        "SELECT o.target,o.projection_store,o.entity_uri,o.action,o.payload_json
         FROM index_outbox o
         JOIN projection_deliveries d ON d.outbox_id=o.id
         WHERE d.id=?1",
        [first_delivery_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;
    assert_eq!(
        first_route,
        (
            "lancedb".to_owned(),
            Some(LABEL_ATOMS_STORE.to_owned()),
            format!("kb://board/{default_board_id}"),
            "rebuild".to_owned(),
            LABEL_ATOMS_PAYLOAD.to_owned(),
        )
    );

    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error=NULL,updated_at=42
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
    )?;
    let pending_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
        |row| row.get(0),
    )?;
    assert_eq!(pending_count, 1, "pending board rebuild must coalesce");

    conn.execute(
        "UPDATE projection_deliveries
         SET status='running',
             claim_owner='owner',
             claim_token='claim',
             claim_lease_token='lease',
             claim_fence_epoch=1,
             claim_generation='gen_1',
             claim_expires_at=4102444800000
         WHERE id=?1",
        [first_delivery_id],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error=NULL,updated_at=43
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
    )?;
    let delivery_statuses: Vec<String> = {
        let mut statement = conn.prepare(
            "SELECT status FROM projection_deliveries
             WHERE store_name=?1 AND board_id=?2
             ORDER BY id",
        )?;
        let rows = statement.query_map(params![LABEL_ATOMS_STORE, default_board_id], |row| {
            row.get(0)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(
        delivery_statuses,
        vec!["running".to_owned(), "pending".to_owned()]
    );

    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error='provider failed',updated_at=44
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
    )?;
    let after_failure_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS_STORE, default_board_id],
        |row| row.get(0),
    )?;
    assert_eq!(after_failure_count, 2);

    insert_board_entity(&conn, "b_label_insert", "label-insert", 45)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES (?1,'b_label_insert',1,NULL,NULL,45)",
        [LABEL_ATOMS_STORE],
    )?;
    let insert_delivery_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND board_id='b_label_insert'",
        [LABEL_ATOMS_STORE],
        |row| row.get(0),
    )?;
    assert_eq!(insert_delivery_count, 1);

    insert_board_entity(&conn, "b_label_failure", "label-failure", 46)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES (?1,'b_label_failure',1,NULL,'provider failed',46)",
        [LABEL_ATOMS_STORE],
    )?;
    let failure_delivery_id: i64 = conn.query_row(
        "SELECT id FROM projection_deliveries
         WHERE store_name=?1 AND board_id='b_label_failure'",
        [LABEL_ATOMS_STORE],
        |row| row.get(0),
    )?;
    conn.execute(
        "UPDATE projection_deliveries
         SET status='running',
             claim_owner='owner',
             claim_token='failure-claim',
             claim_lease_token='lease',
             claim_fence_epoch=1,
             claim_generation='gen_failure',
             claim_expires_at=4102444800000
         WHERE id=?1",
        [failure_delivery_id],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error='provider failed again',updated_at=47
         WHERE store_name=?1 AND board_id='b_label_failure'",
        [LABEL_ATOMS_STORE],
    )?;
    let failure_statuses: Vec<String> = {
        let mut statement = conn.prepare(
            "SELECT status FROM projection_deliveries
             WHERE store_name=?1 AND board_id='b_label_failure'
             ORDER BY id",
        )?;
        statement
            .query_map([LABEL_ATOMS_STORE], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(
        failure_statuses,
        vec!["running".to_owned(), "pending".to_owned()],
        "provider failure must remain recoverable without losing running work"
    );
    Ok(())
}

#[test]
fn projection_label_deliveries_canonical_rollback_leaves_no_outbox_or_delivery()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_label_deliveries_canonical_rollback")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    insert_board_entity(&conn, "b_label_rollback", "label-rollback", 50)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES (?1,'b_label_rollback',0,NULL,NULL,50)",
        [LABEL_ATOMS_STORE],
    )?;
    let outbox_before: i64 =
        conn.query_row("SELECT COUNT(*) FROM index_outbox", [], |row| row.get(0))?;
    let deliveries_before: i64 =
        conn.query_row("SELECT COUNT(*) FROM projection_deliveries", [], |row| {
            row.get(0)
        })?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error=NULL,updated_at=51
         WHERE store_name=?1 AND board_id='b_label_rollback'",
        [LABEL_ATOMS_STORE],
    )?;
    let outbox_inside: i64 =
        conn.query_row("SELECT COUNT(*) FROM index_outbox", [], |row| row.get(0))?;
    let deliveries_inside: i64 =
        conn.query_row("SELECT COUNT(*) FROM projection_deliveries", [], |row| {
            row.get(0)
        })?;
    assert_eq!(outbox_inside, outbox_before + 1);
    assert_eq!(deliveries_inside, deliveries_before + 1);
    conn.execute_batch("ROLLBACK")?;

    let dirty: i64 = conn.query_row(
        "SELECT dirty FROM label_atom_index_boards
         WHERE store_name=?1 AND board_id='b_label_rollback'",
        [LABEL_ATOMS_STORE],
        |row| row.get(0),
    )?;
    let outbox_after: i64 =
        conn.query_row("SELECT COUNT(*) FROM index_outbox", [], |row| row.get(0))?;
    let deliveries_after: i64 =
        conn.query_row("SELECT COUNT(*) FROM projection_deliveries", [], |row| {
            row.get(0)
        })?;
    assert_eq!(dirty, 0);
    assert_eq!(outbox_after, outbox_before);
    assert_eq!(deliveries_after, deliveries_before);
    Ok(())
}

#[test]
fn projection_label_deliveries_upgrade_backfills_each_dirty_board_once() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_label_deliveries_dirty_backfill")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    downgrade_projection_label_delivery_to_v28(&conn)?;
    insert_board_entity(&conn, "b_label_backfill_a", "label-backfill-a", 60)?;
    insert_board_entity(&conn, "b_label_backfill_b", "label-backfill-b", 61)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES (?1,'b_label_backfill_a',1,NULL,NULL,60)",
        [LABEL_ATOMS_STORE],
    )?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES (?1,'b_label_backfill_b',1,NULL,'legacy provider failure',61)",
        [LABEL_ATOMS_STORE],
    )?;
    drop(conn);

    init_database(&temp.path, "upgrade")?;
    init_database(&temp.path, "idempotent")?;

    let conn = connect_file(&temp.path)?;
    let mut statement = conn.prepare(
        "SELECT d.board_id,d.entity_uri,d.action,d.payload_json,o.target,o.projection_store
         FROM projection_deliveries d
         JOIN index_outbox o ON o.id=d.outbox_id
         WHERE d.store_name=?1
           AND d.board_id IN ('b_label_backfill_a','b_label_backfill_b')
         ORDER BY d.board_id",
    )?;
    let rows = statement
        .query_map([LABEL_ATOMS_STORE], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(
        rows,
        vec![
            (
                "b_label_backfill_a".to_owned(),
                "kb://board/b_label_backfill_a".to_owned(),
                "rebuild".to_owned(),
                LABEL_ATOMS_PAYLOAD.to_owned(),
                "lancedb".to_owned(),
                Some(LABEL_ATOMS_STORE.to_owned()),
            ),
            (
                "b_label_backfill_b".to_owned(),
                "kb://board/b_label_backfill_b".to_owned(),
                "rebuild".to_owned(),
                LABEL_ATOMS_PAYLOAD.to_owned(),
                "lancedb".to_owned(),
                Some(LABEL_ATOMS_STORE.to_owned()),
            ),
        ]
    );
    let chunk_leak_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries chunks
         WHERE chunks.store_name='lancedb_chunks'
           AND chunks.outbox_id IN (
             SELECT labels.outbox_id
             FROM projection_deliveries labels
             WHERE labels.store_name=?1
               AND labels.board_id IN ('b_label_backfill_a','b_label_backfill_b')
           )",
        [LABEL_ATOMS_STORE],
        |row| row.get(0),
    )?;
    assert_eq!(chunk_leak_count, 0);
    assert_eq!(foreign_key_violation_count(&conn)?, 0);
    Ok(())
}
