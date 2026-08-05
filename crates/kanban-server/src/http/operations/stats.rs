use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use kanban_protocol::{
    ApiTaskStatus, BlockedReasonCount, BoardQuery, DataEnvelope, QueueStats, StaleClaim,
    StatsResponse, StatusCount,
};
use kanban_service::{KanbanError, TaskStatus};

pub(crate) async fn stats(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<StatsResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    let value = state.application().get_stats(&query.board).await?;
    Ok(Json(DataEnvelope::new(QueueStats {
        board_id: value.board_id,
        generated_at: value.generated_at,
        status_counts: value
            .status_counts
            .into_iter()
            .map(|count| StatusCount {
                status: api_task_status(count.status),
                count: count.count,
            })
            .collect(),
        stale_claims: value
            .stale_claims
            .into_iter()
            .map(|claim| StaleClaim {
                task_id: claim.task_id,
                seq: claim.seq,
                title: claim.title,
                claim_owner: claim.claim_owner,
                claim_expires_at: claim.claim_expires_at,
                last_heartbeat_at: claim.last_heartbeat_at,
                current_run_id: claim.current_run_id,
                retry_count: claim.retry_count,
                max_retries: claim.max_retries,
            })
            .collect(),
        blocked_reasons: value
            .blocked_reasons
            .into_iter()
            .map(|reason| BlockedReasonCount {
                reason: reason.reason,
                count: reason.count,
            })
            .collect(),
        unplanned_active_tasks: value.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: value
            .active_parents_with_incomplete_required_steps,
    })))
}

fn api_task_status(status: TaskStatus) -> ApiTaskStatus {
    match status {
        TaskStatus::Triage => ApiTaskStatus::Triage,
        TaskStatus::Todo => ApiTaskStatus::Todo,
        TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
        TaskStatus::Ready => ApiTaskStatus::Ready,
        TaskStatus::Running => ApiTaskStatus::Running,
        TaskStatus::Blocked => ApiTaskStatus::Blocked,
        TaskStatus::Review => ApiTaskStatus::Review,
        TaskStatus::Done => ApiTaskStatus::Done,
        TaskStatus::Archived => ApiTaskStatus::Archived,
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/stats", get(stats))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn stats_returns_default_board_counts_and_missing_board_error() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let stats: StatsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(stats.data.board_id, "b_default");
        assert!(stats.data.status_counts.is_empty());
        assert!(stats.data.stale_claims.is_empty());
        assert!(stats.data.blocked_reasons.is_empty());

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/stats?board=missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::NotFound);
    }

    #[tokio::test]
    async fn stats_returns_status_counts_for_created_task() {
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
                    "task_id": "t_stats_http",
                    "title": "stats",
                    "description": "stats task",
                    "status": "todo",
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/stats?board=default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let stats: StatsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            stats.data.status_counts,
            vec![StatusCount {
                status: ApiTaskStatus::Todo,
                count: 1,
            }]
        );
    }
}
