use crate::common::*;

#[tokio::test]
async fn comments_api_creates_and_lists_task_comments() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("commented"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({"author":"operator","body":"handoff note"}),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(json["data"]["id"].as_str().unwrap().starts_with("c_"));
    assert_eq!(json["data"]["task_id"], task.id);
    assert_eq!(json["data"]["author"], "operator");
    assert_eq!(json["data"]["body"], "handoff note");
    assert_eq!(json["data"]["kind"], "text");

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await;

    assert_eq!(status, StatusCode::OK);
    let comments = json["data"].as_array().expect("comments array");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "handoff note");

    let events = kanban_sqlite::list_events(&db_path, "default", Some(&task.id)).expect("events");
    assert!(
        events
            .iter()
            .any(|event| event.kind == "task.comment.created")
    );
}
