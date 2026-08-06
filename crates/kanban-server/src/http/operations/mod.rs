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

use crate::state::AppState;
use axum::Router;

pub(crate) fn router(state: AppState) -> Router {
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
        .with_state(state)
}
