use crate::common::*;

#[tokio::test]
async fn transitions_claim_returns_token_run_and_running_task() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "claim me").context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"actor":"worker-a","ttl_ms":300000,"worker_profile":"profile-a"}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["task"]["status"], "running");
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]["task"]);
    let token = json["data"]["claim_token"]
        .as_str()
        .context("claim token")?;
    assert!(token.starts_with("claim_"));
    let run = &json["data"]["run"];
    assert!(run["id"].as_str().context("run id")?.starts_with("r_"));
    assert_eq!(run["task_id"], task.id);
    assert_eq!(run["status"], "running");
    assert_eq!(run["worker_profile"], "profile-a");
    Ok(())
}

#[tokio::test]
async fn transitions_claim_and_heartbeat_reject_nonpositive_ttl_with_bad_request()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "api ttl validation")
        .context("task")?;
    let app = test.router();

    for ttl_ms in [0, -1] {
        let (status, json) = post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/transitions/claim", task.id),
            json!({"actor":"worker-a","ttl_ms":ttl_ms}),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_input");
    }

    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 1_000)
        .context("claim")?;
    for ttl_ms in [0, -1] {
        let (status, json) = post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
            json!({"claim_token":claim.claim_token,"ttl_ms":ttl_ms}),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_input");
    }
    Ok(())
}

#[tokio::test]
async fn transitions_reject_unknown_json_fields_in_mutation_bodies() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("strict json"),
    )?;
    let app = test.router();

    let cases = [
        ("promote", json!({"actor":"tester","actro":"typo"})),
        (
            "specify",
            json!({"actor":"tester","description":"ready","descripton":"typo"}),
        ),
        ("complete", json!({"force":true,"summmary":"typo"})),
        ("reopen", json!({"reason":"retry","reeason":"typo"})),
        ("block", json!({"reason":"waiting","reeason":"typo"})),
        ("archive", json!({"force":true,"froce":"typo"})),
    ];

    for (transition, body) in cases {
        let (status, json) = post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/transitions/{transition}", task.id),
            body,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{transition}: {json}");
        assert_eq!(
            json["error"]["code"], "invalid_input",
            "{transition}: {json}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn transitions_reopen_done_task_requires_reason_and_recomputes_children() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent = create_ready_task_for_test(&db_path, "default", "seed", "reopen parent")
        .context("parent")?;
    let child =
        create_ready_task_for_test(&db_path, "default", "seed", "reopen child").context("child")?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &parent.id, &child.id)?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &parent.id, 60_000)?;
    kanban_sqlite::complete_task_with_summary_and_result(
        &db_path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
        Some("api done"),
        Some(r#"{"api":true}"#),
    )?;
    kanban_sqlite::promote_task(&db_path, "default", "seed", &child.id)?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reopen", parent.id),
        json!({"reason":""}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/reopen", parent.id),
        json!({"actor":"api-user","reason":"run again"}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");
    assert_eq!(json["data"]["completed_at"], serde_json::Value::Null);
    assert_eq!(json["data"]["result_summary"], "api done");
    assert_eq!(json["data"]["result_json"], r#"{"api":true}"#);
    let child = kanban_sqlite::get_task_by_id_global(&db_path, &child.id)?;
    assert_eq!(child.status, kanban_core::TaskStatus::Todo);
    assert!(child.dependency_blocked);
    Ok(())
}

#[tokio::test]
async fn transitions_heartbeat_extends_claim_and_rejects_bad_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "heartbeat me").context("task")?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 1_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":claim.claim_token,"ttl_ms":300000,"note":"still running"}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "running");
    assert!(
        json["data"]["last_heartbeat_at"]
            .as_i64()
            .context("value")?
            >= claim.task.last_heartbeat_at.context("value")?
    );
    assert!(json["data"].get("claim_token").is_none());
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong","ttl_ms":300000}),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "claim_token_mismatch");
    Ok(())
}

#[tokio::test]
async fn transitions_complete_moves_running_done_and_leaves_child_todo() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent =
        create_ready_task_for_test(&db_path, "default", "seed", "parent").context("parent")?;
    let child = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
        std::slice::from_ref(&parent.id),
    )
    .context("child")?;
    assert_eq!(child.status, kanban_core::TaskStatus::Todo);
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &parent.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/complete", parent.id),
        json!({"claim_token":claim.claim_token,"summary":"done","force":false}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");
    let child = kanban_sqlite::get_task_by_id_global(&db_path, &child.id).context("child")?;
    assert_eq!(child.status, kanban_core::TaskStatus::Todo);
    assert!(!child.dependency_blocked);
    Ok(())
}

#[tokio::test]
async fn transitions_submit_review_moves_running_to_review_and_review_is_not_claimable()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "review me").context("task")?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"needs review"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "review");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"ttl_ms":60000}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
    Ok(())
}

#[tokio::test]
async fn transitions_block_unblock_recomputes_target_status() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "parent".into(),
            description: Some("spec".into()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )
    .context("parent")?;
    let child = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked child"),
        std::slice::from_ref(&parent.id),
    )
    .context("child")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/block", child.id),
        json!({"reason":"waiting"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "blocked");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/unblock", child.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "todo");
    Ok(())
}

#[tokio::test]
async fn transitions_archive_hides_task_from_default_list() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("archive me"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/archive", task.id),
        json!({"force":false}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "archived");

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().context("value")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn specify_transition_recomputes_triage_to_ready_scheduled_and_todo() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let ready_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "needs spec".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )
    .context("ready task")?;
    mark_plan_not_required_for_test(&db_path, "default", "seed", &ready_task.id)?;
    let scheduled_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "needs schedule".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )
    .context("scheduled task")?;
    mark_plan_not_required_for_test(&db_path, "default", "seed", &scheduled_task.id)?;
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "parent".into(),
            description: Some("not done".into()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )
    .context("parent")?;
    let todo_task = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "blocked spec".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
        std::slice::from_ref(&parent.id),
    )
    .context("todo task")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", ready_task.id),
        json!({"description":"ready spec"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let future = future_epoch_ms()?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", scheduled_task.id),
        json!({"description":"scheduled spec","scheduled_at":future}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "scheduled");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/specify", todo_task.id),
        json!({"description":"todo spec"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "todo");
    Ok(())
}

#[tokio::test]
async fn reclaim_transition_force_and_expired_close_active_run() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let force_task = create_ready_task_for_test(&db_path, "default", "seed", "force reclaim")
        .context("force task")?;
    let expired_task = create_ready_task_for_test(&db_path, "default", "seed", "expired reclaim")
        .context("expired task")?;
    let maxed_task = create_ready_task_for_test(&db_path, "default", "seed", "max retries reclaim")
        .context("maxed task")?;
    let force_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &force_task.id, 60_000)
            .context("force claim")?;
    let expired_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &expired_task.id, 60_000)
            .context("expired claim")?;
    let maxed_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &maxed_task.id, 60_000)
            .context("maxed claim")?;
    let conn = kanban_sqlite::connect_file(&db_path).context("connect")?;
    for task_id in [&expired_task.id, &maxed_task.id] {
        conn.execute(
            "UPDATE tasks SET claim_expires_at=0 WHERE id=?1",
            (task_id,),
        )
        .context("expire task claim")?;
        conn.execute(
            "UPDATE task_runs SET claim_expires_at=0 WHERE task_id=?1 AND status='running'",
            (task_id,),
        )
        .context("expire run claim")?;
    }
    conn.execute(
        "UPDATE tasks SET retry_count=0, max_retries=1 WHERE id=?1",
        (&maxed_task.id,),
    )
    .context("set max retries")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", force_task.id),
        json!({"force":true}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", expired_task.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/reclaim", maxed_task.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "blocked");
    assert_eq!(json["data"]["status_reason"], "max retries reached");

    let runs = kanban_sqlite::list_runs(&db_path, "default", None).context("runs")?;
    let force_run = runs
        .iter()
        .find(|run| run.id == force_claim.run_id)
        .context("value")?;
    let expired_run = runs
        .iter()
        .find(|run| run.id == expired_claim.run_id)
        .context("value")?;
    let maxed_run = runs
        .iter()
        .find(|run| run.id == maxed_claim.run_id)
        .context("value")?;
    assert_eq!(force_run.status, "canceled");
    assert_eq!(expired_run.status, "expired");
    assert_eq!(maxed_run.status, "expired");
    Ok(())
}

#[tokio::test]
async fn complete_with_summary_stores_task_run_summary_and_result() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "complete summary")
        .context("task")?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/complete", task.id),
        json!({"claim_token":claim.claim_token,"summary":"done summary"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");

    let stored = kanban_sqlite::get_task_by_id_global(&db_path, &task.id).context("task")?;
    let runs = kanban_sqlite::list_runs(&db_path, "default", Some(&task.id)).context("runs")?;
    assert_eq!(stored.result_summary.as_deref(), Some("done summary"));
    assert_eq!(runs[0].summary.as_deref(), Some("done summary"));

    let other = create_ready_task_for_test(&db_path, "default", "seed", "reject result")
        .context("other")?;
    let other_claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &other.id, 60_000)
        .context("claim")?;
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/complete", other.id),
        json!({"claim_token":other_claim.claim_token,"result":{"ok":true}}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");
    let stored = kanban_sqlite::get_task_by_id_global(&db_path, &other.id).context("task")?;
    assert_eq!(stored.result_json.as_deref(), Some(r#"{"ok":true}"#));
    Ok(())
}

#[tokio::test]
async fn submit_review_with_summary_stores_run_summary() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "review summary")
        .context("task")?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"ready for review"}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "review");
    let runs = kanban_sqlite::list_runs(&db_path, "default", Some(&task.id)).context("runs")?;
    assert_eq!(runs[0].summary.as_deref(), Some("ready for review"));
    Ok(())
}

#[tokio::test]
async fn heartbeat_non_running_wrong_token_returns_invalid_transition_not_token_mismatch()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "heartbeat not running")
        .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong"}),
    )
    .await?;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
    Ok(())
}
