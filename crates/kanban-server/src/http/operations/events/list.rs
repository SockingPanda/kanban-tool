use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new()
}
