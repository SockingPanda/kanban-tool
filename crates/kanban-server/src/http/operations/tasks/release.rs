use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_protocol::{ReleaseTaskPath, ReleaseTaskRequest, ReleaseTaskResponse};
use kanban_service::KanbanError;
use kanban_service::ReleaseTaskCommand;

pub(crate) async fn release_task(
    State(state): State<AppState>,
    Path(ReleaseTaskPath { task_id }): Path<ReleaseTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ReleaseTaskRequest>, JsonRejection>,
) -> Result<Json<ReleaseTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .release_task(ReleaseTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
        })
        .await?;
    Ok(Json(ReleaseTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/release",
        post(release_task),
    )
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

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
}
