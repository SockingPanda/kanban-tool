use crate::common::*;

#[tokio::test]
async fn runs_lists_task_runs_without_claim_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("runs"),
    )
    .context("task")?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/runs", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"][0]["id"], claim.run_id);
    assert_eq!(json["data"][0]["status"], "running");
    assert!(json["data"][0].get("claim_token").is_none());
    Ok(())
}

#[tokio::test]
async fn run_log_reads_dispatch_log_content_without_claim_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let log_dir = test.dir_path().join("logs");
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("logged run"),
    )
    .context("task")?;
    let result = kanban_sqlite::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf 'hello log\\n'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::FinishPolicy::Done,
            on_failure: kanban_sqlite::FinishPolicy::Blocked,
            log_dir,
        },
    )
    .context("dispatch")?;
    assert_eq!(result.task_id.as_deref(), Some(task.id.as_str()));
    let run_id = result.run_id.context("run id")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/runs/{run_id}/log")).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["run_id"], run_id);
    assert_eq!(json["data"]["content"], "hello log\n");
    assert_eq!(json["data"]["truncated"], false);
    assert!(json["data"].get("claim_token").is_none());
    Ok(())
}

#[tokio::test]
async fn run_log_rejects_suspicious_log_paths() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("suspicious logged run"),
    )
    .context("task")?;
    let result = kanban_sqlite::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf 'hello log\\n'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::FinishPolicy::Done,
            on_failure: kanban_sqlite::FinishPolicy::Blocked,
            log_dir: test.dir_path().join("logs"),
        },
    )
    .context("dispatch")?;
    let run_id = result.run_id.context("run id")?;
    kanban_sqlite::connect_file(&db_path)
        .context("connect")?
        .execute(
            "UPDATE task_runs SET log_path=?1 WHERE id=?2",
            ("/etc/passwd", run_id.as_str()),
        )
        .context("update")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/runs/{run_id}/log")).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    Ok(())
}

#[tokio::test]
async fn run_log_returns_tail_window_when_log_is_large() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let log_dir = test.dir_path().join("logs");
    kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("large logged run"),
    )
    .context("task")?;
    let result = kanban_sqlite::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf head; python3 - <<'PY'\nprint('x' * 270000, end='')\nPY\nprintf tail"
                .into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::FinishPolicy::Done,
            on_failure: kanban_sqlite::FinishPolicy::Blocked,
            log_dir,
        },
    )
    .context("dispatch")?;
    let run_id = result.run_id.context("run id")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/runs/{run_id}/log")).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["truncated"], true);
    let content = json["data"]["content"].as_str().context("value")?;
    assert!(!content.starts_with("head"), "{content}");
    assert!(content.ends_with("tail"), "{content}");
    Ok(())
}

#[tokio::test]
async fn claim_uses_requested_worker_profile_in_response_run() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("profile claim"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"worker_profile":"reviewer"}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["run"]["worker_profile"], "reviewer");
    Ok(())
}

#[tokio::test]
async fn runs_gets_run_by_id_without_claim_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("get run"),
    )
    .context("task")?;
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/runs/{}", claim.run_id)).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["id"], claim.run_id);
    assert_eq!(json["data"]["task_id"], task.id);
    assert!(json["data"].get("claim_token").is_none());
    Ok(())
}
