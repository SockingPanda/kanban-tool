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
    kanban_sqlite::mark_execution_plan_not_required(
        &db_path,
        "default",
        "seed",
        &stale.id,
        "stale claim fixture",
    )
    .context("mark stale not required")?;
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
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent with incomplete step"),
    )
    .context("parent")?;
    let child = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child step"),
    )
    .context("child")?;
    kanban_sqlite::attach_subtask(
        &db_path,
        "default",
        "seed",
        &parent.id,
        kanban_sqlite::AttachSubtaskInput {
            child_ref: child.id,
            position: None,
            required: true,
        },
    )
    .context("attach subtask")?;
    let unplanned = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "unplanned active".to_owned(),
            description: Some("spec".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("unplanned")?;

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
    assert_eq!(json["data"]["unplanned_active_tasks"], 4);
    assert_eq!(
        json["data"]["active_parents_with_incomplete_required_subtasks"],
        1
    );
    assert_eq!(
        kanban_sqlite::get_task(&db_path, "default", &unplanned.id)?.execution_plan_state,
        kanban_sqlite::StepPlanState::Unplanned
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
    let derived_stores = json["data"]["derived_stores"].as_array().context("value")?;
    assert_eq!(derived_stores.len(), 4);
    assert!(derived_stores.iter().any(|store| {
        store["store_name"] == "lancedb_label_atoms"
            && store["dirty"] == false
            && store["pending_outbox"] == 0
            && store["running_outbox"] == 0
            && store["failed_outbox"] == 0
    }));

    let (status, json) = post_json(app, "/api/v1/maintenance/checkpoint", json!({})).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["busy"], 0);
    assert!(json["data"].get("checkpointed_frames").is_some());
    Ok(())
}
