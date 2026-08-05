use super::super::support::api_run;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_contract::{ListRunsPath, ListRunsResponse};

pub(crate) async fn list_runs(
    State(state): State<AppState>,
    Path(ListRunsPath { task_id }): Path<ListRunsPath>,
) -> Result<Json<ListRunsResponse>, ApiError> {
    let runs = state.application().list_runs(&task_id).await?;
    let data = runs
        .into_iter()
        .map(api_run)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Json(ListRunsResponse { data }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/runs", get(list_runs))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;
    use kanban_application::ClaimTaskCommand;

    #[tokio::test]
    async fn run_list_uses_application_path_and_preserves_run_contract() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let setup_router = build_router(state.clone());

        let response = setup_router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_http_run_list",
                    "title": "HTTP run list",
                    "description": "run list test",
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

        let response = setup_router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_run_list/execution-plan/not-required",
                serde_json::json!({"reason": "single action", "actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = setup_router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_run_list/transitions/promote",
                serde_json::json!({"actor": "planner"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        state
            .application()
            .claim_task_with_run_log_dir(
                ClaimTaskCommand {
                    task_id: "t_http_run_list".into(),
                    actor: "worker".into(),
                    ttl_ms: 300000,
                    worker_profile: Some("http-worker".into()),
                    metadata: serde_json::json!({"source": "http"}),
                },
                &directory.path().join("run-logs"),
            )
            .await
            .unwrap();

        let router = build_router(state);
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_http_run_list/runs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let runs: ListRunsResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(runs.data.len(), 1);
        assert_eq!(runs.data[0].task_id, "t_http_run_list");
        assert_eq!(runs.data[0].worker_profile.as_deref(), Some("http-worker"));
        assert_eq!(runs.data[0].metadata, serde_json::json!({"source": "http"}));
        assert!(runs.data[0].has_log);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/default%231/runs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::InvalidInput);
    }
}
