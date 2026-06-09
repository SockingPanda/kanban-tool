use super::*;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/boards", get(list_boards).post(create_board))
        .route("/api/v1/boards/:board", get(get_board))
        .route("/api/v1/boards/:board/archive", post(archive_board))
        .route("/api/v1/boards/:board/columns", get(list_board_columns))
        .route(
            "/api/v1/boards/:board/tasks",
            get(list_tasks).post(create_task),
        )
        .route("/api/v1/tasks/:task_id", get(get_task).patch(update_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/specify",
            post(specify_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/promote",
            post(promote_task),
        )
        .route("/api/v1/tasks/:task_id/transitions/claim", post(claim_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/reclaim",
            post(reclaim_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/heartbeat",
            post(heartbeat_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/complete",
            post(complete_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/submit-review",
            post(submit_review_task),
        )
        .route("/api/v1/tasks/:task_id/transitions/block", post(block_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/unblock",
            post(unblock_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/archive",
            post(archive_task),
        )
        .route(
            "/api/v1/tasks/:task_id/dependencies",
            get(list_dependencies).post(add_dependency),
        )
        .route(
            "/api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
            delete(remove_dependency),
        )
        .route("/api/v1/tasks/:task_id/runs", get(list_runs))
        .route("/api/v1/runs/:run_id", get(get_run))
        .route("/api/v1/runs/:run_id/log", get(get_run_log))
        .route(
            "/api/v1/tasks/:task_id/comments",
            get(list_comments).post(create_comment),
        )
        .route("/api/v1/stats", get(get_stats))
        .route("/api/v1/search/tasks", get(search_tasks))
        .route("/api/v1/search/status", get(search_status))
        .route("/api/v1/tasks/:task_id/context", get(build_context))
        .route("/api/v1/graph/status", get(graph_status))
        .route("/api/v1/graph/neighbors", get(graph_neighbors))
        .route("/api/v1/vector/status", get(vector_status))
        .route("/api/v1/events", get(list_events))
        .route("/api/v1/stream/events", get(stream_events))
        .route("/api/v1/maintenance/doctor", post(doctor))
        .route("/api/v1/maintenance/checkpoint", post(checkpoint))
        .with_state(state)
}

pub fn build_desktop_router(state: AppState) -> Router {
    build_router(state).layer(desktop_cors_layer())
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
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-kb-actor"),
        ])
}

pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    serve_with_search_sync(addr, state, SearchSyncConfig::disabled("default")).await
}

pub async fn serve_with_search_sync(
    addr: SocketAddr,
    state: AppState,
    search_sync: SearchSyncConfig,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let _search_sync_task = spawn_search_sync_task(state.clone(), search_sync);
    axum::serve(listener, build_desktop_router(state)).await
}
