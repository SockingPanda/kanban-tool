use crate::common::*;

#[tokio::test]
async fn transitions_api_claim_returns_token_run_and_running_task() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("claim me"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"actor":"worker-a","ttl_ms":300000,"worker_profile":"profile-a"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["task"]["status"], "running");
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]["task"]);
    let token = json["data"]["claim_token"].as_str().expect("claim token");
    assert!(token.starts_with("claim_"));
    let run = &json["data"]["run"];
    assert!(run["id"].as_str().expect("run id").starts_with("r_"));
    assert_eq!(run["task_id"], task.id);
    assert_eq!(run["status"], "running");
    assert_eq!(run["worker_profile"], "profile-a");
}

#[tokio::test]
async fn transitions_api_heartbeat_extends_claim_and_rejects_bad_token() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("heartbeat me"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 1_000).expect("claim");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":claim.claim_token,"ttl_ms":300000,"note":"still running"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "running");
    assert!(
        json["data"]["last_heartbeat_at"].as_i64().unwrap()
            >= claim.task.last_heartbeat_at.unwrap()
    );
    assert!(json["data"].get("claim_token").is_none());
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong","ttl_ms":300000}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "claim_token_mismatch");
}

#[tokio::test]
async fn transitions_api_complete_moves_running_done_and_promotes_child() {
    let (_dir, db_path) = temp_db();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )
    .expect("parent");
    let child = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
        std::slice::from_ref(&parent.id),
    )
    .expect("child");
    assert_eq!(child.status, kanban_core::TaskStatus::Todo);
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &parent.id, 60_000)
        .expect("claim");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/complete", parent.id),
        json!({"claim_token":claim.claim_token,"summary":"done","force":false}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");
    let child = kanban_sqlite::get_task_by_id_global(&db_path, &child.id).expect("child");
    assert_eq!(child.status, kanban_core::TaskStatus::Ready);
}

#[tokio::test]
async fn transitions_api_submit_review_moves_running_to_review_and_review_is_not_claimable() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("review me"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"needs review"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "review");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"ttl_ms":60000}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
}

#[tokio::test]
async fn transitions_api_block_unblock_recomputes_target_status() {
    let (_dir, db_path) = temp_db();
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
            metadata_json: "{}".into(),
        },
    )
    .expect("parent");
    let child = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked child"),
        std::slice::from_ref(&parent.id),
    )
    .expect("child");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/block", child.id),
        json!({"reason":"waiting"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "blocked");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/unblock", child.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "todo");
}

#[tokio::test]
async fn transitions_api_archive_hides_task_from_default_list() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("archive me"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/archive", task.id),
        json!({"force":false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "archived");

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn specify_transition_recomputes_triage_to_ready_scheduled_and_todo() {
    let (_dir, db_path) = temp_db();
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
            metadata_json: "{}".into(),
        },
    )
    .expect("ready task");
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
            metadata_json: "{}".into(),
        },
    )
    .expect("scheduled task");
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
            metadata_json: "{}".into(),
        },
    )
    .expect("parent");
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
            metadata_json: "{}".into(),
        },
        std::slice::from_ref(&parent.id),
    )
    .expect("todo task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", ready_task.id),
        json!({"description":"ready spec"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let future = future_epoch_ms();
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", scheduled_task.id),
        json!({"description":"scheduled spec","scheduled_at":future}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "scheduled");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/specify", todo_task.id),
        json!({"description":"todo spec"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "todo");
}

#[tokio::test]
async fn reclaim_transition_force_and_expired_close_active_run() {
    let (_dir, db_path) = temp_db();
    let force_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("force reclaim"),
    )
    .expect("force task");
    let expired_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("expired reclaim"),
    )
    .expect("expired task");
    let maxed_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("max retries reclaim"),
    )
    .expect("maxed task");
    let force_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &force_task.id, 60_000)
            .expect("force claim");
    let expired_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &expired_task.id, -1)
            .expect("expired claim");
    let maxed_claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &maxed_task.id, -1)
        .expect("maxed claim");
    let conn = kanban_sqlite::connect_file(&db_path).expect("connect");
    conn.execute(
        "UPDATE tasks SET retry_count=0, max_retries=1 WHERE id=?1",
        (&maxed_task.id,),
    )
    .expect("set max retries");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", force_task.id),
        json!({"force":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", expired_task.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/reclaim", maxed_task.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "blocked");
    assert_eq!(json["data"]["status_reason"], "max retries reached");

    let runs = kanban_sqlite::list_runs(&db_path, "default", None).expect("runs");
    let force_run = runs
        .iter()
        .find(|run| run.id == force_claim.run_id)
        .unwrap();
    let expired_run = runs
        .iter()
        .find(|run| run.id == expired_claim.run_id)
        .unwrap();
    let maxed_run = runs
        .iter()
        .find(|run| run.id == maxed_claim.run_id)
        .unwrap();
    assert_eq!(force_run.status, "canceled");
    assert_eq!(expired_run.status, "expired");
    assert_eq!(maxed_run.status, "expired");
}

#[tokio::test]
async fn complete_with_summary_stores_task_run_summary_and_result() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("complete summary"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/complete", task.id),
        json!({"claim_token":claim.claim_token,"summary":"done summary"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");

    let stored = kanban_sqlite::get_task_by_id_global(&db_path, &task.id).expect("task");
    let runs = kanban_sqlite::list_runs(&db_path, "default", Some(&task.id)).expect("runs");
    assert_eq!(stored.result_summary.as_deref(), Some("done summary"));
    assert_eq!(runs[0].summary.as_deref(), Some("done summary"));

    let other = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject result"),
    )
    .expect("other");
    let other_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &other.id, 60_000).expect("claim");
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/complete", other.id),
        json!({"claim_token":other_claim.claim_token,"result":{"ok":true}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");
    let stored = kanban_sqlite::get_task_by_id_global(&db_path, &other.id).expect("task");
    assert_eq!(stored.result_json.as_deref(), Some(r#"{"ok":true}"#));
}

#[tokio::test]
async fn submit_review_with_summary_stores_run_summary() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("review summary"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"ready for review"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "review");
    let runs = kanban_sqlite::list_runs(&db_path, "default", Some(&task.id)).expect("runs");
    assert_eq!(runs[0].summary.as_deref(), Some("ready for review"));
}

#[tokio::test]
async fn heartbeat_non_running_wrong_token_returns_invalid_transition_not_token_mismatch() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("heartbeat not running"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong"}),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
}
