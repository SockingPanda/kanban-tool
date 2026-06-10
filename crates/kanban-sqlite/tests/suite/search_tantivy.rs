#![cfg(feature = "tantivy-backend")]

use crate::common::*;

#[test]
fn tantivy_rebuild_marks_only_current_board_outbox_done() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_rebuild_marks_only_current_board_outbox_done")?;
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
    Ok(())
}

#[test]
fn tantivy_sync_marks_only_current_board_outbox_done() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_marks_only_current_board_outbox_done")?;
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
    Ok(())
}

#[test]
fn tantivy_sync_failure_marks_only_current_board_outbox_failed() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_failure_marks_only_current_board_outbox_failed")?;
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
    Ok(())
}

#[test]
fn tantivy_rebuild_searches_task_aggregate_and_keeps_sqlite_hydration_filters() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("tantivy_rebuild_searches_task_aggregate_and_keeps_sqlite_hydration_filters")?;
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
    Ok(())
}

#[test]
fn stale_tantivy_index_falls_back_to_sqlite_before_current_filters_are_applied()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("stale_tantivy_index_falls_back_to_sqlite_before_current_filters_are_applied")?;
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
    Ok(())
}

#[test]
fn tantivy_rebuild_persists_search_state_in_app_settings() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_rebuild_persists_search_state_in_app_settings")?;
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
    Ok(())
}

#[test]
fn tantivy_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending() -> anyhow::Result<()> {
    let temp =
        TempDb::new("tantivy_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending")?;
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
    Ok(())
}

#[test]
fn tantivy_sync_reindexes_task_comment_run_event_and_archive_changes() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_reindexes_task_comment_run_event_and_archive_changes")?;
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
    Ok(())
}

#[test]
fn tantivy_index_ahead_of_database_falls_back_and_sync_rebuilds() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_index_ahead_of_database_falls_back_and_sync_rebuilds")?;
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
    Ok(())
}

#[test]
fn tantivy_state_metadata_mismatch_falls_back_and_sync_rebuilds() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_state_metadata_mismatch_falls_back_and_sync_rebuilds")?;
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
    Ok(())
}

#[test]
fn tantivy_sync_failure_does_not_advance_search_state_watermark() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_failure_does_not_advance_search_state_watermark")?;
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
    Ok(())
}

#[test]
fn tantivy_missing_or_corrupt_index_falls_back_to_sqlite() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_missing_or_corrupt_index_falls_back_to_sqlite")?;
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
    Ok(())
}

#[test]
fn tantivy_status_degrades_metadata_only_index_dir() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_status_degrades_metadata_only_index_dir")?;
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
    Ok(())
}

#[test]
fn tantivy_status_degrades_wrong_schema_index() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_status_degrades_wrong_schema_index")?;
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
    Ok(())
}

#[test]
fn tantivy_literal_special_searches_fall_back_to_sqlite_after_rebuild() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_literal_special_searches_fall_back_to_sqlite_after_rebuild")?;
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
    Ok(())
}
