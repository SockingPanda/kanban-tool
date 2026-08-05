mod list;
mod log;
mod show;

use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(list::router())
        .merge(show::router())
        .merge(log::router())
}
