use serde::{Serialize, de::DeserializeOwned};

use crate::common::*;

fn assert_request_dto_matches_fixture<T: Serialize>(
    dto: T,
    raw_fixture: &str,
) -> anyhow::Result<()> {
    let actual = serde_json::to_value(dto)?;
    let expected: Value = serde_json::from_str(raw_fixture)?;
    assert_eq!(actual, expected);
    Ok(())
}

fn committed_request_fixture<T: DeserializeOwned>(raw_fixture: &str) -> anyhow::Result<Value> {
    let _: T = serde_json::from_str(raw_fixture)?;
    Ok(serde_json::from_str(raw_fixture)?)
}

fn triage_task(
    db_path: &std::path::Path,
    title: &str,
) -> anyhow::Result<kanban_sqlite::api::TaskRecord> {
    Ok(kanban_sqlite::api::create_task(
        db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: title.to_owned(),
            description: None,
            status: None,
            assignee: None,
            priority: 2,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )?)
}

fn unix_time_ms() -> anyhow::Result<i64> {
    Ok(i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}

async fn post_json_response_text(
    app: axum::Router,
    uri: &str,
    body: Value,
) -> anyhow::Result<(StatusCode, String)> {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .context("request")?,
        )
        .await
        .context("response")?;
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .context("body")?
        .to_bytes();
    Ok((status, String::from_utf8(body.to_vec())?))
}

macro_rules! request_producer_witness {
    ($name:ident, $ty:ty, $dto:expr, $fixture:literal) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            let dto: $ty = $dto;
            assert_request_dto_matches_fixture(dto, include_str!($fixture))
        }
    };
}

request_producer_witness!(
    specify_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::SpecifyTaskRequest,
    kanban_contract::SpecifyTaskRequest {
        actor: Some("fixture-specifier".to_owned()),
        description: Some("fixture specification".to_owned()),
        scheduled_at: None,
    },
    "../../../../schemas/fixtures/api/specify-task-request.v1.valid.json"
);
request_producer_witness!(
    promote_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::PromoteTaskRequest,
    kanban_contract::PromoteTaskRequest {
        actor: Some("fixture-promoter".to_owned()),
    },
    "../../../../schemas/fixtures/api/promote-task-request.v1.valid.json"
);
request_producer_witness!(
    claim_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::ClaimTaskRequest,
    kanban_contract::ClaimTaskRequest {
        actor: Some("fixture-worker".to_owned()),
        ttl_ms: 300_000,
        worker_profile: Some("fixture-profile".to_owned()),
        metadata: Some(json!({"source": "schema-fixture"})),
    },
    "../../../../schemas/fixtures/api/claim-task-request.v1.valid.json"
);
request_producer_witness!(
    reclaim_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::ReclaimTaskRequest,
    kanban_contract::ReclaimTaskRequest {
        actor: Some("fixture-reclaimer".to_owned()),
        force: true,
        to_status: Some(kanban_contract::ReclaimTargetStatus::Ready),
        reason: Some("claim expired".to_owned()),
    },
    "../../../../schemas/fixtures/api/reclaim-task-request.v1.valid.json"
);
request_producer_witness!(
    heartbeat_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::HeartbeatTaskRequest,
    kanban_contract::HeartbeatTaskRequest {
        actor: Some("fixture-heartbeat".to_owned()),
        claim_token: "ct_fixture".to_owned(),
        ttl_ms: 300_000,
        note: Some("still running".to_owned()),
    },
    "../../../../schemas/fixtures/api/heartbeat-task-request.v1.valid.json"
);
request_producer_witness!(
    complete_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::CompleteTaskRequest,
    kanban_contract::CompleteTaskRequest {
        actor: Some("fixture-completer".to_owned()),
        claim_token: None,
        force: true,
        summary: Some("fixture complete".to_owned()),
        result: Some(json!({"ok": true, "details": [1, "stable"]})),
    },
    "../../../../schemas/fixtures/api/complete-task-request.v1.valid.json"
);
request_producer_witness!(
    submit_review_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::SubmitReviewTaskRequest,
    kanban_contract::SubmitReviewTaskRequest {
        actor: Some("fixture-reviewer".to_owned()),
        claim_token: None,
        force: true,
        summary: Some("fixture review".to_owned()),
    },
    "../../../../schemas/fixtures/api/submit-review-task-request.v1.valid.json"
);
request_producer_witness!(
    block_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::BlockTaskRequest,
    kanban_contract::BlockTaskRequest {
        actor: Some("fixture-blocker".to_owned()),
        reason: "fixture blocked".to_owned(),
        claim_token: None,
        force: false,
    },
    "../../../../schemas/fixtures/api/block-task-request.v1.valid.json"
);
request_producer_witness!(
    unblock_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::UnblockTaskRequest,
    kanban_contract::UnblockTaskRequest {
        actor: Some("fixture-unblocker".to_owned()),
    },
    "../../../../schemas/fixtures/api/unblock-task-request.v1.valid.json"
);
request_producer_witness!(
    reopen_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::ReopenTaskRequest,
    kanban_contract::ReopenTaskRequest {
        actor: Some("fixture-reopener".to_owned()),
        reason: "fixture retry".to_owned(),
    },
    "../../../../schemas/fixtures/api/reopen-task-request.v1.valid.json"
);
request_producer_witness!(
    archive_task_request_dto_serializes_to_committed_fixture,
    kanban_contract::ArchiveTaskRequest,
    kanban_contract::ArchiveTaskRequest {
        actor: Some("fixture-archiver".to_owned()),
        force: false,
    },
    "../../../../schemas/fixtures/api/archive-task-request.v1.valid.json"
);
request_producer_witness!(
    archive_board_request_dto_serializes_to_committed_fixture,
    kanban_contract::ArchiveBoardRequest,
    kanban_contract::ArchiveBoardRequest {
        actor: Some("fixture-board-archiver".to_owned()),
    },
    "../../../../schemas/fixtures/api/archive-board-request.v1.valid.json"
);
request_producer_witness!(
    add_dependency_request_dto_serializes_to_committed_fixture,
    kanban_contract::AddDependencyRequest,
    kanban_contract::AddDependencyRequest {
        parent_task_id: "t_fixture_parent".to_owned(),
        actor: Some("fixture-dependency".to_owned()),
    },
    "../../../../schemas/fixtures/api/add-dependency-request.v1.valid.json"
);

#[tokio::test]
async fn specify_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::SpecifyTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/specify-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = triage_task(test.db_path(), "specify fixture")?;
    mark_plan_not_required_for_test(test.db_path(), "default", "seed", &task.id)?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/specify", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "ready");
    Ok(())
}

#[tokio::test]
async fn promote_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::PromoteTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/promote-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "promote fixture".to_owned(),
            description: Some("ready specification".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 2,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/promote", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT, "{response}");
    assert_eq!(response["error"]["code"], "execution_plan_required");
    Ok(())
}

#[tokio::test]
async fn claim_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::ClaimTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/claim-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "claim fixture")?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["task"]["status"], "running");
    assert_eq!(response["data"]["run"]["worker_profile"], "fixture-profile");
    let runs = kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&task.id))?;
    assert_eq!(
        serde_json::from_str::<Value>(&runs[0].metadata_json)?,
        json!({"source": "schema-fixture"})
    );
    Ok(())
}

#[tokio::test]
async fn reclaim_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::ReclaimTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/reclaim-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "reclaim fixture")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &task.id, 60_000)?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "ready");
    Ok(())
}

#[tokio::test]
async fn heartbeat_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let mut body = committed_request_fixture::<kanban_contract::HeartbeatTaskRequest>(
        include_str!("../../../../schemas/fixtures/api/heartbeat-task-request.v1.valid.json"),
    )?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "heartbeat fixture")?;
    let claim =
        kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &task.id, 60_000)?;
    body["claim_token"] = json!(claim.claim_token);
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "running");
    assert!(response["data"].get("claim_token").is_none());
    Ok(())
}

#[tokio::test]
async fn complete_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::CompleteTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/complete-task-request.v1.valid.json"
    ))?;
    let expected_result = body["result"].clone();
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "complete fixture")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &task.id, 60_000)?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/complete", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "done");
    let stored = kanban_sqlite::api::get_task_by_id_global(test.db_path(), &task.id)?;
    assert_eq!(
        serde_json::from_str::<Value>(stored.result_json.as_deref().context("result_json")?)?,
        expected_result
    );
    Ok(())
}

#[tokio::test]
async fn submit_review_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::SubmitReviewTaskRequest>(
        include_str!("../../../../schemas/fixtures/api/submit-review-task-request.v1.valid.json"),
    )?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "review fixture")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &task.id, 60_000)?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "review");
    Ok(())
}

#[tokio::test]
async fn block_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::BlockTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/block-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "block fixture")?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/block", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "blocked");
    Ok(())
}

#[tokio::test]
async fn unblock_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::UnblockTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/unblock-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "unblock fixture")?;
    kanban_sqlite::api::block_task(
        test.db_path(),
        "default",
        "seed",
        &task.id,
        "fixture setup",
        None,
        false,
    )?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/unblock", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "ready");
    Ok(())
}

#[tokio::test]
async fn reopen_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::ReopenTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/reopen-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "reopen fixture")?;
    let claim =
        kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &task.id, 60_000)?;
    kanban_sqlite::api::complete_task_with_summary_and_result(
        test.db_path(),
        "default",
        "worker",
        &task.id,
        Some(&claim.claim_token),
        false,
        Some("fixture done"),
        None,
    )?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/reopen", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "ready");
    Ok(())
}

#[tokio::test]
async fn archive_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::ArchiveTaskRequest>(include_str!(
        "../../../../schemas/fixtures/api/archive-task-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "archive fixture")?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/archive", task.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "archived");
    Ok(())
}

#[tokio::test]
async fn archive_board_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body = committed_request_fixture::<kanban_contract::ArchiveBoardRequest>(include_str!(
        "../../../../schemas/fixtures/api/archive-board-request.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture-board".to_owned(),
            name: "Fixture Board".to_owned(),
            description: None,
        },
    )?;
    let (status, response) =
        post_json(test.router(), "/api/v1/boards/fixture-board/archive", body).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["data"]["archived_at"].is_i64());
    Ok(())
}

#[tokio::test]
async fn add_dependency_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let mut body = committed_request_fixture::<kanban_contract::AddDependencyRequest>(
        include_str!("../../../../schemas/fixtures/api/add-dependency-request.v1.valid.json"),
    )?;
    let test = TestApp::new()?;
    let parent = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("fixture parent"),
    )?;
    let child = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("fixture child"),
    )?;
    body["parent_task_id"] = json!(parent.id);
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        body,
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["parents"][0]["id"], parent.id);
    Ok(())
}

#[tokio::test]
async fn lifecycle_invalid_fixtures_reach_real_extractors_and_return_400() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();
    let cases = [
        (
            "/api/v1/tasks/t_missing/transitions/specify",
            include_str!("../../../../schemas/fixtures/api/specify-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/promote",
            include_str!("../../../../schemas/fixtures/api/promote-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/claim",
            include_str!("../../../../schemas/fixtures/api/claim-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/reclaim",
            include_str!("../../../../schemas/fixtures/api/reclaim-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/heartbeat",
            include_str!("../../../../schemas/fixtures/api/heartbeat-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/complete",
            include_str!("../../../../schemas/fixtures/api/complete-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/submit-review",
            include_str!(
                "../../../../schemas/fixtures/api/submit-review-task-request.v1.invalid.json"
            ),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/block",
            include_str!("../../../../schemas/fixtures/api/block-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/unblock",
            include_str!("../../../../schemas/fixtures/api/unblock-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/reopen",
            include_str!("../../../../schemas/fixtures/api/reopen-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/transitions/archive",
            include_str!("../../../../schemas/fixtures/api/archive-task-request.v1.invalid.json"),
        ),
        (
            "/api/v1/boards/missing/archive",
            include_str!("../../../../schemas/fixtures/api/archive-board-request.v1.invalid.json"),
        ),
        (
            "/api/v1/tasks/t_missing/dependencies",
            include_str!("../../../../schemas/fixtures/api/add-dependency-request.v1.invalid.json"),
        ),
    ];

    for (uri, raw_fixture) in cases {
        let (status, response) = request_raw_json(app.clone(), "POST", uri, raw_fixture).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {response}");
        assert_eq!(response["error"]["code"], "invalid_input", "{uri}");
    }
    Ok(())
}

#[tokio::test]
async fn lifecycle_handler_defaults_preserve_ttl_and_force_guards() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let claim_task =
        create_ready_task_for_test(test.db_path(), "default", "seed", "default claim ttl")?;
    let before = unix_time_ms()?;
    let (status, response) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/claim", claim_task.id),
        json!({"actor": "default-ttl-worker"}),
    )
    .await?;
    let after = unix_time_ms()?;
    assert_eq!(status, StatusCode::OK, "{response}");
    let expires_at = response["data"]["task"]["claim_expires_at"]
        .as_i64()
        .context("claim_expires_at")?;
    assert!(
        (before + 300_000..=after + 300_000).contains(&expires_at),
        "claim ttl default drift: before={before} after={after} expires_at={expires_at}"
    );

    let heartbeat_task =
        create_ready_task_for_test(test.db_path(), "default", "seed", "default heartbeat ttl")?;
    let heartbeat_claim = kanban_sqlite::api::claim_task(
        test.db_path(),
        "default",
        "worker",
        &heartbeat_task.id,
        1_000,
    )?;
    let before = unix_time_ms()?;
    let (status, response) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/heartbeat", heartbeat_task.id),
        json!({"claim_token": heartbeat_claim.claim_token}),
    )
    .await?;
    let after = unix_time_ms()?;
    assert_eq!(status, StatusCode::OK, "{response}");
    let expires_at = response["data"]["claim_expires_at"]
        .as_i64()
        .context("heartbeat claim_expires_at")?;
    assert!(
        (before + 300_000..=after + 300_000).contains(&expires_at),
        "heartbeat ttl default drift: before={before} after={after} expires_at={expires_at}"
    );

    let reclaim =
        create_ready_task_for_test(test.db_path(), "default", "seed", "reclaim force guard")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &reclaim.id, 60_000)?;
    let (status, _) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", reclaim.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);

    let complete =
        create_ready_task_for_test(test.db_path(), "default", "seed", "complete force guard")?;
    let complete_claim =
        kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &complete.id, 60_000)?;
    let complete_before = complete_claim.task.clone();
    let complete_run_before =
        kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&complete.id))?
            .into_iter()
            .find(|run| run.id == complete_claim.run_id)
            .context("complete running run")?;
    let (status, response) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/complete", complete.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{response}");
    assert_eq!(response["error"]["code"], "claim_token_mismatch");
    assert_eq!(
        kanban_sqlite::api::get_task_by_id_global(test.db_path(), &complete.id)?,
        complete_before
    );
    assert_eq!(
        kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&complete.id))?
            .into_iter()
            .find(|run| run.id == complete_claim.run_id)
            .context("complete running run after default request")?,
        complete_run_before
    );
    let (status, response) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/complete", complete.id),
        json!({"force": true}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "done");

    let submit_review =
        create_ready_task_for_test(test.db_path(), "default", "seed", "review force guard")?;
    let review_claim = kanban_sqlite::api::claim_task(
        test.db_path(),
        "default",
        "worker",
        &submit_review.id,
        60_000,
    )?;
    let review_before = review_claim.task.clone();
    let review_run_before =
        kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&submit_review.id))?
            .into_iter()
            .find(|run| run.id == review_claim.run_id)
            .context("submit-review running run")?;
    let (status, response) = post_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/transitions/submit-review",
            submit_review.id
        ),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "{response}");
    assert_eq!(response["error"]["code"], "claim_token_mismatch");
    assert_eq!(
        kanban_sqlite::api::get_task_by_id_global(test.db_path(), &submit_review.id)?,
        review_before
    );
    assert_eq!(
        kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&submit_review.id))?
            .into_iter()
            .find(|run| run.id == review_claim.run_id)
            .context("submit-review running run after default request")?,
        review_run_before
    );
    let (status, response) = post_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/transitions/submit-review",
            submit_review.id
        ),
        json!({"force": true}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "review");

    let block = create_ready_task_for_test(test.db_path(), "default", "seed", "block force guard")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &block.id, 60_000)?;
    let (status, _) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/block", block.id),
        json!({"reason": "must not bypass token guard"}),
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let archive =
        create_ready_task_for_test(test.db_path(), "default", "seed", "archive force guard")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &archive.id, 60_000)?;
    let (status, _) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/archive", archive.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn wrong_token_errors_never_echo_supplied_or_actual_claim_tokens() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    for (transition, mut body) in [
        ("heartbeat", json!({})),
        ("complete", json!({})),
        ("submit-review", json!({})),
        ("block", json!({"reason": "合法阻塞原因"})),
    ] {
        let task = create_ready_task_for_test(
            test.db_path(),
            "default",
            "seed",
            &format!("{transition} token privacy fixture"),
        )?;
        let claim =
            kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &task.id, 60_000)?;
        let task_before = claim.task.clone();
        let run_before = kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&task.id))?
            .into_iter()
            .find(|run| run.id == claim.run_id)
            .with_context(|| format!("{transition} running run"))?;
        let actual_token = claim.claim_token;
        let wrong_token = format!("ct_supplied_wrong_token_{transition}");
        body["claim_token"] = json!(wrong_token);

        let (status, response_body) = post_json_response_text(
            app.clone(),
            &format!("/api/v1/tasks/{}/transitions/{transition}", task.id),
            body,
        )
        .await?;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{transition}: {response_body}"
        );
        let response: Value = serde_json::from_str(&response_body)?;
        assert_eq!(
            response["error"]["code"], "claim_token_mismatch",
            "{transition}: {response_body}"
        );
        assert!(
            !response_body.contains(&wrong_token),
            "{transition}: {response_body}"
        );
        assert!(
            !response_body.contains(&actual_token),
            "{transition}: {response_body}"
        );
        assert_eq!(
            kanban_sqlite::api::get_task_by_id_global(test.db_path(), &task.id)?,
            task_before,
            "{transition} wrong-token 不得改变 task"
        );
        assert_eq!(
            kanban_sqlite::api::list_runs(test.db_path(), "default", Some(&task.id))?
                .into_iter()
                .find(|run| run.id == claim.run_id)
                .with_context(|| format!("{transition} running run after wrong-token"))?,
            run_before,
            "{transition} wrong-token 不得改变 run"
        );
    }
    Ok(())
}

#[tokio::test]
async fn lifecycle_request_actor_precedence_remains_body_then_header_then_default()
-> anyhow::Result<()> {
    let test = TestApp::with_actor("fixture-default")?;
    let app = test.router();
    let mut actual = Vec::new();
    for (title, body_actor, header_actor, expected) in [
        (
            "body actor",
            Some("fixture-body"),
            Some("fixture-header"),
            "fixture-body",
        ),
        (
            "header actor",
            None,
            Some("fixture-header"),
            "fixture-header",
        ),
        ("default actor", None, None, "fixture-default"),
    ] {
        let task = create_ready_task_for_test(test.db_path(), "default", "seed", title)?;
        let mut body = json!({"reason": "actor precedence"});
        if let Some(actor) = body_actor {
            body["actor"] = json!(actor);
        }
        let (status, response) = request_json(
            app.clone(),
            "POST",
            &format!("/api/v1/tasks/{}/transitions/block", task.id),
            Some(body),
            header_actor,
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{response}");
        let event = kanban_sqlite::api::list_events(test.db_path(), "default", Some(&task.id))?
            .into_iter()
            .find(|event| event.kind == "task.blocked")
            .context("task.blocked event")?;
        actual.push(event.actor);
        assert_eq!(actual.last().and_then(Option::as_deref), Some(expected));
    }
    Ok(())
}

#[tokio::test]
async fn lifecycle_optional_request_bodies_preserve_missing_content_type_semantics()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let blocked =
        create_ready_task_for_test(test.db_path(), "default", "seed", "optional unblock")?;
    kanban_sqlite::api::block_task(
        test.db_path(),
        "default",
        "seed",
        &blocked.id,
        "setup",
        None,
        false,
    )?;
    let (status, response) = request_json(
        app.clone(),
        "POST",
        &format!("/api/v1/tasks/{}/transitions/unblock", blocked.id),
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");

    let archived =
        create_ready_task_for_test(test.db_path(), "default", "seed", "optional archive")?;
    let (status, response) = request_json(
        app.clone(),
        "POST",
        &format!("/api/v1/tasks/{}/transitions/archive", archived.id),
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");

    let reclaim =
        create_ready_task_for_test(test.db_path(), "default", "seed", "optional reclaim")?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &reclaim.id, 60_000)?;
    let (status, response) = request_json(
        app.clone(),
        "POST",
        &format!("/api/v1/tasks/{}/transitions/reclaim", reclaim.id),
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT, "{response}");

    let promote = triage_task(test.db_path(), "optional promote")?;
    let (status, response) = request_json(
        app.clone(),
        "POST",
        &format!("/api/v1/tasks/{}/transitions/promote", promote.id),
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT, "{response}");

    kanban_sqlite::api::create_board(
        test.db_path(),
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "optional-board".to_owned(),
            name: "Optional Board".to_owned(),
            description: None,
        },
    )?;
    let (status, response) = request_json(
        app,
        "POST",
        "/api/v1/boards/optional-board/archive",
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    Ok(())
}
