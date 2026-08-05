use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::operations::{CompleteStepCommand, ReopenStepCommand, SkipStepCommand};
use kanban_contract::{
    CompleteStepPath, CompleteStepRequest, CompleteStepResponse, ReopenStepPath, ReopenStepRequest,
    ReopenStepResponse, SkipStepPath, SkipStepRequest, SkipStepResponse,
};
use kanban_core::KanbanError;

pub(crate) async fn complete_step(
    State(state): State<AppState>,
    Path(CompleteStepPath { task_id, step_id }): Path<CompleteStepPath>,
    headers: HeaderMap,
    body: Result<Json<CompleteStepRequest>, JsonRejection>,
) -> Result<Json<CompleteStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .complete_step(CompleteStepCommand {
            task_id,
            step_id,
            note: body.note,
            actor,
        })
        .await?;
    Ok(Json(CompleteStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(crate) async fn skip_step(
    State(state): State<AppState>,
    Path(SkipStepPath { task_id, step_id }): Path<SkipStepPath>,
    headers: HeaderMap,
    body: Result<Json<SkipStepRequest>, JsonRejection>,
) -> Result<Json<SkipStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .skip_step(SkipStepCommand {
            task_id,
            step_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(Json(SkipStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(crate) async fn reopen_step(
    State(state): State<AppState>,
    Path(ReopenStepPath { task_id, step_id }): Path<ReopenStepPath>,
    headers: HeaderMap,
    body: Result<Json<ReopenStepRequest>, JsonRejection>,
) -> Result<Json<ReopenStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .reopen_step(ReopenStepCommand {
            task_id,
            step_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(Json(ReopenStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id/done",
            post(complete_step),
        )
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id/skip",
            post(skip_step),
        )
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id/reopen",
            post(reopen_step),
        )
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn step_lifecycle_routes_share_one_application_and_store_path() {
        let directory = tempfile::tempdir().expect("临时数据库目录");
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .expect("打开 host state");
        let router = build_router(state);

        let task = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_step_lifecycle",
                    "idempotency_key": "http-step-lifecycle-task",
                    "title": "HTTP step lifecycle parent",
                    "description": null,
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "seed"
                }),
            ))
            .await
            .expect("创建父任务");
        assert_eq!(task.status(), StatusCode::CREATED);

        let created = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_step_lifecycle/steps",
                serde_json::json!({
                    "idempotency_key": "http-step-lifecycle",
                    "title": "lifecycle step",
                    "body": "step body",
                    "required": true,
                    "actor": "planner"
                }),
            ))
            .await
            .expect("创建 step");
        assert_eq!(created.status(), StatusCode::CREATED);
        let created_body = created.into_body().collect().await.unwrap().to_bytes();
        let created: CreateStepResponse = serde_json::from_slice(&created_body).unwrap();
        let step_id = created.data.steps[0].id.clone();

        let done = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/tasks/t_http_step_lifecycle/steps/{step_id}/done"),
                serde_json::json!({"note": "finished", "actor": "operator"}),
            ))
            .await
            .expect("完成 step");
        assert_eq!(done.status(), StatusCode::OK);
        let done_body = done.into_body().collect().await.unwrap().to_bytes();
        let done: CompleteStepResponse = serde_json::from_slice(&done_body).unwrap();
        assert_eq!(done.data.steps[0].status, ApiStepStatus::Done);
        assert_eq!(
            done.data.steps[0].resolution_note.as_deref(),
            Some("finished")
        );

        let skipped = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/tasks/t_http_step_lifecycle/steps/{step_id}/skip"),
                serde_json::json!({"reason": "not needed", "actor": "operator"}),
            ))
            .await
            .expect("跳过 step");
        assert_eq!(skipped.status(), StatusCode::OK);
        let skipped_body = skipped.into_body().collect().await.unwrap().to_bytes();
        let skipped: SkipStepResponse = serde_json::from_slice(&skipped_body).unwrap();
        assert_eq!(skipped.data.steps[0].status, ApiStepStatus::Skipped);

        let reopened = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/tasks/t_http_step_lifecycle/steps/{step_id}/reopen"),
                serde_json::json!({"reason": "needs revision", "actor": "operator"}),
            ))
            .await
            .expect("重新打开 step");
        assert_eq!(reopened.status(), StatusCode::OK);
        let reopened_body = reopened.into_body().collect().await.unwrap().to_bytes();
        let reopened: ReopenStepResponse = serde_json::from_slice(&reopened_body).unwrap();
        assert_eq!(reopened.data.steps[0].status, ApiStepStatus::Todo);
        assert_eq!(reopened.data.steps[0].resolution_note, None);

        let removed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!(
                        "/api/v1/tasks/t_http_step_lifecycle/steps/{step_id}"
                    ))
                    .header("X-KB-Actor", "operator")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("删除 step");
        assert_eq!(removed.status(), StatusCode::OK);
        let removed_body = removed.into_body().collect().await.unwrap().to_bytes();
        let removed: RemoveStepResponse = serde_json::from_slice(&removed_body).unwrap();
        assert!(removed.data.steps.is_empty());
        assert_eq!(
            removed.data.execution_plan.state,
            ApiExecutionPlanState::Unplanned
        );
    }
}
