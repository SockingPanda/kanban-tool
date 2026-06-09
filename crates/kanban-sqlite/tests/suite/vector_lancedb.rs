use crate::common::*;

#[test]
fn vector_sync_marks_lancedb_outbox_done_without_touching_other_boards() {
    let temp = TempDb::new("vector_sync_marks_lancedb_outbox_done_without_touching_other_boards");
    init_database(&temp.path, "tester").unwrap();
    insert_board(&temp.path, "second", "b_second");
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default board vector task"),
    )
    .unwrap();
    create_task(
        &temp.path,
        "second",
        "tester",
        CreateTask::ready("second board vector task"),
    )
    .unwrap();

    let store = RecordingVectorStore::default();
    let status = sync_vector_store_with(&temp.path, "default", &store).unwrap();
    assert_eq!(status.backend, "test-vector");
    assert!(status.message.contains("synced 1 chunk(s)"));
    assert_eq!(
        store.upserted_texts(),
        vec!["default board vector task\n\nready spec"]
    );
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done"]
    );
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "second"),
        vec!["pending"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let vector = derived
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .unwrap();
    assert!(
        vector.dirty,
        "second board still has pending LanceDB outbox"
    );

    sync_vector_store_with(&temp.path, "second", &store).unwrap();
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "second"),
        vec!["done"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let vector = derived
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .unwrap();
    assert!(!vector.dirty);
    assert!(vector.last_error.is_none());
}

#[test]
fn vector_sync_and_rebuild_use_store_embedding_model() {
    let temp = TempDb::new("vector_sync_and_rebuild_use_store_embedding_model");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("non default model vector task"),
    )
    .unwrap();
    let store = RecordingVectorStore::with_embedding_model("static-test");

    sync_vector_store_with(&temp.path, "default", &store).unwrap();
    assert_eq!(store.upserted_models(), vec!["static-test"]);

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
    )
    .unwrap();
    rebuild_vector_store_with(&temp.path, "default", &store).unwrap();

    assert_eq!(store.upserted_models(), vec!["static-test", "static-test"]);
}

#[test]
fn vector_sync_deletes_archived_task_chunks_and_converges_outbox() {
    let temp = TempDb::new("vector_sync_deletes_archived_task_chunks_and_converges_outbox");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived vector task"),
    )
    .unwrap();
    let store = RecordingVectorStore::default();

    sync_vector_store_with(&temp.path, "default", &store).unwrap();
    archive_task(&temp.path, "default", "tester", &task.id, false).unwrap();
    let status = sync_vector_store_with(&temp.path, "default", &store).unwrap();

    assert!(status.message.contains("synced 0 chunk(s) from 1 job(s)"));
    assert_eq!(
        store.deleted_entity_uris(),
        vec![format!("kb://task/{}", task.id)]
    );
    assert_eq!(store.deleted_board_ids(), vec![task.board_id.as_str()]);
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done", "done"]
    );
    let vector = derived_store_statuses(&temp.path)
        .unwrap()
        .into_iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .unwrap();
    assert!(!vector.dirty);
    assert!(vector.last_error.is_none());
}

#[test]
fn vector_rebuild_deletes_board_before_reindexing_current_tasks() {
    let temp = TempDb::new("vector_rebuild_deletes_board_before_reindexing_current_tasks");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("hard deleted vector task"),
    )
    .unwrap();
    let store = RecordingVectorStore::default();

    sync_vector_store_with(&temp.path, "default", &store).unwrap();
    assert_eq!(
        store.live_texts(),
        vec!["hard deleted vector task\n\nready spec"]
    );

    connect_file(&temp.path)
        .unwrap()
        .execute("DELETE FROM tasks WHERE id=?1", params![task.id])
        .unwrap();
    let status = rebuild_vector_store_with(&temp.path, "default", &store).unwrap();

    assert!(status.message.contains("rebuilt 0 chunk(s)"));
    assert_eq!(
        store.deleted_board_ids(),
        vec![task.board_id.as_str(), task.board_id.as_str()]
    );
    assert!(store.live_texts().is_empty());
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done"]
    );
    let vector = derived_store_statuses(&temp.path)
        .unwrap()
        .into_iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .unwrap();
    assert!(!vector.dirty);
    assert!(vector.last_error.is_none());
}

#[test]
fn vector_adapter_failure_keeps_lancedb_chunks_dirty_and_records_error() {
    let temp = TempDb::new("vector_adapter_failure_keeps_lancedb_chunks_dirty_and_records_error");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("failing vector task"),
    )
    .unwrap();
    let store = FailingVectorStore;

    let error = rebuild_vector_store_with(&temp.path, "default", &store).unwrap_err();
    assert!(error.to_string().contains("dimension mismatch"));

    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.title, task.title, "SQLite task truth is unchanged");
    assert_eq!(
        lancedb_outbox_statuses_for_board(&temp.path, "default"),
        vec!["failed"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let vector = derived
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .unwrap();
    assert!(vector.dirty);
    assert!(
        vector
            .last_error
            .as_deref()
            .unwrap()
            .contains("dimension mismatch")
    );
    let report = doctor_database(&temp.path).unwrap();
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
                .unwrap()
                .contains("dimension mismatch")
    }));
}
