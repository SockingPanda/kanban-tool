use std::{
    path::Path,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use kanban_core::{TaskStatus, new_run_id};
use kanban_sqlite::{
    CreateTask, DispatchOptions, FinishPolicy, TaskPatch, add_dependency, archive_task,
    begin_database_replace, begin_database_runtime, block_task, build_context_pack, claim_task,
    complete_task, connect_file, create_comment, create_task, derived_store_statuses,
    dispatch_once, doctor_database, get_task, init_database, list_dependencies, list_events,
    list_outbox, list_runs, list_tasks, promote_task, rebuild_vector_store_with, search_tasks,
    sync_vector_store_with, unblock_task, update_task,
};
use kanban_vector::{
    EmbeddingChunk, VectorError, VectorHit, VectorQuery, VectorStore, VectorStoreStatus,
};
use rusqlite::{Connection, params};

#[test]
fn task_crud_writes_events_and_hides_archived_by_default() {
    let temp = TempDb::new("task_crud_writes_events_and_hides_archived_by_default");
    init_database(&temp.path, "tester").unwrap();

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "实现 Task CRUD".into(),
            description: Some("规格".into()),
            status: None,
            assignee: None,
            priority: 10,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    assert_eq!(task.seq, 1);
    assert_eq!(task.status, TaskStatus::Ready);
    assert_eq!(
        list_events(&temp.path, "default", Some(&task.id)).unwrap()[0].kind,
        "task.created"
    );

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("实现 Task CRUD v0.5".into()),
            description: None,
            assignee: Some(Some("worker-a".into())),
            priority: Some(20),
            scheduled_at: None,
            due_at: None,
            metadata_json: None,
            expected_lock_version: Some(task.lock_version),
        },
    )
    .unwrap();
    assert_eq!(updated.title, "实现 Task CRUD v0.5");
    assert_eq!(updated.lock_version, task.lock_version + 1);

    archive_task(&temp.path, "default", "tester", &task.id, false).unwrap();
    assert!(
        list_tasks(&temp.path, "default", &[], false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        list_tasks(&temp.path, "default", &[], true).unwrap().len(),
        1
    );
}

#[test]
fn sqlite_search_fallback_matches_task_related_text_with_filters_and_paging() {
    let temp =
        TempDb::new("sqlite_search_fallback_matches_task_related_text_with_filters_and_paging");
    init_database(&temp.path, "tester").unwrap();

    let alpha = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Alpha primary".into(),
            description: Some("plain spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 10,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let beta = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Beta secondary".into(),
            description: Some("mentions fallback needle in the spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let gamma = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Gamma unrelated".into(),
            description: Some("plain spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-b".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Archived fallback needle".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    create_comment(
        &temp.path,
        &alpha.id,
        "tester",
        "comment carries fallback needle",
        None,
    )
    .unwrap();
    archive_task(&temp.path, "default", "tester", &archived.id, false).unwrap();

    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, summary, error, metadata_json) VALUES (?1, ?2, ?3, 'failed', 'token', 'tester', 1, 1, ?4, ?5, '{}')",
        params![new_run_id(), board_id, gamma.id, "run fallback needle summary", "run fallback needle error"],
    )
    .unwrap();

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("fallback needle".into()),
            statuses: vec![TaskStatus::Ready],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();

    assert_eq!(results.meta.backend, "sqlite");
    assert!(!results.meta.stale);
    assert_eq!(
        results
            .hits
            .iter()
            .map(|hit| hit.task_id.as_str())
            .collect::<Vec<_>>(),
        vec![beta.id.as_str(), alpha.id.as_str()]
    );
    assert!(results.hits.iter().all(|hit| hit.snippet.is_some()));
    assert!(results.hits[0].score >= results.hits[1].score);

    let second_page = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("fallback needle".into()),
            statuses: vec![],
            assignee: None,
            include_archived: true,
            limit: 2,
            offset: 2,
        },
    )
    .unwrap();
    assert_eq!(second_page.hits.len(), 2);
    assert!(
        second_page
            .hits
            .iter()
            .any(|hit| hit.task_id == gamma.id || hit.task_id == archived.id)
    );
}

#[test]
fn sqlite_search_rejects_limit_that_cannot_be_bounded_safely() {
    let temp = TempDb::new("sqlite_search_rejects_limit_that_cannot_be_bounded_safely");
    init_database(&temp.path, "tester").unwrap();

    let error = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("anything".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: usize::MAX,
            offset: 0,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("limit must be <= 1000"));
}

#[test]
fn context_broker_hydrates_subject_and_reports_disabled_derived_stores() {
    let temp = TempDb::new("context_broker_hydrates_subject_and_reports_disabled_derived_stores");
    init_database(&temp.path, "tester").unwrap();
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "context broker source".into(),
            description: Some("ready spec broker-needle".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let related = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "related context broker source".into(),
            description: Some("ready spec broker-needle".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let pack = build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 5,
            max_items: 10,
        },
    )
    .unwrap();

    assert_eq!(pack.subject, kanban_entity::EntityUri::task(&subject.id));
    assert_eq!(
        pack.items[0].entity_uri,
        kanban_entity::EntityUri::task(&subject.id)
    );
    assert_eq!(pack.items[0].source, "subject");
    assert!(
        pack.items
            .iter()
            .any(|item| item.entity_uri == kanban_entity::EntityUri::task(&related.id))
    );
    #[cfg(not(feature = "graph-oxigraph"))]
    assert!(
        pack.degraded
            .iter()
            .any(|marker| marker == "graph_disabled")
    );
    #[cfg(feature = "graph-oxigraph")]
    assert!(
        !pack
            .degraded
            .iter()
            .any(|marker| marker == "graph_disabled")
    );
    assert!(
        pack.degraded
            .iter()
            .any(|marker| marker == "vector_disabled")
    );
}

#[test]
fn task_events_fan_out_target_specific_outbox_and_mark_derived_stores_dirty() {
    let temp =
        TempDb::new("task_events_fan_out_target_specific_outbox_and_mark_derived_stores_dirty");
    init_database(&temp.path, "tester").unwrap();

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("outbox fanout"),
    )
    .unwrap();

    let jobs = list_outbox(
        &temp.path,
        kanban_sqlite::OutboxListOptions {
            status: Some("pending".to_owned()),
            limit: 10,
        },
    )
    .unwrap();
    assert_eq!(jobs.len(), 3);
    assert_eq!(
        jobs.iter()
            .map(|job| job.target.as_str())
            .collect::<Vec<_>>(),
        vec!["tantivy", "oxigraph", "lancedb"]
    );
    assert!(
        jobs.iter()
            .all(|job| job.entity_uri == format!("kb://task/{}", task.id))
    );

    let statuses = derived_store_statuses(&temp.path).unwrap();
    for store in ["tantivy_tasks", "oxigraph_relations", "lancedb_chunks"] {
        let status = statuses
            .iter()
            .find(|status| status.store_name == store)
            .unwrap();
        assert!(status.dirty, "{store} should be dirty");
        assert_eq!(status.last_event_id, 0);
    }
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_rebuild_marks_only_current_board_outbox_done() {
    let temp = TempDb::new("tantivy_rebuild_marks_only_current_board_outbox_done");
    init_database(&temp.path, "tester").unwrap();
    insert_board(&temp.path, "other", "b_other");

    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default first"),
    )
    .unwrap();
    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other between default events"),
    )
    .unwrap();
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default second"),
    )
    .unwrap();

    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done", "done"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "other"),
        vec!["pending"]
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_sync_marks_only_current_board_outbox_done() {
    let temp = TempDb::new("tantivy_sync_marks_only_current_board_outbox_done");
    init_database(&temp.path, "tester").unwrap();
    insert_board(&temp.path, "other", "b_other");
    let default = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default indexed"),
    )
    .unwrap();
    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();

    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other pending during default sync"),
    )
    .unwrap();
    update_task(
        &temp.path,
        "default",
        "tester",
        &default.id,
        TaskPatch {
            title: Some("default synced".into()),
            expected_lock_version: Some(default.lock_version),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    kanban_sqlite::sync_search_index(&temp.path, "default").unwrap();

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done", "done"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "other"),
        vec!["pending"]
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_sync_failure_marks_only_current_board_outbox_failed() {
    let temp = TempDb::new("tantivy_sync_failure_marks_only_current_board_outbox_failed");
    init_database(&temp.path, "tester").unwrap();
    insert_board(&temp.path, "other", "b_other");
    let default = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default indexed before failure"),
    )
    .unwrap();
    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();

    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other pending during default failure"),
    )
    .unwrap();
    update_task(
        &temp.path,
        "default",
        "tester",
        &default.id,
        TaskPatch {
            title: Some("default failure candidate".into()),
            expected_lock_version: Some(default.lock_version),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    std::fs::write(
        temp.dir.join("index/v1/tasks/kb-index-meta.json"),
        b"not json",
    )
    .unwrap();

    let err = kanban_sqlite::sync_search_index(&temp.path, "default").unwrap_err();
    assert!(err.to_string().contains("expected ident") || err.to_string().contains("JSON"));

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done", "failed"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "other"),
        vec!["pending"]
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_rebuild_searches_task_aggregate_and_keeps_sqlite_hydration_filters() {
    let temp =
        TempDb::new("tantivy_rebuild_searches_task_aggregate_and_keeps_sqlite_hydration_filters");
    init_database(&temp.path, "tester").unwrap();

    let title = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Tantivy title comet".into(),
            description: Some("plain ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 10,
            scheduled_at: None,
            due_at: Some(1_767_312_000_000),
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let description = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Description task".into(),
            description: Some("description comet payload".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let comment = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Comment task"),
    )
    .unwrap();
    create_comment(
        &temp.path,
        &comment.id,
        "tester",
        "comment comet payload",
        None,
    )
    .unwrap();
    let run = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Run task"),
    )
    .unwrap();
    let event = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Event task"),
    )
    .unwrap();
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Archived comet"),
    )
    .unwrap();

    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, summary, error, metadata_json) VALUES (?1, ?2, ?3, 'failed', 'token', 'tester', 1, 1, ?4, NULL, '{}')",
        params![new_run_id(), board_id, run.id, "run comet summary"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, 'task.comet.event', 'tester', ?4, 1)",
        params![kanban_core::new_event_id(), board_id, event.id, "{\"note\":\"event comet payload\"}"],
    )
    .unwrap();
    drop(conn);
    archive_task(&temp.path, "default", "tester", &archived.id, false).unwrap();

    let status = kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    assert_eq!(status.backend, "tantivy");
    assert!(status.derived_index);
    assert!(!status.stale);
    assert!(temp.dir.join("index/v1/tasks").exists());

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(results.meta.backend, "tantivy");
    assert!(!results.meta.stale);
    let ids = results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    for expected in [&title.id, &description.id, &comment.id, &run.id, &event.id] {
        assert!(
            ids.contains(&expected.as_str()),
            "missing {expected}: {ids:?}"
        );
    }
    assert!(!ids.contains(&archived.id.as_str()));
    assert!(results.hits.iter().all(|hit| hit.snippet.is_some()));

    let hydrated = get_task(&temp.path, "default", &results.hits[0].task_id).unwrap();
    assert_ne!(hydrated.status, TaskStatus::Archived);

    let filtered = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![TaskStatus::Ready],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )
    .unwrap();
    let mut filtered_ids = filtered
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    filtered_ids.sort_unstable();
    let mut expected_ids = vec![title.id.as_str(), description.id.as_str()];
    expected_ids.sort_unstable();
    assert_eq!(filtered_ids, expected_ids);
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn stale_tantivy_index_falls_back_to_sqlite_before_current_filters_are_applied() {
    let temp =
        TempDb::new("stale_tantivy_index_falls_back_to_sqlite_before_current_filters_are_applied");
    init_database(&temp.path, "tester").unwrap();

    let archive_candidate = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Stale comet archived later".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let status_candidate = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Stale comet running later".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let assignee_candidate = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Stale comet reassigned later".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    let indexed = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![TaskStatus::Ready],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(indexed.meta.backend, "tantivy");
    assert_eq!(indexed.hits.len(), 3);

    archive_task(
        &temp.path,
        "default",
        "tester",
        &archive_candidate.id,
        false,
    )
    .unwrap();
    claim_task(
        &temp.path,
        "default",
        "worker",
        &status_candidate.id,
        300_000,
    )
    .unwrap();
    update_task(
        &temp.path,
        "default",
        "tester",
        &assignee_candidate.id,
        TaskPatch {
            assignee: Some(Some("worker-b".into())),
            expected_lock_version: Some(assignee_candidate.lock_version),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    let filtered = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![TaskStatus::Ready],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();

    assert_eq!(filtered.meta.backend, "sqlite");
    assert!(filtered.meta.stale);
    assert!(filtered.meta.index_lag_events.unwrap() > 0);
    assert!(filtered.hits.is_empty(), "{:?}", filtered.hits);
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_rebuild_persists_search_state_in_app_settings() {
    let temp = TempDb::new("tantivy_rebuild_persists_search_state_in_app_settings");
    init_database(&temp.path, "tester").unwrap();
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("state comet"),
    )
    .unwrap();

    let status = kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    assert_eq!(status.backend, "tantivy");
    assert_eq!(status.index_lag_events, Some(0));

    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let state_json: String = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key=?1",
            [format!("search.tasks.state.{board_id}")],
            |row| row.get(0),
        )
        .unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_json).unwrap();
    assert_eq!(state["schema_version"], 1);
    assert_eq!(state["backend"], "tantivy");
    assert_eq!(state["index_name"], "tasks");
    assert_eq!(state["dirty"], false);
    assert_eq!(state["last_event_id"].as_i64(), status.last_event_id);

    let derived = derived_store_statuses(&temp.path).unwrap();
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .unwrap();
    assert_eq!(Some(tantivy.last_event_id), status.last_event_id);
    assert!(!tantivy.dirty);
    assert!(tantivy.last_rebuild_at.is_some());
    assert!(tantivy.last_error.is_none());
    let jobs = list_outbox(
        &temp.path,
        kanban_sqlite::OutboxListOptions {
            status: Some("done".to_owned()),
            limit: 10,
        },
    )
    .unwrap();
    assert!(jobs.iter().any(|job| job.target == "tantivy"));
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending() {
    let temp = TempDb::new("tantivy_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending");
    init_database(&temp.path, "tester").unwrap();
    insert_board(&temp.path, "second", "b_second");
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default board task"),
    )
    .unwrap();
    create_task(
        &temp.path,
        "second",
        "tester",
        CreateTask::ready("second board task"),
    )
    .unwrap();

    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "second"),
        vec!["pending"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .unwrap();
    assert!(
        tantivy.dirty,
        "second board still has pending Tantivy outbox"
    );

    kanban_sqlite::rebuild_search_index(&temp.path, "second").unwrap();

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "second"),
        vec!["done"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .unwrap();
    assert!(!tantivy.dirty);
}

#[cfg(feature = "graph-oxigraph")]
#[test]
fn graph_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending() {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::{OxigraphStore, RelationGraph};

    let temp = TempDb::new("graph_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending");
    init_database(&temp.path, "tester").unwrap();
    insert_board(&temp.path, "second", "b_second");
    let default_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default board graph task"),
    )
    .unwrap();
    let second_task = create_task(
        &temp.path,
        "second",
        "tester",
        CreateTask::ready("second board graph task"),
    )
    .unwrap();

    kanban_sqlite::rebuild_graph_store(&temp.path, "default").unwrap();
    let graph = OxigraphStore::open(kanban_local::graph_store_path(temp.path.clone())).unwrap();
    assert_eq!(
        graph
            .neighbors(
                &EntityUri::task(&default_task.id),
                Some(Predicate::BelongsToBoard),
                10,
            )
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        graph_outbox_statuses_for_board(&temp.path, "default"),
        vec!["done"]
    );
    assert_eq!(
        graph_outbox_statuses_for_board(&temp.path, "second"),
        vec!["pending"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let graph = derived
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .unwrap();
    assert!(graph.dirty, "second board still has pending graph outbox");

    kanban_sqlite::rebuild_graph_store(&temp.path, "second").unwrap();
    let graph = OxigraphStore::open(kanban_local::graph_store_path(temp.path.clone())).unwrap();
    assert_eq!(
        graph
            .neighbors(
                &EntityUri::task(&default_task.id),
                Some(Predicate::BelongsToBoard),
                10,
            )
            .unwrap()
            .len(),
        1,
        "rebuilding the second board must preserve the first board graph"
    );
    assert_eq!(
        graph
            .neighbors(
                &EntityUri::task(&second_task.id),
                Some(Predicate::BelongsToBoard),
                10,
            )
            .unwrap()
            .len(),
        1
    );

    assert_eq!(
        graph_outbox_statuses_for_board(&temp.path, "second"),
        vec!["done"]
    );
    let derived = derived_store_statuses(&temp.path).unwrap();
    let graph = derived
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .unwrap();
    assert!(!graph.dirty);
    let default_status = kanban_sqlite::graph_store_status(&temp.path, "default").unwrap();
    assert!(
        default_status.message.contains("lag=0"),
        "default board has no unfinished graph outbox even though the shared watermark advanced: {}",
        default_status.message
    );
}

#[cfg(feature = "graph-oxigraph")]
#[test]
fn graph_rebuild_persists_board_and_dependency_relations() {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::{OxigraphStore, RelationGraph};

    let temp = TempDb::new("graph_rebuild_persists_board_and_dependency_relations");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph parent"),
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph child"),
    )
    .unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();

    let snapshot = kanban_sqlite::graph_relation_snapshot(&temp.path, "default").unwrap();
    assert!(
        snapshot.iter().any(
            |relation| relation.subject_uri == EntityUri::task(&child.id)
                && relation.predicate == Predicate::DependsOn
                && relation.object_uri == EntityUri::task(&parent.id)
        ),
        "SQLite relation snapshot should mirror the authoritative dependency edge"
    );

    kanban_sqlite::rebuild_graph_store(&temp.path, "default").unwrap();

    let graph = OxigraphStore::open(kanban_local::graph_store_path(temp.path.clone())).unwrap();
    let child_uri = EntityUri::task(&child.id);
    let dependency_neighbors = graph
        .neighbors(&child_uri, Some(Predicate::DependsOn), 10)
        .unwrap();
    assert_eq!(dependency_neighbors.len(), 1);
    assert_eq!(
        dependency_neighbors[0].object_uri,
        EntityUri::task(&parent.id)
    );

    let board_neighbors = graph
        .neighbors(&child_uri, Some(Predicate::BelongsToBoard), 10)
        .unwrap();
    assert_eq!(board_neighbors.len(), 1);
    assert_eq!(
        board_neighbors[0].object_uri,
        EntityUri::board(&child.board_id)
    );
}

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
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_sync_reindexes_task_comment_run_event_and_archive_changes() {
    let temp = TempDb::new("tantivy_sync_reindexes_task_comment_run_event_and_archive_changes");
    init_database(&temp.path, "tester").unwrap();

    let updated = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("updated source"),
    )
    .unwrap();
    let commented = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("comment source"),
    )
    .unwrap();
    let run_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("run source"),
    )
    .unwrap();
    let event_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("event source"),
    )
    .unwrap();
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archive syncneedle"),
    )
    .unwrap();

    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    update_task(
        &temp.path,
        "default",
        "tester",
        &updated.id,
        TaskPatch {
            title: Some("updated syncneedle".into()),
            expected_lock_version: Some(updated.lock_version),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    create_comment(
        &temp.path,
        &commented.id,
        "tester",
        "comment syncneedle",
        None,
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &run_task.id, 300_000).unwrap();
    kanban_sqlite::complete_task_with_summary(
        &temp.path,
        "default",
        "worker",
        &run_task.id,
        Some(&claim.claim_token),
        false,
        Some("run syncneedle"),
    )
    .unwrap();
    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, 'task.sync.event', 'tester', ?4, 1)",
        params![
            kanban_core::new_event_id(),
            board_id,
            event_task.id,
            "{\"note\":\"event syncneedle\"}"
        ],
    )
    .unwrap();
    drop(conn);
    archive_task(&temp.path, "default", "tester", &archived.id, false).unwrap();

    let stale = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("syncneedle".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(stale.meta.backend, "sqlite");
    assert!(stale.meta.stale);
    assert!(stale.meta.index_lag_events.unwrap() > 0);

    let sync = kanban_sqlite::sync_search_index(&temp.path, "default").unwrap();
    assert_eq!(sync.backend, "tantivy");
    assert!(!sync.stale);
    assert_eq!(sync.index_lag_events, Some(0));
    let derived = derived_store_statuses(&temp.path).unwrap();
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .unwrap();
    assert_eq!(Some(tantivy.last_event_id), sync.last_event_id);
    assert!(!tantivy.dirty);
    assert!(tantivy.last_sync_at.is_some());

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("syncneedle".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(results.meta.backend, "tantivy");
    assert!(!results.meta.stale);
    let ids = results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    for expected in [&updated.id, &commented.id, &run_task.id, &event_task.id] {
        assert!(ids.contains(&expected.as_str()), "{ids:?}");
    }
    assert!(!ids.contains(&archived.id.as_str()), "{ids:?}");
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_index_ahead_of_database_falls_back_and_sync_rebuilds() {
    let temp = TempDb::new("tantivy_index_ahead_of_database_falls_back_and_sync_rebuilds");
    init_database(&temp.path, "tester").unwrap();
    let base = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("rollback base comet"),
    )
    .unwrap();
    kanban_sqlite::checkpoint_database(&temp.path).unwrap();
    let snapshot = temp.dir.join("rollback-snapshot.db");
    std::fs::copy(&temp.path, &snapshot).unwrap();

    let future = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("rollback future phantom"),
    )
    .unwrap();
    let rebuilt = kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    assert_eq!(rebuilt.backend, "tantivy");
    assert!(!rebuilt.stale);

    std::fs::copy(&snapshot, &temp.path).unwrap();
    let _ = std::fs::remove_file(temp.path.with_extension("db-wal"));
    let _ = std::fs::remove_file(temp.path.with_extension("db-shm"));

    assert!(get_task(&temp.path, "default", &future.id).is_err());
    let stale = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("phantom".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(stale.meta.backend, "sqlite");
    assert!(stale.meta.stale);
    assert!(stale.meta.index_lag_events.unwrap() > 0);
    assert!(stale.hits.is_empty(), "{:?}", stale.hits);

    let status = kanban_sqlite::search_index_status(&temp.path, "default").unwrap();
    assert_eq!(status.backend, "sqlite");
    assert!(status.derived_index);
    assert!(status.stale);
    assert!(status.index_lag_events.unwrap() > 0);
    assert!(
        status.message.contains("ahead of the database"),
        "{}",
        status.message
    );

    let synced = kanban_sqlite::sync_search_index(&temp.path, "default").unwrap();
    assert_eq!(synced.backend, "tantivy");
    assert!(!synced.stale);
    assert_eq!(synced.index_lag_events, Some(0));

    let repaired = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(repaired.meta.backend, "tantivy");
    assert!(!repaired.meta.stale);
    assert_eq!(repaired.hits.len(), 1);
    assert_eq!(repaired.hits[0].task_id, base.id);
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_state_metadata_mismatch_falls_back_and_sync_rebuilds() {
    let temp = TempDb::new("tantivy_state_metadata_mismatch_falls_back_and_sync_rebuilds");
    init_database(&temp.path, "tester").unwrap();
    let base = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("contract base comet"),
    )
    .unwrap();
    let clean = kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    assert_eq!(clean.backend, "tantivy");
    assert!(!clean.stale);
    kanban_sqlite::checkpoint_database(&temp.path).unwrap();
    let snapshot = temp.dir.join("contract-snapshot.db");
    std::fs::copy(&temp.path, &snapshot).unwrap();

    let future = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("contract future phantom"),
    )
    .unwrap();
    let advanced = kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    assert!(advanced.last_event_id > clean.last_event_id);

    std::fs::copy(&snapshot, &temp.path).unwrap();
    let _ = std::fs::remove_file(temp.path.with_extension("db-wal"));
    let _ = std::fs::remove_file(temp.path.with_extension("db-shm"));

    assert!(get_task(&temp.path, "default", &future.id).is_err());
    let stale = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("phantom".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(stale.meta.backend, "sqlite");
    assert!(stale.meta.stale);
    assert!(stale.meta.index_lag_events.unwrap() > 0);
    assert!(stale.hits.is_empty(), "{:?}", stale.hits);

    let status = kanban_sqlite::search_index_status(&temp.path, "default").unwrap();
    assert_eq!(status.backend, "sqlite");
    assert!(status.derived_index);
    assert!(status.stale);
    assert!(status.index_lag_events.unwrap() > 0);
    assert!(
        status
            .message
            .contains("mismatched search state and metadata watermarks"),
        "{}",
        status.message
    );

    let synced = kanban_sqlite::sync_search_index(&temp.path, "default").unwrap();
    assert_eq!(synced.backend, "tantivy");
    assert!(!synced.stale);
    assert_eq!(synced.last_event_id, clean.last_event_id);
    assert_eq!(synced.index_lag_events, Some(0));

    let repaired = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(repaired.meta.backend, "tantivy");
    assert!(!repaired.meta.stale);
    assert_eq!(repaired.meta.index_lag_events, Some(0));
    assert_eq!(repaired.hits.len(), 1);
    assert_eq!(repaired.hits[0].task_id, base.id);
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_sync_failure_does_not_advance_search_state_watermark() {
    let temp = TempDb::new("tantivy_sync_failure_does_not_advance_search_state_watermark");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("watermark source"),
    )
    .unwrap();
    let rebuilt = kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();
    update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("watermark syncneedle".into()),
            expected_lock_version: Some(task.lock_version),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    std::fs::write(
        temp.dir.join("index/v1/tasks/kb-index-meta.json"),
        b"not json",
    )
    .unwrap();

    let err = kanban_sqlite::sync_search_index(&temp.path, "default").unwrap_err();
    assert!(err.to_string().contains("expected ident") || err.to_string().contains("JSON"));

    let status = kanban_sqlite::search_index_status(&temp.path, "default").unwrap();
    assert_eq!(status.last_event_id, rebuilt.last_event_id);
    assert!(status.stale);
    assert!(status.index_lag_events.unwrap() > 0);
    let derived = derived_store_statuses(&temp.path).unwrap();
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .unwrap();
    assert_eq!(Some(tantivy.last_event_id), rebuilt.last_event_id);
    assert!(tantivy.dirty);
    assert!(tantivy.last_error.is_some());
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_missing_or_corrupt_index_falls_back_to_sqlite() {
    let temp = TempDb::new("tantivy_missing_or_corrupt_index_falls_back_to_sqlite");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Fallback nebula".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let index_dir = temp.dir.join("index/v1/tasks");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(index_dir.join("kb-index-meta.json"), b"not json").unwrap();

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("nebula".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();

    assert_eq!(results.meta.backend, "sqlite");
    assert!(results.meta.stale);
    assert_eq!(results.hits[0].task_id, task.id);
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_status_degrades_metadata_only_index_dir() {
    let temp = TempDb::new("tantivy_status_degrades_metadata_only_index_dir");
    init_database(&temp.path, "tester").unwrap();
    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    drop(conn);

    let index_dir = temp.dir.join("index/v1/tasks");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(
        index_dir.join("kb-index-meta.json"),
        format!(r#"{{"index_version":"tasks-v1","board_id":"{board_id}","last_event_id":null}}"#),
    )
    .unwrap();

    let status = kanban_sqlite::search_index_status(&temp.path, "default").unwrap();
    assert_eq!(status.backend, "sqlite");
    assert!(status.derived_index);
    assert!(status.stale);
    assert!(status.message.contains("degraded"), "{}", status.message);
    assert!(
        status.message.contains("SQLite fallback"),
        "{}",
        status.message
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_status_degrades_wrong_schema_index() {
    let temp = TempDb::new("tantivy_status_degrades_wrong_schema_index");
    init_database(&temp.path, "tester").unwrap();
    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    drop(conn);

    let index_dir = temp.dir.join("index/v1/tasks");
    std::fs::create_dir_all(&index_dir).unwrap();
    let mut builder = tantivy::schema::Schema::builder();
    builder.add_text_field("board_id", tantivy::schema::STRING);
    tantivy::Index::create_in_dir(&index_dir, builder.build()).unwrap();
    std::fs::write(
        index_dir.join("kb-index-meta.json"),
        format!(r#"{{"index_version":"tasks-v1","board_id":"{board_id}","last_event_id":null}}"#),
    )
    .unwrap();

    let status = kanban_sqlite::search_index_status(&temp.path, "default").unwrap();
    assert_eq!(status.backend, "sqlite");
    assert!(status.derived_index);
    assert!(status.stale);
    assert!(status.message.contains("Schema"), "{}", status.message);
    assert!(
        status.message.contains("SQLite fallback"),
        "{}",
        status.message
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn tantivy_literal_special_searches_fall_back_to_sqlite_after_rebuild() {
    let temp = TempDb::new("tantivy_literal_special_searches_fall_back_to_sqlite_after_rebuild");
    init_database(&temp.path, "tester").unwrap();
    let percent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal 100% complete"),
    )
    .unwrap();
    let underscore = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal snake_case token"),
    )
    .unwrap();
    let backslash = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal path C:\\work"),
    )
    .unwrap();
    let quote = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal quote \" token"),
    )
    .unwrap();

    kanban_sqlite::rebuild_search_index(&temp.path, "default").unwrap();

    for (query, expected) in [
        ("100%", percent.id.as_str()),
        ("snake_case", underscore.id.as_str()),
        ("C:\\work", backslash.id.as_str()),
        ("quote \"", quote.id.as_str()),
    ] {
        let results = search_tasks(
            &temp.path,
            kanban_search::SearchQuery {
                board: "default".into(),
                q: Some(query.into()),
                statuses: vec![],
                assignee: None,
                include_archived: false,
                limit: 10,
                offset: 0,
            },
        )
        .unwrap();
        assert_eq!(results.meta.backend, "sqlite", "{query}");
        assert!(results.meta.stale, "{query}");
        assert!(
            results.hits.iter().any(|hit| hit.task_id == expected),
            "{query}: {:?}",
            results.hits
        );
    }
}

#[test]
fn sqlite_task_list_rejects_limit_that_cannot_be_bounded_safely() {
    let temp = TempDb::new("sqlite_task_list_rejects_limit_that_cannot_be_bounded_safely");
    init_database(&temp.path, "tester").unwrap();

    let error = kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            include_archived: false,
            assignee: None,
            search: None,
            sort: kanban_sqlite::TaskListSort::Position,
            limit: usize::MAX,
            offset: 0,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("limit must be <= 1000"));
}

#[test]
fn sqlite_search_treats_like_wildcards_and_escape_characters_as_literal_query_text() {
    let temp = TempDb::new(
        "sqlite_search_treats_like_wildcards_and_escape_characters_as_literal_query_text",
    );
    init_database(&temp.path, "tester").unwrap();

    let title_percent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "literal percent % title".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let description_underscore = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "description literal".into(),
            description: Some("ready spec with literal _ marker".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let comment_percent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("comment literal source"),
    )
    .unwrap();
    let run_underscore = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("run literal source"),
    )
    .unwrap();
    let title_backslash = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "literal backslash \\ title".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let control = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("control plain source"),
    )
    .unwrap();

    create_comment(
        &temp.path,
        &comment_percent.id,
        "tester",
        "comment contains literal % marker",
        None,
    )
    .unwrap();

    let conn = connect_file(&temp.path).unwrap();
    let board_id: String = conn
        .query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, summary, error, metadata_json) VALUES (?1, ?2, ?3, 'failed', 'token', 'tester', 1, 1, ?4, NULL, '{}')",
        params![new_run_id(), board_id, run_underscore.id, "run contains literal _ marker"],
    )
    .unwrap();

    let percent_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("%".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    let percent_ids = percent_results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    assert!(percent_ids.contains(&title_percent.id.as_str()));
    assert!(percent_ids.contains(&comment_percent.id.as_str()));
    assert!(!percent_ids.contains(&control.id.as_str()));

    let underscore_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("_".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    let underscore_ids = underscore_results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    assert!(underscore_ids.contains(&description_underscore.id.as_str()));
    assert!(underscore_ids.contains(&run_underscore.id.as_str()));
    assert!(!underscore_ids.contains(&control.id.as_str()));

    let backslash_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("\\".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    let backslash_ids = backslash_results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(backslash_ids, vec![title_backslash.id.as_str()]);
}

#[test]
fn explicit_ready_create_requires_ready_prerequisites() {
    let temp = TempDb::new("explicit_ready_create_requires_ready_prerequisites");
    init_database(&temp.path, "tester").unwrap();

    let missing_spec = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "not ready".into(),
            description: None,
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap_err();

    assert!(
        missing_spec
            .to_string()
            .contains("ready requires description"),
        "err: {missing_spec}"
    );

    let future_ready = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "future ready".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 60_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap_err();

    assert!(
        future_ready
            .to_string()
            .contains("ready requires scheduled_at to be due"),
        "err: {future_ready}"
    );
}

#[test]
fn force_archive_running_task_closes_active_run() {
    let temp = TempDb::new("force_archive_running_task_closes_active_run");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archive running"),
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000).unwrap();

    let archived = archive_task(&temp.path, "default", "tester", &task.id, true).unwrap();

    assert_eq!(archived.status, TaskStatus::Archived);
    assert!(archived.claim_token.is_none());
    assert!(archived.claim_owner.is_none());
    assert!(archived.claim_expires_at.is_none());
    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    let run = runs.iter().find(|run| run.id == claim.run_id).unwrap();
    assert_eq!(run.status, "canceled");
    assert!(run.finished_at.is_some());
    assert!(runs.iter().all(|run| run.status != "running"));
    assert!(
        list_events(&temp.path, "default", Some(&task.id))
            .unwrap()
            .iter()
            .any(|event| event.kind == "task.archived"
                && event.run_id.as_deref() == Some(&claim.run_id))
    );
}

#[test]
fn block_reason_with_control_chars_writes_valid_event_json() {
    let temp = TempDb::new("block_reason_with_control_chars_writes_valid_event_json");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("block control chars"),
    )
    .unwrap();
    let reason = "line one\nline two\tquote \" slash \\ control \u{0008}";

    let blocked = block_task(
        &temp.path, "default", "tester", &task.id, reason, None, false,
    )
    .unwrap();

    assert_eq!(blocked.status, TaskStatus::Blocked);
    assert_eq!(blocked.status_reason.as_deref(), Some(reason));
    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    let event = events
        .iter()
        .find(|event| event.kind == "task.blocked")
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json).unwrap();
    assert_eq!(payload["reason"], reason);
}

#[test]
fn block_rolls_back_task_state_when_event_insert_fails() {
    let temp = TempDb::new("block_rolls_back_task_state_when_event_insert_fails");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("block rollback"),
    )
    .unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_block_event BEFORE INSERT ON task_events WHEN NEW.kind='task.blocked' BEGIN SELECT RAISE(ABORT, 'forced task.blocked event failure'); END",
            [],
        )
        .unwrap();

    let err = block_task(
        &temp.path, "default", "tester", &task.id, "blocked", None, false,
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("forced task.blocked event failure")
    );
    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Ready);
    assert!(fresh.status_reason.is_none());
}

#[test]
fn update_rolls_back_task_state_when_event_insert_fails() {
    let temp = TempDb::new("update_rolls_back_task_state_when_event_insert_fails");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("original title"),
    )
    .unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_update_event BEFORE INSERT ON task_events WHEN NEW.kind='task.updated' BEGIN SELECT RAISE(ABORT, 'forced task.updated event failure'); END",
            [],
        )
        .unwrap();

    let err = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("changed title".into()),
            priority: Some(99),
            ..TaskPatch::default()
        },
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("forced task.updated event failure"),
        "err: {err}"
    );
    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.title, "original title");
    assert_eq!(fresh.priority, task.priority);
    assert_eq!(fresh.lock_version, task.lock_version);
}

#[test]
fn updating_ready_task_to_future_schedule_makes_it_unclaimable_until_due() {
    let temp = TempDb::new("updating_ready_task_to_future_schedule_makes_it_unclaimable_until_due");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("future scheduled update"),
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(Some(now_ms() + 3_600_000)),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Scheduled);
    assert!(claim_task(&temp.path, "default", "worker", &task.id, 300_000).is_err());
    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap();
    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Scheduled
    );
}

#[test]
fn clearing_schedule_recomputes_complete_task_without_dependencies_to_ready() {
    let temp =
        TempDb::new("clearing_schedule_recomputes_complete_task_without_dependencies_to_ready");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "scheduled complete".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 3_600_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.scheduled_at, None);
    assert_eq!(updated.status, TaskStatus::Ready);
}

#[test]
fn clearing_schedule_recomputes_incomplete_dependencies_to_todo() {
    let temp = TempDb::new("clearing_schedule_recomputes_incomplete_dependencies_to_todo");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("incomplete parent"),
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "scheduled child".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 3_600_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Todo);
}

#[test]
fn clearing_schedule_recomputes_missing_description_to_triage() {
    let temp = TempDb::new("clearing_schedule_recomputes_missing_description_to_triage");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "scheduled missing spec".into(),
            description: None,
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 3_600_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Triage);
}

#[test]
fn updating_description_recomputes_active_triage_to_ready() {
    let temp = TempDb::new("updating_description_recomputes_active_triage_to_ready");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "needs spec".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    assert_eq!(task.status, TaskStatus::Triage);

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            description: Some(Some("ready spec".into())),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Ready);
}

#[test]
fn updating_description_recomputes_active_ready_to_triage() {
    let temp = TempDb::new("updating_description_recomputes_active_ready_to_triage");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("remove spec"),
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            description: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Triage);
}

#[test]
fn claim_complete_and_dependencies_promote_children() {
    let temp = TempDb::new("claim_complete_and_dependencies_promote_children");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务")).unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务")).unwrap();

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Todo
    );

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000).unwrap();
    assert_eq!(claim.task.status, TaskStatus::Running);
    assert!(!claim.claim_token.is_empty());
    assert!(claim.task.current_run_id.is_some());
    let heartbeat = kanban_sqlite::heartbeat_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        &claim.claim_token,
        600_000,
    )
    .unwrap();
    assert!(heartbeat.claim_expires_at > claim.task.claim_expires_at);

    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )
    .unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &parent.id).unwrap().status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
    assert_eq!(
        list_runs(&temp.path, "default", Some(&parent.id)).unwrap()[0].status,
        "succeeded"
    );
}

#[test]
fn block_unblock_recomputes_target_and_cycle_detection_rejects_cycles() {
    let temp = TempDb::new("block_unblock_recomputes_target_and_cycle_detection_rejects_cycles");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务")).unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务")).unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &child.id, &parent.id).unwrap_err();
    assert!(err.to_string().contains("cycle"));

    block_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        "等待输入",
        None,
        false,
    )
    .unwrap();
    let unblocked = unblock_task(&temp.path, "default", "tester", &child.id).unwrap();
    assert_eq!(unblocked.status, TaskStatus::Todo);

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000).unwrap();
    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )
    .unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
}

#[test]
fn dispatch_once_runs_ready_task_and_records_log() {
    let temp = TempDb::new("dispatch_once_runs_ready_task_and_records_log");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("跑 worker"),
    )
    .unwrap();
    let log_dir = temp.dir.join("logs");

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "sh -c 'echo task=$KB_TASK_ID; test -n \"$KB_CLAIM_TOKEN\"'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: log_dir.clone(),
        },
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Done
    );
    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    assert_eq!(runs[0].status, "succeeded");
    let log_path = runs[0].log_path.as_ref().expect("run log path");
    assert!(std::fs::read_to_string(log_path).unwrap().contains("task="));
}

#[test]
fn doctor_resolves_legacy_relative_run_log_paths_against_database_dir() {
    let temp = TempDb::new("doctor_resolves_legacy_relative_run_log_paths_against_database_dir");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("legacy log path"),
    )
    .unwrap();
    let log_dir = temp.dir.join("logs");
    dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf legacy".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir,
        },
    )
    .unwrap();
    let run = list_runs(&temp.path, "default", Some(&task.id)).unwrap()[0].clone();
    let absolute_log_path = Path::new(run.log_path.as_ref().unwrap());
    let relative_log_path = absolute_log_path
        .strip_prefix(&temp.dir)
        .unwrap()
        .to_string_lossy()
        .to_string();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "UPDATE task_runs SET log_path=?1 WHERE id=?2",
            params![relative_log_path, run.id],
        )
        .unwrap();

    let report = doctor_database(&temp.path).unwrap();

    assert_eq!(report.missing_run_logs, 0);
    assert!(report.ok);
}

#[test]
fn doctor_reports_partially_initialized_database_without_bailing() {
    let temp = TempDb::new("doctor_reports_partially_initialized_database_without_bailing");
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL DEFAULT '', applied_at INTEGER NOT NULL)",
            [],
        )
        .unwrap();

    let report = doctor_database(&temp.path).unwrap();

    assert!(!report.ok);
    assert_eq!(report.migration_version, None);
    assert_eq!(report.user_version, 0);
}

#[test]
fn doctor_reports_executable_status_invariant_violations() {
    let temp = TempDb::new("doctor_reports_executable_status_invariant_violations");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("unfinished parent"),
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid ready child"),
    )
    .unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();
    let missing_spec = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid missing spec"),
    )
    .unwrap();
    let future_scheduled = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid future schedule"),
    )
    .unwrap();
    let conn = connect_file(&temp.path).unwrap();
    conn.execute("UPDATE tasks SET status='ready' WHERE id=?1", [&child.id])
        .unwrap();
    conn.execute(
        "UPDATE tasks SET description=NULL WHERE id=?1",
        [&missing_spec.id],
    )
    .unwrap();
    conn.execute(
        "UPDATE tasks SET scheduled_at=?1 WHERE id=?2",
        params![4_102_444_800_000_i64, future_scheduled.id],
    )
    .unwrap();

    let report = doctor_database(&temp.path).unwrap();

    assert!(!report.ok);
    assert_eq!(report.executable_dependency_violations, 1);
    assert_eq!(report.executable_spec_violations, 1);
    assert_eq!(report.executable_schedule_violations, 1);
}

#[test]
fn doctor_counts_each_dependency_cycle_once() {
    let temp = TempDb::new("doctor_counts_each_dependency_cycle_once");
    init_database(&temp.path, "tester").unwrap();
    let a = create_task(&temp.path, "default", "tester", CreateTask::ready("a")).unwrap();
    let b = create_task(&temp.path, "default", "tester", CreateTask::ready("b")).unwrap();
    let c = create_task(&temp.path, "default", "tester", CreateTask::ready("c")).unwrap();
    add_dependency(&temp.path, "default", "tester", &a.id, &b.id).unwrap();
    add_dependency(&temp.path, "default", "tester", &b.id, &c.id).unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) \
             VALUES (?1, ?2, ?3, 1)",
            params![a.board_id, c.id, a.id],
        )
        .unwrap();

    let report = doctor_database(&temp.path).unwrap();

    assert!(!report.ok);
    assert_eq!(report.dependency_cycles, 1);
}

#[test]
fn concurrent_claim_attempts_on_one_ready_task_have_exactly_one_success() {
    let temp = TempDb::new("concurrent_claim_attempts_on_one_ready_task_have_exactly_one_success");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("只允许一个 claim"),
    )
    .unwrap();

    let path = Arc::new(temp.path.clone());
    let task_id = Arc::new(task.id.clone());
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for actor in ["worker-a", "worker-b"] {
        let path = Arc::clone(&path);
        let task_id = Arc::clone(&task_id);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            claim_task(&*path, "default", actor, &task_id, 300_000)
        }));
    }

    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("claim thread should not panic"))
        .collect::<Vec<_>>();
    let successes = results.iter().filter(|result| result.is_ok()).count();
    let failures = results.iter().filter(|result| result.is_err()).count();
    assert_eq!(successes, 1, "results: {results:?}");
    assert_eq!(failures, 1, "results: {results:?}");

    let claimed = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(claimed.status, TaskStatus::Running);
    assert!(claimed.claim_token.is_some());
    assert_eq!(
        list_runs(&temp.path, "default", Some(&task.id))
            .unwrap()
            .iter()
            .filter(|run| run.status == "running")
            .count(),
        1
    );
}

#[test]
fn claimed_task_has_current_run_running_run_and_claimed_event() {
    let temp = TempDb::new("claimed_task_has_current_run_running_run_and_claimed_event");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim invariant"),
    )
    .unwrap();

    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000).unwrap();
    let claimed = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(claimed.status, TaskStatus::Running);
    assert_eq!(
        claimed.current_run_id.as_deref(),
        Some(claim.run_id.as_str())
    );

    let running_runs = list_runs(&temp.path, "default", Some(&task.id))
        .unwrap()
        .into_iter()
        .filter(|run| run.status == "running")
        .collect::<Vec<_>>();
    assert_eq!(running_runs.len(), 1);
    assert_eq!(running_runs[0].id, claim.run_id);

    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    assert!(
        events.iter().any(|event| event.kind == "task.claimed"
            && event.run_id.as_deref() == claimed.current_run_id.as_deref()),
        "events: {events:?}"
    );
}

#[test]
fn dispatch_once_does_not_claim_review_or_dependency_blocked_tasks() {
    let temp = TempDb::new("dispatch_once_does_not_claim_review_or_dependency_blocked_tasks");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "未完成父任务".into(),
            description: Some(String::new()),
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let review_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("review 不可 claim"),
    )
    .unwrap();
    let review_claim =
        claim_task(&temp.path, "default", "worker", &review_task.id, 300_000).unwrap();
    kanban_sqlite::submit_review_task(
        &temp.path,
        "default",
        "worker",
        &review_task.id,
        Some(&review_claim.claim_token),
        false,
    )
    .unwrap();
    let blocked_by_dependency = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("依赖未完成但快照被修回 ready"),
    )
    .unwrap();
    add_dependency(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        &blocked_by_dependency.id,
    )
    .unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "UPDATE tasks SET status='ready' WHERE id=?1",
            [&blocked_by_dependency.id],
        )
        .unwrap();

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap();

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &review_task.id)
            .unwrap()
            .status,
        TaskStatus::Review
    );
    assert_eq!(
        get_task(&temp.path, "default", &blocked_by_dependency.id)
            .unwrap()
            .status,
        TaskStatus::Ready
    );
    assert!(
        list_runs(&temp.path, "default", Some(&blocked_by_dependency.id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn claim_actor_with_quotes_and_control_chars_writes_valid_event_json() {
    let temp = TempDb::new("claim_actor_with_quotes_and_control_chars_writes_valid_event_json");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim JSON escaping"),
    )
    .unwrap();
    let actor = "bad\"actor\nwith\tcontrol";

    let claim = claim_task(&temp.path, "default", actor, &task.id, 300_000).unwrap();

    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Running);
    assert_eq!(fresh.claim_owner.as_deref(), Some(actor));
    assert_eq!(fresh.current_run_id.as_deref(), Some(claim.run_id.as_str()));
    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    let event = events
        .iter()
        .find(|event| event.kind == "task.claimed")
        .expect("task.claimed event");
    assert_eq!(event.actor.as_deref(), Some(actor));
    assert_eq!(event.run_id.as_deref(), Some(claim.run_id.as_str()));
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json).unwrap();
    assert_eq!(payload["claim_owner"], actor);
}

#[test]
fn add_dependency_rolls_back_edge_and_status_when_event_insert_fails() {
    let temp = TempDb::new("add_dependency_rolls_back_edge_and_status_when_event_insert_fails");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "incomplete parent".into(),
            description: None,
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("child")).unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_dependency_added_event BEFORE INSERT ON task_events WHEN NEW.kind='dependency.added' BEGIN SELECT RAISE(ABORT, 'forced dependency.added event failure'); END",
            [],
        )
        .unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap_err();

    assert!(
        err.to_string()
            .contains("forced dependency.added event failure"),
        "err: {err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
    assert!(
        list_dependencies(&temp.path, "default", &child.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn remove_dependency_recomputes_child_to_ready_when_unblocked() {
    let temp = TempDb::new("remove_dependency_recomputes_child_to_ready_when_unblocked");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "unfinished parent".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("child should unblock"),
    )
    .unwrap();

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Todo
    );

    kanban_sqlite::remove_dependency(&temp.path, "default", "tester", &parent.id, &child.id)
        .unwrap();

    let child = get_task(&temp.path, "default", &child.id).unwrap();
    assert_eq!(child.status, TaskStatus::Ready);
    assert!(
        list_events(&temp.path, "default", Some(&child.id))
            .unwrap()
            .iter()
            .any(|event| event.kind == "task.promoted")
    );
}

#[test]
fn dispatch_once_promotes_eligible_scheduled_and_todo_before_claiming() {
    let temp = TempDb::new("dispatch_once_promotes_eligible_scheduled_and_todo_before_claiming");
    init_database(&temp.path, "tester").unwrap();
    let now = now_ms();
    let scheduled = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "due scheduled".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 10,
            scheduled_at: Some(now - 1_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let todo = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "eligible todo".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &scheduled.id)
            .unwrap()
            .status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &todo.id).unwrap().status,
        TaskStatus::Ready
    );
    for task_id in [&scheduled.id, &todo.id] {
        assert!(
            list_events(&temp.path, "default", Some(task_id))
                .unwrap()
                .iter()
                .any(|event| event.kind == "task.promoted"),
            "missing task.promoted for {task_id}"
        );
    }
}

#[test]
fn dispatch_once_heartbeats_while_long_running_command_blocks() {
    let temp = TempDb::new("dispatch_once_heartbeats_while_long_running_command_blocks");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("long worker"),
    )
    .unwrap();

    dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "sleep 0.25".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 100,
            heartbeat_interval_ms: 25,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap();

    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    assert_eq!(runs[0].status, "succeeded");
    let (run_claim_expires_at, run_last_heartbeat_at): (i64, i64) = connect_file(&temp.path)
        .unwrap()
        .query_row(
            "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id=?1",
            [&runs[0].id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(run_last_heartbeat_at > runs[0].started_at);
    assert!(run_claim_expires_at > runs[0].started_at + 100);
    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    let claimed_at = events
        .iter()
        .find(|event| event.kind == "task.claimed")
        .expect("claimed event")
        .created_at;
    let heartbeat = events
        .iter()
        .find(|event| event.kind == "task.heartbeat")
        .expect("heartbeat event");
    assert!(heartbeat.created_at > claimed_at, "events: {events:?}");
}

#[test]
fn manual_block_during_dispatch_is_not_overwritten_to_done() {
    let temp = TempDb::new("manual_block_during_dispatch_is_not_overwritten_to_done");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("manual block race"),
    )
    .unwrap();
    let release = temp.dir.join("release");

    let dispatch_path = temp.path.clone();
    let dispatch_release = release.clone();
    let handle = thread::spawn(move || {
        dispatch_once(
            &dispatch_path,
            "default",
            DispatchOptions {
                actor: "dispatcher".into(),
                command: format!(
                    "while [ ! -f '{}' ]; do sleep 0.01; done; true",
                    dispatch_release.display()
                ),
                worker_profile: "default".into(),
                claim_ttl_ms: 300_000,
                heartbeat_interval_ms: 300_000,
                on_success: FinishPolicy::Done,
                on_failure: FinishPolicy::Blocked,
                log_dir: dispatch_release.parent().unwrap().join("logs"),
            },
        )
    });

    for _ in 0..200 {
        if get_task(&temp.path, "default", &task.id).unwrap().status == TaskStatus::Running {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    block_task(
        &temp.path,
        "default",
        "human",
        &task.id,
        "manual block",
        None,
        true,
    )
    .unwrap();
    std::fs::write(&release, "go").unwrap();
    let result = handle.join().unwrap();
    assert!(result.is_err(), "dispatcher should report finish conflict");

    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Blocked);
    assert_eq!(fresh.status_reason.as_deref(), Some("manual block"));
}

#[test]
fn todo_without_description_is_not_promoted_or_claimed_by_dispatch() {
    let temp = TempDb::new("todo_without_description_is_not_promoted_or_claimed_by_dispatch");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "needs spec".into(),
            description: None,
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap();

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Todo
    );
}

#[test]
fn heartbeat_against_stale_non_running_claim_fails_without_touching_heartbeat_fields() {
    let temp = TempDb::new(
        "heartbeat_against_stale_non_running_claim_fails_without_touching_heartbeat_fields",
    );
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("stale hb"),
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000).unwrap();
    block_task(
        &temp.path,
        "default",
        "human",
        &task.id,
        "manual block",
        None,
        true,
    )
    .unwrap();
    let blocked = get_task(&temp.path, "default", &task.id).unwrap();

    let err = kanban_sqlite::heartbeat_task(
        &temp.path,
        "default",
        "worker",
        &task.id,
        &claim.claim_token,
        600_000,
    )
    .unwrap_err();
    assert!(err.to_string().contains("matching running claim"));

    let after = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(after.status, TaskStatus::Blocked);
    assert_eq!(after.claim_expires_at, blocked.claim_expires_at);
    assert_eq!(after.last_heartbeat_at, blocked.last_heartbeat_at);
}

#[test]
fn worker_large_output_does_not_deadlock_under_heartbeat_wrapper() {
    let temp = TempDb::new("worker_large_output_does_not_deadlock_under_heartbeat_wrapper");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("large output"),
    )
    .unwrap();

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "python3 -c 'import sys; sys.stdout.write(\"x\" * 2000000)'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 100,
            heartbeat_interval_ms: 25,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Done
    );
}

#[test]
fn dispatch_rejects_heartbeat_interval_not_less_than_claim_ttl() {
    let temp = TempDb::new("dispatch_rejects_heartbeat_interval_not_less_than_claim_ttl");
    init_database(&temp.path, "tester").unwrap();
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bad interval"),
    )
    .unwrap();

    let err = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 100,
            heartbeat_interval_ms: 100,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )
    .unwrap_err();

    assert!(err.to_string().contains("heartbeat_interval_ms"));
}

#[test]
fn adding_incomplete_parent_to_running_child_is_rejected_without_force() {
    let temp = TempDb::new("adding_incomplete_parent_to_running_child_is_rejected_without_force");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "incomplete parent".into(),
            description: None,
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("running child"),
    )
    .unwrap();
    claim_task(&temp.path, "default", "worker", &child.id, 300_000).unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap_err();

    assert!(
        err.to_string().contains("running") && err.to_string().contains("dependency"),
        "err: {err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Running
    );
    assert!(
        list_dependencies(&temp.path, "default", &child.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn add_dependency_reloads_child_inside_transaction_before_demoting_ready() {
    let temp = TempDb::new("add_dependency_reloads_child_inside_transaction_before_demoting_ready");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "incomplete parent".into(),
            description: None,
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claimed child"),
    )
    .unwrap();

    let conn = connect_file(&temp.path).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    mark_task_running_in_current_tx(&conn, &child.id);
    let adding = thread::spawn({
        let db_path = temp.path.clone();
        let parent_id = parent.id.clone();
        let child_id = child.id.clone();
        move || add_dependency(&db_path, "default", "tester", &parent_id, &child_id)
    });
    thread::sleep(Duration::from_millis(50));
    conn.execute_batch("COMMIT").unwrap();

    let err = adding.join().unwrap().unwrap_err();
    assert!(
        err.to_string().contains("running") && err.to_string().contains("dependency"),
        "err: {err}"
    );
    let fresh = get_task(&temp.path, "default", &child.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Running);
    assert!(fresh.current_run_id.is_some());
    assert!(
        list_dependencies(&temp.path, "default", &child.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn promote_task_reloads_dependencies_inside_transaction() {
    let temp = TempDb::new("promote_task_reloads_dependencies_inside_transaction");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "unfinished parent".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "manual promote race".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let conn = connect_file(&temp.path).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    conn.execute(
        "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![child.board_id, parent.id, child.id, now_ms()],
    )
    .unwrap();
    let promoting = thread::spawn({
        let db_path = temp.path.clone();
        let child_id = child.id.clone();
        move || promote_task(&db_path, "default", "tester", &child_id)
    });
    thread::sleep(Duration::from_millis(50));
    conn.execute_batch("COMMIT").unwrap();

    let err = promoting.join().unwrap().unwrap_err();
    assert!(err.to_string().contains("dependency"), "err: {err}");
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Todo
    );
}

#[test]
fn unblock_task_reloads_status_inside_transaction_before_recomputing() {
    let temp = TempDb::new("unblock_task_reloads_status_inside_transaction_before_recomputing");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("blocked archive race"),
    )
    .unwrap();
    block_task(
        &temp.path, "default", "tester", &task.id, "waiting", None, false,
    )
    .unwrap();

    let conn = connect_file(&temp.path).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    conn.execute(
        "UPDATE tasks SET status='archived', archived_at=?1, updated_at=?1, lock_version=lock_version+1 WHERE id=?2",
        params![now_ms(), task.id],
    )
    .unwrap();
    let unblocking = thread::spawn({
        let db_path = temp.path.clone();
        let task_id = task.id.clone();
        move || unblock_task(&db_path, "default", "tester", &task_id)
    });
    thread::sleep(Duration::from_millis(50));
    conn.execute_batch("COMMIT").unwrap();

    let err = unblocking.join().unwrap().unwrap_err();
    assert!(err.to_string().contains("unblock"), "err: {err}");
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Archived
    );
}

#[test]
fn database_replace_is_rejected_while_runtime_lock_is_held() {
    let temp = TempDb::new("database_replace_is_rejected_while_runtime_lock_is_held");
    init_database(&temp.path, "tester").unwrap();
    let _runtime_guard = begin_database_runtime(&temp.path).unwrap();

    let err = begin_database_replace(&temp.path).unwrap_err();

    assert!(
        err.to_string().contains("running")
            || err.to_string().contains("runtime")
            || err.to_string().contains("serve/dispatch"),
        "err: {err}"
    );
}

#[test]
fn failed_ready_retry_policy_increments_retry_count_and_blocks_at_max_retries() {
    let temp =
        TempDb::new("failed_ready_retry_policy_increments_retry_count_and_blocks_at_max_retries");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("retry worker"),
    )
    .unwrap();
    set_retry_policy(&temp.path, &task.id, 2);

    for expected_retry_count in [1, 2] {
        let result = dispatch_once(
            &temp.path,
            "default",
            DispatchOptions {
                actor: "dispatcher".into(),
                command: "exit 7".into(),
                worker_profile: "default".into(),
                claim_ttl_ms: 300_000,
                heartbeat_interval_ms: 30_000,
                on_success: FinishPolicy::Done,
                on_failure: FinishPolicy::Ready,
                log_dir: temp.dir.join("logs"),
            },
        )
        .unwrap();
        assert_eq!(result.claimed, 1);
        assert_eq!(result.exit_code, Some(7));
        let fresh = get_task(&temp.path, "default", &task.id).unwrap();
        assert_eq!(fresh.retry_count, expected_retry_count);
        if expected_retry_count == 1 {
            assert_eq!(fresh.status, TaskStatus::Ready);
        } else {
            assert_eq!(fresh.status, TaskStatus::Blocked);
        }
    }
}

#[test]
fn reclaim_expired_increments_retry_count_and_blocks_at_max_retries() {
    let temp = TempDb::new("reclaim_expired_increments_retry_count_and_blocks_at_max_retries");
    init_database(&temp.path, "tester").unwrap();
    let retrying = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("retry reclaim"),
    )
    .unwrap();
    set_retry_policy(&temp.path, &retrying.id, 2);
    let blocking = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("blocking reclaim"),
    )
    .unwrap();
    set_retry_policy(&temp.path, &blocking.id, 1);

    for task in [&retrying, &blocking] {
        claim_task(&temp.path, "default", "worker", &task.id, 1).unwrap();
    }
    thread::sleep(Duration::from_millis(5));

    let reclaimed = kanban_sqlite::reclaim_expired(&temp.path, "default", "dispatcher").unwrap();

    assert_eq!(reclaimed, 2);
    let retrying = get_task(&temp.path, "default", &retrying.id).unwrap();
    assert_eq!(retrying.retry_count, 1);
    assert_eq!(retrying.status, TaskStatus::Ready);
    let blocking = get_task(&temp.path, "default", &blocking.id).unwrap();
    assert_eq!(blocking.retry_count, 1);
    assert_eq!(blocking.status, TaskStatus::Blocked);
    assert!(
        list_events(&temp.path, "default", Some(&blocking.id))
            .unwrap()
            .iter()
            .any(|event| event.kind == "task.reclaimed")
    );
}

#[test]
fn reclaim_expired_skips_task_heartbeated_after_scan_before_claim_tx() {
    let temp = TempDb::new("reclaim_expired_skips_task_heartbeated_after_scan_before_claim_tx");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("heartbeat race"),
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 1).unwrap();
    thread::sleep(Duration::from_millis(5));

    let db_path = temp.path.clone();
    let task_id = task.id.clone();
    let run_id = claim.run_id.clone();
    let claim_token = claim.claim_token.clone();
    let heartbeat_started = Arc::new(Barrier::new(2));
    let release_heartbeat = Arc::new(Barrier::new(2));
    let worker_started = Arc::clone(&heartbeat_started);
    let worker_release = Arc::clone(&release_heartbeat);
    let handle = thread::spawn(move || {
        let conn = connect_file(&db_path).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let extended = now_ms() + 300_000;
        conn.execute(
            "UPDATE tasks SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3 AND status='running' AND claim_token=?4",
            params![extended, extended, task_id, claim_token],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_runs SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3",
            params![extended, extended, run_id],
        )
        .unwrap();
        worker_started.wait();
        worker_release.wait();
        conn.execute_batch("COMMIT").unwrap();
    });

    heartbeat_started.wait();
    let reclaiming = thread::spawn({
        let db_path = temp.path.clone();
        move || kanban_sqlite::reclaim_expired(&db_path, "default", "dispatcher")
    });
    thread::sleep(Duration::from_millis(50));
    release_heartbeat.wait();

    let reclaimed = reclaiming.join().unwrap().unwrap();
    handle.join().unwrap();

    assert_eq!(reclaimed, 0);
    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Running);
    assert!(
        fresh
            .claim_expires_at
            .is_some_and(|expires| expires > now_ms())
    );
}

fn mark_task_running_in_current_tx(conn: &Connection, task_id: &str) {
    let run_id = new_run_id();
    let now = now_ms();
    let claim_token = format!("token-{task_id}");
    let (board_id, claim_owner): (String, String) = conn
        .query_row(
            "SELECT board_id, 'worker' FROM tasks WHERE id=?1",
            [task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    conn.execute(
        "UPDATE tasks SET status='running', claim_token=?1, claim_owner=?2, claim_expires_at=?3, last_heartbeat_at=?4, started_at=COALESCE(started_at, ?4), current_run_id=?5, updated_at=?4, lock_version=lock_version+1 WHERE id=?6",
        params![claim_token, claim_owner, now + 300_000, now, run_id, task_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, ?2, ?3, 'running', 'test', ?4, ?5, ?6, ?7, ?7, '{}')",
        params![run_id, board_id, task_id, claim_token, claim_owner, now + 300_000, now],
    )
    .unwrap();
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-v05-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        if Path::new(&self.dir).exists() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn insert_board(path: &Path, slug: &str, id: &str) {
    connect_file(path)
        .unwrap()
        .execute(
            "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES (?1, ?2, ?3, NULL, 1, 1, NULL)",
            params![id, slug, slug],
        )
        .unwrap();
}

#[derive(Default)]
struct RecordingVectorStore {
    embedding_model: Option<String>,
    live_chunks: std::sync::Mutex<Vec<EmbeddingChunk>>,
    upserted: std::sync::Mutex<Vec<String>>,
    upserted_models: std::sync::Mutex<Vec<String>>,
    deleted: std::sync::Mutex<Vec<String>>,
    deleted_boards: std::sync::Mutex<Vec<String>>,
}

impl RecordingVectorStore {
    fn with_embedding_model(embedding_model: &str) -> Self {
        Self {
            embedding_model: Some(embedding_model.to_owned()),
            ..Self::default()
        }
    }

    fn expected_model(&self) -> &str {
        self.embedding_model
            .as_deref()
            .unwrap_or(kanban_vector::DEFAULT_EMBEDDING_MODEL)
    }

    fn upserted_texts(&self) -> Vec<String> {
        self.upserted.lock().unwrap().clone()
    }

    fn upserted_models(&self) -> Vec<String> {
        self.upserted_models.lock().unwrap().clone()
    }

    fn deleted_entity_uris(&self) -> Vec<String> {
        self.deleted.lock().unwrap().clone()
    }

    fn deleted_board_ids(&self) -> Vec<String> {
        self.deleted_boards.lock().unwrap().clone()
    }

    fn live_texts(&self) -> Vec<String> {
        self.live_chunks
            .lock()
            .unwrap()
            .iter()
            .map(|chunk| chunk.text.clone())
            .collect()
    }
}

impl VectorStore for RecordingVectorStore {
    fn chunk_embedding_model(&self) -> &str {
        self.expected_model()
    }

    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: "test vector store".to_owned(),
        }
    }

    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        if let Some(chunk) = chunks
            .iter()
            .find(|chunk| chunk.embedding_model != self.expected_model())
        {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: self.expected_model().to_owned(),
                actual: chunk.embedding_model.clone(),
            });
        }
        let mut upserted = self.upserted.lock().unwrap();
        upserted.extend(chunks.iter().map(|chunk| chunk.text.clone()));
        let mut upserted_models = self.upserted_models.lock().unwrap();
        upserted_models.extend(chunks.iter().map(|chunk| chunk.embedding_model.clone()));
        let mut live_chunks = self.live_chunks.lock().unwrap();
        for chunk in chunks {
            live_chunks.retain(|live| live.chunk_key() != chunk.chunk_key());
            live_chunks.push(chunk.clone());
        }
        Ok(())
    }

    fn delete_board(&self, board_id: &str) -> Result<(), VectorError> {
        let mut deleted_boards = self.deleted_boards.lock().unwrap();
        deleted_boards.push(board_id.to_owned());
        self.live_chunks
            .lock()
            .unwrap()
            .retain(|chunk| chunk.board_id.as_deref() != Some(board_id));
        Ok(())
    }

    fn delete_entities(&self, entity_uris: &[String]) -> Result<(), VectorError> {
        let mut deleted = self.deleted.lock().unwrap();
        deleted.extend(entity_uris.iter().cloned());
        self.live_chunks.lock().unwrap().retain(|chunk| {
            !entity_uris
                .iter()
                .any(|entity_uri| entity_uri == chunk.chunk.entity_uri.as_str())
        });
        Ok(())
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

struct FailingVectorStore;

impl VectorStore for FailingVectorStore {
    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: "test vector store".to_owned(),
        }
    }

    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::DimensionMismatch {
            expected: 3,
            actual: 2,
        })
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Ok(())
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

#[cfg(feature = "tantivy-backend")]
fn tantivy_outbox_statuses_for_board(path: &Path, board_slug: &str) -> Vec<String> {
    let conn = connect_file(path).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT io.status \
             FROM index_outbox io \
             JOIN task_events e ON e.id=io.source_event_id \
             JOIN boards b ON b.id=e.board_id \
             WHERE b.slug=?1 AND io.target='tantivy' \
             ORDER BY io.id ASC",
        )
        .unwrap();
    stmt.query_map([board_slug], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}

#[cfg(feature = "graph-oxigraph")]
fn graph_outbox_statuses_for_board(path: &Path, board_slug: &str) -> Vec<String> {
    let conn = connect_file(path).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT io.status \
             FROM index_outbox io \
             JOIN task_events e ON e.id=io.source_event_id \
             JOIN boards b ON b.id=e.board_id \
             WHERE b.slug=?1 AND io.target='oxigraph' \
             ORDER BY io.id ASC",
        )
        .unwrap();
    stmt.query_map([board_slug], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}

fn lancedb_outbox_statuses_for_board(path: &Path, board_slug: &str) -> Vec<String> {
    let conn = connect_file(path).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT io.status \
             FROM index_outbox io \
             JOIN task_events e ON e.id=io.source_event_id \
             JOIN boards b ON b.id=e.board_id \
             WHERE b.slug=?1 AND io.target='lancedb' \
             ORDER BY io.id ASC",
        )
        .unwrap();
    stmt.query_map([board_slug], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn set_retry_policy(path: &Path, task_id: &str, max_retries: i64) {
    connect_file(path)
        .unwrap()
        .execute(
            "UPDATE tasks SET max_retries=?1 WHERE id=?2",
            rusqlite::params![max_retries, task_id],
        )
        .unwrap();
}
