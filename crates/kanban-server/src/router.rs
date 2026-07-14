use std::{collections::BTreeSet, future::Future, net::SocketAddr};

use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Method, header},
    middleware::{Next, from_fn_with_state},
    response::Response,
    routing::{MethodFilter, MethodRouter},
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::invalid_input;
use crate::handlers::api::*;
use crate::handlers::health::health;
use crate::i18n::request_locale;
use crate::observability::http_trace_layer;
use crate::state::{AppState, SearchSyncConfig, spawn_search_sync_task_until_shutdown};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ApiRouteOperation {
    operation_id: &'static str,
    adapter_id: &'static str,
    actual_method: kanban_contract::HttpMethod,
    actual_path: &'static str,
    descriptor_method: kanban_contract::HttpMethod,
    descriptor_path: &'static str,
}

struct AuditedMethodRouter<S> {
    path: &'static str,
    operations: Vec<ApiRouteOperation>,
    inner: MethodRouter<S>,
}

struct AuditedRouter<S> {
    inner: Router<S>,
    operations: BTreeSet<ApiRouteOperation>,
    operation_ids: BTreeSet<&'static str>,
    adapter_ids: BTreeSet<&'static str>,
}

impl<S> AuditedRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn new() -> Self {
        Self {
            inner: Router::new(),
            operations: BTreeSet::new(),
            operation_ids: BTreeSet::new(),
            adapter_ids: BTreeSet::new(),
        }
    }

    fn route(mut self, route: AuditedMethodRouter<S>) -> Self {
        for operation in route.operations {
            assert!(
                self.operation_ids.insert(operation.operation_id),
                "duplicate endpoint operation_id: {}",
                operation.operation_id
            );
            assert!(
                self.adapter_ids.insert(operation.adapter_id),
                "duplicate endpoint adapter_id: {}",
                operation.adapter_id
            );
            assert!(
                self.operations.insert(operation),
                "duplicate endpoint method/path: {} {}",
                endpoint_method_name(operation.actual_method),
                operation.actual_path
            );
        }
        self.inner = self.inner.route(route.path, route.inner);
        self
    }

    fn finish(self) -> (Router<S>, BTreeSet<ApiRouteOperation>) {
        (self.inner, self.operations)
    }
}

fn method_filter(method: kanban_contract::HttpMethod) -> MethodFilter {
    match method {
        kanban_contract::HttpMethod::Get => MethodFilter::GET,
        kanban_contract::HttpMethod::Post => MethodFilter::POST,
        kanban_contract::HttpMethod::Put => MethodFilter::PUT,
        kanban_contract::HttpMethod::Patch => MethodFilter::PATCH,
        kanban_contract::HttpMethod::Delete => MethodFilter::DELETE,
    }
}

fn endpoint_operation(
    operation_id: &'static str,
    adapter_id: &'static str,
    actual_path: &'static str,
    actual_method: kanban_contract::HttpMethod,
) -> ApiRouteOperation {
    let descriptor = kanban_contract::endpoint_descriptor(operation_id)
        .unwrap_or_else(|| panic!("unknown endpoint descriptor: {operation_id}"));
    assert!(
        matches!(
            descriptor.surface,
            kanban_contract::ContractSurface::Api | kanban_contract::ContractSurface::Sse
        ),
        "endpoint binding has wrong surface: {operation_id}"
    );
    assert!(
        !adapter_id.is_empty(),
        "endpoint binding requires explicit adapter_id: {operation_id}"
    );
    assert_eq!(
        actual_path, descriptor.path,
        "endpoint binding actual path drift: {operation_id}"
    );
    assert_eq!(
        actual_method, descriptor.method,
        "endpoint binding actual method drift: {operation_id}"
    );
    ApiRouteOperation {
        operation_id,
        adapter_id,
        actual_method,
        actual_path,
        descriptor_method: descriptor.method,
        descriptor_path: descriptor.path,
    }
}

fn endpoint_method_name(method: kanban_contract::HttpMethod) -> &'static str {
    match method {
        kanban_contract::HttpMethod::Get => "GET",
        kanban_contract::HttpMethod::Post => "POST",
        kanban_contract::HttpMethod::Put => "PUT",
        kanban_contract::HttpMethod::Patch => "PATCH",
        kanban_contract::HttpMethod::Delete => "DELETE",
    }
}

macro_rules! endpoint_route {
    ($first_operation:literal => $first_adapter:literal => $first_handler:ident; $($operation:literal => $adapter:literal => $handler:ident;)* ) => {{
        let first = kanban_contract::endpoint_descriptor($first_operation)
            .expect("endpoint descriptor must exist");
        let actual_path = first.path;
        AuditedMethodRouter {
            path: actual_path,
            operations: vec![
                endpoint_operation($first_operation, $first_adapter, actual_path, first.method),
                $(
                    endpoint_operation(
                        $operation,
                        $adapter,
                        actual_path,
                        kanban_contract::endpoint_descriptor($operation)
                            .expect("endpoint descriptor must exist")
                            .method,
                    ),
                )*
            ],
            inner: MethodRouter::new()
                .on(method_filter(first.method), $first_handler)
                $(.on(method_filter(kanban_contract::endpoint_descriptor($operation).expect("endpoint descriptor must exist").method), $handler))* ,
        }
    }};
}

pub fn build_router(state: AppState) -> Router {
    build_api_router(state).layer(http_trace_layer())
}

fn registered_api_routes() -> AuditedRouter<AppState> {
    AuditedRouter::new()
        .route(endpoint_route!("api.health" => "adapter.health" => health;
        ))
        .route(endpoint_route!("api.list-boards" => "adapter.list_boards" => list_boards;
            "api.create-board" => "adapter.create_board" => create_board;
        ))
        .route(endpoint_route!("api.get-board" => "adapter.get_board" => get_board;
        ))
        .route(endpoint_route!("api.archive-board" => "adapter.archive_board" => archive_board;
        ))
        .route(endpoint_route!("api.list-board-columns" => "adapter.list_board_columns" => list_board_columns;
        ))
        .route(endpoint_route!("api.list-board-labels" => "adapter.list_board_labels" => list_board_labels;
            "api.create-board-label" => "adapter.create_board_label" => create_board_label;
        ))
        .route(endpoint_route!("api.list-label-semantics" => "adapter.list_label_semantics" => list_label_semantics;
        ))
        .route(endpoint_route!("api.get-label-semantics" => "adapter.get_label_semantics" => get_label_semantics;
            "api.upsert-label-semantics" => "adapter.upsert_label_semantics" => upsert_label_semantics;
            "api.delete-label-semantics" => "adapter.delete_label_semantics" => delete_label_semantics;
        ))
        .route(endpoint_route!("api.list-label-atoms" => "adapter.list_label_atoms" => list_label_atoms;
        ))
        .route(endpoint_route!("api.explain-label-atom" => "adapter.explain_label_atom" => explain_label_atom;
        ))
        .route(endpoint_route!("api.label-atom-index-status" => "adapter.label_atom_index_status" => label_atom_index_status;
        ))
        .route(endpoint_route!("api.rebuild-label-atom-index" => "adapter.rebuild_label_atom_index" => rebuild_label_atom_index;
        ))
        .route(endpoint_route!("api.query-label-atom-index" => "adapter.query_label_atom_index" => query_label_atom_index;
        ))
        .route(endpoint_route!("api.list-tasks" => "adapter.list_tasks" => list_tasks;
            "api.create-task" => "adapter.create_task" => create_task;
        ))
        .route(endpoint_route!("api.list-tasks-by-status" => "adapter.list_tasks_by_status" => list_tasks_by_status;
        ))
        .route(endpoint_route!("api.list-signals" => "adapter.list_signals" => list_signals;
        ))
        .route(endpoint_route!("api.review-signals" => "adapter.review_signals" => review_signals;
        ))
        .route(endpoint_route!("api.get-signal" => "adapter.get_signal" => get_signal;
        ))
        .route(endpoint_route!("api.board-task-map" => "adapter.board_task_map" => board_task_map;
        ))
        .route(endpoint_route!("api.get-task" => "adapter.get_task" => get_task;
            "api.update-task" => "adapter.update_task" => update_task;
        ))
        .route(endpoint_route!("api.task-neighborhood" => "adapter.task_neighborhood" => task_neighborhood;
        ))
        .route(endpoint_route!("api.list-task-labels" => "adapter.list_task_labels" => list_task_labels;
            "api.add-task-label" => "adapter.add_task_label" => add_task_label;
        ))
        .route(endpoint_route!("api.bootstrap-task-label" => "adapter.bootstrap_task_label" => bootstrap_task_label;
        ))
        .route(endpoint_route!("api.suggest-task-labels" => "adapter.suggest_task_labels" => suggest_task_labels;
        ))
        .route(endpoint_route!("api.list-task-label-proposals" => "adapter.list_task_label_proposals" => list_task_label_proposals;
            "api.propose-task-label" => "adapter.propose_task_label" => propose_task_label;
        ))
        .route(endpoint_route!("api.record-label-ontology-observation" => "adapter.record_label_ontology_observation" => record_label_ontology_observation;
        ))
        .route(endpoint_route!("api.list-label-ontology-signals" => "adapter.list_label_ontology_signals" => list_label_ontology_signals;
        ))
        .route(endpoint_route!("api.review-label-ontology" => "adapter.review_label_ontology" => review_label_ontology;
        ))
        .route(endpoint_route!("api.create-label-ontology-action" => "adapter.create_label_ontology_action" => create_label_ontology_action;
        ))
        .route(endpoint_route!("api.apply-label-ontology-atom" => "adapter.apply_label_ontology_atom" => apply_label_ontology_atom;
        ))
        .route(endpoint_route!("api.revert-label-ontology-mutation" => "adapter.revert_label_ontology_mutation" => revert_label_ontology_mutation;
        ))
        .route(endpoint_route!("api.validate-label-ontology-action" => "adapter.validate_label_ontology_action" => validate_label_ontology_action;
        ))
        .route(endpoint_route!("api.get-label-ontology-signal" => "adapter.get_label_ontology_signal" => get_label_ontology_signal;
        ))
        .route(endpoint_route!("api.get-label-proposal" => "adapter.get_label_proposal" => get_label_proposal;
        ))
        .route(endpoint_route!("api.accept-label-proposal" => "adapter.accept_label_proposal" => accept_label_proposal;
        ))
        .route(endpoint_route!("api.reject-label-proposal" => "adapter.reject_label_proposal" => reject_label_proposal;
        ))
        .route(endpoint_route!("api.remove-task-label" => "adapter.remove_task_label" => remove_task_label;
        ))
        .route(endpoint_route!("api.specify-task" => "adapter.specify_task" => specify_task;
        ))
        .route(endpoint_route!("api.promote-task" => "adapter.promote_task" => promote_task;
        ))
        .route(endpoint_route!("api.claim-task" => "adapter.claim_task" => claim_task;
        ))
        .route(endpoint_route!("api.reopen-task" => "adapter.reopen_task" => reopen_task;
        ))
        .route(endpoint_route!("api.reclaim-task" => "adapter.reclaim_task" => reclaim_task;
        ))
        .route(endpoint_route!("api.heartbeat-task" => "adapter.heartbeat_task" => heartbeat_task;
        ))
        .route(endpoint_route!("api.complete-task" => "adapter.complete_task" => complete_task;
        ))
        .route(endpoint_route!("api.submit-review-task" => "adapter.submit_review_task" => submit_review_task;
        ))
        .route(endpoint_route!("api.block-task" => "adapter.block_task" => block_task;
        ))
        .route(endpoint_route!("api.unblock-task" => "adapter.unblock_task" => unblock_task;
        ))
        .route(endpoint_route!("api.archive-task" => "adapter.archive_task" => archive_task;
        ))
        .route(endpoint_route!("api.list-dependencies" => "adapter.list_dependencies" => list_dependencies;
            "api.add-dependency" => "adapter.add_dependency" => add_dependency;
        ))
        .route(endpoint_route!("api.remove-dependency" => "adapter.remove_dependency" => remove_dependency;
        ))
        .route(endpoint_route!("api.list-steps" => "adapter.list_steps" => list_steps;
            "api.create-step" => "adapter.create_step" => create_step;
        ))
        .route(endpoint_route!("api.update-step" => "adapter.update_step" => update_step;
            "api.remove-step" => "adapter.remove_step" => remove_step;
        ))
        .route(endpoint_route!("api.complete-step" => "adapter.complete_step" => complete_step;
        ))
        .route(endpoint_route!("api.skip-step" => "adapter.skip_step" => skip_step;
        ))
        .route(endpoint_route!("api.reopen-step" => "adapter.reopen_step" => reopen_step;
        ))
        .route(endpoint_route!("api.mark-execution-plan-not-required" => "adapter.mark_execution_plan_not_required" => mark_execution_plan_not_required;
        ))
        .route(endpoint_route!("api.list-runs" => "adapter.list_runs" => list_runs;
        ))
        .route(endpoint_route!("api.get-run" => "adapter.get_run" => get_run;
        ))
        .route(endpoint_route!("api.get-run-log" => "adapter.get_run_log" => get_run_log;
        ))
        .route(endpoint_route!("api.list-comments" => "adapter.list_comments" => list_comments;
            "api.create-comment" => "adapter.create_comment" => create_comment;
        ))
        .route(endpoint_route!("api.get-stats" => "adapter.get_stats" => get_stats;
        ))
        .route(endpoint_route!("api.search-tasks" => "adapter.search_tasks" => search_tasks;
        ))
        .route(endpoint_route!("api.search-tasks-by-status" => "adapter.search_tasks_by_status" => search_tasks_by_status;
        ))
        .route(endpoint_route!("api.search-status" => "adapter.search_status" => search_status;
        ))
        .route(endpoint_route!("api.build-context" => "adapter.build_context" => build_context;
        ))
        .route(endpoint_route!("api.graph-status" => "adapter.graph_status" => graph_status;
        ))
        .route(endpoint_route!("api.graph-neighbors" => "adapter.graph_neighbors" => graph_neighbors;
        ))
        .route(endpoint_route!("api.vector-status" => "adapter.vector_status" => vector_status;
        ))
        .route(endpoint_route!("api.list-events" => "adapter.list_events" => list_events;
        ))
        .route(endpoint_route!("sse.stream-events" => "adapter.stream_events" => stream_events;
        ))
        .route(endpoint_route!("api.doctor" => "adapter.doctor" => doctor;
        ))
        .route(endpoint_route!("api.checkpoint" => "adapter.checkpoint" => checkpoint;
        ))
}

fn build_api_router(state: AppState) -> Router {
    let (router, _) = registered_api_routes().finish();
    router
        .layer(from_fn_with_state(state.clone(), require_existing_database))
        .layer(from_fn_with_state(state.clone(), request_locale))
        .with_state(state)
}

pub fn build_desktop_router(state: AppState) -> Router {
    build_api_router(state)
        .layer(desktop_cors_layer())
        .layer(http_trace_layer())
}

pub fn build_serve_router(state: AppState) -> Router {
    build_api_router(state).layer(http_trace_layer())
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
    serve_with_search_sync_shutdown(addr, state, search_sync, std::future::pending()).await
}

pub async fn serve_with_search_sync_shutdown<S>(
    addr: SocketAddr,
    state: AppState,
    search_sync: SearchSyncConfig,
    shutdown: S,
) -> std::io::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let shutdown_token = CancellationToken::new();
    let search_sync_shutdown = shutdown_token.clone();
    let search_sync_task = spawn_search_sync_task_until_shutdown(
        state.clone(),
        search_sync,
        search_sync_shutdown.clone(),
    );
    let result = axum::serve(listener, build_serve_router(state))
        .with_graceful_shutdown(async move {
            shutdown.await;
            shutdown_token.cancel();
        })
        .await;
    search_sync_shutdown.cancel();
    if let Some(task) = search_sync_task {
        let _ = task.await;
    }
    result
}

#[cfg(test)]
mod contract_catalog_tests {
    use std::collections::BTreeSet;

    use kanban_contract::endpoint_catalog;

    use super::{endpoint_operation, registered_api_routes};

    #[test]
    fn api_route_catalog_matches_exact_contract_catalog() {
        let (_, operations) = registered_api_routes().finish();
        let actual = operations
            .into_iter()
            .map(|operation| {
                format!(
                    "{} {}",
                    super::endpoint_method_name(operation.actual_method),
                    operation.actual_path
                )
            })
            .collect::<BTreeSet<_>>();
        let expected = endpoint_catalog()
            .iter()
            .map(|endpoint| {
                format!(
                    "{} {}",
                    super::endpoint_method_name(endpoint.method),
                    endpoint.path
                )
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            actual, expected,
            "新增、删除或重命名 API method/path 时必须同步精确 contract catalog"
        );
        assert_eq!(actual.len(), 84, "当前 API method/path 基线必须保持显式");
        let (_, operations) = registered_api_routes().finish();
        let actual_ids = operations
            .iter()
            .map(|operation| operation.operation_id)
            .collect::<BTreeSet<_>>();
        let expected_ids = endpoint_catalog()
            .iter()
            .map(|endpoint| endpoint.operation_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_ids, expected_ids,
            "真实注册必须与 descriptor 双向 parity"
        );
        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.adapter_id)
                .collect::<BTreeSet<_>>()
                .len(),
            84,
            "每个真实 handler binding 必须有稳定唯一 adapter_id"
        );
        for operation in &operations {
            assert_eq!(operation.actual_path, operation.descriptor_path);
            assert_eq!(operation.actual_method, operation.descriptor_method);
        }
    }

    #[test]
    #[should_panic(expected = "actual path drift")]
    fn endpoint_binding_rejects_actual_path_drift() {
        endpoint_operation(
            "api.create-board",
            "adapter.test-path",
            "/api/v1/not-boards",
            kanban_contract::HttpMethod::Post,
        );
    }

    #[test]
    #[should_panic(expected = "actual method drift")]
    fn endpoint_binding_rejects_actual_method_drift() {
        endpoint_operation(
            "api.create-board",
            "adapter.test-method",
            "/api/v1/boards",
            kanban_contract::HttpMethod::Get,
        );
    }
}
