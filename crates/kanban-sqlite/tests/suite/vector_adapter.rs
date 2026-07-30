use crate::common::*;

#[test]
fn vector_sync_marks_lancedb_outbox_done_without_touching_other_boards() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_sync_marks_lancedb_outbox_done_without_touching_other_boards")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "second", "b_second")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default board vector task"),
    )?;
    create_task(
        &temp.path,
        "second",
        "tester",
        CreateTask::ready("second board vector task"),
    )?;

    let store = RecordingVectorStore::default();
    let status = sync_vector_store_with(&temp.path, "default", &store)?;
    assert_eq!(status.backend, "test-vector");
    assert!(status.message.contains("synced 2 chunk(s)"));
    assert_eq!(status.dirty, Some(true));
    assert_eq!(status.board_dirty, Some(false));
    assert!(status.diagnostics.iter().any(|code| code == "vector_dirty"));
    let upserted = store.upserted_texts()?;
    assert_eq!(upserted.len(), 2);
    assert!(
        upserted
            .iter()
            .any(|text| text == "default board vector task\n\nready spec")
    );
    assert!(upserted.iter().any(|text| text.contains("task.created")));
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done"]
    );
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "second")?,
        vec!["pending"]
    );
    let derived = derived_store_statuses(&temp.path)?;
    let vector = derived
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks derived store status"))?;
    assert!(
        vector.dirty,
        "second board still has pending LanceDB outbox"
    );

    sync_vector_store_with(&temp.path, "second", &store)?;
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "second")?,
        vec!["done"]
    );
    let derived = derived_store_statuses(&temp.path)?;
    let vector = derived
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks derived store status"))?;
    assert!(!vector.dirty);
    assert!(vector.last_error.is_none());
    Ok(())
}

#[test]
fn vector_sync_and_rebuild_use_store_embedding_model() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_sync_and_rebuild_use_store_embedding_model")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("non default model vector task"),
    )?;
    let store = RecordingVectorStore::with_embedding_model("static-test");

    sync_vector_store_with(&temp.path, "default", &store)?;
    assert_eq!(store.upserted_models()?, vec!["static-test", "static-test"]);

    update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("non default model vector task updated".into()),
            expected_lock_version: Some(task.lock_version),
            ..TaskPatch::default()
        },
    )?;
    rebuild_vector_store_with(&temp.path, "default", &store)?;

    assert_eq!(
        store.upserted_models()?,
        vec!["static-test", "static-test", "static-test", "static-test"]
    );
    Ok(())
}

#[test]
fn vector_sync_deletes_archived_task_chunks_and_converges_outbox() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_sync_deletes_archived_task_chunks_and_converges_outbox")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived vector task"),
    )?;
    let store = RecordingVectorStore::default();

    sync_vector_store_with(&temp.path, "default", &store)?;
    archive_task(&temp.path, "default", "tester", &task.id, false)?;
    let status = sync_vector_store_with(&temp.path, "default", &store)?;

    assert!(status.message.contains("synced 0 chunk(s) from 1 job(s)"));
    assert_eq!(
        store.deleted_entity_uris()?,
        vec![format!("kb://task/{}", task.id)]
    );
    assert_eq!(store.deleted_board_ids()?, vec![task.board_id.as_str()]);
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done", "done"]
    );
    let vector = derived_store_statuses(&temp.path)?
        .into_iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks derived store status"))?;
    assert!(!vector.dirty);
    assert!(vector.last_error.is_none());
    Ok(())
}

#[test]
fn vector_rebuild_deletes_board_before_reindexing_current_tasks() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_rebuild_deletes_board_before_reindexing_current_tasks")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("hard deleted vector task"),
    )?;
    let store = RecordingVectorStore::default();

    sync_vector_store_with(&temp.path, "default", &store)?;
    let live_texts = store.live_texts()?;
    assert_eq!(live_texts.len(), 2);
    assert!(
        live_texts
            .iter()
            .any(|text| text == "hard deleted vector task\n\nready spec")
    );
    assert!(live_texts.iter().any(|text| text.contains("task.created")));

    connect_file(&temp.path)?.execute("DELETE FROM tasks WHERE id=?1", params![task.id])?;
    let status = rebuild_vector_store_with(&temp.path, "default", &store)?;

    assert!(status.message.contains("rebuilt 0 chunk(s)"));
    assert_eq!(
        store.deleted_board_ids()?,
        vec![task.board_id.as_str(), task.board_id.as_str()]
    );
    assert!(store.live_texts()?.is_empty());
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done"]
    );
    let vector = derived_store_statuses(&temp.path)?
        .into_iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks derived store status"))?;
    assert!(!vector.dirty);
    assert!(vector.last_error.is_none());
    Ok(())
}

#[test]
fn vector_adapter_failure_keeps_lancedb_chunks_dirty_and_records_error() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_adapter_failure_keeps_lancedb_chunks_dirty_and_records_error")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("failing vector task"),
    )?;
    connect_file(&temp.path)?.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,projection_store,entity_uri,action,payload_json,
           status,attempts,last_error,created_at,updated_at
         ) VALUES (
           NULL,'lancedb','lancedb_label_atoms',?1,'rebuild',
           '{\"scope\":\"board\",\"version\":1}','pending',0,NULL,1,1
         )",
        [format!("kb://board/{}", task.board_id)],
    )?;
    let store = FailingVectorStore;

    let error = result_err(rebuild_vector_store_with(&temp.path, "default", &store))?;
    assert!(error.to_string().contains("dimension mismatch"));

    let fresh = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh.title, task.title, "SQLite task truth is unchanged");
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["failed"]
    );
    let exact_route: (String, i64, Option<String>) = connect_file(&temp.path)?.query_row(
        "SELECT status,attempts,last_error
         FROM index_outbox
         WHERE projection_store='lancedb_label_atoms'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(
        exact_route,
        ("pending".to_owned(), 0, None),
        "legacy chunk provider failure must not fail exact label work"
    );
    let derived = derived_store_statuses(&temp.path)?;
    let vector = derived
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks derived store status"))?;
    assert!(vector.dirty);
    assert!(
        vector
            .last_error
            .as_deref()
            .ok_or_else(|| test_error("expected lancedb_chunks error"))?
            .contains("dimension mismatch")
    );
    let report = doctor_database(&temp.path)?;
    assert!(!report.ok);
    assert_eq!(report.outbox_failed, 1);
    assert_eq!(report.derived_error_stores, 1);
    assert!(report.derived_stores.iter().any(|store| {
        store.store_name == "lancedb_chunks"
            && store.dirty
            && store.failed_outbox == 1
            && store
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("dimension mismatch"))
    }));
    Ok(())
}

#[test]
fn vector_legacy_route_does_not_complete_dirty_or_count_label_selector() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_legacy_route_does_not_complete_dirty_or_count_label_selector")?;
    let init = init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("legacy chunks route"),
    )?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,projection_store,entity_uri,action,payload_json,
           status,attempts,last_error,created_at,updated_at
         ) VALUES (
           NULL,'lancedb','lancedb_label_atoms',?1,'rebuild',
           '{\"scope\":\"board\",\"version\":1}','pending',0,NULL,1,1
         )",
        [format!("kb://board/{}", init.board_id)],
    )?;
    drop(conn);

    let before = doctor_database(&temp.path)?;
    let chunks_before = before
        .derived_stores
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks doctor status"))?;
    assert_eq!(
        chunks_before.pending_outbox, 1,
        "exact label selector must not count as legacy chunk work"
    );

    sync_vector_store_with(&temp.path, "default", &RecordingVectorStore::default())?;

    let conn = connect_file(&temp.path)?;
    let exact_route: (String, i64, Option<String>) = conn.query_row(
        "SELECT status,attempts,last_error
         FROM index_outbox
         WHERE projection_store='lancedb_label_atoms'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(exact_route, ("pending".to_owned(), 0, None));
    let chunks = derived_store_statuses(&temp.path)?
        .into_iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks"))?;
    assert!(
        !chunks.dirty,
        "exact label selector must not keep the legacy chunks store dirty"
    );
    let after = doctor_database(&temp.path)?;
    let chunks_after = after
        .derived_stores
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks doctor status"))?;
    assert_eq!(chunks_after.pending_outbox, 0);
    Ok(())
}
