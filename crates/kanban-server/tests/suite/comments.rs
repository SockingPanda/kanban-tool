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
    assert_eq!(json["data"]["author_type"], "human");
    assert_eq!(json["data"]["agent_type"], serde_json::Value::Null);

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    let comments = json["data"].as_array().context("comments array")?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "handoff note");
    assert_eq!(comments[0]["author_type"], "human");

    let events =
        kanban_sqlite::list_events(&db_path, "default", Some(&task.id)).context("events")?;
    assert!(
        events
            .iter()
            .any(|event| event.kind == "task.comment.created")
    );
    Ok(())
}

#[tokio::test]
async fn comments_accept_agent_identity_and_reject_non_agent_type() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("agent comment"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "agent handoff",
            "kind": "worker",
            "author_type": "agent",
            "agent_type": "executor"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["kind"], "worker");
    assert_eq!(json["data"]["author_type"], "agent");
    assert_eq!(json["data"]["agent_type"], "executor");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "bad handoff",
            "author_type": "human",
            "agent_type": "executor"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("agent_type")
    );
    Ok(())
}

#[tokio::test]
async fn comments_accept_decision_kind_with_default_and_agent_identity() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("decision comment"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "Problem: choose policy. Options: a/b. Choice: a. Reason: simpler. Risk/validation: API test.",
            "kind": "decision"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["kind"], "decision");
    assert_eq!(json["data"]["author_type"], "human");
    assert_eq!(json["data"]["agent_type"], serde_json::Value::Null);

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "codex",
            "body": "Problem: choose validation. Options: CLI/API. Choice: both. Reason: both surfaces changed. Risk/validation: targeted tests.",
            "kind": "decision",
            "author_type": "agent",
            "agent_type": "codex"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["kind"], "decision");
    assert_eq!(json["data"]["author_type"], "agent");
    assert_eq!(json["data"]["agent_type"], "codex");
    Ok(())
}
