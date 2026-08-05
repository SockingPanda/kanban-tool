use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::SubmitReviewTaskCommand;
use kanban_contract::{SubmitReviewTaskPath, SubmitReviewTaskRequest, SubmitReviewTaskResponse};
use kanban_core::KanbanError;

pub(crate) async fn submit_review_task(
    State(state): State<AppState>,
    Path(SubmitReviewTaskPath { task_id }): Path<SubmitReviewTaskPath>,
    headers: HeaderMap,
    body: Result<Json<SubmitReviewTaskRequest>, JsonRejection>,
) -> Result<Json<SubmitReviewTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .submit_review_task(SubmitReviewTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            force: body.force,
            summary: body.summary,
        })
        .await?;
    Ok(Json(SubmitReviewTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/submit-review",
        post(submit_review_task),
    )
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

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
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/claim",
                serde_json::json!({"actor": "dispatcher", "ttl_ms": 300000}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let response = router
            .oneshot(json_request(
                "/api/v1/tasks/t_http_review/transitions/complete",
                serde_json::json!({
                    "actor": "reviewer",
                    "summary": "approved",
                    "result": {"approved": true}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let completed: CompleteTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(completed.data.status, ApiTaskStatus::Done);
        assert_eq!(
            completed.data.current_run_id.as_deref(),
            Some(claim.data.run.id.as_str())
        );
        assert_eq!(completed.data.result_summary.as_deref(), Some("approved"));
        assert_eq!(
            completed.data.result,
            Some(serde_json::json!({"approved": true}))
        );
    }
}
