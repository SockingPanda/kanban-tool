mod list;
mod record;
mod review;
mod show;
mod support;

use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(record::router())
        .merge(list::router())
        .merge(show::router())
        .merge(review::router())
}
