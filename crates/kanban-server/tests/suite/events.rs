use crate::common::*;

#[tokio::test]
async fn events_api_after_limit_returns_ordered_events_and_next_after() {
    let (_dir, db_path) = temp_db();
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first"),
    )
    .expect("first");
    let second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("second"),
    )
    .expect("second");
    let all = kanban_sqlite::list_events(&db_path, "default", None).expect("events");
    let after = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&first.id))
        .expect("first task event")
        .id;
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&after={after}&limit=1"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let events = json["data"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["task_id"], second.id);
    assert!(events[0]["id"].as_i64().unwrap() > after);
    assert_eq!(json["meta"]["next_after"], events[0]["id"]);
    assert!(events[0]["event_id"].as_str().unwrap().starts_with("e_"));
    assert_eq!(events[0]["kind"], "task.created");
    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn events_api_filters_by_task_id_for_detail_timeline() {
    let (_dir, db_path) = temp_db();
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first timeline"),
    )
    .expect("first");
    let second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("second timeline"),
    )
    .expect("second");
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &first.id,
        "waiting on local input",
        None,
        false,
    )
    .expect("block first");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&task_id={}", first.id),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let events = json["data"].as_array().expect("events array");
    assert!(events.len() >= 2);
    assert!(
        events
            .iter()
            .all(|event| event["task_id"].as_str() == Some(first.id.as_str()))
    );
    assert!(!events.iter().any(|event| event["task_id"] == second.id));
    assert!(events.iter().any(|event| event["kind"] == "task.blocked"));
}
