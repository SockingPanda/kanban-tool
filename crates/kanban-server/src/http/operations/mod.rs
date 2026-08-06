mod attachments;
mod boards;
pub(crate) mod comments;
mod context;
#[cfg(test)]
mod contract_adoption;
mod dependencies;
mod entities;
mod events;
mod graph;
mod health;
mod labels;
mod maintenance;
mod ontology;
mod runs;
mod search;
mod signals;
mod stats;
mod steps;
mod support;
pub(crate) mod tasks;
#[cfg(test)]
pub(crate) mod test_support;

use std::cell::RefCell;

use crate::state::AppState;
use axum::Router;
use kanban_protocol::HttpMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RegisteredApiRoute {
    pub method: HttpMethod,
    pub path: &'static str,
}

thread_local! {
    static ROUTE_CAPTURE: RefCell<Option<Vec<RegisteredApiRoute>>> = const { RefCell::new(None) };
}

pub(crate) fn registered_path(method: HttpMethod, path: &'static str) -> &'static str {
    ROUTE_CAPTURE.with(|capture| {
        if let Some(routes) = capture.borrow_mut().as_mut() {
            routes.push(RegisteredApiRoute { method, path });
        }
    });
    path
}

pub(crate) fn registered_paths(path: &'static str, methods: &'static [HttpMethod]) -> &'static str {
    for &method in methods {
        registered_path(method, path);
    }
    path
}

#[cfg(test)]
pub(crate) fn capture_registered_routes<R>(
    build: impl FnOnce() -> R,
) -> (R, Vec<RegisteredApiRoute>) {
    let previous = ROUTE_CAPTURE.with(|capture| capture.borrow_mut().replace(Vec::new()));
    let result = build();
    let routes = ROUTE_CAPTURE.with(|capture| capture.borrow_mut().take().unwrap_or_default());
    ROUTE_CAPTURE.with(|capture| {
        *capture.borrow_mut() = previous;
    });
    (result, routes)
}

pub(crate) fn router(state: AppState) -> Router {
    router_without_state().with_state(state)
}

fn router_without_state() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(maintenance::router())
        .merge(boards::router())
        .merge(attachments::router())
        .merge(tasks::router())
        .merge(labels::router())
        .merge(comments::router())
        .merge(context::router())
        .merge(dependencies::router())
        .merge(entities::router())
        .merge(graph::router())
        .merge(steps::router())
        .merge(runs::router())
        .merge(search::router())
        .merge(stats::router())
        .merge(ontology::router())
        .merge(signals::router())
        .merge(events::router())
        .merge(crate::vector::router())
}

#[cfg(test)]
pub(crate) fn registered_api_routes() -> Vec<RegisteredApiRoute> {
    let (_router, routes) = capture_registered_routes(router_without_state);
    routes
}
