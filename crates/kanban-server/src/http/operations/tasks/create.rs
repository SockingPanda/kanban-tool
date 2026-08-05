use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use kanban_service::CreateTaskCommand;
use kanban_core::{KanbanError, TaskStatus, new_task_id};
use kanban_protocol::{ApiCreateTaskStatus, CreateTaskPath, CreateTaskRequest, CreateTaskResponse};

fn create_status(status: ApiCreateTaskStatus) -> TaskStatus {
    match status {
        ApiCreateTaskStatus::Triage => TaskStatus::Triage,
        ApiCreateTaskStatus::Todo => TaskStatus::Todo,
        ApiCreateTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiCreateTaskStatus::Ready => TaskStatus::Ready,
    }
}

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(CreateTaskPath { board }): Path<CreateTaskPath>,
    headers: HeaderMap,
    body: Result<Json<CreateTaskRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateTaskResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .create_task(CreateTaskCommand {
            task_id: body.task_id.unwrap_or_else(new_task_id),
            board,
            idempotency_key: body.idempotency_key,
            title: body.title,
            description: body.description,
            requested_status: body.status.map(create_status),
            assignee: body.assignee,
            priority: body.priority,
            scheduled_at: body.scheduled_at,
            due_at: body.due_at,
            max_retries: body.max_retries,
            metadata: body.metadata.unwrap_or_default(),
            labels: body.labels,
            depends_on: body.depends_on,
            actor,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            data: api_task(task)?,
        }),
    ))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards/:board/tasks", post(create_task))
}
#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn task_create_duplicate_id_without_idempotency_key_returns_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let body = serde_json::json!({
            "task_id": "t_http_duplicate_id",
            "title": "HTTP duplicate",
            "description": "duplicate id",
            "priority": 1,
            "metadata": {},
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

        let response = router
            .oneshot(json_request("/api/v1/boards/default/tasks", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::Conflict);
    }

    #[tokio::test]
    async fn task_create_duplicate_id_with_different_idempotency_keys_returns_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let body = serde_json::json!({
            "task_id": "t_http_duplicate_keyed_id",
            "idempotency_key": "http-duplicate-key-1",
            "title": "HTTP keyed duplicate",
            "description": "first payload",
            "priority": 1,
            "metadata": {},
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

        let mut conflicting = body;
        conflicting["idempotency_key"] = serde_json::json!("http-duplicate-key-2");
        conflicting["description"] = serde_json::json!("different payload");
        let response = router
            .oneshot(json_request("/api/v1/boards/default/tasks", conflicting))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::Conflict);
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
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let details: kanban_protocol::GetTaskDetailsResponse =
            serde_json::from_slice(&bytes).unwrap();
        assert_eq!(details.data.task.id, "t_http_create");
        assert!(details.data.ontology.degraded);

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
}
