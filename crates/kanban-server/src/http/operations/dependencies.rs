mod create;
mod list;
mod remove;
pub(crate) mod support;

use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(create::router())
        .merge(list::router())
        .merge(remove::router())
}
