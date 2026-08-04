use std::{future::Future, net::SocketAddr};

use axum::{
    Router,
    http::{HeaderValue, Method, header},
    routing::get,
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    handlers::{create_task, health, list_board_columns, list_boards, list_tasks},
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/boards", get(list_boards))
        .route("/api/v1/boards/:board/columns", get(list_board_columns))
        .route(
            "/api/v1/boards/:board/tasks",
            get(list_tasks).post(create_task),
        )
        .layer(desktop_cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    serve_with_shutdown(addr, state, std::future::pending()).await
}

pub async fn serve_with_shutdown<S>(
    addr: SocketAddr,
    state: AppState,
    shutdown: S,
) -> std::io::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "kanban serve only accepts a loopback address",
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

fn desktop_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:1420"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("https://tauri.localhost"),
            HeaderValue::from_static("tauri://localhost"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::HeaderName::from_static("x-kb-actor"),
        ])
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use kanban_contract::{
        ApiErrorCode, ApiExecutionPlanState, ApiTaskStatus, CreateTaskResponse, ErrorEnvelope,
        ListBoardColumnsResponse, ListBoardsResponse, ListTasksResponse,
    };
    use tower::ServiceExt;

    use super::*;

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

    #[tokio::test]
    async fn task_create_closes_the_application_and_idempotency_path() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let body = serde_json::json!({
            "task_id": "t_http_create",
            "idempotency_key": "http-create-1",
            "title": "HTTP create",
            "description": "ready spec",
            "status": "ready",
            "assignee": null,
            "priority": 1,
            "scheduled_at": null,
            "due_at": null,
            "max_retries": 2,
            "metadata": {"source": "http"},
            "labels": [],
            "depends_on": [],
            "actor": "body-actor"
        });

        let response = router
            .clone()
            .oneshot(json_request("/api/v1/boards/default/tasks", body.clone()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let created: CreateTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(created.data.id, "t_http_create");
        assert_eq!(created.data.status, ApiTaskStatus::Todo);
        assert_eq!(
            created.data.execution_plan_state,
            ApiExecutionPlanState::Unplanned
        );

        let mut replay = body.clone();
        replay["task_id"] = serde_json::json!("t_http_retry");
        let response = router
            .clone()
            .oneshot(json_request("/api/v1/boards/default/tasks", replay))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let replayed: CreateTaskResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(replayed.data.id, created.data.id);
        assert_eq!(replayed.data.seq, created.data.seq);

        let mut conflict = body;
        conflict["task_id"] = serde_json::json!("t_http_conflict");
        conflict["title"] = serde_json::json!("Different");
        let response = router
            .clone()
            .oneshot(json_request("/api/v1/boards/default/tasks", conflict))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::IdempotencyConflict);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/tasks?status=todo&priority=1&limit=25&offset=0&sort=-updated_at")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let list: ListTasksResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(list.data.len(), 1);
        assert_eq!(list.data[0].id, created.data.id);
        assert_eq!(list.meta.total, 1);
        assert_eq!(list.meta.limit, 25);
    }

    fn json_request(uri: &str, value: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap()
    }
}
