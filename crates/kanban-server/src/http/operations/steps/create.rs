use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use kanban_service::CreateStepCommand;
use kanban_core::KanbanError;
use kanban_protocol::{CreateStepPath, CreateStepRequest, CreateStepResponse};

pub(crate) async fn create_step(
    State(state): State<AppState>,
    Path(CreateStepPath { task_id }): Path<CreateStepPath>,
    headers: HeaderMap,
    body: Result<Json<CreateStepRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateStepResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .create_step(CreateStepCommand {
            task_id,
            idempotency_key: body.idempotency_key,
            title: body.title,
            body: body.body,
            linked_task_id: body.linked_task_ref,
            position: body.position,
            required: body.required,
            actor,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateStepResponse {
            data: api_task_steps(steps)?,
        }),
    ))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/steps", post(create_step))
}
#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn step_create_and_list_use_application_path_and_entity_local_idempotency() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let task = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_step",
                    "idempotency_key": "http-step-task",
                    "title": "HTTP step parent",
                    "description": null,
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(task.status(), StatusCode::CREATED);

        let request = serde_json::json!({
            "idempotency_key": "http-step-retry",
            "title": "first step",
            "body": "step body",
            "position": null,
            "required": true,
            "actor": "planner"
        });
        let first = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_step/steps",
                request.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_body = first.into_body().collect().await.unwrap().to_bytes();
        let first: CreateStepResponse = serde_json::from_slice(&first_body).unwrap();
        assert!(first.data.steps[0].id.starts_with("step_"));
        assert_eq!(first.data.steps[0].title, "first step");
        assert_eq!(first.data.steps[0].position, 1024);
        assert_eq!(
            first.data.execution_plan.state,
            ApiExecutionPlanState::Planned
        );

        let replay = router
            .clone()
            .oneshot(json_request("/api/v1/tasks/t_http_step/steps", request))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::CREATED);
        let replay_body = replay.into_body().collect().await.unwrap().to_bytes();
        let replay: CreateStepResponse = serde_json::from_slice(&replay_body).unwrap();
        assert_eq!(replay.data.steps, first.data.steps);

        let listed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/tasks/t_http_step/steps")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let listed_body = listed.into_body().collect().await.unwrap().to_bytes();
        let listed: ListStepsResponse = serde_json::from_slice(&listed_body).unwrap();
        assert_eq!(listed.data.steps, first.data.steps);

        let updated = router
            .clone()
            .oneshot(patch_json_request(
                &format!("/api/v1/tasks/t_http_step/steps/{}", first.data.steps[0].id),
                serde_json::json!({
                    "title": "updated step",
                    "body": null,
                    "position": 2048,
                    "required": false,
                    "actor": "reviewer"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(updated.status(), StatusCode::OK);
        let updated_body = updated.into_body().collect().await.unwrap().to_bytes();
        let updated: kanban_protocol::UpdateStepResponse =
            serde_json::from_slice(&updated_body).unwrap();
        assert_eq!(updated.data.steps[0].title, "updated step");
        assert_eq!(updated.data.steps[0].body.as_deref(), Some("step body"));
        assert_eq!(updated.data.steps[0].position, 2048);
        assert!(!updated.data.steps[0].required);
        assert_eq!(updated.data.steps[0].status, ApiStepStatus::Todo);

        let invalid = router
            .clone()
            .oneshot(patch_json_request(
                &format!("/api/v1/tasks/t_http_step/steps/{}", first.data.steps[0].id),
                serde_json::json!({
                    "linked_task_ref": "t_http_step",
                    "unlink_task": true,
                    "actor": "reviewer"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

        let conflict = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_step/steps",
                serde_json::json!({
                    "idempotency_key": "http-step-retry",
                    "title": "changed step",
                    "body": "step body",
                    "required": true,
                    "actor": "planner"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let conflict_body = conflict.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&conflict_body).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::IdempotencyConflict);
    }
}
