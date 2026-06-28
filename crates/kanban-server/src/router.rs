use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Method, header},
    middleware::{Next, from_fn_with_state},
    response::Response,
    routing::{delete, get, patch, post},
};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::invalid_input;
use crate::handlers::api::*;
use crate::handlers::health::health;
use crate::state::{AppState, SearchSyncConfig, spawn_search_sync_task};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/boards", get(list_boards).post(create_board))
        .route("/api/v1/boards/:board", get(get_board))
        .route("/api/v1/boards/:board/archive", post(archive_board))
        .route("/api/v1/boards/:board/columns", get(list_board_columns))
        .route(
            "/api/v1/boards/:board/labels",
            get(list_board_labels).post(create_board_label),
        )
        .route(
            "/api/v1/boards/:board/labels/semantics",
            get(list_label_semantics),
        )
        .route(
            "/api/v1/boards/:board/labels/:label_id/semantics",
            get(get_label_semantics)
                .put(upsert_label_semantics)
                .delete(delete_label_semantics),
        )
        .route("/api/v1/boards/:board/labels/atoms", get(list_label_atoms))
        .route(
            "/api/v1/boards/:board/labels/atoms/:atom_ref/explain",
            get(explain_label_atom),
        )
        .route(
            "/api/v1/boards/:board/labels/atom-index/status",
            get(label_atom_index_status),
        )
        .route(
            "/api/v1/boards/:board/labels/atom-index/rebuild",
            post(rebuild_label_atom_index),
        )
        .route(
            "/api/v1/boards/:board/labels/atom-index/query",
            get(query_label_atom_index),
        )
        .route(
            "/api/v1/boards/:board/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/api/v1/boards/:board/tasks/by-status",
            get(list_tasks_by_status),
        )
        .route("/api/v1/boards/:board/task-map", get(board_task_map))
        .route("/api/v1/tasks/:task_id", get(get_task).patch(update_task))
        .route(
            "/api/v1/tasks/:task_id/neighborhood",
            get(task_neighborhood),
        )
        .route(
            "/api/v1/tasks/:task_id/labels",
            get(list_task_labels).post(add_task_label),
        )
        .route(
            "/api/v1/tasks/:task_id/labels/bootstrap",
            post(bootstrap_task_label),
        )
        .route(
            "/api/v1/tasks/:task_id/labels/suggestions",
            get(suggest_task_labels),
        )
        .route(
            "/api/v1/tasks/:task_id/label-proposals",
            get(list_task_label_proposals).post(propose_task_label),
        )
        .route(
            "/api/v1/tasks/:task_id/label-ontology/observations",
            post(record_label_ontology_observation),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/signals",
            get(list_label_ontology_signals),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/review",
            get(review_label_ontology),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/actions",
            post(create_label_ontology_action),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/apply/atom",
            post(apply_label_ontology_atom),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/revert",
            post(revert_label_ontology_mutation),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/validate",
            post(validate_label_ontology_action),
        )
        .route(
            "/api/v1/label-ontology/signals/:signal_id",
            get(get_label_ontology_signal),
        )
        .route(
            "/api/v1/label-proposals/:proposal_id",
            get(get_label_proposal),
        )
        .route(
            "/api/v1/label-proposals/:proposal_id/accept",
            post(accept_label_proposal),
        )
        .route(
            "/api/v1/label-proposals/:proposal_id/reject",
            post(reject_label_proposal),
        )
        .route(
            "/api/v1/tasks/:task_id/labels/:label_id",
            delete(remove_task_label),
        )
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
            "/api/v1/tasks/:task_id/transitions/reopen",
            post(reopen_task),
        )
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
        .route(
            "/api/v1/tasks/:task_id/steps",
            get(list_steps).post(create_step),
        )
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id",
            patch(update_step).delete(remove_step),
        )
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
        .route(
            "/api/v1/tasks/:task_id/execution-plan/not-required",
            post(mark_execution_plan_not_required),
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
        .route(
            "/api/v1/search/tasks/by-status",
            get(search_tasks_by_status),
        )
        .route("/api/v1/search/status", get(search_status))
        .route("/api/v1/tasks/:task_id/context", get(build_context))
        .route("/api/v1/graph/status", get(graph_status))
        .route("/api/v1/graph/neighbors", get(graph_neighbors))
        .route("/api/v1/vector/status", get(vector_status))
        .route("/api/v1/events", get(list_events))
        .route("/api/v1/stream/events", get(stream_events))
        .route("/api/v1/maintenance/doctor", post(doctor))
        .route("/api/v1/maintenance/checkpoint", post(checkpoint))
        .layer(from_fn_with_state(state.clone(), require_existing_database))
        .with_state(state)
}

pub fn build_desktop_router(state: AppState) -> Router {
    build_router(state).layer(desktop_cors_layer())
}

pub fn build_serve_router(state: AppState) -> Router {
    build_router(state)
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
            header::HeaderName::from_static("x-kb-actor"),
        ])
}

async fn require_existing_database(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Result<Response, crate::error::ApiError> {
    if !state.db_path().is_file() {
        return Err(invalid_input(format!(
            "database file is missing: {}",
            state.db_path().display()
        )));
    }
    Ok(next.run(request).await)
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
    axum::serve(listener, build_serve_router(state)).await
}
