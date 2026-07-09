#![cfg(feature = "tantivy-backend")]

use crate::common::*;

fn mark_ready_fixture(db_path: &std::path::Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::api::mark_execution_plan_not_required(
        db_path,
        "default",
        "tantivy-search-test",
        task_id,
        "search fixture does not need steps",
    )?;
    Ok(())
}

#[test]
fn tantivy_rebuild_marks_only_current_board_outbox_done() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_rebuild_marks_only_current_board_outbox_done")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "other", "b_other")?;

    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default first"),
    )?;
    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other between default events"),
    )?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default second"),
    )?;

    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done", "done"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "other")?,
        vec!["pending"]
    );
    Ok(())
}

#[test]
fn tantivy_sync_marks_only_current_board_outbox_done() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_marks_only_current_board_outbox_done")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "other", "b_other")?;
    let default = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default indexed"),
    )?;
    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;

    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other pending during default sync"),
    )?;
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
    )?;

    kanban_sqlite::api::sync_search_index(&temp.path, "default")?;

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done", "done"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "other")?,
        vec!["pending"]
    );
    Ok(())
}

#[test]
fn tantivy_sync_failure_marks_only_current_board_outbox_failed() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_failure_marks_only_current_board_outbox_failed")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "other", "b_other")?;
    let default = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default indexed before failure"),
    )?;
    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;

    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other pending during default failure"),
    )?;
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
    )?;
    std::fs::write(
        temp.dir.join("index/v1/tasks/kb-index-meta.json"),
        b"not json",
    )?;

    let err = result_err(kanban_sqlite::api::sync_search_index(&temp.path, "default"))?;
    assert!(err.to_string().contains("expected ident") || err.to_string().contains("JSON"));

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done", "failed"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "other")?,
        vec!["pending"]
    );
    Ok(())
}

#[test]
fn tantivy_rebuild_searches_task_aggregate_and_keeps_sqlite_hydration_filters() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("tantivy_rebuild_searches_task_aggregate_and_keeps_sqlite_hydration_filters")?;
    init_database(&temp.path, "tester")?;

    let title = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Tantivy title comet".into(),
            description: Some("plain ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 1,
            scheduled_at: None,
            due_at: Some(1_767_312_000_000),
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    mark_ready_fixture(&temp.path, &title.id)?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    mark_ready_fixture(&temp.path, &description.id)?;
    let comment = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Comment task"),
    )?;
    create_comment(
        &temp.path,
        &comment.id,
        "tester",
        "comment comet payload",
        None,
    )?;
    let run = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Run task"),
    )?;
    let event = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Event task"),
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Archived comet"),
    )?;

    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, summary, error, metadata_json) VALUES (?1, ?2, ?3, 'failed', 'token', 'tester', 1, 1, ?4, NULL, '{}')",
        params![new_run_id(), board_id, run.id, "run comet summary"],
    )
    ?;
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, 'task.comet.event', 'tester', ?4, 1)",
        params![kanban_core::new_event_id(), board_id, event.id, "{\"note\":\"event comet payload\"}"],
    )
    ?;
    drop(conn);
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;

    let status = kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
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
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )?;
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

    let hydrated = get_task(&temp.path, "default", &results.hits[0].task_id)?;
    assert_ne!(hydrated.status, TaskStatus::Archived);

    let filtered = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![TaskStatus::Ready],
            labels: vec![],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )?;
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
    init_database(&temp.path, "tester")?;

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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    mark_ready_fixture(&temp.path, &archive_candidate.id)?;
    mark_ready_fixture(&temp.path, &status_candidate.id)?;
    mark_ready_fixture(&temp.path, &assignee_candidate.id)?;
    let assignee_candidate = get_task(&temp.path, "default", &assignee_candidate.id)?;

    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
    let indexed = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![TaskStatus::Ready],
            labels: vec![],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(indexed.meta.backend, "tantivy");
    assert_eq!(indexed.hits.len(), 3);

    archive_task(
        &temp.path,
        "default",
        "tester",
        &archive_candidate.id,
        false,
    )?;
    claim_task(
        &temp.path,
        "default",
        "worker",
        &status_candidate.id,
        300_000,
    )?;
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
    )?;

    let filtered = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![TaskStatus::Ready],
            labels: vec![],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;

    assert_eq!(filtered.meta.backend, "sqlite");
    assert!(filtered.meta.stale);
    assert!(
        filtered
            .meta
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );
    assert!(filtered.hits.is_empty(), "{:?}", filtered.hits);
    Ok(())
}

#[test]
fn tantivy_rebuild_persists_search_state_in_app_settings() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_rebuild_persists_search_state_in_app_settings")?;
    init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("state comet"),
    )?;

    let status = kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
    assert_eq!(status.backend, "tantivy");
    assert_eq!(status.index_lag_events, Some(0));

    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    let state_json: String = conn.query_row(
        "SELECT value_json FROM app_settings WHERE key=?1",
        [format!("search.tasks.state.{board_id}")],
        |row| row.get(0),
    )?;
    let state: serde_json::Value = serde_json::from_str(&state_json)?;
    assert_eq!(state["schema_version"], 1);
    assert_eq!(state["backend"], "tantivy");
    assert_eq!(state["index_name"], "tasks");
    assert_eq!(state["dirty"], false);
    assert_eq!(state["last_event_id"].as_i64(), status.last_event_id);

    let derived = derived_store_statuses(&temp.path)?;
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .ok_or_else(|| test_error("missing tantivy_tasks derived store status"))?;
    assert_eq!(Some(tantivy.last_event_id), status.last_event_id);
    assert!(!tantivy.dirty);
    assert!(tantivy.last_rebuild_at.is_some());
    assert!(tantivy.last_error.is_none());
    let jobs = list_outbox(
        &temp.path,
        kanban_sqlite::api::OutboxListOptions {
            status: Some("done".to_owned()),
            limit: 10,
        },
    )?;
    assert!(jobs.iter().any(|job| job.target == "tantivy"));
    Ok(())
}

#[test]
fn tantivy_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending() -> anyhow::Result<()> {
    let temp =
        TempDb::new("tantivy_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "second", "b_second")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default board task"),
    )?;
    create_task(
        &temp.path,
        "second",
        "tester",
        CreateTask::ready("second board task"),
    )?;

    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done"]
    );
    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "second")?,
        vec!["pending"]
    );
    let derived = derived_store_statuses(&temp.path)?;
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .ok_or_else(|| test_error("missing tantivy_tasks derived store status"))?;
    assert!(
        tantivy.dirty,
        "second board still has pending Tantivy outbox"
    );

    kanban_sqlite::api::rebuild_search_index(&temp.path, "second")?;

    assert_eq!(
        tantivy_outbox_statuses_for_board(&temp.path, "second")?,
        vec!["done"]
    );
    let derived = derived_store_statuses(&temp.path)?;
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .ok_or_else(|| test_error("missing tantivy_tasks derived store status"))?;
    assert!(!tantivy.dirty);
    Ok(())
}

#[test]
fn tantivy_sync_reindexes_task_comment_run_event_and_archive_changes() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_sync_reindexes_task_comment_run_event_and_archive_changes")?;
    init_database(&temp.path, "tester")?;

    let updated = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("updated source"),
    )?;
    let commented = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("comment source"),
    )?;
    let run_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("run source"),
    )?;
    let event_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("event source"),
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archive syncneedle"),
    )?;
    for task in [&updated, &commented, &run_task, &event_task, &archived] {
        mark_ready_fixture(&temp.path, &task.id)?;
    }
    let updated = get_task(&temp.path, "default", &updated.id)?;

    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
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
    )?;
    create_comment(
        &temp.path,
        &commented.id,
        "tester",
        "comment syncneedle",
        None,
    )?;
    let claim = claim_task(&temp.path, "default", "worker", &run_task.id, 300_000)?;
    kanban_sqlite::api::complete_task_with_summary(
        &temp.path,
        "default",
        "worker",
        &run_task.id,
        Some(&claim.claim_token),
        false,
        Some("run syncneedle"),
    )?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, 'task.sync.event', 'tester', ?4, 1)",
        params![
            kanban_core::new_event_id(),
            board_id,
            event_task.id,
            "{\"note\":\"event syncneedle\"}"
        ],
    )
    ?;
    drop(conn);
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;

    let stale = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("syncneedle".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )?;
    assert_eq!(stale.meta.backend, "sqlite");
    assert!(stale.meta.stale);
    assert!(
        stale
            .meta
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );

    let sync = kanban_sqlite::api::sync_search_index(&temp.path, "default")?;
    assert_eq!(sync.backend, "tantivy");
    assert!(!sync.stale);
    assert_eq!(sync.index_lag_events, Some(0));
    let derived = derived_store_statuses(&temp.path)?;
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .ok_or_else(|| test_error("missing tantivy_tasks derived store status"))?;
    assert_eq!(Some(tantivy.last_event_id), sync.last_event_id);
    assert!(!tantivy.dirty);
    assert!(tantivy.last_sync_at.is_some());

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("syncneedle".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )?;
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
    init_database(&temp.path, "tester")?;
    let base = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("rollback base comet"),
    )?;
    kanban_sqlite::api::checkpoint_database(&temp.path)?;
    let snapshot = temp.dir.join("rollback-snapshot.db");
    std::fs::copy(&temp.path, &snapshot)?;

    let future = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("rollback future phantom"),
    )?;
    let rebuilt = kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
    assert_eq!(rebuilt.backend, "tantivy");
    assert!(!rebuilt.stale);

    std::fs::copy(&snapshot, &temp.path)?;
    let _ = std::fs::remove_file(temp.path.with_extension("db-wal"));
    let _ = std::fs::remove_file(temp.path.with_extension("db-shm"));

    assert!(get_task(&temp.path, "default", &future.id).is_err());
    let stale = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("phantom".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(stale.meta.backend, "sqlite");
    assert!(stale.meta.stale);
    assert!(
        stale
            .meta
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );
    assert!(stale.hits.is_empty(), "{:?}", stale.hits);

    let status = kanban_sqlite::api::search_index_status(&temp.path, "default")?;
    assert_eq!(status.backend, "sqlite");
    assert!(status.derived_index);
    assert!(status.stale);
    assert!(
        status
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );
    assert!(
        status.message.contains("ahead of the database"),
        "{}",
        status.message
    );

    let synced = kanban_sqlite::api::sync_search_index(&temp.path, "default")?;
    assert_eq!(synced.backend, "tantivy");
    assert!(!synced.stale);
    assert_eq!(synced.index_lag_events, Some(0));

    let repaired = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("comet".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(repaired.meta.backend, "tantivy");
    assert!(!repaired.meta.stale);
    assert_eq!(repaired.hits.len(), 1);
    assert_eq!(repaired.hits[0].task_id, base.id);
    Ok(())
}

#[test]
fn tantivy_state_metadata_mismatch_falls_back_and_sync_rebuilds() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_state_metadata_mismatch_falls_back_and_sync_rebuilds")?;
    init_database(&temp.path, "tester")?;
    let base = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("contract base comet"),
    )?;
    let clean = kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
    assert_eq!(clean.backend, "tantivy");
    assert!(!clean.stale);
    kanban_sqlite::api::checkpoint_database(&temp.path)?;
    let snapshot = temp.dir.join("contract-snapshot.db");
    std::fs::copy(&temp.path, &snapshot)?;

    let future = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("contract future phantom"),
    )?;
    let advanced = kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
    assert!(advanced.last_event_id > clean.last_event_id);

    std::fs::copy(&snapshot, &temp.path)?;
    let _ = std::fs::remove_file(temp.path.with_extension("db-wal"));
    let _ = std::fs::remove_file(temp.path.with_extension("db-shm"));

    assert!(get_task(&temp.path, "default", &future.id).is_err());
    let stale = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("phantom".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(stale.meta.backend, "sqlite");
    assert!(stale.meta.stale);
    assert!(
        stale
            .meta
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );
    assert!(stale.hits.is_empty(), "{:?}", stale.hits);

    let status = kanban_sqlite::api::search_index_status(&temp.path, "default")?;
    assert_eq!(status.backend, "sqlite");
    assert!(status.derived_index);
    assert!(status.stale);
    assert!(
        status
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );
    assert!(
        status
            .message
            .contains("mismatched search state and metadata watermarks"),
        "{}",
        status.message
    );

    let synced = kanban_sqlite::api::sync_search_index(&temp.path, "default")?;
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
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
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
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("watermark source"),
    )?;
    let rebuilt = kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;
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
    )?;
    std::fs::write(
        temp.dir.join("index/v1/tasks/kb-index-meta.json"),
        b"not json",
    )?;

    let err = result_err(kanban_sqlite::api::sync_search_index(&temp.path, "default"))?;
    assert!(err.to_string().contains("expected ident") || err.to_string().contains("JSON"));

    let status = kanban_sqlite::api::search_index_status(&temp.path, "default")?;
    assert_eq!(status.last_event_id, rebuilt.last_event_id);
    assert!(status.stale);
    assert!(
        status
            .index_lag_events
            .ok_or_else(|| test_error("expected index lag events"))?
            > 0
    );
    let derived = derived_store_statuses(&temp.path)?;
    let tantivy = derived
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .ok_or_else(|| test_error("missing tantivy_tasks derived store status"))?;
    assert_eq!(Some(tantivy.last_event_id), rebuilt.last_event_id);
    assert!(tantivy.dirty);
    assert!(tantivy.last_error.is_some());
    Ok(())
}

#[test]
fn tantivy_missing_or_corrupt_index_falls_back_to_sqlite() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_missing_or_corrupt_index_falls_back_to_sqlite")?;
    init_database(&temp.path, "tester")?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    let index_dir = temp.dir.join("index/v1/tasks");
    std::fs::create_dir_all(&index_dir)?;
    std::fs::write(index_dir.join("kb-index-meta.json"), b"not json")?;

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("nebula".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;

    assert_eq!(results.meta.backend, "sqlite");
    assert!(results.meta.stale);
    assert_eq!(results.hits[0].task_id, task.id);
    Ok(())
}

#[test]
fn tantivy_status_degrades_metadata_only_index_dir() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_status_degrades_metadata_only_index_dir")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    drop(conn);

    let index_dir = temp.dir.join("index/v1/tasks");
    std::fs::create_dir_all(&index_dir)?;
    std::fs::write(
        index_dir.join("kb-index-meta.json"),
        format!(r#"{{"index_version":"tasks-v1","board_id":"{board_id}","last_event_id":null}}"#),
    )?;

    let status = kanban_sqlite::api::search_index_status(&temp.path, "default")?;
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
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    drop(conn);

    let index_dir = temp.dir.join("index/v1/tasks");
    std::fs::create_dir_all(&index_dir)?;
    let mut builder = tantivy::schema::Schema::builder();
    builder.add_text_field("board_id", tantivy::schema::STRING);
    tantivy::Index::create_in_dir(&index_dir, builder.build())?;
    std::fs::write(
        index_dir.join("kb-index-meta.json"),
        format!(r#"{{"index_version":"tasks-v1","board_id":"{board_id}","last_event_id":null}}"#),
    )?;

    let status = kanban_sqlite::api::search_index_status(&temp.path, "default")?;
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
    init_database(&temp.path, "tester")?;
    let percent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal 100% complete"),
    )?;
    let underscore = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal snake_case token"),
    )?;
    let backslash = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal path C:\\work"),
    )?;
    let quote = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("literal quote \" token"),
    )?;

    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;

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
                labels: vec![],
                assignee: None,
                include_archived: false,
                limit: 10,
                offset: 0,
            },
        )?;
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

#[test]
fn tantivy_exact_task_ref_searches_fall_back_to_sqlite_after_rebuild() -> anyhow::Result<()> {
    let temp = TempDb::new("tantivy_exact_task_ref_searches_fall_back_to_sqlite_after_rebuild")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;

    let first = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("first exact ref task"),
    )?;
    mark_ready_fixture(&temp.path, &first.id)?;
    let mentions_one = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("title mentions 1 but is not task one"),
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived exact ref task"),
    )?;
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;
    let other = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other board same seq"),
    )?;

    kanban_sqlite::api::rebuild_search_index(&temp.path, "default")?;

    for query in ["1", "#1", "default#1", "default/#1", first.id.as_str()] {
        let results = search_tasks(
            &temp.path,
            kanban_search::SearchQuery {
                board: "default".into(),
                q: Some(query.to_owned()),
                statuses: vec![TaskStatus::Ready],
                labels: vec![],
                assignee: None,
                include_archived: false,
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(results.meta.backend, "sqlite", "{query}");
        assert!(results.meta.stale, "{query}");
        assert_eq!(
            results
                .hits
                .iter()
                .map(|hit| hit.task_id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str()],
            "{query}"
        );
    }

    let numeric_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("1".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    assert!(
        !numeric_results
            .hits
            .iter()
            .any(|hit| hit.task_id == mentions_one.id)
    );

    for query in ["other#1", "other/#1", other.id.as_str(), "#3"] {
        let results = search_tasks(
            &temp.path,
            kanban_search::SearchQuery {
                board: "default".into(),
                q: Some(query.to_owned()),
                statuses: vec![],
                labels: vec![],
                assignee: None,
                include_archived: false,
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(results.meta.backend, "sqlite", "{query}");
        assert_eq!(
            results
                .hits
                .iter()
                .map(|hit| hit.task_id.as_str())
                .collect::<Vec<_>>(),
            Vec::<&str>::new(),
            "{query}"
        );
    }

    let archived_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("#3".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: true,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(
        archived_results
            .hits
            .iter()
            .map(|hit| hit.task_id.as_str())
            .collect::<Vec<_>>(),
        vec![archived.id.as_str()]
    );
    Ok(())
}
