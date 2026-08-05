use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_service::BlockTaskCommand;
use kanban_core::KanbanError;
use kanban_protocol::{BlockTaskPath, BlockTaskRequest, BlockTaskResponse};

pub(crate) async fn block_task(
    State(state): State<AppState>,
    Path(BlockTaskPath { task_id }): Path<BlockTaskPath>,
    headers: HeaderMap,
    body: Result<Json<BlockTaskRequest>, JsonRejection>,
) -> Result<Json<BlockTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .block_task(BlockTaskCommand {
            task_id,
            actor,
            reason: body.reason,
            claim_token: body.claim_token,
            force: body.force,
        })
        .await?;
    Ok(Json(BlockTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/transitions/block", post(block_task))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn task_block_closes_non_running_and_running_application_paths() {
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
                    "task_id": "t_http_block_todo",
                    "idempotency_key": "http-block-todo",
                    "title": "HTTP block todo",
                    "description": "block specification",
                    "status": "todo",
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
        let block_fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../schemas/fixtures/api/block-task-request.v1.valid.json"
        ))
        .unwrap();
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_block_todo/transitions/block",
                block_fixture,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let blocked: BlockTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(blocked.data.status, ApiTaskStatus::Blocked);
        assert_eq!(
            blocked.data.status_reason.as_deref(),
            Some("fixture blocked")
        );
        assert_eq!(blocked.data.current_run_id, None);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_block_running",
                    "idempotency_key": "http-block-running",
                    "title": "HTTP block running",
                    "description": "block running specification",
                    "status": "todo",
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
                "/api/v1/tasks/t_http_block_running/execution-plan/not-required",
                serde_json::json!({"reason": "single action", "actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_block_running/transitions/promote",
                serde_json::json!({"actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_block_running/transitions/claim",
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
                "reason": "waiting",
                "claim_token": "wrong-token"
            }),
            serde_json::json!({
                "actor": "other-worker",
                "reason": "waiting",
                "claim_token": claim_token
            }),
        ] {
            let response = router
                .clone()
                .oneshot(json_request(
                    "/api/v1/tasks/t_http_block_running/transitions/block",
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
            .oneshot(json_request(
                "/api/v1/tasks/t_http_block_running/transitions/block",
                serde_json::json!({
                    "actor": "worker",
                    "reason": "waiting on dependency",
                    "claim_token": claim.data.claim_token
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let blocked: BlockTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(blocked.data.status, ApiTaskStatus::Blocked);
        assert_eq!(
            blocked.data.status_reason.as_deref(),
            Some("waiting on dependency")
        );
        assert_eq!(blocked.data.claim_owner, None);
        assert_eq!(blocked.data.claim_expires_at, None);
        assert_eq!(blocked.data.last_heartbeat_at, None);
        assert_eq!(
            blocked.data.current_run_id.as_deref(),
            Some(claim.data.run.id.as_str())
        );
        assert_eq!(blocked.data.lock_version, claim.data.task.lock_version + 1);
    }
}
