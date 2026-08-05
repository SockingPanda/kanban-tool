use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_protocol::{ApiRunLog, GetRunLogPath, GetRunLogResponse};

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/runs/:run_id/log", get(get_run_log))
}

pub(crate) async fn get_run_log(
    State(state): State<AppState>,
    Path(GetRunLogPath { run_id }): Path<GetRunLogPath>,
) -> Result<Json<GetRunLogResponse>, ApiError> {
    let log = state.application().get_run_log(&run_id).await?;
    Ok(Json(GetRunLogResponse {
        data: ApiRunLog {
            run_id: log.run_id,
            content: log.content,
            truncated: log.truncated,
        },
    }))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use crate::http::operations::test_support::*;
    use kanban_application::{
        ClaimTaskCommand, CreateTaskCommand, MarkExecutionPlanNotRequiredCommand,
        PromoteTaskCommand,
    };
    use kanban_core::TaskStatus;
    use kanban_protocol::{ApiErrorCode, ErrorEnvelope, GetRunLogResponse};

    #[tokio::test]
    async fn run_log_route_uses_the_application_and_returns_contract_shape() {
        let directory = tempfile::tempdir().unwrap();
        let log_root = tempfile::tempdir().unwrap();
        let canonical_log_root = fs::canonicalize(log_root.path()).unwrap();
        let state = AppState::open_with_run_log_root(
            directory.path().join("kanban.db"),
            "test",
            Some(canonical_log_root.clone()),
        )
        .await
        .unwrap();

        let task = state
            .application()
            .create_task(CreateTaskCommand {
                task_id: "t_http_run_log".to_owned(),
                board: "default".to_owned(),
                idempotency_key: None,
                title: "HTTP run log".to_owned(),
                description: Some("has a log".to_owned()),
                requested_status: Some(TaskStatus::Todo),
                assignee: None,
                priority: 1,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata: BTreeMap::new(),
                labels: Vec::new(),
                depends_on: Vec::new(),
                actor: "test".to_owned(),
            })
            .await
            .unwrap();
        state
            .application()
            .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
                task_id: task.id.clone(),
                reason: "small task".to_owned(),
                actor: "test".to_owned(),
            })
            .await
            .unwrap();
        state
            .application()
            .promote_task(PromoteTaskCommand {
                task_id: task.id.clone(),
                actor: "test".to_owned(),
            })
            .await
            .unwrap();
        let claim = state
            .application()
            .claim_task_with_run_log_dir(
                ClaimTaskCommand {
                    task_id: task.id,
                    actor: "worker".to_owned(),
                    ttl_ms: 60_000,
                    worker_profile: Some("test".to_owned()),
                    metadata: serde_json::json!({}),
                },
                &canonical_log_root,
            )
            .await
            .unwrap();
        let run_id = claim.run.id.clone();
        let log_path = claim.run.log_path.clone().unwrap();
        fs::write(log_path, "hello from run").unwrap();

        let router = build_router(state);
        let response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{run_id}/log"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let payload: GetRunLogResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload.data.run_id, run_id);
        assert_eq!(payload.data.content, "hello from run");
        assert!(!payload.data.truncated);
    }

    #[tokio::test]
    async fn run_log_route_maps_missing_and_invalid_ids_to_standard_envelopes() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        for (uri, expected_status, expected_code) in [
            (
                "/api/v1/runs/r_missing/log",
                StatusCode::NOT_FOUND,
                ApiErrorCode::NotFound,
            ),
            (
                "/api/v1/runs/not-a-run/log",
                StatusCode::BAD_REQUEST,
                ApiErrorCode::InvalidInput,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error.error.code, expected_code);
        }
    }
}
