use crate::common::*;

#[tokio::test]
async fn comments_creates_and_lists_task_comments() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("commented"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({"author":"operator","body":"handoff note"}),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert!(
        json["data"]["id"]
            .as_str()
            .context("value")?
            .starts_with("c_")
    );
    assert_eq!(json["data"]["task_id"], task.id);
    assert_eq!(json["data"]["author"], "operator");
    assert_eq!(json["data"]["body"], "handoff note");
    assert_eq!(json["data"]["kind"], "text");

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    let comments = json["data"].as_array().context("comments array")?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "handoff note");

    let events =
        kanban_sqlite::list_events(&db_path, "default", Some(&task.id)).context("events")?;
    assert!(
        events
            .iter()
            .any(|event| event.kind == "task.comment.created")
    );
    Ok(())
}
