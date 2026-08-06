mod archive;
mod block;
mod claim;
mod create;
mod done;
mod heartbeat;
mod list;
mod list_support;
mod plan_not_required;
mod promote;
mod reclaim;
mod release;
mod reopen;
mod review;
mod show;
mod specify;
pub(crate) mod support;
mod unblock;
mod update;

use crate::state::AppState;
use axum::Router;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(create::router())
        .merge(list::router())
        .merge(show::router())
        .merge(update::router())
        .merge(specify::router())
        .merge(unblock::router())
        .merge(reopen::router())
        .merge(reclaim::router())
        .merge(archive::router())
        .merge(plan_not_required::router())
        .merge(promote::router())
        .merge(claim::router())
        .merge(heartbeat::router())
        .merge(release::router())
        .merge(review::router())
        .merge(done::router())
        .merge(block::router())
}
