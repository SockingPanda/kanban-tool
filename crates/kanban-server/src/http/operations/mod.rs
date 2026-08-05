mod boards;
mod comments;
mod dependencies;
mod events;
mod health;
mod runs;
mod steps;
mod support;
mod tasks;
#[cfg(test)]
pub(crate) mod test_support;

use crate::state::AppState;
use axum::Router;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(boards::router())
        .merge(tasks::router())
        .merge(comments::router())
        .merge(dependencies::router())
        .merge(steps::router())
        .with_state(state)
}
