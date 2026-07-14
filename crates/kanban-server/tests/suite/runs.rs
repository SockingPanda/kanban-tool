use crate::common::*;

#[tokio::test]
async fn runs_lists_task_runs_without_claim_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "runs").context("task")?;
    let claim = kanban_sqlite::api::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/runs", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"][0]["id"], claim.run_id);
    assert_eq!(json["data"][0]["status"], "running");
    assert!(json["data"][0].get("claim_token").is_none());
    assert!(json["data"][0].get("log_path").is_none());
    Ok(())
}

#[tokio::test]
async fn run_log_reads_dispatch_log_content_without_claim_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let log_dir = test.dir_path().join("logs");
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "logged run").context("task")?;
    let result = kanban_sqlite::api::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::api::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf 'hello log\\n'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::api::FinishPolicy::Done,
            on_failure: kanban_sqlite::api::FinishPolicy::Blocked,
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
    let _task = create_ready_task_for_test(&db_path, "default", "seed", "suspicious logged run")
        .context("task")?;
    let result = kanban_sqlite::api::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::api::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf 'hello log\\n'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::api::FinishPolicy::Done,
            on_failure: kanban_sqlite::api::FinishPolicy::Blocked,
            log_dir: test.dir_path().join("logs"),
        },
    )
    .context("dispatch")?;
    let run_id = result.run_id.context("run id")?;
    kanban_test_support::connect_file(&db_path)
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
    let _task = create_ready_task_for_test(&db_path, "default", "seed", "large logged run")
        .context("task")?;
    let result = kanban_sqlite::api::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::api::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf head; python3 - <<'PY'\nprint('x' * 270000, end='')\nPY\nprintf tail"
                .into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::api::FinishPolicy::Done,
            on_failure: kanban_sqlite::api::FinishPolicy::Blocked,
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
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "profile claim").context("task")?;
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
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "get run").context("task")?;
    let claim = kanban_sqlite::api::claim_task(&db_path, "default", "worker", &task.id, 60_000)
        .context("claim")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/runs/{}", claim.run_id)).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["id"], claim.run_id);
    assert_eq!(json["data"]["task_id"], task.id);
    assert!(json["data"].get("claim_token").is_none());
    assert!(json["data"].get("log_path").is_none());
    Ok(())
}

#[tokio::test]
async fn runs_fail_closed_for_malformed_persisted_metadata() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "malformed run metadata")?;
    let claim = kanban_sqlite::api::claim_task(&db_path, "default", "worker", &task.id, 60_000)?;
    let conn = kanban_test_support::connect_file(&db_path)?;
    conn.execute_batch("PRAGMA ignore_check_constraints = ON")?;
    conn.execute(
        "UPDATE task_runs SET metadata_json = '{invalid' WHERE id = ?1",
        [claim.run_id.as_str()],
    )?;
    drop(conn);

    let (status, json) = get_json(test.router(), &format!("/api/v1/runs/{}", claim.run_id)).await?;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"]["code"], "internal");
    assert!(
        json["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("metadata_json")),
        "{json}"
    );
    Ok(())
}

#[tokio::test]
async fn runs_preserve_board_isolation_and_archived_history_without_private_paths()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    let default_task = create_ready_task_for_test(&db, "default", "seed", "default run")?;
    let other_task = create_ready_task_for_test(&db, "other", "seed", "other run")?;
    let default_claim =
        kanban_sqlite::api::claim_task(&db, "default", "worker", &default_task.id, 60_000)?;
    let other_claim =
        kanban_sqlite::api::claim_task(&db, "other", "worker", &other_task.id, 60_000)?;
    kanban_sqlite::api::archive_task(&db, "default", "seed", &default_task.id, true)?;
    let app = test.router();

    let (status, list) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/runs", default_task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["data"].as_array().context("runs")?.len(), 1);
    assert_eq!(list["data"][0]["id"], default_claim.run_id);
    assert_ne!(list["data"][0]["id"], other_claim.run_id);
    assert!(list["data"][0].get("claim_token").is_none());
    assert!(list["data"][0].get("log_path").is_none());

    let (status, get) = get_json(app, &format!("/api/v1/runs/{}", default_claim.run_id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(get["data"]["status"], "canceled");
    assert!(get["data"].get("claim_token").is_none());
    assert!(get["data"].get("log_path").is_none());
    Ok(())
}

#[tokio::test]
async fn run_endpoints_return_not_found_for_unknown_identity() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();
    for path in ["/api/v1/tasks/t_missing/runs", "/api/v1/runs/r_missing"] {
        let (status, body) = get_json(app.clone(), path).await?;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }
    Ok(())
}
