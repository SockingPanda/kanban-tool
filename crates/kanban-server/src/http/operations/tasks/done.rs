use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::CompleteTaskCommand;
use kanban_core::KanbanError;
use kanban_protocol::{CompleteTaskPath, CompleteTaskRequest, CompleteTaskResponse};

pub(crate) async fn complete_task(
    State(state): State<AppState>,
    Path(CompleteTaskPath { task_id }): Path<CompleteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<CompleteTaskRequest>, JsonRejection>,
) -> Result<Json<CompleteTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .complete_task(CompleteTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            force: body.force,
            summary: body.summary,
            result: body.result,
        })
        .await?;
    Ok(Json(CompleteTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/complete",
        post(complete_task),
    )
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn task_done_closes_the_running_application_path() {
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
                    "task_id": "t_http_done",
                    "idempotency_key": "http-done",
                    "title": "HTTP done",
                    "description": "done specification",
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
                "/api/v1/tasks/t_http_done/execution-plan/not-required",
                serde_json::json!({"reason": "single action", "actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_done/transitions/promote",
                serde_json::json!({"actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_done/transitions/claim",
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
                    "/api/v1/tasks/t_http_done/transitions/complete",
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
                "/api/v1/tasks/t_http_done/transitions/complete",
                serde_json::json!({
                    "actor": "worker",
                    "claim_token": claim.data.claim_token,
                    "summary": "done through HTTP",
                    "result": {"ok": true}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let completed: CompleteTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(completed.data.status, ApiTaskStatus::Done);
        assert_eq!(completed.data.claim_owner, None);
        assert_eq!(completed.data.claim_expires_at, None);
        assert_eq!(completed.data.last_heartbeat_at, None);
        assert_eq!(
            completed.data.current_run_id.as_deref(),
            Some(claim.data.run.id.as_str())
        );
        assert_eq!(completed.data.completed_at, Some(completed.data.updated_at));
        assert_eq!(
            completed.data.result_summary.as_deref(),
            Some("done through HTTP")
        );
        assert_eq!(completed.data.result, Some(serde_json::json!({"ok": true})));
        assert_eq!(
            completed.data.lock_version,
            claim.data.task.lock_version + 1
        );
    }
}
