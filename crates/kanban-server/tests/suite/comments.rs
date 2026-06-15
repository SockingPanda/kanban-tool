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
    assert_eq!(json["data"]["kind"], "note");
    assert_eq!(json["data"]["author_type"], "user");
    assert_eq!(json["data"]["agent_type"], serde_json::Value::Null);
    assert_eq!(json["data"]["metadata_json"], "{}");

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    let comments = json["data"].as_array().context("comments array")?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "handoff note");
    assert_eq!(comments[0]["author_type"], "user");

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
            "kind": "note",
            "author_type": "agent",
            "agent_type": "executor",
            "metadata": {"source": "api"}
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["kind"], "note");
    assert_eq!(json["data"]["author_type"], "agent");
    assert_eq!(json["data"]["agent_type"], "executor");
    assert_eq!(json["data"]["metadata_json"], r#"{"source":"api"}"#);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "bad handoff",
            "author_type": "user",
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
            "kind": "decision",
            "metadata": decision_metadata()
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["kind"], "decision");
    assert_eq!(json["data"]["author_type"], "user");
    assert_eq!(json["data"]["agent_type"], serde_json::Value::Null);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "codex",
            "body": "Problem: choose validation. Options: CLI/API. Choice: both. Reason: both surfaces changed. Risk/validation: targeted tests.",
            "kind": "decision",
            "author_type": "agent",
            "agent_type": "codex",
            "metadata": decision_metadata()
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["kind"], "decision");
    assert_eq!(json["data"]["author_type"], "agent");
    assert_eq!(json["data"]["agent_type"], "codex");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "bad metadata",
            "metadata": []
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("metadata_json")
    );
    Ok(())
}

#[tokio::test]
async fn comments_reject_invalid_decision_metadata_schema() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("invalid decision comment"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "bad decision",
            "kind": "decision",
            "metadata": {
                "options": [{"slug": "a", "title": "A", "detail": "A"}],
                "selected": "missing",
                "reason": "because"
            }
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("selected")
    );
    Ok(())
}

fn decision_metadata() -> serde_json::Value {
    json!({
        "options": [
            {
                "slug": "sqlite",
                "title": "Use SQLite metadata",
                "detail": "Keep the decision payload in task comment metadata."
            },
            {
                "slug": "table",
                "title": "Add a table",
                "detail": "Store decisions in a separate table."
            }
        ],
        "selected": "sqlite",
        "reason": "Keeps decisions local to the discussion.",
        "risk": "Schema drift would make older comments ambiguous.",
        "verification": "API tests cover valid and invalid decision comments."
    })
}
