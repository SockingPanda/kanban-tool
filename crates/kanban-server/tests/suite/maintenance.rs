use crate::common::*;

#[tokio::test]
async fn stats_api_reports_stale_claims_and_blocked_reason_counts() {
    let (_dir, db_path) = temp_db();
    let stale = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("stale claim"),
    )
    .expect("stale task");
    kanban_sqlite::claim_task(&db_path, "default", "worker", &stale.id, -1).expect("claim");
    let blocked_a = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked a"),
    )
    .expect("blocked a");
    let blocked_b = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked b"),
    )
    .expect("blocked b");
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &blocked_a.id,
        "waiting on operator",
        None,
        true,
    )
    .expect("block a");
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &blocked_b.id,
        "waiting on operator",
        None,
        true,
    )
    .expect("block b");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, "/api/v1/stats?board=default").await;

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
            .unwrap()
            .iter()
            .any(|count| count["status"] == "running" && count["count"] == 1)
    );
}

#[tokio::test]
async fn maintenance_api_reports_doctor_and_checkpoint_results() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(app.clone(), "/api/v1/maintenance/doctor", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["integrity_check"], "ok");
    assert_eq!(json["data"]["dependency_cycles"], 0);
    assert_eq!(json["data"]["archived_dependency_edges"], 0);
    assert_eq!(json["data"]["missing_run_logs"], 0);
    assert_eq!(json["data"]["suspicious_run_log_paths"], 0);
    assert_eq!(json["data"]["outbox_pending"], 0);
    assert_eq!(json["data"]["derived_dirty_stores"], 0);
    assert_eq!(json["data"]["derived_error_stores"], 0);
    assert_eq!(json["data"]["derived_stores"].as_array().unwrap().len(), 3);

    let (status, json) = post_json(app, "/api/v1/maintenance/checkpoint", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["busy"], 0);
    assert!(json["data"].get("checkpointed_frames").is_some());
}
