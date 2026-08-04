use std::{future::Future, net::SocketAddr};

use axum::{
    Router,
    http::{HeaderValue, Method, header},
    routing::{get, post},
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    handlers::{
        claim_task, create_task, get_task, health, heartbeat_task, list_board_columns, list_boards,
        list_tasks, mark_execution_plan_not_required, promote_task, release_task,
        submit_review_task,
    },
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/boards", get(list_boards))
        .route("/api/v1/boards/:board/columns", get(list_board_columns))
        .route(
            "/api/v1/boards/:board/tasks",
            get(list_tasks).post(create_task),
        )
        .route("/api/v1/tasks/:task_id", get(get_task))
        .route(
            "/api/v1/tasks/:task_id/execution-plan/not-required",
            post(mark_execution_plan_not_required),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/promote",
            post(promote_task),
        )
        .route("/api/v1/tasks/:task_id/transitions/claim", post(claim_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/heartbeat",
            post(heartbeat_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/release",
            post(release_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/submit-review",
            post(submit_review_task),
        )
        .layer(desktop_cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    serve_with_shutdown(addr, state, std::future::pending()).await
}

pub async fn serve_with_shutdown<S>(
    addr: SocketAddr,
    state: AppState,
    shutdown: S,
) -> std::io::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "kanban serve only accepts a loopback address",
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

fn desktop_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:1420"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("https://tauri.localhost"),
            HeaderValue::from_static("tauri://localhost"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::HeaderName::from_static("x-kb-actor"),
        ])
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use kanban_contract::{
        ApiErrorCode, ApiExecutionPlanState, ApiRunStatus, ApiTaskStatus, ClaimTaskResponse,
        CreateTaskResponse, ErrorEnvelope, GetTaskResponse, HeartbeatTaskResponse,
        ListBoardColumnsResponse, ListBoardsResponse, ListTasksResponse,
        MarkExecutionPlanNotRequiredResponse, PromoteTaskResponse, ReleaseTaskResponse,
        SubmitReviewTaskResponse,
    };
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn board_queries_use_the_initialized_host_database() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let boards: ListBoardsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(boards.data[0].slug, "default");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/columns")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let columns: ListBoardColumnsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(columns.data.len(), 9);
    }

    #[tokio::test]
    async fn task_release_closes_the_application_path() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_release",
                    "idempotency_key": "http-release",
                    "title": "HTTP release",
                    "description": "release specification",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_release/execution-plan/not-required",
                serde_json::json!({"reason": "single action", "actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_release/transitions/promote",
                serde_json::json!({"actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_release/transitions/claim",
                serde_json::json!({"actor": "worker", "ttl_ms": 300000}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let claim: ClaimTaskResponse = serde_json::from_slice(&bytes).unwrap();
        let claim_token = claim.data.claim_token.clone();

        for body in [
            serde_json::json!({
                "actor": "worker",
                "claim_token": "wrong-token"
            }),
            serde_json::json!({
                "actor": "other-worker",
                "claim_token": claim_token
            }),
        ] {
            let response = router
                .clone()
                .oneshot(json_request(
                    "/api/v1/tasks/t_http_release/transitions/release",
                    body,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error.error.code, ApiErrorCode::ClaimTokenMismatch);
        }

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_release/transitions/release",
                serde_json::json!({
                    "actor": "worker",
                    "claim_token": claim.data.claim_token
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let released: ReleaseTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(released.data.status, ApiTaskStatus::Ready);
        assert_eq!(released.data.claim_owner, None);
        assert_eq!(released.data.claim_expires_at, None);
        assert_eq!(released.data.last_heartbeat_at, None);
        assert_eq!(released.data.current_run_id, None);
        assert_eq!(released.data.lock_version, claim.data.task.lock_version + 1);

        let response = router
            .oneshot(json_request(
                "/api/v1/tasks/t_http_release/transitions/claim",
                serde_json::json!({"actor": "second-worker", "ttl_ms": 300000}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let reclaimed: ClaimTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reclaimed.data.task.status, ApiTaskStatus::Running);
        assert_ne!(reclaimed.data.run.id, claim.data.run.id);
    }

    #[tokio::test]
    async fn task_review_closes_the_application_path() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_review",
                    "idempotency_key": "http-review",
                    "title": "HTTP review",
                    "description": "review specification",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/execution-plan/not-required",
                serde_json::json!({"reason": "single action", "actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/promote",
                serde_json::json!({"actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/claim",
                serde_json::json!({"actor": "worker", "ttl_ms": 300000}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let claim: ClaimTaskResponse = serde_json::from_slice(&bytes).unwrap();
        let claim_token = claim.data.claim_token.clone();

        for body in [
            serde_json::json!({
                "actor": "worker"
            }),
            serde_json::json!({
                "actor": "worker",
                "claim_token": "wrong-token"
            }),
            serde_json::json!({
                "actor": "worker",
                "claim_token": format!(" {} ", claim.data.claim_token)
            }),
            serde_json::json!({
                "actor": "other-worker",
                "claim_token": claim_token
            }),
        ] {
            let response = router
                .clone()
                .oneshot(json_request(
                    "/api/v1/tasks/t_http_review/transitions/submit-review",
                    body,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error.error.code, ApiErrorCode::ClaimTokenMismatch);
        }

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/submit-review",
                serde_json::json!({
                    "actor": "worker",
                    "claim_token": claim.data.claim_token,
                    "result": {"unexpected": true}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::InvalidInput);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/submit-review",
                serde_json::json!({
                    "actor": "worker",
                    "claim_token": claim.data.claim_token,
                    "summary": "ready for review"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let reviewed: SubmitReviewTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reviewed.data.status, ApiTaskStatus::Review);
        assert_eq!(reviewed.data.claim_owner, None);
        assert_eq!(reviewed.data.claim_expires_at, None);
        assert_eq!(reviewed.data.last_heartbeat_at, None);
        assert_eq!(
            reviewed.data.current_run_id.as_deref(),
            Some(claim.data.run.id.as_str())
        );
        assert_eq!(
            reviewed.data.result_summary.as_deref(),
            Some("ready for review")
        );
        assert_eq!(reviewed.data.completed_at, None);
        assert_eq!(reviewed.data.lock_version, claim.data.task.lock_version + 1);

        let response = router
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/claim",
                serde_json::json!({"actor": "dispatcher", "ttl_ms": 300000}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn task_create_closes_the_application_and_idempotency_path() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let body = serde_json::json!({
            "task_id": "t_http_create",
            "idempotency_key": "http-create-1",
            "title": "HTTP create",
            "description": "ready spec",
            "status": "ready",
            "assignee": null,
            "priority": 1,
            "scheduled_at": null,
            "due_at": null,
            "max_retries": 2,
            "metadata": {"source": "http"},
            "labels": [],
            "depends_on": [],
            "actor": "body-actor"
        });

        let response = router
            .clone()
            .oneshot(json_request("/api/v1/boards/default/tasks", body.clone()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let created: CreateTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(created.data.id, "t_http_create");
        assert_eq!(created.data.status, ApiTaskStatus::Todo);
        assert_eq!(
            created.data.execution_plan_state,
            ApiExecutionPlanState::Unplanned
        );

        let mut replay = body.clone();
        replay["task_id"] = serde_json::json!("t_http_retry");
        let response = router
            .clone()
            .oneshot(json_request("/api/v1/boards/default/tasks", replay))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let replayed: CreateTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(replayed.data.id, created.data.id);
        assert_eq!(replayed.data.seq, created.data.seq);

        let mut conflict = body;
        conflict["task_id"] = serde_json::json!("t_http_conflict");
        conflict["title"] = serde_json::json!("Different");
        let response = router
            .clone()
            .oneshot(json_request("/api/v1/boards/default/tasks", conflict))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::IdempotencyConflict);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/tasks?status=todo&priority=1&limit=25&offset=0&sort=-updated_at")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let list: ListTasksResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(list.data.len(), 1);
        assert_eq!(list.data[0].id, created.data.id);
        assert_eq!(list.meta.total, 1);
        assert_eq!(list.meta.limit, 25);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_create/execution-plan/not-required",
                serde_json::json!({
                    "reason": "small task",
                    "actor": "planner"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let plan: MarkExecutionPlanNotRequiredResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(plan.data.task_id, created.data.id);
        assert_eq!(plan.data.state, ApiExecutionPlanState::NotRequired);
        assert_eq!(plan.data.reason.as_deref(), Some("small task"));

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_create/transitions/promote",
                serde_json::json!({"actor": "promoter"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let promoted: PromoteTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(promoted.data.id, created.data.id);
        assert_eq!(promoted.data.status, ApiTaskStatus::Ready);
        assert_eq!(
            promoted.data.execution_plan_state,
            ApiExecutionPlanState::NotRequired
        );

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_http_create")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let shown: GetTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(shown.data.id, created.data.id);
        assert_eq!(shown.data.status, ApiTaskStatus::Ready);
        assert_eq!(
            shown.data.execution_plan_state,
            ApiExecutionPlanState::NotRequired
        );
        assert!(shown.meta.is_none());

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_create/transitions/claim",
                serde_json::json!({
                    "actor": "worker",
                    "ttl_ms": 300000,
                    "worker_profile": "manual",
                    "metadata": {"source": "http"}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let claim: ClaimTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(claim.data.task.status, ApiTaskStatus::Running);
        assert_eq!(claim.data.run.status, ApiRunStatus::Running);
        assert_eq!(
            claim.data.task.current_run_id.as_deref(),
            Some(claim.data.run.id.as_str())
        );
        assert_eq!(claim.data.run.worker_profile.as_deref(), Some("manual"));
        assert_eq!(
            claim.data.run.metadata,
            serde_json::json!({"source": "http"})
        );
        assert!(claim.data.claim_token.starts_with("claim_"));
        assert_eq!(
            claim.data.claim_expires_at,
            claim.data.task.claim_expires_at
        );
        let claim_token = claim.data.claim_token.clone();
        let previous_expiry = claim.data.claim_expires_at;
        let previous_lock_version = claim.data.task.lock_version;

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_create/transitions/heartbeat",
                serde_json::json!({
                    "actor": "worker",
                    "claim_token": claim_token,
                    "ttl_ms": 600000,
                    "note": "still working"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let heartbeat: HeartbeatTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(heartbeat.data.status, ApiTaskStatus::Running);
        assert_eq!(heartbeat.data.lock_version, previous_lock_version + 1);
        assert!(heartbeat.data.claim_expires_at.unwrap() > previous_expiry.unwrap());
        assert!(heartbeat.data.last_heartbeat_at.is_some());

        for body in [
            serde_json::json!({
                "actor": "worker",
                "claim_token": "wrong-token",
                "ttl_ms": 600000
            }),
            serde_json::json!({
                "actor": "different-worker",
                "claim_token": claim.data.claim_token,
                "ttl_ms": 600000
            }),
        ] {
            let response = router
                .clone()
                .oneshot(json_request(
                    "/api/v1/tasks/t_http_create/transitions/heartbeat",
                    body,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error.error.code, ApiErrorCode::ClaimTokenMismatch);
        }

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_http_create")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let after_failed_heartbeats: GetTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            after_failed_heartbeats.data.lock_version,
            heartbeat.data.lock_version
        );
        assert_eq!(
            after_failed_heartbeats.data.claim_expires_at,
            heartbeat.data.claim_expires_at
        );

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_create/transitions/claim",
                serde_json::json!({"actor": "second-worker"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::ClaimConflict);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_claim_race",
                    "idempotency_key": "http-claim-race",
                    "title": "HTTP claim race",
                    "description": "ready spec",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_claim_race/execution-plan/not-required",
                serde_json::json!({"reason": "single action", "actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_claim_race/transitions/promote",
                serde_json::json!({"actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let (first, second) = tokio::join!(
            router.clone().oneshot(json_request(
                "/api/v1/tasks/t_http_claim_race/transitions/claim",
                serde_json::json!({"actor": "worker-a"}),
            )),
            router.clone().oneshot(json_request(
                "/api/v1/tasks/t_http_claim_race/transitions/claim",
                serde_json::json!({"actor": "worker-b"}),
            ))
        );
        let first = first.unwrap();
        let second = second.unwrap();
        let statuses = [first.status(), second.status()];
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == StatusCode::OK)
                .count(),
            1
        );
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == StatusCode::CONFLICT)
                .count(),
            1
        );
        let loser = if first.status() == StatusCode::CONFLICT {
            first
        } else {
            second
        };
        let bytes = loser.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::ClaimConflict);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_http_create?include=ontology")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::FeatureNotAvailable);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::NotFound);
    }

    fn json_request(uri: &str, value: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap()
    }
}
