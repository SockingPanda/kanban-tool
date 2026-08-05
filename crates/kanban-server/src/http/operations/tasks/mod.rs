mod block;
mod claim;
mod create;
mod done;
mod heartbeat;
mod list;
mod list_support;
mod plan_not_required;
mod promote;
mod release;
mod review;
mod show;
pub(crate) mod support;

use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(create::router())
        .merge(list::router())
        .merge(show::router())
        .merge(plan_not_required::router())
        .merge(promote::router())
        .merge(claim::router())
        .merge(heartbeat::router())
        .merge(release::router())
        .merge(review::router())
        .merge(done::router())
        .merge(block::router())
}
