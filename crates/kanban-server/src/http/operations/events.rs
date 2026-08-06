pub(crate) mod list;
mod stream;

use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new().merge(list::router()).merge(stream::router())
}
