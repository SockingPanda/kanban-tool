use super::api_board;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use kanban_protocol::{ListBoardsQuery, ListBoardsResponse};

pub(crate) async fn list_boards(
    State(state): State<AppState>,
    Query(query): Query<ListBoardsQuery>,
) -> Result<Json<ListBoardsResponse>, ApiError> {
    let data = state
        .application()
        .list_boards(query.include_archived)
        .await?
        .into_iter()
        .map(api_board)
        .collect();
    Ok(Json(ListBoardsResponse { data }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards", get(list_boards))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn board_queries_use_the_initialized_host_database() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let boards: ListBoardsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(boards.data[0].slug, "default");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/columns")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let columns: ListBoardColumnsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(columns.data.len(), 9);
    }
}
