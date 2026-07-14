use crate::common::*;

#[tokio::test]
async fn comments_creates_and_lists_task_comments() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("commented"),
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
    assert_eq!(json["data"]["metadata"], json!({}));

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    let comments = json["data"].as_array().context("comments array")?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "handoff note");
    assert_eq!(comments[0]["author_type"], "user");

    let events =
        kanban_sqlite::api::list_events(&db_path, "default", Some(&task.id)).context("events")?;
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
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("agent comment"),
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
    assert_eq!(json["data"]["metadata"], json!({"source": "api"}));

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
async fn comments_note_metadata_decision_key_collisions_roundtrip_losslessly() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("open note metadata"),
    )?;
    let app = test.router();
    let metadata = json!({
        "selected": 7,
        "risk": null,
        "options": "opaque",
        "nested": {"keep": [true, 1]}
    });

    let (status, response) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "opaque metadata",
            "kind": "note",
            "metadata": metadata
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "response: {response}");
    assert_eq!(response["data"]["metadata"], metadata);

    let (status, response) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["data"][0]["metadata"], metadata);
    Ok(())
}

#[tokio::test]
async fn comments_signal_metadata_discriminator_collision_commits_once_and_roundtrips_losslessly()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("open signal metadata"),
    )?;
    let app = test.router();
    let metadata = json!({"type": "signal_link", "custom": true});

    let (status, response) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({
            "author": "operator",
            "body": "user-owned signal metadata",
            "kind": "signal",
            "metadata": metadata
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "response: {response}");
    assert_eq!(response["data"]["metadata"], metadata);

    let (status, response) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["data"][0]["metadata"], metadata);
    let stored = kanban_sqlite::api::list_comments(&db_path, &task.id)?;
    assert_eq!(stored.len(), 1, "create must commit exactly one comment");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored[0].metadata_json)?,
        metadata
    );
    Ok(())
}

#[tokio::test]
async fn comments_accept_decision_kind_with_default_and_agent_identity() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("decision comment"),
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
            .contains("metadata_json"),
        "response: {json}"
    );
    Ok(())
}

#[tokio::test]
async fn comments_reject_invalid_decision_metadata_schema() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("invalid decision comment"),
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

#[tokio::test]
async fn comments_identity_and_kind_matrix_preserves_service_rules() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("identity matrix"),
    )?;
    let app = test.router();
    for (label, payload, author_type, agent_type, kind) in [
        (
            "agent-null",
            json!({"author":"a","body":"agent null","author_type":"agent","agent_type":null}),
            "agent",
            serde_json::Value::Null,
            "note",
        ),
        (
            "agent-blank",
            json!({"author":"a","body":"agent blank","author_type":"agent","agent_type":"  "}),
            "agent",
            serde_json::Value::Null,
            "note",
        ),
        (
            "signal-user",
            json!({"author":"u","body":"signal user","kind":"signal","author_type":"user"}),
            "user",
            serde_json::Value::Null,
            "signal",
        ),
        (
            "signal-agent",
            json!({"author":"a","body":"signal agent","kind":"signal","author_type":"agent","agent_type":"reviewer"}),
            "agent",
            json!("reviewer"),
            "signal",
        ),
    ] {
        let (status, value) = post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/comments", task.id),
            payload,
        )
        .await?;
        assert_eq!(status, StatusCode::CREATED, "{label}: {value}");
        assert_eq!(value["data"]["author_type"], author_type, "{label}");
        assert_eq!(value["data"]["agent_type"], agent_type, "{label}");
        assert_eq!(value["data"]["kind"], kind, "{label}");
    }
    for (label, payload) in [
        (
            "unknown-author",
            json!({"author":"x","body":"bad author","author_type":"robot"}),
        ),
        (
            "unknown-kind",
            json!({"author":"x","body":"bad kind","kind":"other"}),
        ),
    ] {
        let (status, value) = post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/comments", task.id),
            payload,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{label}: {value}");
        assert_eq!(value["error"]["code"], "invalid_input", "{label}: {value}");
    }
    Ok(())
}
