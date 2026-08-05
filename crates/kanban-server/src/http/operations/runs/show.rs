use super::super::support::api_run;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_contract::{GetRunPath, GetRunResponse};

pub(crate) async fn get_run(
    State(state): State<AppState>,
    Path(GetRunPath { run_id }): Path<GetRunPath>,
) -> Result<Json<GetRunResponse>, ApiError> {
    let run = state.application().get_run(&run_id).await?;
    Ok(Json(GetRunResponse {
        data: api_run(run)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/runs/:run_id", get(get_run))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn run_show_reads_the_run_created_by_the_canonical_claim_path() {
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
                    "task_id": "t_run_show",
                    "title": "run show",
                    "description": null,
                    "status": "todo",
                    "assignee": null,
                    "priority": 3,
                    "scheduled_at": null,
                    "due_at": null,
                    "max_retries": 1,
                    "metadata": {"source": "run-show-test"},
                    "labels": [],
                    "depends_on": [],
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_run_show/execution-plan/not-required",
                serde_json::json!({"reason": "no steps", "actor": "tester"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_run_show/transitions/promote",
                serde_json::json!({"actor": "tester"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_run_show/transitions/claim",
                serde_json::json!({
                    "actor": "worker",
                    "ttl_ms": 300000,
                    "worker_profile": "manual",
                    "metadata": {"source": "run-show-test"}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let claim: ClaimTaskResponse = serde_json::from_slice(&bytes).unwrap();
        let run_id = claim.data.run.id.clone();
        assert_eq!(claim.data.run.status, ApiRunStatus::Running);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{run_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let shown: GetRunResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(shown.data.id, run_id);
        assert_eq!(shown.data.task_id, "t_run_show");
        assert_eq!(shown.data.status, ApiRunStatus::Running);
        assert_eq!(shown.data.worker_profile.as_deref(), Some("manual"));
        assert_eq!(
            shown.data.metadata,
            serde_json::json!({"source": "run-show-test"})
        );
        assert!(!shown.data.has_log);
    }

    #[tokio::test]
    async fn run_show_returns_standard_errors_for_missing_and_invalid_ids() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/runs/r_missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::NotFound);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/runs/default%231")
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
