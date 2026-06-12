use crate::common::*;

#[tokio::test]
async fn stats_reports_stale_claims_and_blocked_reason_counts() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let stale = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("stale claim"),
    )
    .context("stale task")?;
    kanban_sqlite::claim_task(&db_path, "default", "worker", &stale.id, 60_000).context("claim")?;
    let conn = kanban_sqlite::connect_file(&db_path).context("connect")?;
    conn.execute(
        "UPDATE tasks SET claim_expires_at=0 WHERE id=?1",
        (&stale.id,),
    )
    .context("expire task claim")?;
    conn.execute(
        "UPDATE task_runs SET claim_expires_at=0 WHERE task_id=?1 AND status='running'",
        (&stale.id,),
    )
    .context("expire run claim")?;
    let blocked_a = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked a"),
    )
    .context("blocked a")?;
    let blocked_b = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked b"),
    )
    .context("blocked b")?;
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &blocked_a.id,
        "waiting on operator",
        None,
        true,
    )
    .context("block a")?;
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &blocked_b.id,
        "waiting on operator",
        None,
        true,
    )
    .context("block b")?;
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/stats?board=default").await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["stale_claims"][0]["task_id"], stale.id);
    assert_eq!(
        json["data"]["blocked_reasons"][0]["reason"],
        "waiting on operator"
    );
    assert_eq!(json["data"]["blocked_reasons"][0]["count"], 2);
    assert!(
        json["data"]["status_counts"]
            .as_array()
            .context("value")?
            .iter()
            .any(|count| count["status"] == "running" && count["count"] == 1)
    );
    Ok(())
}

#[tokio::test]
async fn maintenance_reports_doctor_and_checkpoint_results() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = post_json(app.clone(), "/api/v1/maintenance/doctor", json!({})).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["integrity_check"], "ok");
    assert_eq!(json["data"]["dependency_cycles"], 0);
    assert_eq!(json["data"]["archived_dependency_edges"], 0);
    assert_eq!(json["data"]["missing_run_logs"], 0);
    assert_eq!(json["data"]["suspicious_run_log_paths"], 0);
    assert_eq!(json["data"]["outbox_pending"], 0);
    assert_eq!(json["data"]["derived_dirty_stores"], 0);
    assert_eq!(json["data"]["derived_error_stores"], 0);
    assert_eq!(
        json["data"]["derived_stores"]
            .as_array()
            .context("value")?
            .len(),
        3
    );

    let (status, json) = post_json(app, "/api/v1/maintenance/checkpoint", json!({})).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["busy"], 0);
    assert!(json["data"].get("checkpointed_frames").is_some());
    Ok(())
}
