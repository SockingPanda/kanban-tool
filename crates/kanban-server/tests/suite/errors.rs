use crate::common::*;

#[tokio::test]
async fn api_error_response_contract_produces_fixture() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let (status, json) = request_json_with_accept_language(
        test.router(),
        "GET",
        "/api/v1/boards/default/tasks?limit=100000",
        None,
        "zh-CN",
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let typed: kanban_contract::ErrorEnvelope = serde_json::from_value(json.clone())?;
    assert_eq!(serde_json::to_value(&typed)?, json);
    assert_eq!(
        typed.error.code,
        kanban_contract::ApiErrorCode::InvalidInput
    );
    let fixture: kanban_contract::ErrorEnvelope = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/error-response.v1.valid.json"
    ))?;
    assert_eq!(
        serde_json::to_value(&typed)?,
        serde_json::to_value(fixture)?
    );
    Ok(())
}

#[tokio::test]
async fn api_error_response_real_route_preserves_typed_json() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let (status, json) = request_json_with_accept_language(
        test.router(),
        "GET",
        "/api/v1/boards/default/tasks?limit=100000",
        None,
        "zh-CN",
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let typed: kanban_contract::ErrorEnvelope = serde_json::from_value(json.clone())?;
    assert_eq!(serde_json::to_value(&typed)?, json);
    assert_eq!(
        typed.error.code,
        kanban_contract::ApiErrorCode::InvalidInput
    );
    assert_eq!(typed.error.message, "输入无效：limit 必须小于等于 1000");
    Ok(())
}

#[tokio::test]
async fn api_error_response_is_localized_without_changing_shape() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let (status, zh) = request_json_with_accept_language(
        test.router(),
        "GET",
        "/api/v1/boards/default/tasks?limit=100000",
        None,
        "zh-CN",
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, en) = request_json_with_accept_language(
        test.router(),
        "GET",
        "/api/v1/boards/default/tasks?limit=100000",
        None,
        "en-US",
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let zh_typed: kanban_contract::ErrorEnvelope = serde_json::from_value(zh)?;
    let en_typed: kanban_contract::ErrorEnvelope = serde_json::from_value(en)?;
    assert_eq!(zh_typed.error.code, en_typed.error.code);
    assert_eq!(zh_typed.error.message, "输入无效：limit 必须小于等于 1000");
    assert_eq!(
        en_typed.error.message,
        "invalid input: limit must be <= 1000"
    );
    Ok(())
}

#[tokio::test]
async fn api_error_wrong_token_is_forbidden_and_claim_private() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "error contract conflict")?;
    let claim = kanban_sqlite::api::claim_task(&db_path, "default", "worker", &task.id, 1_000)?;
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong","ttl_ms":300000}),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let typed: kanban_contract::ErrorEnvelope = serde_json::from_value(json.clone())?;
    assert_eq!(
        typed.error.code,
        kanban_contract::ApiErrorCode::ClaimTokenMismatch
    );
    let raw = serde_json::to_string(&json)?;
    assert!(!raw.contains(&claim.claim_token));
    assert!(!raw.contains("wrong"));
    assert!(json.get("claim_token").is_none());
    assert_eq!(serde_json::to_value(&typed)?, json);
    Ok(())
}

#[test]
fn api_error_response_contract_consumes_fixture() -> anyhow::Result<()> {
    let fixture: kanban_contract::ErrorEnvelope = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/error-response.v1.valid.json"
    ))?;
    assert_eq!(
        serde_json::to_value(&fixture)?,
        serde_json::json!({"error":{"code":"invalid_input","message":"输入无效：limit 必须小于等于 1000"}})
    );
    Ok(())
}

#[tokio::test]
async fn api_error_review_task_cannot_be_claimed_returns_typed_conflict() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();
    let db_path = test.db_path().to_path_buf();
    let task = create_ready_task_for_test(&db_path, "default", "seed", "error contract review")?;
    let claim = kanban_sqlite::api::claim_task(&db_path, "default", "worker", &task.id, 300_000)?;
    let (status, _) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"needs review"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let app = test.router();
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"actor":"worker-2","ttl_ms":300000}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    let typed: kanban_contract::ErrorEnvelope = serde_json::from_value(json.clone())?;
    assert_eq!(
        typed.error.code,
        kanban_contract::ApiErrorCode::InvalidTransition
    );
    assert_eq!(serde_json::to_value(&typed)?, json);
    Ok(())
}

#[tokio::test]
async fn api_error_real_service_routes_cover_reachable_conflict_classifications()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let unplanned = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "needs plan".into(),
            description: Some("ready specification".into()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/promote", unplanned.id),
        json!({"actor":"tester"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "execution_plan_required");

    let stepped = create_ready_task_for_test(&db_path, "default", "seed", "incomplete steps")?;
    kanban_sqlite::api::create_step(
        &db_path,
        "default",
        "seed",
        &stepped.id,
        kanban_sqlite::api::CreateStepInput {
            title: "required".into(),
            body: None,
            linked_task_ref: None,
            position: None,
            required: true,
        },
    )?;
    let claim = kanban_sqlite::api::claim_task(&db_path, "default", "worker", &stepped.id, 60_000)?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/complete", stepped.id),
        json!({"claim_token":claim.claim_token,"summary":"blocked by step"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "steps_incomplete");

    let parent = create_ready_task_for_test(&db_path, "default", "seed", "blocking parent")?;
    let child = kanban_sqlite::api::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("blocked child"),
        std::slice::from_ref(&parent.id),
    )?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/promote", child.id),
        json!({"actor":"tester"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "dependency_blocked");

    Ok(())
}
